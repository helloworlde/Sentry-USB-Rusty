//! Archive-sync size guard.
//!
//! The guard prevents a corrupted local `drive-data.json` (disk hiccup,
//! partial write, snapshot corruption) from silently overwriting the
//! archive-side copy. Once the store has grown past `MIN_THRESHOLD_BYTES`,
//! any subsequent sync that shrinks the file below `RATIO` × last-good
//! size is refused.
//!
//! **Fail-open by design:** a missing or corrupt cache file allows the
//! sync. Losing the guard temporarily is better than blocking a legitimate
//! update forever.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use thiserror::Error;

/// Minimum cached size (bytes) required before the ratio guard kicks in.
/// Below this, always allow the sync — tiny datasets don't need
/// corruption protection because there's little to lose.
pub const MIN_THRESHOLD_BYTES: i64 = 10 * 1024 * 1024; // 10 MB

/// Minimum fraction of `last_size` that a new sync must meet. 0.5 means
/// "new file must be at least half the size of the last successful sync".
pub const RATIO: f64 = 0.5;

/// Cache file recording the last-successful-sync size in bytes. Lives on
/// `/mutable` so it survives reboots; ~20 bytes so no disk-pressure impact.
pub const DEFAULT_CACHE_PATH: &str = "/mutable/.drive-data-last-sync";

/// Returned when the guard refuses to overwrite an archive copy because
/// the new file is dramatically smaller than the last known good sync —
/// the signature of the data-loss scenario this guard was built to
/// prevent.
#[derive(Debug, Error)]
#[error(
    "size guard: refusing to sync {new_size} bytes — less than {ratio_pct:.0}% of last successful sync ({last_size} bytes). \
     Local file may be corrupted; archive preserved."
)]
pub struct SyncGuardError {
    pub new_size: i64,
    pub last_size: i64,
    pub ratio_pct: f64,
}

/// Return `Ok(())` if a sync of `new_size` bytes should proceed, or
/// `Err(SyncGuardError)` if the new file is dramatically smaller than
/// the last known good sync (and the last sync was above the minimum
/// threshold). Fails open: `last_size <= 0` always allows the sync.
pub fn check_sync_size_guard(new_size: i64, last_size: i64) -> Result<(), SyncGuardError> {
    if last_size <= 0 {
        return Ok(());
    }
    if last_size < MIN_THRESHOLD_BYTES {
        return Ok(());
    }
    let min_allowed = (last_size as f64 * RATIO) as i64;
    if new_size >= min_allowed {
        return Ok(());
    }
    Err(SyncGuardError {
        new_size,
        last_size,
        ratio_pct: RATIO * 100.0,
    })
}

/// On-disk format tag for the cache. v2 baselines measure the compact
/// JSON export (with telemetry sections); bare-integer baselines predate
/// it and measured pretty-printed output roughly 3x larger. Honoring a
/// bare baseline across that format change would flag every upgrader's
/// first compact export as a phantom "corruption" and block archive sync
/// forever — so bare integers read as "no baseline" (fail open, guard
/// re-baselines on the next successful sync). The shell-side guard in
/// `run/post-archive-process.sh` parses the same format and must stay in
/// lockstep.
const CACHE_FORMAT_PREFIX: &str = "v2:";

/// Read the last-successful-sync size in bytes, or `0` if the cache file
/// doesn't exist, is unreadable/corrupt, or holds a pre-v2 baseline.
/// Fail-open by design: a corrupted cache must not block syncs.
pub fn read_sync_cache<P: AsRef<Path>>(path: P) -> i64 {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return 0, // missing or unreadable → fail open
    };
    let Some(raw) = data.trim().strip_prefix(CACHE_FORMAT_PREFIX) else {
        return 0; // pre-v2 (or garbage) baseline → fail open, re-baseline
    };
    let n: i64 = match raw.parse() {
        Ok(n) => n,
        Err(_) => return 0, // unparseable → fail open
    };
    if n < 0 {
        0
    } else {
        n
    }
}

/// Atomically record `size` as the last-successful-sync size via
/// tmp + rename. Returns an error only on actual I/O failure (caller
/// should log-and-continue — a missed cache write is not fatal).
pub fn write_sync_cache<P: AsRef<Path>>(path: P, size: i64) -> Result<()> {
    if size < 0 {
        anyhow::bail!("write_sync_cache: size must be non-negative, got {}", size);
    }
    let path = path.as_ref();
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(format!("{}{}", CACHE_FORMAT_PREFIX, size).as_bytes())?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_fails_open_on_first_sync() {
        assert!(check_sync_size_guard(100, 0).is_ok());
    }

    #[test]
    fn guard_fails_open_below_threshold() {
        // 1 MB last sync — below 10 MB threshold — allow any new size.
        assert!(check_sync_size_guard(10, 1024 * 1024).is_ok());
        assert!(check_sync_size_guard(100_000, 1024 * 1024).is_ok());
    }

    #[test]
    fn guard_refuses_truncated_sync() {
        // 20 MB last, 5 MB new → well under 50% → refuse.
        let err = check_sync_size_guard(5_000_000, 20_000_000).unwrap_err();
        assert_eq!(err.new_size, 5_000_000);
        assert_eq!(err.last_size, 20_000_000);
    }

    #[test]
    fn guard_allows_small_shrink() {
        // 20 MB last, 15 MB new → 75% retained — allow.
        assert!(check_sync_size_guard(15_000_000, 20_000_000).is_ok());
    }

    #[test]
    fn guard_allows_growth() {
        assert!(check_sync_size_guard(100_000_000, 20_000_000).is_ok());
    }

    #[test]
    fn cache_read_fail_open_on_missing() {
        assert_eq!(read_sync_cache("/nonexistent/sync-cache-test"), 0);
    }

    /// Helper: unique temp path that gets cleaned up on drop.
    struct TmpPath(std::path::PathBuf);
    impl TmpPath {
        fn new(name: &str) -> Self {
            let pid = std::process::id();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!("sentryusb-test-{}-{}-{}", name, pid, nanos));
            TmpPath(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TmpPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("tmp"));
        }
    }

    #[test]
    fn cache_read_fail_open_on_corrupt() {
        let tmp = TmpPath::new("corrupt");
        std::fs::write(tmp.path(), "not-a-number").unwrap();
        assert_eq!(read_sync_cache(tmp.path()), 0);
    }

    #[test]
    fn cache_bare_integer_baseline_resets_on_format_upgrade() {
        // Baselines written before the compact-JSON export were bare
        // integers measured against pretty-printed output ~3x larger;
        // honoring them would block every upgrader's first compact sync
        // as a phantom "corruption". Bare integers read as "no baseline".
        let tmp = TmpPath::new("legacy-bare");
        std::fs::write(tmp.path(), "1491297340").unwrap();
        assert_eq!(read_sync_cache(tmp.path()), 0);
    }

    #[test]
    fn cache_reads_v2_format() {
        let tmp = TmpPath::new("v2-read");
        std::fs::write(tmp.path(), "v2:42000000").unwrap();
        assert_eq!(read_sync_cache(tmp.path()), 42_000_000);
    }

    #[test]
    fn cache_writes_v2_format() {
        // Pin the on-disk format — the shell-side guard in
        // post-archive-process.sh parses this exact shape.
        let tmp = TmpPath::new("v2-write");
        write_sync_cache(tmp.path(), 42_000_000).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path()).unwrap(),
            "v2:42000000"
        );
    }

    #[test]
    fn cache_roundtrip() {
        let tmp = TmpPath::new("roundtrip");
        write_sync_cache(tmp.path(), 42_000_000).unwrap();
        assert_eq!(read_sync_cache(tmp.path()), 42_000_000);
    }

    #[test]
    fn cache_negative_size_rejected() {
        let tmp = TmpPath::new("negative");
        assert!(write_sync_cache(tmp.path(), -1).is_err());
    }
}
