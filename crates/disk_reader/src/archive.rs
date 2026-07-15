//! The SentryUSB footage archive: snapshots on the `backingfiles` XFS
//! partition, merged into one virtual TeslaCam tree.
//!
//! Layout (written by crates/usb_gadget/src/snapshot.rs on the Pi):
//!   /backingfiles/cam_disk.bin                 — live gadget image (newest data)
//!   /backingfiles/snapshots/snap-NNNNNN/snap.bin      — reflinked snapshot
//!   /backingfiles/snapshots/snap-NNNNNN/snap.bin.toc  — "<size> <relpath>" lines
//!
//! Each *.bin is an MBR image whose first partition holds an exFAT/FAT32
//! filesystem with the Tesla-written TeslaCam tree. The merged view is the
//! union of file paths across snapshots, newest snapshot winning.

use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail, Context, Result};

use crate::{
    camfs::{CamEntry, CamFs},
    device::{Disk, Window},
    gpt,
    mbr,
    xfs::fs::{XfsFile, XfsFs},
};

const SNAPSHOTS_DIR: &str = "snapshots";
const LIVE_IMAGE: &str = "cam_disk.bin";

/// A CAM filesystem stacked over the partition window of a snapshot image
/// inside the XFS volume.
type Cam = CamFs<SubFile<XfsFile<Window>>>;

/// SAFETY: `fatfs::FileSystem` is `!Send` only because its `FsOptions`
/// holds `&'static dyn OemCpConverter/TimeProvider` without a `Sync`
/// bound. We construct it exclusively with `FsOptions::new()` defaults,
/// whose implementations (`LossyOemCpConverter`, `DefaultTimeProvider`)
/// are stateless unit structs, so moving the filesystem between threads
/// is sound. Everything else in the stack is naturally `Send`.
struct SendCam(Cam);
unsafe impl Send for SendCam {}

/// One source of clips: a snapshot's snap.bin or the live cam_disk.bin.
/// `ord` sorts oldest-first; the live image is always newest.
#[derive(Clone, Debug)]
pub struct Source {
    pub ord: u64,
    /// Path of the image inside the XFS volume, e.g. "snapshots/snap-000001/snap.bin".
    pub image_path: PathBuf,
    pub label: String,
    pub toc: Option<Vec<TocEntry>>,
}

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub size: u64,
    /// Relative to the CAM filesystem root, e.g. "TeslaCam/SavedClips/…/x.mp4".
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct FileMeta {
    /// Index into `Archive::sources`.
    pub source: usize,
    pub size: u64,
}

pub struct Archive {
    xfs: Arc<Mutex<XfsFs<Window>>>,
    pub sources: Vec<Source>,
    /// Merged virtual tree: CAM-relative path -> winning source + size.
    files: BTreeMap<String, FileMeta>,
    /// Lazily opened CAM filesystems, keyed by source index. The inner
    /// Mutex makes the fatfs variant (RefCell inside) shareable across the
    /// HTTP server's threads.
    open_cams: Mutex<HashMap<usize, Arc<Mutex<SendCam>>>>,
    pub warnings: Vec<String>,
}

pub struct ProbeResult {
    pub backingfiles: gpt::GptPartition,
}

/// Check whether `disk` looks like a SentryUSB SSD and return the
/// backingfiles partition if so.
pub fn probe(disk: &Disk) -> Result<ProbeResult> {
    let mut whole = disk.whole();
    let parts = gpt::read_partitions(&mut whole).context("reading GPT")?;
    let backingfiles = parts
        .iter()
        .find(|p| {
            if p.name.eq_ignore_ascii_case("backingfiles") {
                return true;
            }
            // Label lives in the XFS superblock too; GPT name is what
            // parted set. Fall back to sniffing XFS magic on p2.
            let mut w = disk.window(p.start, p.len.min(4096));
            let mut magic = [0u8; 4];
            use std::io::{Seek, SeekFrom};
            w.seek(SeekFrom::Start(0)).ok();
            w.read_exact(&mut magic).ok();
            &magic == b"XFSB" && p.index == 2
        })
        .cloned()
        .ok_or_else(|| anyhow!("no backingfiles/XFS partition found — not a SentryUSB disk"))?;
    Ok(ProbeResult { backingfiles })
}

impl Archive {
    pub fn open(disk: &Disk) -> Result<Self> {
        let probe = probe(disk)?;
        let part = &probe.backingfiles;
        let window = disk.window(part.start, part.len);
        let len = window.len();
        let xfs = XfsFs::open(window, len).context("opening backingfiles XFS")?;
        let xfs = Arc::new(Mutex::new(xfs));
        let mut warnings = Vec::new();

        // Enumerate sources: snapshots (ordered by number) then the live image.
        let mut sources = Vec::new();
        {
            let mut fs = xfs.lock().unwrap();
            let snapdirs = match fs.read_dir(Path::new(SNAPSHOTS_DIR)) {
                Ok(entries) => entries,
                Err(e) => {
                    warnings.push(format!("no snapshots directory readable: {e:#}"));
                    Vec::new()
                }
            };
            for d in snapdirs {
                let Some(numpart) = d.name.strip_prefix("snap-") else {
                    continue;
                };
                let Ok(num) = numpart.parse::<u64>() else {
                    continue;
                };
                let dir = PathBuf::from(SNAPSHOTS_DIR).join(&d.name);
                let image_path = dir.join("snap.bin");
                if fs.file_size(&image_path).is_err() {
                    warnings.push(format!("{}: no snap.bin, skipping", d.name));
                    continue;
                }
                sources.push(Source {
                    ord: num,
                    label: d.name.clone(),
                    toc: None,
                    image_path,
                });
            }
            sources.sort_by_key(|s| s.ord);
            // The live image is the newest source of all.
            if fs.file_size(Path::new(LIVE_IMAGE)).is_ok() {
                sources.push(Source {
                    ord: u64::MAX,
                    label: "live".to_string(),
                    toc: None,
                    image_path: PathBuf::from(LIVE_IMAGE),
                });
            }
            if sources.is_empty() {
                bail!("no snapshots and no cam_disk.bin on backingfiles — nothing to browse");
            }
        }

        let mut archive = Archive {
            xfs,
            sources,
            files: BTreeMap::new(),
            open_cams: Mutex::new(HashMap::new()),
            warnings,
        };
        archive.load_tocs();
        archive.build_merged_tree()?;
        Ok(archive)
    }

    fn load_tocs(&mut self) {
        for s in &mut self.sources {
            let toc_path = PathBuf::from(format!("{}.toc", s.image_path.display()));
            match XfsFile::open(&self.xfs, &toc_path) {
                Ok(mut f) => {
                    let mut text = String::new();
                    if f.read_to_string(&mut text).is_ok() {
                        s.toc = Some(parse_toc(&text));
                    }
                }
                Err(_) => {
                    // Live image and interrupted snapshots have no TOC.
                }
            }
        }
    }

    fn build_merged_tree(&mut self) -> Result<()> {
        // Oldest to newest; later inserts overwrite, so newest wins.
        let source_data: Vec<(usize, Option<Vec<TocEntry>>)> = self
            .sources
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.toc.clone()))
            .collect();
        for (idx, toc) in source_data {
            let entries = match toc {
                Some(toc) => toc,
                None => match self.walk_cam(idx) {
                    Ok(entries) => entries,
                    Err(e) => {
                        self.warnings.push(format!(
                            "{}: unreadable CAM filesystem ({e:#}), skipping",
                            self.sources[idx].label
                        ));
                        continue;
                    }
                },
            };
            for e in entries {
                self.files.insert(
                    e.path,
                    FileMeta {
                        source: idx,
                        size: e.size,
                    },
                );
            }
        }
        if self.files.is_empty() {
            bail!("archive contains no files");
        }
        Ok(())
    }

    /// Recursively list every file of a source's CAM filesystem (fallback
    /// when there is no .toc).
    fn walk_cam(&self, source: usize) -> Result<Vec<TocEntry>> {
        let cam = self.cam(source)?;
        let cam = cam.lock().unwrap();
        let mut out = Vec::new();
        let mut stack = vec![String::new()];
        while let Some(dir) = stack.pop() {
            for entry in cam.0.read_dir(&dir)? {
                let path = if dir.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{dir}/{}", entry.name)
                };
                if entry.is_dir {
                    stack.push(path);
                } else {
                    out.push(TocEntry {
                        size: entry.size,
                        path,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Lazily open (and cache) the CAM filesystem of a source.
    fn cam(&self, source: usize) -> Result<Arc<Mutex<SendCam>>> {
        if let Some(c) = self.open_cams.lock().unwrap().get(&source) {
            return Ok(Arc::clone(c));
        }
        let s = &self.sources[source];
        let mut image = XfsFile::open(&self.xfs, &s.image_path)
            .with_context(|| format!("open {}", s.image_path.display()))?;
        let part = mbr::first_partition(&mut image)
            .with_context(|| format!("{}: bad image MBR", s.label))?;
        // Re-open so CamFs owns a fresh file positioned via its own window.
        let image = XfsFile::open(&self.xfs, &s.image_path)?;
        let part_view = SubFile::new(image, part.start, part.len);
        let cam = Arc::new(Mutex::new(SendCam(
            CamFs::open(part_view).with_context(|| format!("{}: CAM fs", s.label))?,
        )));
        self.open_cams
            .lock()
            .unwrap()
            .insert(source, Arc::clone(&cam));
        Ok(cam)
    }

    /// List a directory of the merged virtual tree ("" = root).
    pub fn read_dir(&self, dir: &str) -> Vec<CamEntry> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{}/", dir.trim_matches('/'))
        };
        let mut dirs = BTreeMap::new();
        let mut files = Vec::new();
        for (path, meta) in self.files.range(prefix.clone()..) {
            let Some(rest) = path.strip_prefix(&prefix) else {
                break;
            };
            match rest.split_once('/') {
                Some((child, _)) => {
                    dirs.entry(child.to_string()).or_insert(());
                }
                None => files.push(CamEntry {
                    name: rest.to_string(),
                    is_dir: false,
                    size: meta.size,
                }),
            }
        }
        let mut out: Vec<CamEntry> = dirs
            .into_keys()
            .map(|name| CamEntry {
                name,
                is_dir: true,
                size: 0,
            })
            .collect();
        out.extend(files);
        out
    }

    pub fn stat(&self, path: &str) -> Option<&FileMeta> {
        self.files.get(path.trim_matches('/'))
    }

    /// Read a byte range of a file in the merged tree.
    pub fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>> {
        let path = path.trim_matches('/');
        let meta = self
            .files
            .get(path)
            .ok_or_else(|| anyhow!("no such file: {path}"))?;
        let cam = self.cam(meta.source)?;
        let result = cam.lock().unwrap().0.read_file_range(path, offset, len);
        result
    }

    /// Read a whole (small) file.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let path = path.trim_matches('/');
        let meta = self
            .files
            .get(path)
            .ok_or_else(|| anyhow!("no such file: {path}"))?;
        let cam = self.cam(meta.source)?;
        let result = cam.lock().unwrap().0.read_file(path);
        result
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn source_by_label(&self, label: &str) -> Result<&Source> {
        self.sources
            .iter()
            .find(|s| s.label == label)
            .ok_or_else(|| anyhow!("no source labeled {label} (see probe output)"))
    }

    /// Debug: the raw exFAT/FAT32 boot sector of a source's CAM partition,
    /// plus the MBR partition type.
    pub fn debug_cam_boot(&self, label: &str) -> Result<(Vec<u8>, u8)> {
        let s = self.source_by_label(label)?;
        let mut image = XfsFile::open(&self.xfs, &s.image_path)?;
        let part = mbr::first_partition(&mut image)?;
        let mut view = SubFile::new(XfsFile::open(&self.xfs, &s.image_path)?, part.start, part.len);
        let mut boot = vec![0u8; 512];
        use std::io::Read as _;
        view.read_exact(&mut boot)?;
        Ok((boot, part.part_type))
    }

    /// Debug: the entry-type byte of the first `n` root-directory entries
    /// of a source's exFAT CAM filesystem (assumes root dir is contiguous
    /// from its first cluster — true for freshly-mkfs'd images).
    pub fn debug_root_entry_types(&self, label: &str, n: usize) -> Result<Vec<u8>> {
        let (boot, _) = self.debug_cam_boot(label)?;
        let s = self.source_by_label(label)?;
        let bps = 1u64 << boot[108];
        let spc = 1u64 << boot[109];
        let heap = u64::from(u32::from_le_bytes(boot[88..92].try_into().unwrap()));
        let root = u64::from(u32::from_le_bytes(boot[96..100].try_into().unwrap()));
        let root_off = (heap + (root - 2) * spc) * bps;

        let mut image = XfsFile::open(&self.xfs, &s.image_path)?;
        let part = mbr::first_partition(&mut image)?;
        let mut view = SubFile::new(XfsFile::open(&self.xfs, &s.image_path)?, part.start, part.len);
        use std::io::{Read as _, Seek as _, SeekFrom};
        view.seek(SeekFrom::Start(root_off))?;
        let mut buf = vec![0u8; 32 * n];
        view.read_exact(&mut buf)?;
        Ok(buf.chunks(32).map(|e| e[0]).collect())
    }
}

fn parse_toc(text: &str) -> Vec<TocEntry> {
    text.lines()
        .filter_map(|line| {
            let (size, path) = line.split_once(' ')?;
            Some(TocEntry {
                size: size.parse().ok()?,
                path: path.trim_start_matches('/').to_string(),
            })
        })
        .collect()
}

/// `Read + Seek` over a byte range of an inner `Read + Seek` (the CAM
/// partition inside a snap.bin).
pub struct SubFile<T> {
    inner: T,
    start: u64,
    len: u64,
    pos: u64,
}

impl<T: std::io::Read + std::io::Seek> SubFile<T> {
    pub fn new(inner: T, start: u64, len: u64) -> Self {
        Self {
            inner,
            start,
            len,
            pos: 0,
        }
    }
}

impl<T: std::io::Read + std::io::Seek> std::io::Read for SubFile<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::SeekFrom;
        if self.pos >= self.len {
            return Ok(0);
        }
        let want = buf.len().min((self.len - self.pos) as usize);
        self.inner.seek(SeekFrom::Start(self.start + self.pos))?;
        let n = self.inner.read(&mut buf[..want])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl<T: std::io::Read + std::io::Seek> std::io::Seek for SubFile<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        use std::io::SeekFrom;
        let newpos = match pos {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(off) => self.len.checked_add_signed(off),
            SeekFrom::Current(off) => self.pos.checked_add_signed(off),
        };
        match newpos {
            Some(p) => {
                self.pos = p;
                Ok(p)
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek out of range",
            )),
        }
    }
}
