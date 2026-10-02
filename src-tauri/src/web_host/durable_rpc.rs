//! Durable dispatch coordination kept separate from wire validation and session limits.
use super::request_outcome::{
    OutcomeAdmission, OutcomeError, OutcomeStatus, RequestOutcome, RequestOutcomeStore,
};
use super::rpc::{RpcDispatcher, RpcEnvelope, RpcErrorCode, RpcResponse};
use std::sync::Arc;

pub(super) struct DispatchContext<'a> {
    pub principal: &'a str,
    pub request: &'a RpcEnvelope,
    pub body: &'a [u8],
    pub now: u64,
}

pub(super) fn dispatch(
    store: &RequestOutcomeStore,
    context: DispatchContext<'_>,
    dispatcher: &Arc<dyn RpcDispatcher>,
    action: impl FnOnce(Option<&str>) -> RpcResponse,
) -> RpcResponse {
    let DispatchContext {
        principal,
        request,
        body,
        now,
    } = context;
    match store.begin(principal, request, body, now) {
        Ok(OutcomeAdmission::New { creation_nonce }) => {
            let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||action(creation_nonce.as_deref()))) {
                Ok(response) => response,
                Err(panic) => {
                    let _ = store.interrupt(principal, &request.request_id);
                    std::panic::resume_unwind(panic);
                }
            };
            // A successful canonical mutation is still successful when receipt publication
            // fails. The already-durable pending receipt fences replay after interruption.
            if store.finish(principal, &request.request_id, &response).is_err() {
                eprintln!("video-creater-host: durable outcome publication unavailable");
            }
            response
        }
        Ok(OutcomeAdmission::Existing(outcome)) => {
            let _ = resolve(store, principal, &outcome.request_id, dispatcher);
            RpcResponse::error(&request.request_id, RpcErrorCode::OutcomeUnknown,
                "The original request was already accepted. Resolve its durable outcome and refresh canonical state; do not retry with a new identity.", None)
        }
        Err(OutcomeError::Conflict) => RpcResponse::error(&request.request_id, RpcErrorCode::Conflict, "request ID was already used with a different payload", None),
        Err(OutcomeError::Expired) => RpcResponse::error(&request.request_id, RpcErrorCode::OutcomeExpired, "The request identity expired. Its earlier outcome is unavailable; reconcile current state before editing.", None),
        Err(_) => RpcResponse::error(&request.request_id, RpcErrorCode::Busy, "Durable recovery storage is unavailable or full. The request was not dispatched.", None),
    }
}

pub(super) fn resolve(
    store: &RequestOutcomeStore,
    principal: &str,
    request_id: &str,
    dispatcher: &Arc<dyn RpcDispatcher>,
) -> Result<RequestOutcome, OutcomeError> {
    let outcome = store.lookup(principal, request_id)?;
    if outcome.operation == "remote_create_project" && outcome.status == OutcomeStatus::Interrupted
    {
        if let Some(nonce) = store.creation_nonce(principal, request_id) {
            if let Some(result) = dispatcher.recover_durable_creation(&nonce) {
                return store.finish(
                    principal,
                    request_id,
                    &RpcResponse::success(request_id, result),
                );
            }
        }
    }
    Ok(outcome)
}
