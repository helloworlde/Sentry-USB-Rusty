//! Replacement for upstream xfuse's `block_reader.rs`.
//!
//! Upstream's `BlockReader` owned a `File` and used OS ioctls for media
//! size. Ours is generic over any `Read + Seek` (a plain image file, a raw
//! device, or a partition window of either) with the size supplied by the
//! caller, but keeps the exact buffering semantics the vendored parsers
//! rely on: a sector-aligned block buffer whose size callers adjust with
//! `set_bufsize`, plus `BufRead` and bincode `Reader` impls.

use std::io::{self, BufRead, Read, Result as IoResult, Seek, SeekFrom};

use bincode_next::{de::read::Reader, error::DecodeError};

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug)]
pub struct BlockReader<R> {
    inner: R,
    block: Vec<u8>,
    idx: usize,
    /// The absolute minimum that we can read in any operation
    sectorsize: usize,
    /// Volume size in bytes.
    pub size: u64,
}

impl<R: Read + Seek> BlockReader<R> {
    pub fn new(inner: R, size: u64) -> Self {
        let sectorsize = SECTOR_SIZE;
        Self {
            inner,
            block: vec![0u8; sectorsize],
            idx: sectorsize,
            sectorsize,
            size,
        }
    }

    fn refill(&mut self) -> IoResult<()> {
        self.inner.read_exact(&mut self.block)?;
        self.idx = 0;
        Ok(())
    }

    fn buffered(&self) -> usize {
        self.block.len() - self.idx
    }

    fn refill_if_empty(&mut self) -> IoResult<()> {
        if self.buffered() == 0 {
            self.refill()?;
        }
        Ok(())
    }

    /// The current size of the buffer
    pub fn bufsize(&self) -> usize {
        self.block.len()
    }

    /// Change the reader's bufsize.  It will be rounded up to a multiple of the sectorsize.
    /// After this operation, the buffer should be considered undefined until the next absolute
    /// Seek operation.
    pub fn set_bufsize(&mut self, bufsize: usize) {
        let remainder = bufsize & (self.sectorsize - 1);
        let bufsize = if remainder > 0 {
            bufsize + self.sectorsize - remainder
        } else {
            bufsize
        };
        self.block.resize(bufsize, 0u8);
        self.idx = bufsize;
    }
}

impl<R: Read + Seek> Read for BlockReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.refill_if_empty()?;
        let num = buf.len().min(self.buffered());
        let buf = &mut buf[0..num];
        buf.copy_from_slice(&self.block[self.idx..(self.idx + num)]);
        self.idx += num;
        Ok(num)
    }
}

impl<R: Read + Seek> BufRead for BlockReader<R> {
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        self.refill_if_empty()?;
        Ok(&self.block[self.idx..])
    }

    fn consume(&mut self, amt: usize) {
        assert!(amt <= self.buffered());
        self.idx += amt;
    }
}

impl<R: Read + Seek> Seek for BlockReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let bs = self.bufsize() as u64;
        match pos {
            SeekFrom::Start(pos) => {
                let real = self.inner.seek(SeekFrom::Start(pos / bs * bs))?;
                let rem = pos - real;
                assert!(rem < bs);

                self.refill()?;
                self.idx = rem as usize;

                Ok(real + rem)
            }
            SeekFrom::Current(offset) => {
                let real = self.inner.stream_position()?;
                let cur = real - self.block.len() as u64 + self.idx as u64;
                let newidx = offset + self.idx as i64;
                if newidx >= 0 && newidx < self.bufsize() as i64 {
                    // The data is already buffered; just adjust the pointer
                    self.idx = newidx as usize;
                    Ok(real - self.block.len() as u64 + newidx as u64)
                } else if cur as i64 + offset < 0 {
                    Err(io::Error::from_raw_os_error(
                        crate::xfs::shim::EINVAL,
                    ))
                } else {
                    self.seek(SeekFrom::Start((cur as i64 + offset) as u64))
                }
            }
            SeekFrom::End(_) => todo!("SeekFrom::End()"),
        }
    }
}

impl<R: Read + Seek> Reader for BlockReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), DecodeError> {
        self.read_exact(bytes).map_err(|inner| DecodeError::Io {
            inner,
            additional: bytes.len(),
        })
    }

    fn peek_read(&mut self, n: usize) -> Option<&[u8]> {
        self.block[self.idx..].get(..n)
    }

    fn consume(&mut self, n: usize) {
        <Self as std::io::BufRead>::consume(self, n);
    }
}
