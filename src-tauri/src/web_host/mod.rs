pub mod artifact;
pub mod assets;
pub mod config;
pub mod dispatcher;
mod durable_rpc;
pub mod editor_lease;
pub mod event_hub;
pub mod http;
pub mod idempotency;
pub mod identity;
pub mod media;
pub mod pairing;
pub mod project_catalog;
pub mod registry;
pub mod request_outcome;
pub mod resource_ticket;
pub mod rpc;
pub mod session;
pub mod upload;

pub mod tailscale {
    pub use crate::remote_access::*;
}

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use config::HostConfig;
use dispatcher::HostDispatcher;
use http::{host_router, HostHttpConfig};
use pairing::generate_pairing_code;
use project_catalog::ProjectCatalog;

pub const PROTOCOL_VERSION: u32 = crate::REMOTE_PROTOCOL_VERSION;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthPayload {
    pub status: &'static str,
    pub protocol_version: u32,
    pub build_version: &'static str,
}

pub fn health_payload() -> HealthPayload {
    HealthPayload {
        status: "ok",
        protocol_version: PROTOCOL_VERSION,
        build_version: env!("CARGO_PKG_VERSION"),
    }
}

pub fn router(config: &HostConfig) -> Router {
    assets::asset_router(&config.assets_dir)
        .route("/healthz", get(|| async { Json(health_payload()) }))
}

pub async fn run(config: HostConfig) -> Result<(), String> {
    config.validate()?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|error| format!("web host bind failed: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("web host address failed: {error}"))?;
    let pairing_code = test_pairing_code()?.unwrap_or_else(generate_pairing_code);
    let public_origin =
        std::env::var("VIDEO_CREATER_HOST_ORIGIN").unwrap_or_else(|_| format!("http://{address}"));
    let state_dir = host_state_dir()?;
    let project_catalog = ProjectCatalog::new(project_roots(&state_dir))?;
    let app = host_router(
        HostHttpConfig {
            assets_dir: config.assets_dir.clone(),
            session_file: state_dir.join("remote-devices.json"),
            public_origin,
            host_label: host_label(),
            pairing_code: pairing_code.clone(),
            reusable_pairing_code: test_pairing_reusable(),
            issued_at: unix_time(),
            now_override: None,
            uploads_dir: state_dir.join("uploads"),
            upload_max_bytes: 1024 * 1024 * 1024,
            upload_reserve_bytes: 512 * 1024 * 1024,
            project_catalog: project_catalog.clone(),
            allow_direct_test_identity: test_direct_identity(),
        },
        Arc::new(HostDispatcher::with_project_catalog(project_catalog)),
    )?
    .route("/healthz", get(|| async { Json(health_payload()) }));
    println!(
        "{}",
        serde_json::json!({"event":"ready","address":address.to_string(),"protocolVersion":PROTOCOL_VERSION,"pairingCode":pairing_code})
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|error| format!("web host failed: {error}"))?;
    Ok(())
}

#[cfg(debug_assertions)]
fn test_pairing_code() -> Result<Option<String>, String> {
    if std::env::var("VIDEO_CREATER_REMOTE_E2E").as_deref() != Ok("1") {
        return Ok(None);
    }
    let code = std::env::var("VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE")
        .map_err(|_| "VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE is required in remote E2E mode")?;
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("VIDEO_CREATER_REMOTE_E2E_PAIRING_CODE must contain exactly six digits".into());
    }
    Ok(Some(code))
}

#[cfg(debug_assertions)]
fn test_pairing_reusable() -> bool {
    std::env::var("VIDEO_CREATER_REMOTE_E2E").as_deref() == Ok("1")
}

#[cfg(debug_assertions)]
fn test_direct_identity() -> bool {
    std::env::var("VIDEO_CREATER_REMOTE_E2E").as_deref() == Ok("1")
}

#[cfg(not(debug_assertions))]
fn test_direct_identity() -> bool {
    false
}

#[cfg(not(debug_assertions))]
fn test_pairing_reusable() -> bool {
    false
}

#[cfg(not(debug_assertions))]
fn test_pairing_code() -> Result<Option<String>, String> {
    Ok(None)
}

fn host_label() -> String {
    std::env::var("VIDEO_CREATER_HOST_LABEL")
        .ok()
        .filter(|label| !label.trim().is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "Video Creater host".into())
        .chars()
        .take(100)
        .collect()
}

fn project_roots(state_dir: &std::path::Path) -> Vec<PathBuf> {
    std::env::var_os("VIDEO_CREATER_PROJECT_ROOTS")
        .filter(|value| !value.is_empty())
        .map(|value| std::env::split_paths(&value).collect())
        .filter(|roots: &Vec<PathBuf>| !roots.is_empty())
        .unwrap_or_else(|| vec![state_dir.join("projects")])
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn host_state_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("VIDEO_CREATER_HOST_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("video-creater"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/video-creater"))
        .ok_or_else(|| {
            "VIDEO_CREATER_HOST_DATA_DIR is required when no home directory is available"
                .to_string()
        })
}

async fn shutdown_signal() {
    let control_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = control_c => {}, _ = terminate => {} }
}
