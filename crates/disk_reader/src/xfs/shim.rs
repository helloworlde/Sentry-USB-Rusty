//! Portability shim for the vendored libxfuse sources.
//!
//! Upstream xfuse targets FreeBSD/Linux and leans on `fuser` for the
//! `FileType`/`FileAttr` reply types and on `libc` for errno / `S_IF*` /
//! `SEEK_*` constants. We only need the on-disk parsing, and it has to
//! compile on Windows too, so every one of those names is defined here
//! with fixed values (the numeric values only travel between our own
//! modules — they never reach an OS API).

#![allow(non_camel_case_types)]

pub type c_int = i32;
pub type mode_t = u32;

// errno values (Linux numbering; used purely as internal error codes)
pub const ENOENT: c_int = 2;
pub const EIO: c_int = 5;
pub const ENXIO: c_int = 6;
pub const ENODATA: c_int = 61;
pub const ENOATTR: c_int = ENODATA;
pub const EDESTADDRREQ: c_int = 89;
pub const ERANGE: c_int = 34;
pub const EINVAL: c_int = 22;

// File mode bits (identical on every unix; only compared against inode modes)
pub const S_IFMT: mode_t = 0o170000;
pub const S_IFSOCK: mode_t = 0o140000;
pub const S_IFLNK: mode_t = 0o120000;
pub const S_IFREG: mode_t = 0o100000;
pub const S_IFBLK: mode_t = 0o060000;
pub const S_IFDIR: mode_t = 0o040000;
pub const S_IFCHR: mode_t = 0o020000;
pub const S_IFIFO: mode_t = 0o010000;

// lseek whences used by the extent-map hole/data search helpers
pub const SEEK_DATA: c_int = 3;
pub const SEEK_HOLE: c_int = 4;

/// Build an `OsString` from raw on-disk name bytes. Unix keeps the bytes
/// verbatim; Windows has no byte-backed `OsString`, so names go through a
/// lossy UTF-8 conversion (Tesla clip names are plain ASCII).
pub fn os_string_from_bytes(v: Vec<u8>) -> std::ffi::OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(v)
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(&v).into_owned().into()
    }
}

/// The reverse of [`os_string_from_bytes`], for name comparisons.
pub fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        s.to_string_lossy().into_owned().into_bytes()
    }
}

/// Stand-in for `fuser::FileType`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    NamedPipe,
    CharDevice,
    BlockDevice,
    Directory,
    RegularFile,
    Symlink,
    Socket,
}

/// Stand-in for `fuser::FileAttr` (fields mirror upstream so the vendored
/// `dinode_core.rs` conversion compiles unchanged).
#[derive(Clone, Copy, Debug)]
pub struct FileAttr {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: std::time::SystemTime,
    pub mtime: std::time::SystemTime,
    pub ctime: std::time::SystemTime,
    pub crtime: std::time::SystemTime,
    pub kind: FileType,
    pub perm: u16,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
    pub blksize: u32,
    pub flags: u32,
}
