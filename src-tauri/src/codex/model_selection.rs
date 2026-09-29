//! Keeps Codex turns on a model the bundled app-server can use.
//!
//! The app-server reads the user's `~/.codex/config.toml`, which can select a
//! model that only a newer Codex supports. The pinned server still starts the
//! thread on that model, and the turn then fails with "requires a newer version
//! of Codex". Before each turn the thread's model is checked against the
//! server's `model/list`; an unsupported model is replaced on `turn/start`
//! (which overrides the model for that turn and later turns). When the list is
//! unavailable, the upgrade error is detected and the turn is retried once.

use super::app_server::{
    request_codex_app_server_until, run_codex_turn_pump, CodexAppServerError,
    CodexAppServerTransport, CompletedCodexTurn,
};
use crate::process_supervisor::CancellationSignal;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Instant;

/// Default model that `model/list` of the pinned `@openai/codex` sidecar
/// advertises. Used only when the server can't list its models.
pub const PINNED_CODEX_FALLBACK_MODEL: &str = "gpt-5.5";
/// The sidecar version `PINNED_CODEX_FALLBACK_MODEL` was read from; a test
/// keeps it equal to the `@openai/codex` pin in `package.json`.
pub const PINNED_CODEX_VERSION: &str = "0.141.0";

const MODEL_LIST_MAX_PAGES: u64 = 8;
/// Request ids after `initialize`, `thread/*` and `turn/start` (start + 0..=2).
const MODEL_LIST_REQUEST_OFFSET: u64 = 3;
const RETRY_TURN_REQUEST_OFFSET: u64 = MODEL_LIST_REQUEST_OFFSET + MODEL_LIST_MAX_PAGES;
const UNSUPPORTED_MODEL_ERROR_MARKERS: &[&str] = &["requires a newer version of codex"];

static LATEST_MODEL_NOTICE: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexListedModel {
    pub id: String,
    pub model: String,
    pub hidden: bool,
    pub is_default: bool,
}

/// What the pre-turn check decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexModelChoice {
    /// The model the thread reported, if any.
    pub thread_model: Option<String>,
    /// The server's models, or `None` when it couldn't list them.
    pub listed: Option<Vec<CodexListedModel>>,
    /// Model to send on `turn/start` instead of the thread's model.
    pub turn_model: Option<String>,
}

/// The notice from the most recent model check, for the agent settings status.
pub fn latest_codex_model_notice() -> Option<String> {
    LATEST_MODEL_NOTICE
        .lock()
        .map(|notice| notice.clone())
        .unwrap_or_default()
}

fn record_codex_model_notice(notice: Option<String>) {
    if let Ok(mut latest) = LATEST_MODEL_NOTICE.lock() {
        *latest = notice;
    }
}

pub fn unsupported_codex_model_notice(model: &str) -> String {
    format!("Your Codex config selects a model this app's Codex can't use; using {model}.")
}

pub fn build_model_list_request(request_id: u64, cursor: Option<&str>) -> Value {
    json!({
        "id": request_id,
        "method": "model/list",
        "params": { "includeHidden": true, "cursor": cursor },
    })
}

/// Parses one `model/list` page (`ModelListResponse`) into its models and next cursor.
pub fn parse_model_list_page(
    result: &Value,
) -> Result<(Vec<CodexListedModel>, Option<String>), CodexAppServerError> {
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CodexAppServerError::IncompatibleProtocol("model/list result is missing data".into())
        })?;
    let models = data
        .iter()
        .filter_map(|entry| {
            let model = entry.get("model").and_then(Value::as_str)?.to_string();
            Some(CodexListedModel {
                id: entry
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| model.clone()),
                model,
                hidden: entry
                    .get("hidden")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                is_default: entry
                    .get("isDefault")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect();
    let cursor = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok((models, cursor))
}

pub fn is_listed_model(models: &[CodexListedModel], model: &str) -> bool {
    models
        .iter()
        .any(|listed| listed.model == model || listed.id == model)
}

/// The server's default picker model, else its first visible model.
pub fn default_listed_model(models: &[CodexListedModel]) -> Option<&str> {
    models
        .iter()
        .find(|listed| listed.is_default && !listed.hidden)
        .or_else(|| models.iter().find(|listed| !listed.hidden))
        .map(|listed| listed.model.as_str())
}

pub fn is_unsupported_model_error(error: &CodexAppServerError) -> bool {
    let message = match error {
        CodexAppServerError::TurnFailed(message) => message,
        CodexAppServerError::RpcError { message, .. } => message,
        _ => return false,
    };
    let message = message.to_lowercase();
    UNSUPPORTED_MODEL_ERROR_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
}

/// Lists the server's models, or `Ok(None)` when it rejects `model/list`.
fn list_codex_models<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<Option<Vec<CodexListedModel>>, CodexAppServerError> {
    let mut models = Vec::new();
    let mut cursor = None::<String>;
    for page in 0..MODEL_LIST_MAX_PAGES {
        let request = build_model_list_request(request_id_start + page, cursor.as_deref());
        let result =
            match request_codex_app_server_until(transport, request, deadline, cancellation) {
                Ok(result) => result,
                Err(CodexAppServerError::RpcError { .. }) => return Ok(None),
                Err(error) => return Err(error),
            };
        let Ok((page_models, next_cursor)) = parse_model_list_page(&result) else {
            return Ok(None);
        };
        models.extend(page_models);
        match next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(Some(models))
}

/// Checks the model a started or resumed thread reports against `model/list`.
pub fn choose_codex_turn_model<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    thread_response: &Value,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CodexModelChoice, CodexAppServerError> {
    let Some(thread_model) = thread_response.get("model").and_then(Value::as_str) else {
        return Ok(CodexModelChoice::default());
    };
    let listed = list_codex_models(
        transport,
        request_id_start + MODEL_LIST_REQUEST_OFFSET,
        deadline,
        cancellation,
    )?;
    let turn_model = listed
        .as_deref()
        .filter(|models| !models.is_empty() && !is_listed_model(models, thread_model))
        .and_then(default_listed_model)
        .map(str::to_string);
    Ok(CodexModelChoice {
        thread_model: Some(thread_model.to_string()),
        listed,
        turn_model,
    })
}

/// The model to retry with after `error`, if the error means the model is unsupported.
fn retry_model_after(error: &CodexAppServerError, choice: &CodexModelChoice) -> Option<String> {
    if choice.turn_model.is_some() || !is_unsupported_model_error(error) {
        return None;
    }
    let candidate = choice
        .listed
        .as_deref()
        .and_then(default_listed_model)
        .unwrap_or(PINNED_CODEX_FALLBACK_MODEL);
    (choice.thread_model.as_deref() != Some(candidate)).then(|| candidate.to_string())
}

/// Runs `turn_request` on the chosen model and retries once on a supported
/// model if the server rejects the thread's model. Returns the completed turn
/// and the notice to show when the model was replaced.
pub fn run_codex_turn_with_supported_model<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    mut turn_request: Value,
    thread_id: &str,
    choice: &CodexModelChoice,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<(CompletedCodexTurn, Option<String>), CodexAppServerError> {
    if let Some(model) = &choice.turn_model {
        turn_request["params"]["model"] = json!(model);
    }
    let first = run_codex_turn_pump(
        transport,
        turn_request.clone(),
        thread_id,
        deadline,
        cancellation,
    );
    let (completed, model) = match first {
        Ok(completed) => (completed, choice.turn_model.clone()),
        Err(error) => {
            let Some(model) = retry_model_after(&error, choice) else {
                return Err(error);
            };
            turn_request["id"] = json!(request_id_start + RETRY_TURN_REQUEST_OFFSET);
            turn_request["params"]["model"] = json!(model);
            let completed =
                run_codex_turn_pump(transport, turn_request, thread_id, deadline, cancellation)?;
            (completed, Some(model))
        }
    };
    let notice = model.as_deref().map(unsupported_codex_model_notice);
    if notice.is_some() || choice.listed.is_some() {
        record_codex_model_notice(notice.clone());
    }
    Ok((completed, notice))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(model: &str, hidden: bool, is_default: bool) -> CodexListedModel {
        CodexListedModel {
            id: model.to_string(),
            model: model.to_string(),
            hidden,
            is_default,
        }
    }

    #[test]
    fn model_list_page_parses_the_pinned_schema_shape() {
        let (models, cursor) = parse_model_list_page(&json!({
            "data": [{
                "id": "gpt-5.5", "model": "gpt-5.5", "displayName": "gpt-5.5",
                "description": "", "hidden": false, "isDefault": true,
                "defaultReasoningEffort": "medium", "supportedReasoningEfforts": [],
            }],
            "nextCursor": "next",
        }))
        .unwrap();
        assert_eq!(models, vec![listed("gpt-5.5", false, true)]);
        assert_eq!(cursor.as_deref(), Some("next"));
        assert!(parse_model_list_page(&json!({})).is_err());
    }

    #[test]
    fn default_model_prefers_the_visible_default_then_the_first_visible_model() {
        let models = vec![
            listed("codex-auto-review", true, true),
            listed("gpt-5.4", false, false),
            listed("gpt-5.5", false, true),
        ];
        assert_eq!(default_listed_model(&models), Some("gpt-5.5"));
        assert_eq!(default_listed_model(&models[..2]), Some("gpt-5.4"));
        assert_eq!(default_listed_model(&models[..1]), None);
    }

    #[test]
    fn only_the_upgrade_error_counts_as_an_unsupported_model() {
        assert!(is_unsupported_model_error(
            &CodexAppServerError::TurnFailed(
                r#"{"message":"The 'gpt-5.6-sol' model requires a newer version of Codex."}"#
                    .into()
            )
        ));
        assert!(is_unsupported_model_error(&CodexAppServerError::RpcError {
            code: -32600,
            message: "The 'x' model Requires a newer version of Codex".into(),
        }));
        assert!(!is_unsupported_model_error(
            &CodexAppServerError::TurnFailed("You've hit your usage limit".into())
        ));
        assert!(!is_unsupported_model_error(&CodexAppServerError::Deadline));
    }

    /// No-usage probe of the real sidecar: an ephemeral thread and `model/list`, no turn.
    #[test]
    fn pinned_codex_model_check_runs_without_a_turn() {
        use super::super::app_server::{
            codex_app_server_command, codex_app_server_deadline, initialize_codex_app_server,
            StdioCodexAppServerTransport,
        };
        let Ok(binary) = std::env::var("VIDEO_CREATER_PINNED_CODEX_BINARY") else {
            return;
        };
        let cwd = std::env::temp_dir();
        let mut transport =
            StdioCodexAppServerTransport::spawn(&codex_app_server_command(&binary)).unwrap();
        initialize_codex_app_server(&mut transport, 1).unwrap();
        let deadline = codex_app_server_deadline();
        let thread = request_codex_app_server_until(
            &mut transport,
            json!({
                "id": 2,
                "method": "thread/start",
                "params": { "cwd": cwd, "sandbox": "read-only", "approvalPolicy": "never", "ephemeral": true },
            }),
            deadline,
            None,
        )
        .unwrap();
        let choice = choose_codex_turn_model(&mut transport, 1, &thread, deadline, None).unwrap();
        let listed = choice
            .listed
            .clone()
            .expect("the pinned sidecar lists its models");
        eprintln!(
            "thread model {:?}; listed {:?}; turn model {:?}",
            choice.thread_model,
            listed.iter().map(|model| &model.model).collect::<Vec<_>>(),
            choice.turn_model
        );
        let thread_model = choice.thread_model.as_deref().expect("thread model");
        if is_listed_model(&listed, thread_model) {
            assert_eq!(choice.turn_model, None);
        } else {
            assert_eq!(choice.turn_model.as_deref(), default_listed_model(&listed));
        }
        assert!(is_listed_model(&listed, PINNED_CODEX_FALLBACK_MODEL));
        assert!(transport.terminate().unwrap().reaped);
    }

    #[test]
    fn pinned_fallback_model_tracks_the_codex_package_pin() {
        let manifest: Value = serde_json::from_str(include_str!("../../../package.json")).unwrap();
        assert_eq!(
            manifest["dependencies"]["@openai/codex"]
                .as_str()
                .or_else(|| manifest["devDependencies"]["@openai/codex"].as_str()),
            Some(PINNED_CODEX_VERSION),
            "re-read the pinned sidecar's model/list default when bumping @openai/codex"
        );
    }
}
