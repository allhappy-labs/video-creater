use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::app_service::operation::AuthorizationScope;

use super::idempotency::{IdempotencyDecision, IdempotencyStore};
use super::registry::{RegistryError, RpcRegistry};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcEnvelope {
    pub request_id: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_lease_token: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcErrorCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    RateLimited,
    Busy,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorBody {
    pub code: RpcErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcErrorBody>,
}

impl RpcResponse {
    pub fn success(request_id: impl Into<String>, result: Value) -> Self {
        Self {
            request_id: request_id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(
        request_id: impl Into<String>,
        code: RpcErrorCode,
        message: impl Into<String>,
        retry_after_ms: Option<u64>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            ok: false,
            result: None,
            error: Some(RpcErrorBody {
                code,
                message: sanitize_message(&message.into()),
                retry_after_ms,
            }),
        }
    }
}

fn sanitize_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(|part| {
            if part.starts_with('/')
                || (part.len() > 2
                    && part.as_bytes()[1] == b':'
                    && matches!(part.as_bytes()[2], b'/' | b'\\'))
            {
                "[path]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLimitError {
    Concurrent,
    Rate { retry_after_ms: u64 },
}

#[derive(Debug, Default)]
struct SessionWindow {
    started_at: u64,
    request_count: usize,
    in_flight: usize,
}

pub struct SessionLimiter {
    max_requests: usize,
    max_concurrent: usize,
    window_seconds: u64,
    sessions: Arc<Mutex<HashMap<String, SessionWindow>>>,
}

impl SessionLimiter {
    pub fn new(max_requests: usize, max_concurrent: usize, window_seconds: u64) -> Self {
        Self {
            max_requests: max_requests.max(1),
            max_concurrent: max_concurrent.max(1),
            window_seconds: window_seconds.max(1),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn begin(&self, session_id: &str, now: u64) -> Result<SessionPermit, SessionLimitError> {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let window = sessions
            .entry(session_id.to_owned())
            .or_insert_with(|| SessionWindow {
                started_at: now,
                ..SessionWindow::default()
            });
        if now.saturating_sub(window.started_at) >= self.window_seconds {
            window.started_at = now;
            window.request_count = 0;
        }
        if window.in_flight >= self.max_concurrent {
            return Err(SessionLimitError::Concurrent);
        }
        if window.request_count >= self.max_requests {
            return Err(SessionLimitError::Rate {
                retry_after_ms: window
                    .started_at
                    .saturating_add(self.window_seconds)
                    .saturating_sub(now)
                    .saturating_mul(1_000),
            });
        }
        window.request_count += 1;
        window.in_flight += 1;
        Ok(SessionPermit {
            session_id: session_id.to_owned(),
            sessions: Arc::clone(&self.sessions),
        })
    }
}

#[derive(Debug)]
pub struct SessionPermit {
    session_id: String,
    sessions: Arc<Mutex<HashMap<String, SessionWindow>>>,
}

impl Drop for SessionPermit {
    fn drop(&mut self) {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(window) = sessions.get_mut(&self.session_id) {
            window.in_flight = window.in_flight.saturating_sub(1);
        }
    }
}

pub trait RpcDispatcher: Send + Sync {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String>;
}

pub struct RpcEngine {
    registry: RpcRegistry,
    idempotency: IdempotencyStore,
    limiter: SessionLimiter,
    dispatcher: Arc<dyn RpcDispatcher>,
}

impl RpcEngine {
    pub fn new(dispatcher: Arc<dyn RpcDispatcher>) -> Self {
        Self {
            registry: RpcRegistry::from_inventory(),
            idempotency: IdempotencyStore::new(512, 300),
            limiter: SessionLimiter::new(60, 4, 60),
            dispatcher,
        }
    }

    pub fn execute(
        &self,
        session_id: &str,
        scopes: &BTreeSet<AuthorizationScope>,
        body: &[u8],
        now: u64,
    ) -> RpcResponse {
        if body.len() > 32 * 1024 * 1024 {
            return RpcResponse::error(
                "unknown",
                RpcErrorCode::InvalidRequest,
                "request body is too large",
                None,
            );
        }
        let request: RpcEnvelope = match serde_json::from_slice(body) {
            Ok(request) => request,
            Err(_) => {
                return RpcResponse::error(
                    "unknown",
                    RpcErrorCode::InvalidRequest,
                    "request body is not a valid RPC envelope",
                    None,
                )
            }
        };
        if request.request_id.is_empty() || request.request_id.len() > 128 {
            return RpcResponse::error(
                request.request_id,
                RpcErrorCode::InvalidRequest,
                "request ID is invalid",
                None,
            );
        }
        if let Err(error) = self.registry.validate(&request, scopes, body.len()) {
            let (code, message) = registry_error(error);
            return RpcResponse::error(&request.request_id, code, message, None);
        }
        let _permit = match self.limiter.begin(session_id, now) {
            Ok(permit) => permit,
            Err(SessionLimitError::Concurrent) => {
                return RpcResponse::error(
                    &request.request_id,
                    RpcErrorCode::Busy,
                    "too many concurrent requests",
                    Some(250),
                )
            }
            Err(SessionLimitError::Rate { retry_after_ms }) => {
                return RpcResponse::error(
                    &request.request_id,
                    RpcErrorCode::RateLimited,
                    "request rate limit exceeded",
                    Some(retry_after_ms),
                )
            }
        };
        let dispatched = self.idempotency.execute_once(
            session_id,
            &request.request_id,
            body,
            now,
            || match self.dispatcher.dispatch(&request) {
                Ok(result) => RpcResponse::success(&request.request_id, result),
                Err(error) if error.contains("revision conflict") => RpcResponse::error(
                    &request.request_id,
                    RpcErrorCode::Conflict,
                    "project revision conflict",
                    None,
                ),
                Err(error) => {
                    eprintln!(
                        "video-creater-host: operation {} failed: {}",
                        request.operation,
                        redact_dispatch_error(&error)
                    );
                    RpcResponse::error(
                        &request.request_id,
                        RpcErrorCode::Internal,
                        "operation failed internally",
                        None,
                    )
                }
            },
        );
        match dispatched {
            Ok(response) | Err(IdempotencyDecision::Replay(response)) => response,
            Err(IdempotencyDecision::PayloadConflict) => RpcResponse::error(
                &request.request_id,
                RpcErrorCode::Conflict,
                "request ID was already used with a different payload",
                None,
            ),
            Err(IdempotencyDecision::New) => RpcResponse::error(
                &request.request_id,
                RpcErrorCode::Internal,
                "request coordination failed",
                None,
            ),
        }
    }
}

fn redact_dispatch_error(error: &str) -> &str {
    if error.contains('/') || error.contains('\\') {
        "operation failed with a path-bearing diagnostic"
    } else {
        error
    }
}

fn registry_error(error: RegistryError) -> (RpcErrorCode, &'static str) {
    match error {
        RegistryError::UnknownOperation => (RpcErrorCode::NotFound, "operation is not available"),
        RegistryError::Forbidden => (RpcErrorCode::Forbidden, "operation is not permitted"),
        RegistryError::BodyTooLarge => (RpcErrorCode::InvalidRequest, "request body is too large"),
        RegistryError::ProjectRequired => (RpcErrorCode::InvalidRequest, "project ID is required"),
        RegistryError::ExpectedRevisionRequired => (
            RpcErrorCode::InvalidRequest,
            "expected revision is required",
        ),
        RegistryError::EditorLeaseRequired => (
            RpcErrorCode::InvalidRequest,
            "editor lease token is required",
        ),
    }
}
