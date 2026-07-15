//! sentryusb-disk-reader: read-only desktop access to SentryUSB SSDs.
//!
//! Phase 1 operates on plain image files (or, on Linux, a raw /dev node
//! opened as a file), which exercises the entire GPT → XFS → MBR →
//! exFAT/FAT32 stack:
//!
//!   sentryusb-disk-reader probe <disk-image>
//!   sentryusb-disk-reader ls    <disk-image> [TeslaCam/SavedClips]
//!   sentryusb-disk-reader cat   <disk-image> <TeslaCam/...> > clip.mp4

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::{Parser, Subcommand};

mod archive;
mod camfs;
mod device;
mod gpt;
mod mbr;
// The vendored parser carries code paths we don't exercise (xattrs,
// symlinks, realtime); keep the diff against upstream minimal instead of
// pruning them.
#[allow(dead_code)]
mod xfs;

#[derive(Parser)]
#[command(name = "sentryusb-disk-reader", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether a disk (image) is a SentryUSB SSD and summarize it.
    Probe { disk: PathBuf },
    /// List a directory of the merged snapshot archive.
    Ls {
        disk: PathBuf,
        /// Path inside the merged CAM tree, e.g. TeslaCam/SavedClips
        #[arg(default_value = "")]
        path: String,
    },
    /// Write a file from the merged archive to stdout.
    Cat { disk: PathBuf, path: String },
    /// Debug: list a directory of a bare XFS volume image.
    XfsLs {
        image: PathBuf,
        #[arg(default_value = "/")]
        path: PathBuf,
    },
    /// Debug: write a file from a bare XFS volume image to stdout.
    XfsCat { image: PathBuf, path: PathBuf },
    /// Debug: list a directory of a cam_disk.bin/snap.bin-style image
    /// (MBR + exFAT/FAT32).
    ImgLs {
        image: PathBuf,
        #[arg(default_value = "")]
        path: String,
    },
    /// Debug: write a file (or byte range) from a snap.bin-style image to
    /// stdout.
    ImgCat {
        image: PathBuf,
        path: String,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long)]
        len: Option<usize>,
    },
}

fn open_cam_image(
    image: &Path,
) -> Result<camfs::CamFs<archive::SubFile<std::fs::File>>> {
    let mut f = std::fs::File::open(image)?;
    let part = mbr::first_partition(&mut f)?;
    camfs::CamFs::open(archive::SubFile::new(f, part.start, part.len))
}

fn open_archive(disk: &Path) -> Result<archive::Archive> {
    let disk = device::Disk::open_image(disk)?;
    archive::Archive::open(&disk)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::Probe { disk } => {
            let dev = device::Disk::open_image(&disk)?;
            let probe = archive::probe(&dev)?;
            println!(
                "SentryUSB disk: backingfiles partition #{} at {} ({} GiB)",
                probe.backingfiles.index,
                probe.backingfiles.start,
                probe.backingfiles.len >> 30,
            );
            let ar = archive::Archive::open(&dev)?;
            println!("sources ({}):", ar.sources.len());
            for s in &ar.sources {
                println!(
                    "  {:12} {} (toc: {})",
                    s.label,
                    s.image_path.display(),
                    s.toc.as_ref().map_or("no".into(), |t| t.len().to_string()),
                );
            }
            println!("merged files: {}", ar.file_count());
            for w in &ar.warnings {
                eprintln!("warning: {w}");
            }
        }
        Command::Ls { disk, path } => {
            let ar = open_archive(&disk)?;
            for e in ar.read_dir(&path) {
                if e.is_dir {
                    println!("{}/", e.name);
                } else {
                    println!("{:>12} {}", e.size, e.name);
                }
            }
        }
        Command::XfsLs { image, path } => {
            let dev = device::Disk::open_image(&image)?;
            let size = dev.size;
            let mut fs = xfs::fs::XfsFs::open(dev.whole(), size)?;
            for e in fs.read_dir(&path)? {
                println!("{:?}\t{}\t{}", e.kind, e.ino, e.name);
            }
        }
        Command::XfsCat { image, path } => {
            let dev = device::Disk::open_image(&image)?;
            let size = dev.size;
            let fs = xfs::fs::XfsFs::open(dev.whole(), size)?;
            let fs = std::sync::Arc::new(std::sync::Mutex::new(fs));
            let mut f = xfs::fs::XfsFile::open(&fs, &path)?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            std::io::copy(&mut f, &mut out)?;
        }
        Command::ImgLs { image, path } => {
            let cam = open_cam_image(&image)?;
            for e in cam.read_dir(&path)? {
                if e.is_dir {
                    println!("{}/", e.name);
                } else {
                    println!("{:>12} {}", e.size, e.name);
                }
            }
        }
        Command::ImgCat {
            image,
            path,
            offset,
            len,
        } => {
            let cam = open_cam_image(&image)?;
            let buf = match len {
                Some(len) => cam.read_file_range(&path, offset, len)?,
                None if offset == 0 => cam.read_file(&path)?,
                None => {
                    let full = cam.read_file(&path)?;
                    full.get(offset as usize..).unwrap_or(&[]).to_vec()
                }
            };
            std::io::stdout().write_all(&buf)?;
        }
        Command::Cat { disk, path } => {
            let ar = open_archive(&disk)?;
            let meta = ar
                .stat(&path)
                .ok_or_else(|| anyhow::anyhow!("no such file: {path}"))?;
            let size = meta.size;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            let mut offset = 0u64;
            const CHUNK: usize = 4 << 20;
            while offset < size {
                let want = CHUNK.min((size - offset) as usize);
                let buf = ar.read_range(&path, offset, want)?;
                if buf.is_empty() {
                    break;
                }
                out.write_all(&buf)?;
                offset += buf.len() as u64;
            }
        }
    }
    Ok(())
}
