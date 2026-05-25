//! One-shot CLI for keep-awake BLE actions.
//!
//! Replaces the `tesla-control wake|sentry-mode|charge-port-open|...`
//! shell-outs in `run/awake_start`. Each invocation:
//!   1. Loads VIN + BLE_ADAPTER from /root/sentryusb.conf
//!   2. Loads the keypair from /root/.ble/key_private.pem
//!   3. Spawns a PersistentSession, runs ONE signed action, exits
//!
//! Per-invocation connection overhead is the same as the previous
//! tesla-control path (~1-2s including scan + handshake). The
//! long-running PersistentSession optimization isn't relevant for
//! awake_start's nudge cycle — each nudge is one command minutes
//! apart, so there's no benefit to keeping a session warm between
//! them.
//!
//! Usage:
//!   sentryusb-ble-action <verb>
//!
//! Verbs:
//!   wake               - VEHICLE_SECURITY RKE wake
//!   sentry-on          - turn Sentry Mode on
//!   sentry-off         - turn Sentry Mode off
//!   charge-port-open   - open the charge port
//!   charge-port-close  - close the charge port
//!
//! Exit codes:
//!   0 success
//!   1 invalid usage
//!   2 config error (missing VIN, missing key file)
//!   3 BLE error (scan/connect/handshake failed)
//!   4 action rejected by car (returns the fault code as stderr line)

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use sentryusb_tesla_ble::{
    actions::{self, ActionPayload},
    keys::KeyPair,
    manager::PersistentSession,
};
use tracing::{error, info};

const KEY_FILE: &str = "/root/.ble/key_private.pem";
const CONFIG_FILE: &str = "/root/sentryusb.conf";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,btleplug=warn".into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let verb = match std::env::args().nth(1) {
        Some(v) => v,
        None => {
            eprintln!(
                "usage: sentryusb-ble-action <wake|sentry-on|sentry-off|charge-port-open|charge-port-close>"
            );
            return ExitCode::from(1);
        }
    };
    let action = match verb.as_str() {
        "wake" => actions::wake_vehicle(),
        "sentry-on" => actions::set_sentry_mode(true),
        "sentry-off" => actions::set_sentry_mode(false),
        "charge-port-open" => actions::charge_port_open(),
        "charge-port-close" => actions::charge_port_close(),
        other => {
            eprintln!("unknown verb '{}'", other);
            return ExitCode::from(1);
        }
    };

    match run(verb.as_str(), action).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e:#}");
            // Map error categories to distinct exit codes so
            // awake_start can choose to retry vs log-and-skip.
            let msg = format!("{e:#}");
            if msg.contains("config") || msg.contains("TESLA_BLE_VIN") || msg.contains("key file")
            {
                ExitCode::from(2)
            } else if msg.contains("fault code") {
                ExitCode::from(4)
            } else {
                ExitCode::from(3)
            }
        }
    }
}

async fn run(verb: &str, action: ActionPayload) -> Result<()> {
    let (vin, adapter) = load_config()?;
    info!(
        "sentryusb-ble-action: verb={} domain={:?} inner={} bytes vin={}…{}",
        verb,
        action.domain,
        action.inner.len(),
        &vin[..3],
        &vin[vin.len() - 4..]
    );

    let keypair = KeyPair::load(Path::new(KEY_FILE))
        .with_context(|| format!("loading BLE key file {KEY_FILE}"))?;
    let session = PersistentSession::start(keypair, vin, adapter);

    // One-shot — wrap in an outer timeout so the script doesn't hang
    // indefinitely if the car never advertises.
    let resp = tokio::time::timeout(
        Duration::from_secs(60),
        session.send_action(action),
    )
    .await
    .context("BLE action timed out after 60s")?;
    session.shutdown().await;

    match resp {
        Ok(bytes) => {
            info!("action OK; decrypted response = {} bytes", bytes.len());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Read TESLA_BLE_VIN and (optionally) BLE_ADAPTER from sentryusb.conf.
/// Returns (vin, Some(adapter)) or fails if VIN missing.
fn load_config() -> Result<(String, Option<String>)> {
    let raw = std::fs::read_to_string(CONFIG_FILE)
        .with_context(|| format!("reading {CONFIG_FILE}"))?;
    let mut vin: Option<String> = None;
    let mut adapter: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("export TESLA_BLE_VIN=") {
            vin = Some(unquote(rest).to_uppercase());
        } else if let Some(rest) = trimmed.strip_prefix("export BLE_ADAPTER=") {
            adapter = Some(unquote(rest));
        }
    }
    let vin = vin.context("TESLA_BLE_VIN not set in /root/sentryusb.conf")?;
    if vin.len() != 17 {
        anyhow::bail!("TESLA_BLE_VIN must be 17 chars, got {}", vin.len());
    }
    Ok((vin, adapter))
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"'))
        || (t.starts_with('\'') && t.ends_with('\''))
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}
