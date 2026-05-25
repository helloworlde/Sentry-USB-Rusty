//! GET /api/logs/bluetooth — single-page BLE diagnostic dump.
//!
//! Lives under the existing /api/logs route prefix so it shows up
//! as a "Bluetooth" tab in the Logs UI without needing a new
//! rendering component. Returns text/plain — a structured report
//! split into sections that lets users (or support) see at a glance:
//!   * which adapter is in use
//!   * is the sampler service running, since when
//!   * what does observe() think the car is doing
//!   * latest state + body-controller sample ages
//!   * recent failure counts + the freshest journal lines
//!
//! Pulls everything live (sysfs, systemctl, filesystem mtime, the
//! telemetry DB, journalctl). No caching — each fetch reflects the
//! current moment.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::router::AppState;

const CAM_DISK_PATH: &str = "/backingfiles/cam_disk.bin";

pub async fn get_ble_debug(State(s): State<AppState>) -> Response {
    let mut out = String::with_capacity(4096);
    let now = unix_now();

    section(&mut out, "Service");
    write_service_status(&mut out).await;

    section(&mut out, "Adapter");
    write_adapter_status(&mut out);

    section(&mut out, "Car observation (drives the sampler's phase machine)");
    write_observation(&mut out, now);

    section(&mut out, "Sample database (last 10 minutes)");
    write_sample_db(&mut out, &s, now).await;

    section(&mut out, "Recent sampler journal (filtered)");
    write_journal(&mut out, 60).await;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        out,
    )
        .into_response()
}

fn section(out: &mut String, title: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("===== ");
    out.push_str(title);
    out.push_str(" =====\n");
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn write_service_status(out: &mut String) {
    // Active state from systemctl. is-active prints "active" /
    // "inactive" / "failed" + exit code reflects the same.
    let active = tokio::process::Command::new("systemctl")
        .args(["is-active", "sentryusb-telemetry"])
        .output()
        .await
        .ok()
        .and_then(|o| {
            String::from_utf8(o.stdout).ok()
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "<systemctl unavailable>".into());
    out.push_str("status: ");
    out.push_str(&active);
    out.push('\n');

    // Uptime since ActiveEnterTimestamp — humanize.
    if let Ok(o) = tokio::process::Command::new("systemctl")
        .args([
            "show",
            "sentryusb-telemetry",
            "-p",
            "ActiveEnterTimestamp",
            "--value",
        ])
        .output()
        .await
    {
        let ts = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !ts.is_empty() {
            out.push_str("started: ");
            out.push_str(&ts);
            out.push('\n');
        }
    }
}

fn write_adapter_status(out: &mut String) {
    // Mirrors the picker logic in ble.rs::adapter_source.
    if let Ok(entries) = std::fs::read_dir("/sys/class/bluetooth") {
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("hci") && !n.contains(':') {
                    Some(n)
                } else {
                    None
                }
            })
            .collect();
        ids.sort();
        for id in ids {
            let label = match crate::ble::adapter_source(&id) {
                "onboard" => "Pi built-in (UART)",
                _ => "USB dongle",
            };
            let addr = std::fs::read_to_string(format!(
                "/sys/class/bluetooth/{id}/address"
            ))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "?".into());
            // Soft-blocked = rfkilled
            let blocked = std::fs::read_to_string(format!(
                "/sys/class/bluetooth/{id}/rfkill0/soft"
            ))
            .or_else(|_| {
                // rfkill index varies; just probe a few common ones
                (0..4)
                    .find_map(|i| {
                        std::fs::read_to_string(format!(
                            "/sys/class/bluetooth/{id}/rfkill{i}/soft"
                        ))
                        .ok()
                    })
                    .ok_or(std::io::Error::other(""))
            })
            .ok()
            .map(|s| s.trim().to_string());
            let blocked_label = match blocked.as_deref() {
                Some("1") => " [rfkill BLOCKED]",
                Some("0") => "",
                _ => "",
            };
            out.push_str(&format!(
                "  {} = {} ({}){}\n",
                id, label, addr, blocked_label
            ));
        }
    } else {
        out.push_str("  /sys/class/bluetooth missing — bluez not running?\n");
    }
    // Currently-configured adapter from sentryusb.conf.
    let configured = std::fs::read_to_string("/root/sentryusb.conf")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.trim().strip_prefix("export BLE_ADAPTER=").map(|v| {
                    v.trim_matches(|c| c == '"' || c == '\'').to_string()
                }))
        })
        .unwrap_or_else(|| "<unset (defaults to hci0)>".into());
    out.push_str(&format!("configured BLE_ADAPTER: {}\n", configured));
}

fn write_observation(out: &mut String, now: i64) {
    // The sampler's phase machine reads observe() each tick. Its
    // output drives whether the parked-awake refresh fires every
    // 3 min (climate/charge) and every 30 min (TPMS), or whether
    // the sampler stays in body-controller-only mode.
    //
    // Source of truth: mtime of /backingfiles/cam_disk.bin (the
    // gadget LUN backing file). Tesla writes to it every ~60s while
    // the car is on (driving OR Sentry OR charging triggers).
    let mtime = std::fs::metadata(CAM_DISK_PATH)
        .and_then(|m| m.modified())
        .ok();
    match mtime {
        Some(t) => {
            let ts = t
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let age = (now - ts).max(0);
            let state = if age < 90 {
                "Awake (last write < 90s)"
            } else if age < 300 {
                "Idle  (90s..5m) — between thresholds; parked-awake refresh paused"
            } else {
                "Asleep (>5m) — body-controller polls only"
            };
            out.push_str(&format!(
                "cam_disk.bin last written: {}s ago\n",
                age
            ));
            out.push_str(&format!("derived car state: {}\n", state));
            out.push_str(
                "\n\
                 If the car is parked + Sentry/charging but state shows\n\
                 Idle/Asleep here, Tesla isn't writing dashcam clips\n\
                 frequently enough for the sampler to know the car is\n\
                 awake — the parked-awake refresh won't fire. This is\n\
                 the most common cause of 'why is my climate/battery\n\
                 stale for >3 min while parked-awake?'\n",
            );
        }
        None => {
            out.push_str(&format!(
                "cam_disk.bin missing or unreadable ({}). \
                 Sampler treats this as Asleep.\n",
                CAM_DISK_PATH
            ));
        }
    }
}

async fn write_sample_db(out: &mut String, s: &AppState, now: i64) {
    let store = s.drives.store.clone();
    let res = tokio::task::spawn_blocking(move || {
        store.with_locked_conn(|conn| {
            let state_ts: Option<i64> = conn
                .query_row(
                    "SELECT ts FROM telemetry_samples WHERE source='state' \
                     ORDER BY ts DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .ok();
            let bc_ts: Option<i64> = conn
                .query_row(
                    "SELECT ts FROM telemetry_samples WHERE source='body_controller' \
                     ORDER BY ts DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .ok();
            let since = now - 600;
            let total: i64 = conn
                .query_row(
                    "SELECT count(*) FROM telemetry_samples WHERE ts >= ?1",
                    (since,),
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let state_n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM telemetry_samples \
                     WHERE ts >= ?1 AND source='state'",
                    (since,),
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let bc_n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM telemetry_samples \
                     WHERE ts >= ?1 AND source='body_controller'",
                    (since,),
                    |r| r.get(0),
                )
                .unwrap_or(0);
            (state_ts, bc_ts, total, state_n, bc_n)
        })
    })
    .await
    .ok();
    let (state_ts, bc_ts, total, state_n, bc_n) = res.unwrap_or((None, None, 0, 0, 0));
    out.push_str(&format!(
        "last state poll:           {}\n",
        format_age(state_ts, now),
    ));
    out.push_str(&format!(
        "last body-controller poll: {}\n",
        format_age(bc_ts, now),
    ));
    out.push_str(&format!(
        "samples last 10 min:       {} total  ({} state, {} body-controller)\n",
        total, state_n, bc_n,
    ));
}

fn format_age(ts: Option<i64>, now: i64) -> String {
    match ts {
        Some(t) => {
            let age = (now - t).max(0);
            if age < 60 {
                format!("{}s ago", age)
            } else if age < 3600 {
                format!("{}m {}s ago", age / 60, age % 60)
            } else {
                format!("{}h {}m ago", age / 3600, (age % 3600) / 60)
            }
        }
        None => "<never>".into(),
    }
}

async fn write_journal(out: &mut String, lines: usize) {
    let cmd = tokio::process::Command::new("journalctl")
        .args([
            "-u",
            "sentryusb-telemetry",
            "-n",
            &lines.to_string(),
            "--no-pager",
            "--output=short-iso",
        ])
        .output()
        .await;
    match cmd {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout);
            // Filter to the noisy-but-useful patterns: state-poll
            // success/fail summary lines, body-controller summary,
            // PersistentSession lifecycle, parked-awake refresh
            // attempts, scan/connect milestones.
            let interesting = [
                "state-poll:",
                "body-controller poll:",
                "PersistentSession:",
                "parked-awake",
                "scanning for Tesla",
                "found target vehicle",
                "connecting to vehicle GATT",
                "session-info from",
                "user_presence flipped",
                "dropping to body-controller",
                "resuming full state polls",
                "WARN",
                "ERROR",
            ];
            for line in raw.lines() {
                if interesting.iter().any(|p| line.contains(p)) {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        Ok(o) => {
            out.push_str(&format!(
                "journalctl exited {}: {}\n",
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stderr).trim(),
            ));
        }
        Err(e) => {
            out.push_str(&format!("journalctl failed to spawn: {}\n", e));
        }
    }
}
