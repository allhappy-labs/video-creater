use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Extension, Path as AxumPath, Query, State};
use axum::http::Request;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::app_service::operation::{AuthorizationScope, MutationClass};
use crate::project::import::import_media_files_with_names;
use crate::project::split::load_split_project;

use super::artifact::{attachment_header, export_artifact, safe_download_name};
use super::assets::asset_router;
use super::editor_lease::{LeaseError, LeaseManager};
use super::event_hub::{EventHub, ResumeResult};
use super::identity::{trusted_proxy_identity, ProxyIdentity, ProxyMode};
use super::pairing::PairingManager;
use super::project_catalog::ProjectCatalog;
use super::registry::RpcRegistry;
use super::resource_ticket::{
    ResourceAccess, ResourceTicketError, ResourceTicketStore, TicketStatus,
};
use super::rpc::{RpcDispatcher, RpcEngine, RpcResponse};
use super::session::{SessionError, SessionStore};
use super::upload::{UploadError, UploadStore};

#[derive(Debug, Clone)]
pub struct HostHttpConfig {
    pub assets_dir: PathBuf,
    pub session_file: PathBuf,
    pub public_origin: String,
    pub host_label: String,
    pub pairing_code: String,
    pub reusable_pairing_code: bool,
    pub issued_at: u64,
    pub now_override: Option<u64>,
    pub uploads_dir: PathBuf,
    pub upload_max_bytes: u64,
    pub upload_reserve_bytes: u64,
    pub project_catalog: ProjectCatalog,
    /// Allows direct, non-proxied requests only in the explicit debug E2E harness.
    pub allow_direct_test_identity: bool,
}

struct HostHttpState {
    origin: String,
    host_label: String,
    now_override: Option<u64>,
    pairing: PairingManager,
    sessions: SessionStore,
    rpc: RpcEngine,
    events: Arc<EventHub>,
    uploads: UploadStore,
    projects: ProjectCatalog,
    leases: LeaseManager,
    resource_tickets: ResourceTicketStore,
    allow_direct_test_identity: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairRequest {
    code: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportUploadRequest {
    project_id: String,
    expected_revision: u64,
}

#[derive(Debug, Clone, Copy)]
enum ImportUploadFailure {
    LeaseHeld,
    LeaseInvalid,
    ProjectNotFound,
    RevisionConflict,
    UploadNotFound,
    ImportFailed,
    Internal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaTicketRequest {
    project_id: String,
    relative_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactTicketRequest {
    project_id: String,
    artifact_id: String,
}

#[derive(Debug, Deserialize, Default)]
struct LeaseRequest {
    #[serde(default)]
    takeover: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairResponse {
    pub authenticated: bool,
    pub session_id: String,
    pub csrf_token: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    pub protocol_version: u32,
    pub host_label: String,
}

pub fn host_router(
    config: HostHttpConfig,
    dispatcher: Arc<dyn RpcDispatcher>,
) -> Result<Router, String> {
    let upload_body_limit = usize::try_from(config.upload_max_bytes).unwrap_or(usize::MAX);
    let uploads = UploadStore::new(
        config.uploads_dir,
        config.upload_max_bytes,
        config.upload_reserve_bytes,
    )
    .map_err(|_| "upload staging could not be opened".to_string())?;
    let cleanup_now = config.now_override.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });
    uploads
        .cleanup_expired(cleanup_now, 24 * 60 * 60)
        .map_err(|_| "expired upload cleanup failed".to_string())?;
    let state = Arc::new(HostHttpState {
        origin: config.public_origin.trim_end_matches('/').to_owned(),
        host_label: config.host_label,
        now_override: config.now_override,
        pairing: if config.reusable_pairing_code {
            PairingManager::reusable_for_tests(&config.pairing_code, config.issued_at)
        } else {
            PairingManager::new(&config.pairing_code, config.issued_at)
        },
        sessions: SessionStore::load(
            config.session_file,
            &config.public_origin,
            30 * 24 * 60 * 60,
        )
        .map_err(|_| "remote session store could not be opened".to_string())?,
        rpc: RpcEngine::new(dispatcher),
        events: Arc::new(EventHub::new(1_024, 128)),
        uploads,
        projects: config.project_catalog,
        leases: LeaseManager::new(30, 10),
        resource_tickets: ResourceTicketStore::new(5 * 60, 30 * 60),
        allow_direct_test_identity: config.allow_direct_test_identity,
    });
    let api = Router::new()
        .route("/api/v1/pair", post(pair))
        .route("/api/v1/session", get(session))
        .route("/api/v1/session/logout", post(logout_session))
        .route("/api/v1/session/rotate", post(rotate_session))
        .route("/api/v1/admin/sessions", get(list_sessions))
        .route(
            "/api/v1/admin/sessions/{session_id}/revoke",
            post(revoke_session),
        )
        .route(
            "/api/v1/admin/sessions/revoke-all",
            post(revoke_all_sessions),
        )
        .route(
            "/api/v1/rpc",
            post(rpc).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route("/api/v1/events", get(events))
        .route(
            "/api/v1/uploads",
            post(stage_upload).layer(DefaultBodyLimit::max(upload_body_limit)),
        )
        .route("/api/v1/uploads/{upload_id}/import", post(import_upload))
        .route("/api/v1/resource-tickets/media", post(mint_media_tickets))
        .route(
            "/api/v1/resource-tickets/artifact",
            post(mint_artifact_ticket),
        )
        .route(
            "/api/v1/media/{ticket}",
            get(media_resource).head(media_resource),
        )
        .route(
            "/api/v1/artifacts/{ticket}",
            get(artifact_resource).head(artifact_resource),
        )
        .route("/api/v1/projects/{project_id}/lease", post(project_lease))
        .with_state(Arc::clone(&state))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            require_trusted_identity,
        ));
    Ok(asset_router(&config.assets_dir).merge(api))
}

async fn require_trusted_identity(
    State(state): State<Arc<HostHttpState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let identity = if state.allow_direct_test_identity {
        Some(ProxyIdentity {
            login: "remote-e2e@localhost".into(),
            display_name: "Remote E2E".into(),
        })
    } else {
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .copied()
            .and_then(|ConnectInfo(address)| {
                trusted_proxy_identity(address.ip(), ProxyMode::TailscaleServe, &headers)
            })
    };
    let Some(identity) = identity else {
        return forbidden("request did not come from the trusted Tailscale Serve proxy");
    };
    if let Some(credential) = session_cookie(request.headers()) {
        if let Err(error) =
            state
                .sessions
                .authenticate_for_identity(credential, &identity.login, state.now())
        {
            // A revoked/expired browser must be able to replace its own cookie by pairing again.
            // A valid cookie bound to a different Tailscale identity remains a hard failure.
            if request.uri().path() != "/api/v1/pair" || error == SessionError::Identity {
                return unauthorized();
            }
        }
    }
    request.extensions_mut().insert(identity);
    next.run(request).await
}

async fn project_lease(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<LeaseRequest>,
) -> Response {
    let session =
        match authorize_state_change(&state, &headers, "editor access authentication failed") {
            Ok(session) => session,
            Err(response) => return *response,
        };
    if state.projects.resolve(&project_id).is_err() {
        return not_found("project_not_found");
    }
    let lease = if request.takeover {
        state
            .leases
            .takeover(&project_id, &session.session_id, state.now())
    } else {
        state
            .leases
            .acquire_or_renew_for_session(&project_id, &session.session_id, state.now())
    };
    match lease {
        Ok(lease) => Json(json!({
            "mode":"editor",
            "editorLeaseToken":lease.token,
            "expiresAt":lease.expires_at
        }))
        .into_response(),
        Err(LeaseError::Held) => {
            let editor_display_name = state
                .leases
                .current(&project_id, state.now())
                .and_then(|lease| {
                    state
                        .sessions
                        .list()
                        .into_iter()
                        .find(|candidate| candidate.session_id == lease.session_id)
                })
                .map(|session| session.display_name)
                .unwrap_or_else(|| "Another device".into());
            (
                StatusCode::CONFLICT,
                Json(json!({"mode":"readOnly","editorDisplayName":editor_display_name})),
            )
                .into_response()
        }
        Err(_) => forbidden("editor access could not be acquired"),
    }
}

async fn mint_media_tickets(
    State(state): State<Arc<HostHttpState>>,
    headers: HeaderMap,
    Json(request): Json<MediaTicketRequest>,
) -> Response {
    let session =
        match authorize_state_change(&state, &headers, "media ticket authentication failed") {
            Ok(session) => session,
            Err(response) => return *response,
        };
    if request.relative_paths.is_empty() || request.relative_paths.len() > 1_024 {
        return bad_request("media ticket request is invalid");
    }
    let project_dir = match state.projects.resolve(&request.project_id) {
        Ok(path) => path,
        Err(_) => return not_found("project_not_found"),
    };
    let project = match load_split_project(&project_dir) {
        Ok(project) => project,
        Err(_) => return not_found("project_not_found"),
    };
    let mut allowed = serde_json::to_value(&project)
        .map(|value| collect_relative_paths(&value))
        .unwrap_or_default();
    allowed.extend(recorded_render_output_paths(&project_dir));
    let mut urls = BTreeMap::new();
    for relative_path in request.relative_paths {
        if relative_path.len() > 1_024 || !allowed.contains(&relative_path) {
            return forbidden("media resource is not recorded by this project");
        }
        let Ok(path) = super::media::resolve_project_resource(
            &state.projects,
            &request.project_id,
            &relative_path,
        ) else {
            continue;
        };
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(safe_download_name)
            .unwrap_or_else(|| "media".into());
        let ticket = state.resource_tickets.mint(
            &session.session_id,
            &request.project_id,
            ResourceAccess::Media,
            &relative_path,
            &display_name,
            state.now(),
        );
        urls.insert(
            format!("{}/{}", request.project_id, relative_path),
            format!("/api/v1/media/{}", ticket.token),
        );
    }
    Json(json!({"urls":urls})).into_response()
}

async fn mint_artifact_ticket(
    State(state): State<Arc<HostHttpState>>,
    headers: HeaderMap,
    Json(request): Json<ArtifactTicketRequest>,
) -> Response {
    let session =
        match authorize_state_change(&state, &headers, "artifact ticket authentication failed") {
            Ok(session) => session,
            Err(response) => return *response,
        };
    let project_dir = match state.projects.resolve(&request.project_id) {
        Ok(path) => path,
        Err(_) => return not_found("project_not_found"),
    };
    let project = match load_split_project(&project_dir) {
        Ok(project) => project,
        Err(_) => return not_found("project_not_found"),
    };
    let Some(artifact) = export_artifact(&project, &request.artifact_id) else {
        return not_found("artifact_not_found");
    };
    if super::media::resolve_project_resource(&state.projects, &request.project_id, &artifact.path)
        .is_err()
    {
        return not_found("artifact_not_found");
    }
    let display_name = safe_download_name(&artifact.path);
    let ticket = state.resource_tickets.mint(
        &session.session_id,
        &request.project_id,
        ResourceAccess::Artifact,
        &artifact.path,
        &display_name,
        state.now(),
    );
    Json(json!({"url":format!("/api/v1/artifacts/{}", ticket.token),"expiresAt":ticket.expires_at}))
        .into_response()
}

async fn media_resource(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(ticket): AxumPath<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    serve_resource(state, ticket, method, headers, ResourceAccess::Media).await
}

async fn artifact_resource(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(ticket): AxumPath<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    serve_resource(state, ticket, method, headers, ResourceAccess::Artifact).await
}

async fn serve_resource(
    state: Arc<HostHttpState>,
    ticket: String,
    method: Method,
    headers: HeaderMap,
    access: ResourceAccess,
) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let session = match state.sessions.authenticate(credential, state.now()) {
        Ok(session) => session,
        Err(_) => return unauthorized(),
    };
    let validated =
        match state
            .resource_tickets
            .validate(&ticket, &session.session_id, access, state.now())
        {
            Ok(validated) => validated,
            Err(ResourceTicketError::Scope) => {
                return forbidden("resource ticket is not valid for this session")
            }
            Err(_) => return not_found("resource_not_found"),
        };
    if validated.status == TicketStatus::Expired {
        return match state.resource_tickets.refresh(
            &ticket,
            &session.session_id,
            access,
            state.now(),
        ) {
            Ok(refreshed) => {
                let prefix = if access == ResourceAccess::Media {
                    "media"
                } else {
                    "artifacts"
                };
                Response::builder()
                    .status(StatusCode::TEMPORARY_REDIRECT)
                    .header(
                        header::LOCATION,
                        format!("/api/v1/{prefix}/{}", refreshed.token),
                    )
                    .header(header::CACHE_CONTROL, "private, no-store")
                    .body(Body::empty())
                    .unwrap_or_else(|_| internal())
            }
            Err(_) => not_found("resource_not_found"),
        };
    }
    let path = match super::media::resolve_project_resource(
        &state.projects,
        &validated.grant.project_id,
        &validated.grant.relative_path,
    ) {
        Ok(path) => path,
        Err(_) => return not_found("resource_not_found"),
    };
    let total = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata.len(),
        Err(_) => return not_found("resource_not_found"),
    };
    let range = match super::media::parse_single_range(
        headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        total,
    ) {
        Ok(range) => range,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "private, no-store")
                .body(Body::empty())
                .unwrap_or_else(|_| internal())
        }
    };
    let (status, start, end) = range
        .map(|range| (StatusCode::PARTIAL_CONTENT, range.start, range.end))
        .unwrap_or((StatusCode::OK, 0, total.saturating_sub(1)));
    let length = if total == 0 { 0 } else { end - start + 1 };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, super::media::content_type(&path))
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, no-store")
        .header("x-content-type-options", "nosniff");
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    if access == ResourceAccess::Artifact {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            attachment_header(&validated.grant.display_name),
        );
    }
    if method == Method::HEAD || length == 0 {
        return builder.body(Body::empty()).unwrap_or_else(|_| internal());
    }
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(_) => return not_found("resource_not_found"),
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return internal();
    }
    let stream = ReaderStream::new(file.take(length));
    builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| internal())
}

fn authorize_state_change(
    state: &HostHttpState,
    headers: &HeaderMap,
    message: &str,
) -> Result<super::session::SessionView, Box<Response>> {
    let Some(credential) = session_cookie(headers) else {
        return Err(Box::new(unauthorized()));
    };
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state
        .sessions
        .authorize_mutation(credential, csrf, origin, state.now())
        .map_err(|_| Box::new(forbidden(message)))
}

fn collect_relative_paths(value: &serde_json::Value) -> BTreeSet<String> {
    fn visit(value: &serde_json::Value, paths: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, paths);
                }
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    if key == "relativePath" {
                        if let Some(path) = value.as_str() {
                            paths.insert(path.to_owned());
                        }
                    } else {
                        visit(value, paths);
                    }
                }
            }
            _ => {}
        }
    }
    let mut paths = BTreeSet::new();
    visit(value, &mut paths);
    paths
}

fn recorded_render_output_paths(project_dir: &std::path::Path) -> BTreeSet<String> {
    let Ok(bytes) = std::fs::read(project_dir.join("renders/index.json")) else {
        return BTreeSet::new();
    };
    let Ok(index) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return BTreeSet::new();
    };
    index
        .get("reports")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|report| report.get("outputPath").and_then(serde_json::Value::as_str))
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

async fn import_upload(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(upload_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<ImportUploadRequest>,
) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let session = match state
        .sessions
        .authorize_mutation(credential, csrf, origin, state.now())
    {
        Ok(session) => session,
        Err(_) => return forbidden("upload import authentication failed"),
    };
    let state_for_import = Arc::clone(&state);
    let now = state.now();
    let outcome = tokio::task::spawn_blocking(move || {
        let lease = state_for_import
            .leases
            .acquire_or_renew_for_session(&request.project_id, &session.session_id, now)
            .map_err(|error| match error {
                LeaseError::Held => ImportUploadFailure::LeaseHeld,
                _ => ImportUploadFailure::LeaseInvalid,
            })?;
        let token = lease.token.clone();
        state_for_import
            .leases
            .with_valid_lease(
                &request.project_id,
                &session.session_id,
                &token,
                now,
                || {
                    let project_dir = state_for_import
                        .projects
                        .resolve(&request.project_id)
                        .map_err(|_| ImportUploadFailure::ProjectNotFound)?;
                    let project = load_split_project(&project_dir)
                        .map_err(|_| ImportUploadFailure::ProjectNotFound)?;
                    if project.content_revision != request.expected_revision {
                        return Err(ImportUploadFailure::RevisionConflict);
                    }
                    let source = state_for_import
                        .uploads
                        .completed_path(&upload_id)
                        .map_err(|_| ImportUploadFailure::UploadNotFound)?;
                    let metadata = state_for_import
                        .uploads
                        .metadata(&upload_id)
                        .map_err(|_| ImportUploadFailure::UploadNotFound)?;
                    let names = BTreeMap::from([(source.clone(), metadata.display_name)]);
                    let imported =
                        import_media_files_with_names(&project_dir, project, &[source], &names)
                            .map_err(|_| ImportUploadFailure::ImportFailed)?;
                    let _ = state_for_import.uploads.revoke(&upload_id);
                    let mut value = serde_json::to_value(imported)
                        .map_err(|_| ImportUploadFailure::Internal)?;
                    if let Some(object) = value.as_object_mut() {
                        object.insert("editorLeaseToken".into(), json!(token));
                    }
                    Ok(value)
                },
            )
            .map_err(|_| ImportUploadFailure::LeaseInvalid)?
    })
    .await;
    match outcome {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(ImportUploadFailure::LeaseHeld)) => (StatusCode::CONFLICT, Json(json!({"error":"editor_lease_held","message":"another device is editing this project"}))).into_response(),
        Ok(Err(ImportUploadFailure::LeaseInvalid)) => forbidden("editor lease could not be acquired"),
        Ok(Err(ImportUploadFailure::ProjectNotFound)) => not_found("project_not_found"),
        Ok(Err(ImportUploadFailure::RevisionConflict)) => (StatusCode::CONFLICT, Json(json!({"error":"revision_conflict"}))).into_response(),
        Ok(Err(ImportUploadFailure::UploadNotFound)) => not_found("upload_not_found"),
        Ok(Err(ImportUploadFailure::ImportFailed)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"media_import_failed","message":"uploaded media could not be imported"}))).into_response(),
        Ok(Err(ImportUploadFailure::Internal)) | Err(_) => internal(),
    }
}

async fn stage_upload(
    State(state): State<Arc<HostHttpState>>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if state
        .sessions
        .authorize_mutation(credential, csrf, origin, state.now())
        .is_err()
    {
        return forbidden("upload authentication failed");
    }
    let Some(declared_size) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return bad_request("upload content length is required");
    };
    let uploads = state.uploads.clone();
    let display_name = query.name;
    let mut writer = match tokio::task::spawn_blocking(move || {
        uploads.begin(&display_name, declared_size)
    })
    .await
    {
        Ok(Ok(writer)) => writer,
        Ok(Err(error)) => return upload_error(error),
        Err(_) => return internal(),
    };
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(_) => return bad_request("upload stream failed"),
        };
        let written = tokio::task::spawn_blocking(move || {
            let result = writer.write_chunk(&chunk);
            (writer, result)
        })
        .await;
        match written {
            Ok((next_writer, Ok(()))) => writer = next_writer,
            Ok((_next_writer, Err(error))) => return upload_error(error),
            Err(_) => return internal(),
        }
    }
    match tokio::task::spawn_blocking(move || writer.finish()).await {
        Ok(Ok(completed)) => Json(completed).into_response(),
        Ok(Err(error)) => upload_error(error),
        Err(_) => internal(),
    }
}

async fn events(
    State(state): State<Arc<HostHttpState>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let protocols = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let csrf = protocol_value(protocols, "vc-csrf.").unwrap_or_default();
    let after_sequence = protocol_value(protocols, "vc-resume.")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let session = match state
        .sessions
        .authorize_mutation(credential, csrf, origin, state.now())
    {
        Ok(session) => session,
        Err(_) => return forbidden("event stream authentication failed"),
    };
    let hub = Arc::clone(&state.events);
    let state_for_socket = Arc::clone(&state);
    ws.protocols(["vc-events"]).on_upgrade(move |socket| {
        event_socket(
            socket,
            hub,
            state_for_socket,
            session.session_id,
            after_sequence,
        )
    })
}

async fn event_socket(
    socket: WebSocket,
    hub: Arc<EventHub>,
    state: Arc<HostHttpState>,
    session_id: String,
    after_sequence: u64,
) {
    let source = match hub.subscribe(after_sequence) {
        Ok(source) => source,
        Err(ResumeResult::SnapshotRequired {
            oldest_sequence,
            latest_sequence,
        }) => {
            let (mut sender, _) = socket.split();
            let message = json!({
                "type": "snapshotRequired",
                "oldestSequence": oldest_sequence,
                "latestSequence": latest_sequence,
            });
            let _ = sender.send(Message::Text(message.to_string().into())).await;
            state.leases.disconnected(&session_id, state.now());
            return;
        }
        Err(ResumeResult::Events(_)) => {
            state.leases.disconnected(&session_id, state.now());
            return;
        }
    };
    let (forward, mut outbound) = tokio::sync::mpsc::channel(32);
    std::thread::spawn(move || {
        while let Ok(event) = source.recv() {
            if forward.blocking_send(event).is_err() {
                break;
            }
        }
    });
    let (mut sender, mut receiver) = socket.split();
    loop {
        tokio::select! {
            event = outbound.recv() => {
                let Some(event) = event else { break };
                let Ok(text) = serde_json::to_string(&event) else { break };
                if sender.send(Message::Text(text.into())).await.is_err() { break }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    state.leases.disconnected(&session_id, state.now());
}

async fn pair(
    State(state): State<Arc<HostHttpState>>,
    Extension(identity): Extension<ProxyIdentity>,
    headers: HeaderMap,
    Json(request): Json<PairRequest>,
) -> Response {
    if exact_origin(&headers, &state.origin).is_err() {
        return forbidden("request origin is not allowed");
    }
    if request.display_name.trim().is_empty() || request.display_name.len() > 100 {
        return bad_request("device name is invalid");
    }
    if state.pairing.verify(&request.code, state.now()).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"pairing_failed"})),
        )
            .into_response();
    }
    let issued = match state.sessions.issue(
        request.display_name.trim(),
        Some(&identity.login),
        state.now(),
    ) {
        Ok(issued) => issued,
        Err(_) => return internal(),
    };
    let mut response = Json(PairResponse {
        authenticated: true,
        session_id: issued.session_id,
        csrf_token: issued.csrf_token,
        protocol_version: super::PROTOCOL_VERSION,
    })
    .into_response();
    if let Ok(value) = HeaderValue::from_str(&issued.cookie_header) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn session(State(state): State<Arc<HostHttpState>>, headers: HeaderMap) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(SessionResponse {
                authenticated: false,
                session_id: None,
                display_name: None,
                csrf_token: None,
                protocol_version: super::PROTOCOL_VERSION,
                host_label: state.host_label.clone(),
            }),
        )
            .into_response();
    };
    let view = match state.sessions.authenticate(credential, state.now()) {
        Ok(view) => view,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(SessionResponse {
                    authenticated: false,
                    session_id: None,
                    display_name: None,
                    csrf_token: None,
                    protocol_version: super::PROTOCOL_VERSION,
                    host_label: state.host_label.clone(),
                }),
            )
                .into_response()
        }
    };
    let csrf_token = match state.sessions.refresh_csrf(&view.session_id) {
        Ok(token) => token,
        Err(_) => return internal(),
    };
    Json(SessionResponse {
        authenticated: true,
        session_id: Some(view.session_id),
        display_name: Some(view.display_name),
        csrf_token: Some(csrf_token),
        protocol_version: super::PROTOCOL_VERSION,
        host_label: state.host_label.clone(),
    })
    .into_response()
}

async fn list_sessions(State(state): State<Arc<HostHttpState>>, headers: HeaderMap) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let current = match state.sessions.authenticate(credential, state.now()) {
        Ok(session) => session,
        Err(_) => return unauthorized(),
    };
    Json(json!({
        "currentSessionId": current.session_id,
        "sessions": state.sessions.list(),
    }))
    .into_response()
}

async fn logout_session(State(state): State<Arc<HostHttpState>>, headers: HeaderMap) -> Response {
    if authorize_state_change(&state, &headers, "logout authentication failed").is_err() {
        return forbidden("logout authentication failed");
    }
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    if state.sessions.logout(credential).is_err() {
        return unauthorized();
    }
    let mut response = Json(json!({"loggedOut":true})).into_response();
    clear_session_cookie(&mut response);
    response
}

async fn rotate_session(State(state): State<Arc<HostHttpState>>, headers: HeaderMap) -> Response {
    if authorize_state_change(&state, &headers, "session rotation authentication failed").is_err() {
        return forbidden("session rotation authentication failed");
    }
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let issued = match state.sessions.rotate(credential, state.now()) {
        Ok(issued) => issued,
        Err(_) => return unauthorized(),
    };
    let mut response = Json(json!({
        "authenticated": true,
        "sessionId": issued.session_id,
        "csrfToken": issued.csrf_token,
        "protocolVersion": super::PROTOCOL_VERSION,
    }))
    .into_response();
    if let Ok(cookie) = HeaderValue::from_str(&issued.cookie_header) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

async fn revoke_session(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(session_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let current =
        match authorize_state_change(&state, &headers, "session revoke authentication failed") {
            Ok(current) => current,
            Err(response) => return *response,
        };
    match state.sessions.revoke(&session_id) {
        Ok(()) => {
            let mut response = Json(json!({"revoked":true,"sessionId":session_id})).into_response();
            if current.session_id == session_id {
                clear_session_cookie(&mut response);
            }
            response
        }
        Err(_) => not_found("session_not_found"),
    }
}

async fn revoke_all_sessions(
    State(state): State<Arc<HostHttpState>>,
    headers: HeaderMap,
) -> Response {
    if authorize_state_change(&state, &headers, "session revoke authentication failed").is_err() {
        return forbidden("session revoke authentication failed");
    }
    match state.sessions.revoke_all() {
        Ok(()) => {
            let mut response = Json(json!({"revoked":true})).into_response();
            clear_session_cookie(&mut response);
            response
        }
        Err(_) => internal(),
    }
}

async fn rpc(State(state): State<Arc<HostHttpState>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(credential) = session_cookie(&headers) else {
        return unauthorized();
    };
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let session = match state
        .sessions
        .authorize_mutation(credential, csrf, origin, state.now())
    {
        Ok(session) => session,
        Err(SessionError::Origin | SessionError::Csrf) => {
            return forbidden("request authentication failed")
        }
        Err(_) => return unauthorized(),
    };
    let scopes = BTreeSet::from([
        AuthorizationScope::Session,
        AuthorizationScope::ProjectRead,
        AuthorizationScope::ProjectWrite,
        AuthorizationScope::HostAdmin,
    ]);
    let envelope = serde_json::from_slice::<super::rpc::RpcEnvelope>(&body).ok();
    let mutation_lease = envelope.as_ref().and_then(|envelope| {
        let operation = RpcRegistry::from_inventory().operation(&envelope.operation)?;
        (operation.requires_project && operation.mutation != MutationClass::Read).then(|| {
            (
                envelope.request_id.clone(),
                envelope.project_id.clone().unwrap_or_default(),
                envelope.editor_lease_token.clone().unwrap_or_default(),
                !envelope.operation.starts_with("cancel_"),
            )
        })
    });
    let state_for_rpc = Arc::clone(&state);
    let session_id = session.session_id;
    let now = state.now();
    let body = body.to_vec();
    let response = tokio::task::spawn_blocking(move || {
        if let Some((request_id, project_id, token, hold_during_operation)) = mutation_lease {
            if !hold_during_operation {
                return state_for_rpc
                    .leases
                    .validate_owner(&project_id, &session_id, &token)
                    .map(|()| state_for_rpc.rpc.execute(&session_id, &scopes, &body, now))
                    .map_err(|_| request_id);
            }
            return state_for_rpc
                .leases
                .with_valid_lease(&project_id, &session_id, &token, now, || {
                    state_for_rpc.rpc.execute(&session_id, &scopes, &body, now)
                })
                .map_err(|_| request_id);
        }
        Ok(state_for_rpc.rpc.execute(&session_id, &scopes, &body, now))
    })
    .await;
    match response {
        Ok(Ok(response)) => {
            if response.ok {
                if let Some(envelope) = envelope.as_ref() {
                    if RpcRegistry::from_inventory()
                        .operation(&envelope.operation)
                        .is_some_and(|operation| operation.mutation != MutationClass::Read)
                    {
                        state.events.publish(
                            "project-changed",
                            envelope.project_id.as_deref(),
                            json!({
                                "operation": envelope.operation,
                                "requestId": envelope.request_id,
                                "result": response.result.clone(),
                            }),
                        );
                    }
                }
            }
            Json(response).into_response()
        }
        Ok(Err(request_id)) => (
            StatusCode::CONFLICT,
            Json(RpcResponse::error(
                request_id,
                super::rpc::RpcErrorCode::Conflict,
                "editing access is held by another device",
                None,
            )),
        )
            .into_response(),
        Err(_) => internal(),
    }
}

impl HostHttpState {
    fn now(&self) -> u64 {
        self.now_override.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        })
    }
}

fn exact_origin(headers: &HeaderMap, expected: &str) -> Result<(), ()> {
    (headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        == Some(expected))
    .then_some(())
    .ok_or(())
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("vc_session="))
}

fn protocol_value<'a>(protocols: &'a str, prefix: &str) -> Option<&'a str> {
    protocols
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":"invalid_request","message":message})),
    )
        .into_response()
}
fn forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error":"forbidden","message":message})),
    )
        .into_response()
}
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":"unauthorized"})),
    )
        .into_response()
}

fn clear_session_cookie(response: &mut Response) {
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "vc_session=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
}

fn not_found(code: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error":code}))).into_response()
}

fn upload_error(error: UploadError) -> Response {
    let (status, code, message) = match error {
        UploadError::InvalidName => (
            StatusCode::BAD_REQUEST,
            "invalid_name",
            "file name is invalid",
        ),
        UploadError::TooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "file exceeds the upload limit",
        ),
        UploadError::SizeMismatch => (
            StatusCode::BAD_REQUEST,
            "size_mismatch",
            "uploaded size does not match",
        ),
        UploadError::InsufficientSpace => (
            StatusCode::INSUFFICIENT_STORAGE,
            "insufficient_space",
            "host storage is too low",
        ),
        UploadError::Busy => (
            StatusCode::TOO_MANY_REQUESTS,
            "upload_busy",
            "too many uploads are already in progress",
        ),
        UploadError::UnsupportedMedia => (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media",
            "file is not supported media",
        ),
        UploadError::InvalidId | UploadError::Io => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "upload_failed",
            "upload could not be completed",
        ),
    };
    (status, Json(json!({"error":code,"message":message}))).into_response()
}
fn internal() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":"internal"})),
    )
        .into_response()
}
