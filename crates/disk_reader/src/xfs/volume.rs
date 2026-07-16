//! Replacement for upstream xfuse's `volume.rs`.
//!
//! Upstream put the FUSE server here plus a process-global copy of the
//! superblock that the `Decode` impls need. We drop the FUSE server (see
//! `fs.rs` for the path-based API) but must keep the global, because the
//! vendored parsing modules read it during decoding. The one-volume-per-
//! process limitation is inherited from upstream and fine for our use: the
//! helper opens a single SentryUSB disk for its whole lifetime.

use std::sync::OnceLock;

use super::sb::Sb;

pub(super) static SUPERBLOCK: OnceLock<Sb> = OnceLock::new();
