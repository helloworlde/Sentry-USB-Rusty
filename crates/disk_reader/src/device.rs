//! Device access layer.
//!
//! Phase 1 supports plain image files, which already exercises the whole
//! parsing stack (`probe`/`ls`/`cat` on an image). Raw-device support
//! (macOS `/dev/rdiskN`, Windows `\\.\PhysicalDriveN` with sector-aligned
//! reads, Linux `/dev/sdX`) lands in Phase 2 behind this same trait.

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};

/// A byte-addressable, cloneable, thread-safe view of a disk. Clones share
/// one file handle and serialize their positioned reads through a mutex.
#[derive(Clone)]
pub struct Disk {
    inner: Arc<Mutex<File>>,
    pub size: u64,
}

impl Disk {
    pub fn open_image(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let size = file
            .metadata()
            .with_context(|| format!("stat {}", path.display()))?
            .len();
        Ok(Self {
            inner: Arc::new(Mutex::new(file)),
            size,
        })
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let mut f = self.inner.lock().unwrap();
        f.seek(SeekFrom::Start(offset))?;
        f.read(buf)
    }

    /// A `Read + Seek` window over `[start, start+len)` of the disk.
    pub fn window(&self, start: u64, len: u64) -> Window {
        Window {
            disk: self.clone(),
            start,
            len,
            pos: 0,
        }
    }

    /// Window over the whole disk.
    pub fn whole(&self) -> Window {
        self.window(0, self.size)
    }
}

pub struct Window {
    disk: Disk,
    start: u64,
    len: u64,
    pos: u64,
}

impl Window {
    pub fn len(&self) -> u64 {
        self.len
    }

    /// A sub-window relative to this window's start.
    pub fn sub(&self, start: u64, len: u64) -> Window {
        let len = len.min(self.len.saturating_sub(start));
        self.disk.window(self.start + start, len)
    }
}

impl Read for Window {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let want = buf.len().min((self.len - self.pos) as usize);
        let n = self.disk.read_at(self.start + self.pos, &mut buf[..want])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Window {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
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
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek out of range",
            )),
        }
    }
}
