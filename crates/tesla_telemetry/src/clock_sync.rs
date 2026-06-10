//! Set the Pi's system clock from the vehicle's GPS-derived timestamp.
//!
//! Why: Tesla's car has an always-accurate clock (GPS-synced). Every
//! `state X` response includes a `timestamp` field in the car's clock
//! frame. If our local Pi clock is significantly off — typical when
//! there's no RTC battery and WiFi isn't reachable at boot — we can
//! correct it from the first successful BLE response, with no
//! dependency on NTP or internet access.
//!
//! Design:
//!   * Latency-compensated: adds a fixed ~50 ms transmit latency to the
//!     vehicle timestamp (empirically constant regardless of RTT — see
//!     RESPONSE_LATENCY_COMPENSATION_MS), landing within ~12 ms of an
//!     NTP reference even though the BLE call took 1-5 s.
//!   * NTP-friendly: only adjusts when local-vs-vehicle delta exceeds
//!     a threshold (default 5 min). Avoids fighting NTP's normal sub-
//!     second drift correction.
//!   * RTC-friendly: if `/dev/rtc0` exists, also writes the corrected
//!     time to the RTC so it survives reboots.
//!   * One-shot per startup window: once we've corrected the clock,
//!     subsequent responses are within tolerance so we leave them be.
//!
//! Threading: `clock_settime` modifies CLOCK_REALTIME but does NOT
//! affect CLOCK_MONOTONIC, so any `Instant` values still measure
//! elapsed time correctly across the adjustment.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

/// Threshold below which we leave the local clock alone. NTP and a
/// healthy RTC both keep the clock within a few seconds; if the delta
/// is < 5 minutes, assume one of those did its job and don't second-
/// guess it. Above 5 minutes the clock is meaningfully wrong (typical
/// non-RTC cold-boot states are years off, so this triggers cleanly).
const ADJUSTMENT_THRESHOLD_MS: i64 = 300_000;

/// Constant one-way latency from "car stamps timestamp" to "we read
/// the response", in milliseconds. Empirically measured against an
/// NTP-set reference clock — the actual delta clustered tightly
/// around -55ms with low variance regardless of round-trip time.
/// Tesla stamps the timestamp just before transmitting the response,
/// so this is essentially the BLE response transit time. Adding it
/// brings our clock-sync accuracy from ~54ms to ~12ms vs NTP.
const RESPONSE_LATENCY_COMPENSATION_MS: i64 = 50;

// The hand-rolled RFC 3339 parsers that used to live here served the
// shell-out sampler's JSON timestamps; the in-process BLE path reads
// protobuf Timestamps directly, so they were removed with it.

/// If the local clock is meaningfully wrong (>5 min from vehicle), set
/// it to vehicle time. Optionally persist to RTC.
///
/// Args:
///   * `vehicle_ts_ms` — the timestamp Tesla sent us, in ms-since-epoch
///     (includes the fractional seconds Tesla provides, e.g. `.794Z`)
///   * `request_started_at` — monotonic Instant from before we sent
///     the BLE request, used for diagnostic RTT logging
///
/// Returns true if we adjusted the clock; false if delta was below
/// threshold (normal case once everything's synced).
pub fn maybe_set_clock_from_vehicle(
    vehicle_ts_ms: i64,
    request_started_at: Instant,
) -> bool {
    let local_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    // Empirically (validated against NTP-set reference clock, see
    // README), Tesla stamps the response timestamp just before
    // transmitting — NOT at the midpoint of processing. The one-way
    // latency from "car stamps" to "we receive" is a consistent
    // ~50ms regardless of RTT. Adding it brings us from ~54ms avg
    // error (no comp) down to ~12ms avg error (with comp).
    let rtt_ms = request_started_at.elapsed().as_millis() as i64;
    let corrected_target_ms = vehicle_ts_ms + RESPONSE_LATENCY_COMPENSATION_MS;

    let delta_ms = corrected_target_ms - local_ms;
    if delta_ms.abs() < ADJUSTMENT_THRESHOLD_MS {
        // Already close enough — leave it alone. Avoids fighting
        // NTP / RTC adjustments that are doing their job.
        return false;
    }

    info!(
        "system clock differs from vehicle by {}ms (local={}ms, vehicle={}ms, rtt={}ms); \
         adjusting to vehicle time",
        delta_ms, local_ms, corrected_target_ms, rtt_ms
    );

    // Actually set the system clock with millisecond precision via
    // tv_nsec. Requires CAP_SYS_TIME; the telemetry daemon runs as
    // root so this works.
    let secs = corrected_target_ms / 1000;
    let ms_remainder = corrected_target_ms % 1000;
    // `tv_nsec` is `i64` on x86_64 Linux but `i32` on aarch64 — use
    // libc::c_long so this compiles cleanly on both.
    let ts = libc::timespec {
        tv_sec: secs as libc::time_t,
        tv_nsec: (ms_remainder * 1_000_000) as libc::c_long,
    };
    let rc = unsafe { libc::clock_settime(libc::CLOCK_REALTIME, &ts) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        warn!("clock_settime failed: {} (errno={})", err, err.raw_os_error().unwrap_or(0));
        return false;
    }

    // If an RTC battery is present, also write the corrected time
    // there so the next boot starts from the right time without
    // needing the BLE-sync dance again. Best-effort.
    if std::path::Path::new("/dev/rtc0").exists() {
        match std::process::Command::new("hwclock").args(["-w"]).output() {
            Ok(out) if out.status.success() => {
                info!("wrote corrected time to RTC (hwclock -w)");
            }
            Ok(out) => {
                warn!(
                    "hwclock -w returned {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            Err(e) => warn!("hwclock -w failed to run: {e}"),
        }
    }

    true
}

