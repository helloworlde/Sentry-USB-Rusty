//! `apt-get install` with one-shot retry on failure.
//!
//! The Debian mirrors behind `deb.debian.org` (Fastly CDN) regularly
//! serve a transient mismatch between the `Packages` index a Pi just
//! fetched via `apt-get update` and the pool it's now trying to fetch
//! `.deb` files from: different POPs / backend mirrors at different
//! sync states, plus the fact that aged Pi OS images carry baked-in
//! lists pointing at versions Debian has since superseded and pruned.
//! Either case shows up to the user as `404 Not Found` on a perfectly
//! well-formed URL.
//!
//! Every `apt-get install` in this crate should go through
//! [`apt_install`] so a single 404 doesn't abort the whole setup.

use std::time::Duration;

use anyhow::{Context, Result};

/// Run `apt-get install -y <packages>` with a one-shot retry. On the
/// first failure, refresh the package index and try once more. The
/// progress callback receives a single line announcing the retry; on
/// success it isn't called at all.
pub async fn apt_install(
    progress: impl Fn(&str),
    packages: &[&str],
    timeout: Duration,
) -> Result<()> {
    let mut args: Vec<&str> = vec!["-y", "install"];
    args.extend(packages);

    if sentryusb_shell::run_with_timeout(timeout, "apt-get", &args).await.is_ok() {
        return Ok(());
    }

    progress("Refreshing package index and retrying...");
    let _ = sentryusb_shell::run_with_timeout(
        Duration::from_secs(300),
        "apt-get", &["update"],
    ).await;
    sentryusb_shell::run_with_timeout(timeout, "apt-get", &args).await
        .context("apt-get install failed after refresh + retry")?;
    Ok(())
}
