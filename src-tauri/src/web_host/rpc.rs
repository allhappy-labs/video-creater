use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::app_service::error::{ServiceError, ServiceErrorCode};
use crate::app_service::operation::{AuthorizationScope, MutationClass, OperationDescriptor};

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
    OutcomeUnknown,
    OutcomeExpired,
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

// Classify only the descriptor returned by registry validation; request names alone
// cannot reserve cancellation capacity or bypass authorization/ownership checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionRequestClass {
    Read,
    Mutation,
    Cancellation,
}

impl SessionRequestClass {
    fn for_operation(operation: &OperationDescriptor) -> Self {
        if operation.mutation == MutationClass::JobMutation && operation.name.starts_with("cancel_")
        {
            Self::Cancellation
        } else if operation.mutation == MutationClass::Read {
            Self::Read
        } else {
            Self::Mutation
        }
    }
}

#[derive(Debug, Default)]
struct SessionWindow {
    started_at: u64,
    request_count: usize,
    read_count: usize,
    cancellation_count: usize,
    in_flight: usize,
    cancellation_in_flight: usize,
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
        self.begin_classified(session_id, now, SessionRequestClass::Mutation)
    }

    fn begin_classified(
        &self,
        session_id: &str,
        now: u64,
        class: SessionRequestClass,
    ) -> Result<SessionPermit, SessionLimitError> {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Expired idle windows carry no active permit or current rate history.
        // Ordinary and cancellation permits both keep their original entry alive.
        sessions.retain(|_, window| {
            window.in_flight != 0
                || window.cancellation_in_flight != 0
                || now.saturating_sub(window.started_at) < self.window_seconds
        });
        if !sessions.contains_key(session_id) && sessions.len() >= 2048 {
            return Err(SessionLimitError::Concurrent);
        }
        let window = sessions
            .entry(session_id.to_owned())
            .or_insert_with(|| SessionWindow {
                started_at: now,
                ..SessionWindow::default()
            });
        if now.saturating_sub(window.started_at) >= self.window_seconds {
            window.started_at = now;
            window.request_count = 0;
            window.read_count = 0;
            window.cancellation_count = 0;
        }
        // Reads and ordinary mutations share the original concurrency bound.
        // One independent cancellation slot remains available while those calls stall.
        let cancellation = class == SessionRequestClass::Cancellation;
        let at_capacity = if cancellation {
            window.cancellation_in_flight >= 1
        } else {
            window.in_flight >= self.max_concurrent
        };
        if at_capacity {
            return Err(SessionLimitError::Concurrent);
        }
        // Production uses a 60-second window: polling has 300 reads, ordinary
        // mutations retain 60 requests, and cancellation has a finite 30-request reserve.
        let (count, maximum) = match class {
            SessionRequestClass::Read => (window.read_count, 300),
            SessionRequestClass::Mutation => (window.request_count, self.max_requests),
            SessionRequestClass::Cancellation => (window.cancellation_count, 30),
        };
        if count >= maximum {
            return Err(SessionLimitError::Rate {
                retry_after_ms: window
                    .started_at
                    .saturating_add(self.window_seconds)
                    .saturating_sub(now)
                    .saturating_mul(1_000),
            });
        }
        match class {
            SessionRequestClass::Read => window.read_count += 1,
            SessionRequestClass::Mutation => window.request_count += 1,
            SessionRequestClass::Cancellation => window.cancellation_count += 1,
        }
        if cancellation {
            window.cancellation_in_flight += 1;
        } else {
            window.in_flight += 1;
        }
        Ok(SessionPermit {
            cancellation,
            session_id: session_id.to_owned(),
            sessions: Arc::clone(&self.sessions),
        })
    }
}

#[derive(Debug)]
pub struct SessionPermit {
    cancellation: bool,
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
            if self.cancellation {
                window.cancellation_in_flight = window.cancellation_in_flight.saturating_sub(1);
            } else {
                window.in_flight = window.in_flight.saturating_sub(1);
            }
        }
    }
}

pub trait RpcDispatcher: Send + Sync {
    /// Attach the host event stream to shared long-running service operations.
    fn set_event_sink(&self, _events: Arc<dyn crate::app_service::events::EventSink>) {}

    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String>;

    /// Only the host creation operation uses this server-reserved commit witness.
    fn dispatch_with_creation_nonce(
        &self,
        request: &RpcEnvelope,
        _nonce: Option<&str>,
    ) -> Result<Value, ServiceError> {
        self.dispatch_typed(request)
    }

    fn recover_durable_creation(&self, _nonce: &str) -> Option<Value> {
        None
    }

    /// Incremental typed application boundary for migrated operation slices.
    fn dispatch_typed(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        self.dispatch(request).map_err(ServiceError::internal)
    }
}

pub struct RpcEngine {
    registry: RpcRegistry,
    idempotency: IdempotencyStore,
    limiter: SessionLimiter,
    dispatcher: Arc<dyn RpcDispatcher>,
    outcomes: Option<Arc<super::request_outcome::RequestOutcomeStore>>,
}

impl RpcEngine {
    pub fn new(dispatcher: Arc<dyn RpcDispatcher>) -> Self {
        Self {
            registry: RpcRegistry::from_inventory(),
            idempotency: IdempotencyStore::new(512, 300),
            limiter: SessionLimiter::new(60, 4, 60),
            dispatcher,
            outcomes: None,
        }
    }

    pub fn with_outcome_store(
        dispatcher: Arc<dyn RpcDispatcher>,
        store: Arc<super::request_outcome::RequestOutcomeStore>,
    ) -> Self {
        Self {
            outcomes: Some(store),
            ..Self::new(dispatcher)
        }
    }

    pub(super) fn outcome_permit(
        &self,
        session_id: &str,
        now: u64,
    ) -> Result<SessionPermit, SessionLimitError> {
        self.limiter
            .begin_classified(session_id, now, SessionRequestClass::Read)
    }

    pub fn resolve_outcome(
        &self,
        principal: &str,
        request_id: &str,
    ) -> Result<super::request_outcome::RequestOutcome, super::request_outcome::OutcomeError> {
        super::durable_rpc::resolve(
            self.outcomes
                .as_ref()
                .ok_or(super::request_outcome::OutcomeError::Unavailable)?,
            principal,
            request_id,
            &self.dispatcher,
        )
    }

    pub fn execute(
        &self,
        session_id: &str,
        scopes: &BTreeSet<AuthorizationScope>,
        body: &[u8],
        now: u64,
    ) -> RpcResponse {
        self.execute_for_principal(session_id, session_id, scopes, body, now)
    }

    pub fn execute_for_principal(
        &self,
        session_id: &str,
        principal: &str,
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
        let operation = match self.registry.validate(&request, scopes, body.len()) {
            Ok(operation) => operation,
            Err(error) => {
                let (code, message) = registry_error(error);
                return RpcResponse::error(&request.request_id, code, message, None);
            }
        };
        let class = SessionRequestClass::for_operation(operation);
        let _permit = match self.limiter.begin_classified(session_id, now, class) {
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
        let dispatch = || {
            let action = |nonce: Option<&str>| match self
                .dispatcher
                .dispatch_with_creation_nonce(&request, nonce)
            {
                Ok(result) => RpcResponse::success(&request.request_id, result),
                Err(error) => {
                    let code = service_error_rpc_code(error.code());
                    if let Some(detail) = error.internal_detail() {
                        eprintln!(
                            "video-creater-host: operation {} failed: {}",
                            request.operation,
                            redact_dispatch_error(detail)
                        );
                    }
                    RpcResponse::error(&request.request_id, code, error.to_string(), None)
                }
            };
            match self
                .outcomes
                .as_ref()
                .filter(|_| class != SessionRequestClass::Read)
            {
                Some(store) => super::durable_rpc::dispatch(
                    store,
                    super::durable_rpc::DispatchContext {
                        principal,
                        request: &request,
                        body,
                        now,
                    },
                    &self.dispatcher,
                    action,
                ),
                None => action(None),
            }
        };
        let dispatched = if class == SessionRequestClass::Cancellation {
            self.idempotency.execute_once_cancellation(
                session_id,
                &request.request_id,
                body,
                now,
                dispatch,
            )
        } else {
            self.idempotency
                .execute_once(session_id, &request.request_id, body, now, dispatch)
        };
        match dispatched {
            Ok(response) => response,
            Err(IdempotencyDecision::Replay(response)) => *response,
            Err(IdempotencyDecision::PayloadConflict) => RpcResponse::error(
                &request.request_id,
                RpcErrorCode::Conflict,
                "request ID was already used with a different payload",
                None,
            ),
            Err(IdempotencyDecision::Capacity) => RpcResponse::error(
                &request.request_id,
                RpcErrorCode::Busy,
                "too many pending cancellation requests",
                Some(250),
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

fn service_error_rpc_code(code: ServiceErrorCode) -> RpcErrorCode {
    match code {
        ServiceErrorCode::InvalidInput => RpcErrorCode::InvalidRequest,
        ServiceErrorCode::Unauthorized => RpcErrorCode::Unauthorized,
        ServiceErrorCode::Forbidden => RpcErrorCode::Forbidden,
        ServiceErrorCode::NotFound => RpcErrorCode::NotFound,
        ServiceErrorCode::RevisionConflict | ServiceErrorCode::LeaseConflict => {
            RpcErrorCode::Conflict
        }
        ServiceErrorCode::Busy => RpcErrorCode::Busy,
        ServiceErrorCode::UnavailableCapability
        | ServiceErrorCode::Cancelled
        | ServiceErrorCode::Internal => RpcErrorCode::Internal,
    }
}

#[cfg(test)]
mod typed_error_tests {
    use super::*;
    use serde_json::json;

    struct ConflictDispatcher;
    impl RpcDispatcher for ConflictDispatcher {
        fn dispatch(&self, _request: &RpcEnvelope) -> Result<Value, String> {
            Err("unexpected diagnostic mentions revision conflict".into())
        }
        fn dispatch_typed(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
            if request.request_id == "typed-conflict" {
                Err(ServiceError::revision_conflict(1, 2))
            } else {
                self.dispatch(request).map_err(ServiceError::internal)
            }
        }
    }

    #[test]
    fn rpc_maps_typed_conflicts_without_matching_diagnostic_strings() {
        let engine = RpcEngine::new(Arc::new(ConflictDispatcher));
        let scopes = BTreeSet::from([AuthorizationScope::ProjectRead]);
        for (id, expected) in [
            ("typed-conflict", RpcErrorCode::Conflict),
            ("internal-detail", RpcErrorCode::Internal),
        ] {
            let body = serde_json::to_vec(&RpcEnvelope {
                request_id: id.into(),
                operation: "load_job_progress_from_split_project_folder".into(),
                project_id: Some("project-test".into()),
                expected_revision: None,
                editor_lease_token: None,
                payload: json!({}),
            })
            .unwrap();
            let response = engine.execute("session", &scopes, &body, 1);
            assert_eq!(response.error.unwrap().code, expected);
        }
    }
}

#[cfg(test)]
#[path = "rpc_limiter_tests.rs"]
mod limiter_tests;
