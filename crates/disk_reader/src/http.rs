//! Localhost HTTP API for Sentry Studio.
//!
//! The helper may run elevated, so the surface is deliberately small and
//! locked down: binds 127.0.0.1 only, every request must carry the
//! 128-bit hex token (query param `token` or `X-Auth-Token` header — query
//! param because `<video>` elements cannot set headers), and the whole
//! process is read-only by construction. Startup rendezvous happens via
//! `--handshake-file` (elevated children lose stdout): `{port, token, pid}`
//! written 0600.
//!
//! Routes:
//!   GET /healthz                     liveness (auth required, like all)
//!   GET /api/info                    archive summary
//!   GET /fs/{*path}?op=ls            directory listing (JSON, shaped like
//!                                    Studio's fs:readDir results)
//!   GET /fs/{*path}                  file bytes; supports Range

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Path as AxPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use crate::{archive::Archive, device::Disk};

/// The archive opens in the background after the server is already up, so
/// Studio can watch progress instead of staring at a silent process.
enum ArchiveState {
    Opening,
    Ready(Archive),
    Failed(String),
}

struct AppState {
    archive: Mutex<ArchiveState>,
    /// Human-readable progress while Opening.
    progress: Mutex<String>,
    token: String,
    last_request: AtomicU64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct AuthQuery {
    token: Option<String>,
    op: Option<String>,
}

fn authorized(state: &AppState, headers: &HeaderMap, q: &AuthQuery) -> bool {
    let presented = q
        .token
        .as_deref()
        .or_else(|| headers.get("x-auth-token").and_then(|v| v.to_str().ok()));
    // Constant-time-ish compare; both sides are fixed-length hex.
    match presented {
        Some(p) if p.len() == state.token.len() => {
            p.bytes()
                .zip(state.token.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
        }
        _ => false,
    }
}

pub async fn serve(
    disk: Disk,
    port: u16,
    handshake_file: Option<PathBuf>,
    idle_exit_secs: u64,
) -> Result<()> {
    let mut token_bytes = [0u8; 16];
    getrandom::fill(&mut token_bytes)
        .map_err(|e| anyhow::anyhow!("crypto random: {e}"))?;
    let token = hex::encode(token_bytes);

    let state = Arc::new(AppState {
        archive: Mutex::new(ArchiveState::Opening),
        progress: Mutex::new("Starting".to_string()),
        token: token.clone(),
        last_request: AtomicU64::new(now_secs()),
    });

    // Index the archive off-thread; the server answers /api/status while
    // this runs (81 snapshots over USB can take a while).
    {
        let state = Arc::clone(&state);
        tokio::task::spawn_blocking(move || {
            let progress_state = Arc::clone(&state);
            let result = Archive::open_with_progress(&disk, &move |msg| {
                *progress_state.progress.lock().unwrap() = msg;
            });
            *state.archive.lock().unwrap() = match result {
                Ok(a) => ArchiveState::Ready(a),
                Err(e) => {
                    tracing::error!("archive open failed: {e:#}");
                    ArchiveState::Failed(format!("{e:#}"))
                }
            };
            state.last_request.store(now_secs(), Ordering::Relaxed);
        });
    }

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/status", get(api_status))
        .route("/api/info", get(api_info))
        .route("/fs/{*path}", get(fs_get))
        .with_state(Arc::clone(&state));

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    let local = listener.local_addr()?;

    let handshake = serde_json::json!({
        "port": local.port(),
        "token": token,
        "pid": std::process::id(),
    })
    .to_string();
    match &handshake_file {
        Some(path) => {
            write_private(path, &handshake)?;
            tracing::info!("handshake written to {}", path.display());
        }
        None => println!("{handshake}"),
    }

    if idle_exit_secs > 0 {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                tick.tick().await;
                let idle = now_secs().saturating_sub(state.last_request.load(Ordering::Relaxed));
                if idle > idle_exit_secs {
                    tracing::info!("idle for {idle}s, exiting");
                    std::process::exit(0);
                }
            }
        });
    }

    tracing::info!("serving on http://{local}");
    axum::serve(listener, app).await.context("serve")?;
    Ok(())
}

fn write_private(path: &std::path::Path, contents: &str) -> Result<()> {
    use std::io::Write as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .with_context(|| format!("write {}", path.display()))?;
    f.write_all(contents.as_bytes())?;
    Ok(())
}

async fn healthz(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AuthQuery>,
) -> Response {
    if !authorized(&state, &headers, &q) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.last_request.store(now_secs(), Ordering::Relaxed);
    "ok".into_response()
}

async fn api_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AuthQuery>,
) -> Response {
    if !authorized(&state, &headers, &q) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.last_request.store(now_secs(), Ordering::Relaxed);
    let body = match &*state.archive.lock().unwrap() {
        ArchiveState::Opening => serde_json::json!({
            "state": "opening",
            "progress": state.progress.lock().unwrap().clone(),
        }),
        ArchiveState::Ready(_) => serde_json::json!({ "state": "ready" }),
        ArchiveState::Failed(e) => serde_json::json!({ "state": "failed", "error": e }),
    };
    Json(body).into_response()
}

/// Respond 503 (opening) / 500 (failed) unless the archive is ready.
macro_rules! require_ready {
    ($state:expr) => {
        match &*$state {
            ArchiveState::Ready(ar) => ar,
            ArchiveState::Opening => {
                return (StatusCode::SERVICE_UNAVAILABLE, "archive still opening").into_response()
            }
            ArchiveState::Failed(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.clone()).into_response()
            }
        }
    };
}

async fn api_info(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AuthQuery>,
) -> Response {
    if !authorized(&state, &headers, &q) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.last_request.store(now_secs(), Ordering::Relaxed);
    let guard = state.archive.lock().unwrap();
    let ar = require_ready!(guard);
    Json(serde_json::json!({
        "fileCount": ar.file_count(),
        "sources": ar.sources.iter().map(|s| s.label.clone()).collect::<Vec<_>>(),
        "warnings": ar.warnings,
    }))
    .into_response()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "mp4" => "video/mp4",
        "json" => "application/json",
        "png" => "image/png",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

/// Parse a `Range: bytes=a-b` header against a file of `size` bytes.
fn parse_range(headers: &HeaderMap, size: u64) -> Option<(u64, u64)> {
    let raw = headers.get(header::RANGE)?.to_str().ok()?;
    let spec = raw.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (start_s, end_s) = spec.split_once('-')?;
    if start_s.is_empty() {
        // suffix form: last N bytes
        let n: u64 = end_s.parse().ok()?;
        let start = size.saturating_sub(n);
        return Some((start, size.saturating_sub(1)));
    }
    let start: u64 = start_s.parse().ok()?;
    let end: u64 = if end_s.is_empty() {
        size.saturating_sub(1)
    } else {
        end_s.parse::<u64>().ok()?.min(size.saturating_sub(1))
    };
    (start <= end && start < size).then_some((start, end))
}

async fn fs_get(
    State(state): State<Arc<AppState>>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
    Query(q): Query<AuthQuery>,
) -> Response {
    if !authorized(&state, &headers, &q) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.last_request.store(now_secs(), Ordering::Relaxed);
    let path = path.trim_matches('/').to_string();

    if q.op.as_deref() == Some("ls") {
        let guard = state.archive.lock().unwrap();
        let ar = require_ready!(guard);
        let entries: Vec<_> = ar
            .read_dir(&path)
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "isDirectory": e.is_dir,
                    "isFile": !e.is_dir,
                    "size": e.size,
                })
            })
            .collect();
        return Json(entries).into_response();
    }

    let size = {
        let guard = state.archive.lock().unwrap();
        let ar = require_ready!(guard);
        match ar.stat(&path) {
            Some(meta) => meta.size,
            None => return (StatusCode::NOT_FOUND, "no such file").into_response(),
        }
    };

    let range = parse_range(&headers, size);
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    let len = if size == 0 { 0 } else { end - start + 1 };

    // Stream the range in chunks off the blocking archive reader.
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(4);
    let stream_state = Arc::clone(&state);
    let stream_path = path.clone();
    tokio::task::spawn_blocking(move || {
        const CHUNK: usize = 1 << 20;
        let mut offset = start;
        let mut remaining = len;
        while remaining > 0 {
            let want = CHUNK.min(remaining as usize);
            let chunk = {
                let guard = stream_state.archive.lock().unwrap();
                match &*guard {
                    ArchiveState::Ready(ar) => ar.read_range(&stream_path, offset, want),
                    _ => break,
                }
            };
            stream_state
                .last_request
                .store(now_secs(), Ordering::Relaxed);
            match chunk {
                Ok(buf) if !buf.is_empty() => {
                    let n = buf.len() as u64;
                    if tx.blocking_send(Ok(bytes::Bytes::from(buf))).is_err() {
                        break; // client went away
                    }
                    offset += n;
                    remaining -= n;
                }
                Ok(_) => break,
                Err(e) => {
                    let _ = tx.blocking_send(Err(std::io::Error::other(format!("{e:#}"))));
                    break;
                }
            }
        }
    });

    let body = Body::from_stream(ReceiverStream::new(rx));
    let mut resp = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, content_type(&path))
        .header(header::CONTENT_LENGTH, len);
    resp = if range.is_some() {
        resp.status(StatusCode::PARTIAL_CONTENT).header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{size}"),
        )
    } else {
        resp.status(StatusCode::OK)
    };
    resp.body(body).unwrap()
}

