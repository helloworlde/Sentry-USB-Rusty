//! Reads the single-partition MBR that `sfdisk` writes inside
//! `cam_disk.bin`/`snap.bin` (see crates/setup/src/disk_images.rs), giving
//! the byte range of the CAM filesystem (exFAT type 0x07 or FAT32 0x0c).

use std::io::{Read, Seek, SeekFrom};

use anyhow::{bail, Context, Result};

const SECTOR: u64 = 512;

#[derive(Clone, Copy, Debug)]
pub struct MbrPartition {
    pub part_type: u8,
    pub start: u64,
    pub len: u64,
}

pub fn first_partition<R: Read + Seek>(image: &mut R) -> Result<MbrPartition> {
    let mut sector = [0u8; 512];
    image.seek(SeekFrom::Start(0)).context("seek to MBR")?;
    image.read_exact(&mut sector).context("read MBR")?;
    if sector[510..512] != [0x55, 0xAA] {
        bail!("no MBR boot signature in image");
    }
    for i in 0..4 {
        let e = &sector[446 + i * 16..446 + (i + 1) * 16];
        let part_type = e[4];
        if part_type == 0 {
            continue;
        }
        let start_lba = u32::from_le_bytes(e[8..12].try_into().unwrap()) as u64;
        let num_sectors = u32::from_le_bytes(e[12..16].try_into().unwrap()) as u64;
        return Ok(MbrPartition {
            part_type,
            start: start_lba * SECTOR,
            len: num_sectors * SECTOR,
        });
    }
    bail!("no partitions in image MBR");
}
