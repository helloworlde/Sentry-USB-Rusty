//! Filesystem inside the CAM partition of a snapshot image: exFAT when the
//! Pi was set up with USE_EXFAT and kernel support, FAT32 otherwise.
//!
//! FAT32 lands first (via `fatfs`); exFAT follows once validated against a
//! real Tesla-written image.

use std::io::{self, Read, Seek, SeekFrom, Write};

use anyhow::{bail, Context, Result};

#[derive(Clone, Debug)]
pub struct CamEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// What filesystem the CAM partition carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamFsKind {
    Fat32,
    Exfat,
}

pub fn sniff<R: Read + Seek>(part: &mut R) -> Result<CamFsKind> {
    let mut boot = [0u8; 512];
    part.seek(SeekFrom::Start(0)).context("seek to boot sector")?;
    part.read_exact(&mut boot).context("read boot sector")?;
    if &boot[3..11] == b"EXFAT   " {
        Ok(CamFsKind::Exfat)
    } else if &boot[82..90] == b"FAT32   " {
        Ok(CamFsKind::Fat32)
    } else {
        bail!("CAM partition is neither exFAT nor FAT32");
    }
}

/// `fatfs` insists on `Read + Write + Seek` even for read-only use; this
/// adapter satisfies the bound while guaranteeing we can never write.
pub struct ReadOnly<T>(pub T);

impl<T: Read> Read for ReadOnly<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl<T: Seek> Seek for ReadOnly<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl<T> Write for ReadOnly<T> {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "disk_reader is strictly read-only",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A read-only view of a CAM filesystem.
pub enum CamFs<T: Read + Seek + Send + 'static> {
    Fat32(fatfs::FileSystem<ReadOnly<T>>),
}

impl<T: Read + Seek + Send + 'static> CamFs<T> {
    pub fn open(mut part: T) -> Result<Self> {
        match sniff(&mut part)? {
            CamFsKind::Fat32 => {
                part.seek(SeekFrom::Start(0))?;
                let fs = fatfs::FileSystem::new(
                    ReadOnly(part),
                    fatfs::FsOptions::new().update_accessed_date(false),
                )
                .context("open FAT32 CAM filesystem")?;
                Ok(CamFs::Fat32(fs))
            }
            CamFsKind::Exfat => {
                bail!("exFAT CAM filesystems are not supported yet (coming in Phase 1b)")
            }
        }
    }

    pub fn read_dir(&self, path: &str) -> Result<Vec<CamEntry>> {
        match self {
            CamFs::Fat32(fs) => {
                let dir = if path.is_empty() || path == "/" {
                    fs.root_dir()
                } else {
                    fs.root_dir()
                        .open_dir(path)
                        .with_context(|| format!("open dir {path}"))?
                };
                let mut out = Vec::new();
                for entry in dir.iter() {
                    let entry = entry.context("read dir entry")?;
                    let name = entry.file_name();
                    if name == "." || name == ".." {
                        continue;
                    }
                    out.push(CamEntry {
                        name,
                        is_dir: entry.is_dir(),
                        size: entry.len(),
                    });
                }
                Ok(out)
            }
        }
    }

    /// Read a whole file (used for small metadata like event.json).
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        match self {
            CamFs::Fat32(fs) => {
                let mut f = fs
                    .root_dir()
                    .open_file(path)
                    .with_context(|| format!("open file {path}"))?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)
                    .with_context(|| format!("read {path}"))?;
                Ok(buf)
            }
        }
    }

    /// Read `len` bytes at `offset` of a file (for Range streaming).
    pub fn read_file_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>> {
        match self {
            CamFs::Fat32(fs) => {
                let mut f = fs
                    .root_dir()
                    .open_file(path)
                    .with_context(|| format!("open file {path}"))?;
                f.seek(SeekFrom::Start(offset))
                    .with_context(|| format!("seek {path}"))?;
                let mut buf = vec![0u8; len];
                let mut filled = 0;
                while filled < len {
                    let n = f.read(&mut buf[filled..]).context("read range")?;
                    if n == 0 {
                        break;
                    }
                    filled += n;
                }
                buf.truncate(filled);
                Ok(buf)
            }
        }
    }
}
