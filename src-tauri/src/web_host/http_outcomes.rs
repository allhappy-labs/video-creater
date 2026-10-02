//! Recovery shares trusted identity/CSRF and finite read permits. Disk I/O runs off Tokio workers.
use super::*;
use crate::web_host::request_outcome::OutcomeError;
use crate::web_host::rpc::SessionLimitError;

#[derive(Deserialize)]
pub(super) struct OutcomeQuery {
    after: Option<String>,
}

fn session_view(
    state: &HostHttpState,
    headers: &HeaderMap,
) -> Result<super::super::session::SessionView, Box<Response>> {
    let credential = session_cookie(headers).ok_or_else(|| Box::new(unauthorized()))?;
    let session = state
        .sessions
        .authenticate(credential, state.now())
        .map_err(|_| Box::new(unauthorized()))?;
    if session.identity.is_none() {
        return Err(Box::new(unauthorized()));
    }
    Ok(session)
}
fn outcome_error(error: OutcomeError) -> Response {
    match error {
        OutcomeError::NotFound => not_found("outcome_not_found"),
        OutcomeError::Pending => (
            StatusCode::CONFLICT,
            Json(json!({"error":"outcome_pending"})),
        )
            .into_response(),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"outcome_unavailable"})),
        )
            .into_response(),
    }
}
fn busy() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"outcome_busy", "retryAfterMs":250})),
    )
        .into_response()
}

async fn offload(
    state: Arc<HostHttpState>,
    session_id: &str,
    work: impl FnOnce(Arc<HostHttpState>) -> Response + Send + 'static,
) -> Response {
    let permit = match state.rpc.outcome_permit(session_id, state.now()) {
        Ok(permit) => permit,
        Err(SessionLimitError::Concurrent) => return busy(),
        Err(SessionLimitError::Rate { retry_after_ms }) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error":"rate_limited","retryAfterMs":retry_after_ms})),
            )
                .into_response()
        }
    };
    let global = match state.outcome_slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return busy(),
    };
    tokio::task::spawn_blocking(move || {
        let _permits = (permit, global);
        work(state)
    })
    .await
    .unwrap_or_else(|_| internal())
}

pub(super) async fn list_outcomes(
    State(state): State<Arc<HostHttpState>>,
    headers: HeaderMap,
    Query(query): Query<OutcomeQuery>,
) -> Response {
    let session = match session_view(&state, &headers) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    if query.after.as_ref().is_some_and(|id| id.len() > 128) {
        return bad_request("invalid outcome cursor");
    }
    let principal = session.identity.expect("authenticated identity checked");
    offload(state, &session.session_id, move |state| {
        match state
            .outcomes
            .outstanding(&principal, query.after.as_deref())
        {
            Ok(outcomes) => {
                let resolved: Result<Vec<_>, _> = outcomes
                    .into_iter()
                    .map(|outcome| state.rpc.resolve_outcome(&principal, &outcome.request_id))
                    .collect();
                match resolved {
                    Ok(outcomes) => {
                        let next = if outcomes.len() == 256 {
                            outcomes.last().map(|outcome| outcome.request_id.clone())
                        } else {
                            None
                        };
                        Json(json!({"outcomes":outcomes,"nextCursor":next})).into_response()
                    }
                    Err(error) => outcome_error(error),
                }
            }
            Err(error) => outcome_error(error),
        }
    })
    .await
}

pub(super) async fn lookup_outcome(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(request_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let session = match session_view(&state, &headers) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    if request_id.len() > 128 {
        return bad_request("invalid request identity");
    }
    let principal = session.identity.expect("authenticated identity checked");
    offload(state, &session.session_id, move |state| {
        match state.rpc.resolve_outcome(&principal, &request_id) {
            Ok(outcome) => Json(outcome).into_response(),
            Err(OutcomeError::NotFound) => match state.outcomes.replay_fenced_absent(&request_id, state.now()) {
                Ok(true) => (StatusCode::GONE, Json(json!({"error":"outcome_expired", "requestId":request_id, "replayFenced":true}))).into_response(),
                Ok(false) => outcome_error(OutcomeError::NotFound),
                Err(error) => outcome_error(error),
            },
            Err(error) => outcome_error(error),
        }
    })
    .await
}

pub(super) async fn acknowledge_outcome(
    State(state): State<Arc<HostHttpState>>,
    AxumPath(request_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let session = match authorize_state_change(
        &state,
        &headers,
        "outcome reconciliation authentication failed",
    ) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Some(principal) = session.identity else {
        return unauthorized();
    };
    if request_id.len() > 128 {
        return bad_request("invalid request identity");
    }
    offload(state, &session.session_id, move |state| {
        match state
            .outcomes
            .acknowledge_at(&principal, &request_id, state.now())
        {
            Ok(()) => Json(json!({"acknowledged":true})).into_response(),
            Err(error) => outcome_error(error),
        }
    })
    .await
}
