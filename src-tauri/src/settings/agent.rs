use crate::agent::claude_cli::{resolve_claude_executable, ClaudeAuthMode, ClaudeTurnReadiness};
use crate::codex::app_server::{
    build_app_server_initialize_request, bundled_codex_executable,
    decode_app_server_initialize_response, resolve_codex_app_server_command,
};
use crate::codex::mcp_server::{
    build_mcp_initialize_request, decode_mcp_initialize_response, McpInitializeResponseError,
};
use crate::codex::model_selection::latest_codex_model_notice;
use crate::codex::proposal::{
    validate_codex_edit_proposal, CodexEditProposal, CodexProposalClip, CodexRenderReview,
    CODEX_EDIT_PROPOSAL_VALIDATOR_SCHEMA_VERSION,
};
use crate::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use crate::project::model::{MediaAsset, MediaKind, VideoProject};
use crate::settings::fixtures::SchemaV2ProjectFixture;
use crate::settings::health::{SettingsComponentHealth, SettingsHealthState};
use crate::settings::operations::{
    SettingsOperation, SettingsOperationError, SettingsOperationKind, SettingsOperationState,
};
use crate::settings::process_probe::{
    run_process_probe, ProcessProbeCleanup, ProcessProbeError, ProcessProbeExitState,
    ProcessProbeRequest, ProcessProbeResult, ProcessProbeStdoutFormat,
    ProcessProbeTerminationState,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CODEX_COMPONENT_ID: &str = "agent.codex";
const CLAUDE_COMPONENT_ID: &str = "agent.claude";
const MCP_COMPONENT_ID: &str = "agent.mcpServer";
const PROPOSAL_VALIDATOR_COMPONENT_ID: &str = "agent.proposalValidator";
const PROPOSAL_VALIDATOR_FIXTURE_VERSION: u32 = 1;
const INVALID_VISUAL_FIXTURE_ISSUE_CODE: &str = "codex.proposal.invalidOverlayTiming";
const CODEX_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const CODEX_PROBE_MAX_STDOUT_BYTES: usize = 64 * 1024;
const CODEX_PROBE_MAX_STDERR_BYTES: usize = 64 * 1024;
/// Both Claude probes are local and answer immediately, so they share the Codex
/// probe's budget.
const CLAUDE_PROBE_TIMEOUT: Duration = CODEX_PROBE_TIMEOUT;
const CLAUDE_PROBE_MAX_STDOUT_BYTES: usize = CODEX_PROBE_MAX_STDOUT_BYTES;
const CLAUDE_PROBE_MAX_STDERR_BYTES: usize = CODEX_PROBE_MAX_STDERR_BYTES;
const CLAUDE_DIAGNOSTIC_DETAIL_MAX_CHARS: usize = 512;
const CODEX_INITIALIZE_REQUEST_ID: u64 = 1;
const MCP_INITIALIZE_REQUEST_ID: u64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpClientConfigurationState {
    pub action_label: String,
    pub configuration: Option<String>,
    pub executable: PathBuf,
    pub project_dir: Option<PathBuf>,
}

pub fn probe_codex_app_server() -> SettingsComponentHealth {
    let health = match bundled_codex_executable() {
        Ok(executable) => probe_codex_app_server_with(&executable, CODEX_PROBE_TIMEOUT),
        Err(error) => codex_health(
            SettingsHealthState::Failed,
            "The bundled Codex runtime could not be resolved.",
            "agent.codex.launchFailed",
            Some(error.to_string()),
            BTreeMap::new(),
            current_timestamp(),
        ),
    };
    with_codex_model_notice(health, latest_codex_model_notice())
}

/// Tells the user, on a ready Codex item, that the last turn replaced the model
/// their Codex config selects.
fn with_codex_model_notice(
    mut health: SettingsComponentHealth,
    notice: Option<String>,
) -> SettingsComponentHealth {
    if let Some(notice) = notice.filter(|_| health.state == SettingsHealthState::Ready) {
        health.summary = format!("{} {notice}", health.summary);
        health.provenance.insert("modelNotice".to_string(), notice);
    }
    health
}

/// Where the readiness probe looks for the user's own `claude`.
///
/// Deliberately the same resolver a turn uses, so the row can never report a
/// binary the turn would not actually run.
fn resolve_claude_probe_executable(executable_override: Option<&str>) -> Option<PathBuf> {
    let configured = executable_override
        .map(str::trim)
        .filter(|configured| !configured.is_empty())
        .map(Path::new);
    resolve_claude_executable(configured)
}

fn anthropic_api_key_present() -> bool {
    std::env::var_os("ANTHROPIC_API_KEY")
        .and_then(|value| value.into_string().ok())
        .is_some_and(|value| !value.trim().is_empty())
}

/// Readiness for the Claude backend. Both probes are local and spend no tokens:
/// `claude --version` answers "is it runnable?" and `claude auth status` answers
/// "is it signed in, and how?" from the on-disk credentials.
pub fn probe_claude_cli(executable_override: Option<&str>) -> SettingsComponentHealth {
    claude_readiness(executable_override).0
}

/// The routing verdict for the Claude backend, from the same probes as the readiness row.
///
/// Automatic prefers Claude, so the router needs more than "installed": it needs to know that
/// a turn would actually authenticate, and on which credential.
pub fn claude_turn_readiness(executable_override: Option<&str>) -> ClaudeTurnReadiness {
    claude_readiness(executable_override).1
}

/// One pair of probes, two answers: what the settings row says, and what the router decides.
/// Kept together so the row can never describe a credential the turn would not use.
pub fn claude_readiness(
    executable_override: Option<&str>,
) -> (SettingsComponentHealth, ClaudeTurnReadiness) {
    match resolve_claude_probe_executable(executable_override) {
        Some(executable) => probe_claude_cli_with(
            &executable,
            CLAUDE_PROBE_TIMEOUT,
            anthropic_api_key_present(),
        ),
        None => (
            claude_health(
                SettingsHealthState::NotConfigured,
                "Claude isn't installed. Install it from claude.com, then sign in with your subscription.",
                "agent.claude.missing",
                None,
                BTreeMap::new(),
                current_timestamp(),
            ),
            ClaudeTurnReadiness::Unavailable,
        ),
    }
}

/// The `authMethod` a subscription login reports. Anything else is a login this app does not
/// claim to understand, and it is described as "not a subscription" rather than guessed at.
const CLAUDE_SUBSCRIPTION_AUTH_METHOD: &str = "claude.ai";

fn probe_claude_cli_with(
    executable: &Path,
    timeout: Duration,
    api_key_present: bool,
) -> (SettingsComponentHealth, ClaudeTurnReadiness) {
    let checked_at = current_timestamp();
    let mut provenance =
        BTreeMap::from([("executable".to_string(), executable.display().to_string())]);

    let version = match probe_claude_version(executable, timeout) {
        Ok(version) => version,
        Err(detail) => {
            return unavailable_claude(claude_health(
                SettingsHealthState::Failed,
                "Claude is installed but didn't start.",
                "agent.claude.launchFailed",
                Some(detail),
                provenance,
                checked_at,
            ));
        }
    };
    if !version.is_empty() {
        provenance.insert("version".to_string(), version.clone());
    }

    let status = match probe_claude_auth_status(executable, timeout) {
        Ok(status) => status,
        Err(ClaudeAuthProbeError::Malformed(detail)) => {
            return unavailable_claude(claude_health(
                SettingsHealthState::Failed,
                "Claude is installed but its sign-in status could not be read.",
                "agent.claude.malformedStatus",
                Some(detail),
                provenance,
                checked_at,
            ));
        }
        Err(ClaudeAuthProbeError::Failed(detail)) => {
            return unavailable_claude(claude_health(
                SettingsHealthState::Failed,
                "Claude is installed but didn't start.",
                "agent.claude.launchFailed",
                Some(detail),
                provenance,
                checked_at,
            ));
        }
    };

    if let Some(auth_method) = status.auth_method.as_deref().filter(|it| !it.is_empty()) {
        provenance.insert("authMethod".to_string(), auth_method.to_string());
    }
    if let Some(subscription) = status
        .subscription_type
        .as_deref()
        .filter(|it| !it.is_empty())
    {
        provenance.insert("subscriptionType".to_string(), subscription.to_string());
    }

    // The account's own identity stays out of the row: `authMethod` and `subscriptionType`
    // are enough to say which credential pays, and an email or an organisation id would be
    // the user's data on screen for no benefit.
    if status.logged_in {
        let subscription =
            status.auth_method.as_deref().map(str::trim) == Some(CLAUDE_SUBSCRIPTION_AUTH_METHOD);
        let (summary, code, auth) = if subscription {
            (
                subscription_summary(&version, status.subscription_type.as_deref()),
                "agent.claude.ready",
                ClaudeAuthMode::Subscription,
            )
        } else {
            (
                other_login_summary(&version),
                "agent.claude.readyOtherAuth",
                ClaudeAuthMode::OtherLogin,
            )
        };
        return (
            claude_health(
                SettingsHealthState::Ready,
                &summary,
                code,
                None,
                provenance,
                checked_at,
            ),
            ClaudeTurnReadiness::Ready(auth),
        );
    }
    // An API key is never required, but it is not refused either: with no login it is the only
    // credential a turn could use, so it is passed through and named on the row.
    if api_key_present {
        return (
            claude_health(
                SettingsHealthState::Ready,
                &api_key_summary(&version),
                "agent.claude.apiKey",
                None,
                provenance,
                checked_at,
            ),
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::ApiKey),
        );
    }
    unavailable_claude(claude_health(
        SettingsHealthState::ActionRequired,
        CLAUDE_SIGN_IN_SUMMARY,
        "agent.claude.notLoggedIn",
        None,
        provenance,
        checked_at,
    ))
}

/// The sign-in instruction. Named so a test can assert the exact words rather than a fragment:
/// it points at the subscription login and mentions no API key, because the product is built for
/// a user with their own Claude plan, and an API key is optional at most.
pub const CLAUDE_SIGN_IN_SUMMARY: &str =
    "Claude is installed but not signed in. Sign in to Claude with your subscription: run `claude` in a terminal and use /login.";

fn unavailable_claude(
    health: SettingsComponentHealth,
) -> (SettingsComponentHealth, ClaudeTurnReadiness) {
    (health, ClaudeTurnReadiness::Unavailable)
}

fn claude_version_prefix(version: &str) -> String {
    if version.is_empty() {
        "Claude".to_string()
    } else {
        format!("Claude {version}")
    }
}

fn subscription_summary(version: &str, subscription_type: Option<&str>) -> String {
    let plan = subscription_type
        .map(str::trim)
        .filter(|plan| !plan.is_empty())
        .map(|plan| format!(" ({plan} plan)"))
        .unwrap_or_default();
    format!(
        "{} is signed in with your Claude subscription{plan}. No API key is needed.",
        claude_version_prefix(version)
    )
}

fn other_login_summary(version: &str) -> String {
    format!(
        "{} is signed in, but not with a Claude subscription.",
        claude_version_prefix(version)
    )
}

fn api_key_summary(version: &str) -> String {
    format!(
        "{} isn't signed in, so turns will use the Anthropic API key from the environment. Sign in with your subscription to use it instead.",
        claude_version_prefix(version)
    )
}

/// `claude --version` prints `2.1.270 (Claude Code)` — a plain line, not JSON —
/// so the shared probe surfaces it as a malformed-JSON error whose captured line
/// is the answer. Only the leading version number is kept, so nothing downstream
/// repeats the CLI's own product name.
fn probe_claude_version(executable: &Path, timeout: Duration) -> Result<String, String> {
    match run_claude_probe(
        executable,
        &["--version"],
        timeout,
        ProcessProbeStdoutFormat::JsonLines,
    ) {
        Ok(result) if result.exit_state.success => Ok(result
            .response_lines
            .first()
            .map(claude_version_from_value)
            .unwrap_or_default()),
        Ok(result) => Err(probe_failure_detail(&result.stderr, "")),
        Err(ProcessProbeError::MalformedJson {
            line,
            exit_state,
            stderr,
            ..
        }) => {
            if exit_state.success {
                Ok(claude_version_from_line(&line))
            } else {
                Err(probe_failure_detail(&stderr, &line))
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

fn claude_version_from_line(line: &str) -> String {
    line.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

fn claude_version_from_value(value: &Value) -> String {
    match value {
        Value::String(text) => claude_version_from_line(text),
        other => claude_version_from_line(&other.to_string()),
    }
}

enum ClaudeAuthProbeError {
    Malformed(String),
    Failed(String),
}

/// Only the three fields the readiness row needs are read back. The real payload also carries
/// the account's email, organisation id and organisation name, and local directory paths; not
/// deserializing them is the simplest guarantee that none of them can reach a summary, a
/// provenance map or a log.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeAuthStatus {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    auth_method: Option<String>,
    #[serde(default)]
    subscription_type: Option<String>,
}

fn probe_claude_auth_status(
    executable: &Path,
    timeout: Duration,
) -> Result<ClaudeAuthStatus, ClaudeAuthProbeError> {
    // `claude auth status` pretty-prints one JSON object, so it is read as a document.
    match run_claude_probe(
        executable,
        &["auth", "status"],
        timeout,
        ProcessProbeStdoutFormat::JsonDocument,
    ) {
        Ok(result) => {
            let Some(payload) = result
                .response_lines
                .iter()
                .find(|line| line.is_object())
                .cloned()
            else {
                return Err(ClaudeAuthProbeError::Malformed(
                    "claude auth status returned no JSON object".to_string(),
                ));
            };
            serde_json::from_value::<ClaudeAuthStatus>(payload)
                .map_err(|error| ClaudeAuthProbeError::Malformed(error.to_string()))
        }
        Err(ProcessProbeError::MalformedJson { detail, .. }) => {
            Err(ClaudeAuthProbeError::Malformed(detail))
        }
        Err(error) => Err(ClaudeAuthProbeError::Failed(error.to_string())),
    }
}

fn run_claude_probe(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
    stdout_format: ProcessProbeStdoutFormat,
) -> Result<ProcessProbeResult, ProcessProbeError> {
    run_process_probe(ProcessProbeRequest {
        program: executable.to_path_buf(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        stdin_lines: Vec::new(),
        timeout,
        max_stdout_bytes: CLAUDE_PROBE_MAX_STDOUT_BYTES,
        max_stderr_bytes: CLAUDE_PROBE_MAX_STDERR_BYTES,
        terminate_after_response_id: None,
        stdout_format,
    })
}

fn probe_failure_detail(stderr: &str, stdout_line: &str) -> String {
    let source = if stderr.trim().is_empty() {
        stdout_line
    } else {
        stderr
    };
    let trimmed = source.trim();
    if trimmed.chars().count() <= CLAUDE_DIAGNOSTIC_DETAIL_MAX_CHARS {
        return trimmed.to_string();
    }
    let tail = trimmed
        .chars()
        .skip(trimmed.chars().count() - CLAUDE_DIAGNOSTIC_DETAIL_MAX_CHARS)
        .collect::<String>();
    format!("…{tail}")
}

fn claude_health(
    state: SettingsHealthState,
    summary: &str,
    diagnostic_code: &str,
    diagnostic_detail: Option<String>,
    provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    SettingsComponentHealth {
        id: CLAUDE_COMPONENT_ID.to_string(),
        label: "Claude CLI".to_string(),
        state,
        summary: summary.to_string(),
        action_id: Some("agent.claude.selfTest".to_string()),
        action_label: Some("Re-check Claude".to_string()),
        last_checked_at: checked_at,
        diagnostic_code: Some(diagnostic_code.to_string()),
        diagnostic_detail,
        provenance,
    }
}

pub fn probe_mcp_server() -> SettingsComponentHealth {
    let executable = bundled_mcp_server_executable();
    probe_mcp_server_with(&executable, MCP_PROBE_TIMEOUT)
}

pub fn probe_proposal_validator() -> SettingsComponentHealth {
    let checked_at = current_timestamp();
    let (project, request, valid_proposal, invalid_proposal) = proposal_validator_fixture();
    let initial_project = project.clone();
    let mut provenance = BTreeMap::from([
        (
            "validatorSchemaVersion".to_string(),
            CODEX_EDIT_PROPOSAL_VALIDATOR_SCHEMA_VERSION.to_string(),
        ),
        (
            "fixtureVersion".to_string(),
            PROPOSAL_VALIDATOR_FIXTURE_VERSION.to_string(),
        ),
        (
            "expectedInvalidFixtureIssueCode".to_string(),
            INVALID_VISUAL_FIXTURE_ISSUE_CODE.to_string(),
        ),
        ("projectActionsWritten".to_string(), "0".to_string()),
    ]);

    let valid_edl = match validate_codex_edit_proposal(&project, &request, &valid_proposal) {
        Ok(edl) => edl,
        Err(error) => {
            provenance.insert(
                "validFixtureIssueCode".to_string(),
                error.stable_code().to_string(),
            );
            return proposal_validator_health(
                SettingsHealthState::Failed,
                "The proposal validator rejected its valid EDL-first fixture.",
                "agent.proposalValidator.validFixtureRejected",
                Some(error.to_string()),
                provenance,
                checked_at,
            );
        }
    };
    provenance.insert("validFixtureResult".to_string(), "accepted".to_string());
    provenance.insert(
        "selectedDurationSeconds".to_string(),
        valid_edl.duration_seconds().to_string(),
    );

    let invalid_error = match validate_codex_edit_proposal(&project, &request, &invalid_proposal) {
        Ok(_) => {
            return proposal_validator_health(
                SettingsHealthState::Failed,
                "The proposal validator accepted a visual layer outside the selected duration.",
                "agent.proposalValidator.invalidFixtureAccepted",
                None,
                provenance,
                checked_at,
            );
        }
        Err(error) => error,
    };
    let invalid_issue_code = invalid_error.stable_code();
    provenance.insert(
        "invalidFixtureIssueCode".to_string(),
        invalid_issue_code.to_string(),
    );
    if invalid_issue_code != INVALID_VISUAL_FIXTURE_ISSUE_CODE {
        return proposal_validator_health(
            SettingsHealthState::Failed,
            "The proposal validator returned an unexpected issue for invalid visual timing.",
            "agent.proposalValidator.unexpectedIssue",
            Some(invalid_error.to_string()),
            provenance,
            checked_at,
        );
    }

    if project != initial_project {
        return proposal_validator_health(
            SettingsHealthState::Failed,
            "The proposal validator changed canonical project state.",
            "agent.proposalValidator.stateMutation",
            None,
            provenance,
            checked_at,
        );
    }
    provenance.insert("canonicalProjectUnchanged".to_string(), "true".to_string());

    proposal_validator_health(
        SettingsHealthState::Ready,
        "Proposal validation accepted the EDL-first fixture and rejected invalid visual timing.",
        "agent.proposalValidator.ready",
        None,
        provenance,
        checked_at,
    )
}

pub fn mcp_client_configuration(active_project_dir: Option<&Path>) -> McpClientConfigurationState {
    mcp_client_configuration_with(&bundled_mcp_server_executable(), active_project_dir)
}

pub fn run_agent_component_self_test(
    component_id: &str,
    claude_executable_override: Option<&str>,
) -> SettingsOperation {
    if component_id == CODEX_COMPONENT_ID {
        return settings_operation_for_health(component_id, probe_codex_app_server());
    }
    if component_id == MCP_COMPONENT_ID {
        return settings_operation_for_health(component_id, probe_mcp_server());
    }
    if component_id == PROPOSAL_VALIDATOR_COMPONENT_ID {
        return settings_operation_for_health(component_id, probe_proposal_validator());
    }
    if component_id == CLAUDE_COMPONENT_ID {
        return settings_operation_for_health(
            component_id,
            probe_claude_cli(claude_executable_override),
        );
    }
    run_agent_component_self_test_with(component_id, Path::new(""), CODEX_PROBE_TIMEOUT)
}

fn proposal_validator_fixture() -> (
    VideoProject,
    EditJobRequest,
    CodexEditProposal,
    CodexEditProposal,
) {
    let mut project = VideoProject::new_empty(
        "proposal-validator-health".to_string(),
        "Proposal Validator Health".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "fixture-media".to_string(),
        name: Some("Validator fixture".to_string()),
        relative_path: "fixtures/proposal-validator.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    let request = EditJobRequest {
        media_id: "fixture-media".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Build a deterministic EDL-first validator fixture.".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let valid_proposal = CodexEditProposal {
        media_id: "fixture-media".to_string(),
        clips: vec![
            CodexProposalClip {
                media_id: "fixture-media".to_string(),
                source_in: 1.0,
                source_out: 28.5,
                reason: "opening hook and setup".to_string(),
            },
            CodexProposalClip {
                media_id: "fixture-media".to_string(),
                source_in: 45.25,
                source_out: 62.75,
                reason: "payoff and closing beat".to_string(),
            },
        ],
        captions: Vec::new(),
        overlays: vec![json!({
            "kind": "lower_third",
            "startSeconds": 5.0,
            "durationSeconds": 3.0,
            "visualTreatment": "compact translucent lower third",
            "motion": "quick slide and fade",
            "safeZone": "inside ten percent margins",
            "avoid": "full-width opaque slabs"
        })],
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 45.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    };
    let mut invalid_proposal = valid_proposal.clone();
    invalid_proposal.overlays[0]["startSeconds"] = json!(44.0);
    invalid_proposal.overlays[0]["durationSeconds"] = json!(2.0);

    (project, request, valid_proposal, invalid_proposal)
}

fn proposal_validator_health(
    state: SettingsHealthState,
    summary: &str,
    diagnostic_code: &str,
    diagnostic_detail: Option<String>,
    provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    SettingsComponentHealth {
        id: PROPOSAL_VALIDATOR_COMPONENT_ID.to_string(),
        label: "Proposal validation".to_string(),
        state,
        summary: summary.to_string(),
        action_id: Some("agent.proposalValidator.selfTest".to_string()),
        action_label: Some("Run proposal-validator self-test".to_string()),
        last_checked_at: checked_at,
        diagnostic_code: Some(diagnostic_code.to_string()),
        diagnostic_detail,
        provenance,
    }
}

fn probe_mcp_server_with(mcp_binary: &Path, timeout: Duration) -> SettingsComponentHealth {
    let checked_at = current_timestamp();
    let executable = mcp_binary.display().to_string();
    let provenance = mcp_command_provenance(mcp_binary, None);
    if !mcp_binary.is_file() {
        return mcp_health(
            SettingsHealthState::ActionRequired,
            "The bundled Video Creater MCP server is missing.",
            "agent.mcp.missing",
            Some(format!("MCP executable is missing: {executable}")),
            provenance,
            checked_at,
        );
    }

    let fixture = match SchemaV2ProjectFixture::materialize() {
        Ok(fixture) => fixture,
        Err(error) => {
            return mcp_health(
                SettingsHealthState::Failed,
                "The MCP self-test project could not be created.",
                "agent.mcp.fixtureFailed",
                Some(error),
                provenance,
                checked_at,
            );
        }
    };
    let mut provenance = mcp_command_provenance(mcp_binary, Some(fixture.path()));
    provenance.insert(
        "fixtureSchemaVersion".to_string(),
        fixture.schema_version().to_string(),
    );
    let initialize_request = build_mcp_initialize_request(MCP_INITIALIZE_REQUEST_ID);
    let stdin_line = match serde_json::to_string(&initialize_request) {
        Ok(line) => line,
        Err(error) => {
            return mcp_health(
                SettingsHealthState::Failed,
                "The MCP initialize request could not be encoded.",
                "agent.mcp.launchFailed",
                Some(error.to_string()),
                provenance,
                checked_at,
            );
        }
    };

    match run_process_probe(ProcessProbeRequest {
        program: mcp_binary.to_path_buf(),
        args: vec![
            "--project-dir".to_string(),
            fixture.path().to_string_lossy().into_owned(),
        ],
        stdin_lines: vec![stdin_line],
        timeout,
        max_stdout_bytes: CODEX_PROBE_MAX_STDOUT_BYTES,
        max_stderr_bytes: CODEX_PROBE_MAX_STDERR_BYTES,
        terminate_after_response_id: Some(MCP_INITIALIZE_REQUEST_ID),
        stdout_format: ProcessProbeStdoutFormat::JsonLines,
    }) {
        Ok(result) => mcp_health_from_probe(result, provenance, checked_at),
        Err(error) => mcp_health_from_probe_error(error, provenance, checked_at),
    }
}

fn mcp_health_from_probe(
    result: ProcessProbeResult,
    mut provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    provenance.insert(
        "elapsedMillis".to_string(),
        result.elapsed_millis.to_string(),
    );
    provenance.insert(
        "exitCode".to_string(),
        result
            .exit_state
            .code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string()),
    );
    provenance.insert("terminated".to_string(), result.terminated.to_string());
    if !result.stderr.is_empty() {
        provenance.insert("stderr".to_string(), result.stderr.clone());
    }

    let Some(response) = result
        .response_lines
        .iter()
        .find(|message| message.get("id").and_then(Value::as_u64) == Some(1))
        .cloned()
    else {
        return mcp_health(
            SettingsHealthState::ActionRequired,
            "The MCP server returned an incompatible initialize response.",
            "agent.mcp.malformedResponse",
            Some("MCP server did not return the initialize response.".to_string()),
            provenance,
            checked_at,
        );
    };
    provenance.insert("initializeResponse".to_string(), response.to_string());
    if !result.exit_state.success {
        return mcp_health(
            SettingsHealthState::Failed,
            "The MCP server exited unsuccessfully after initialize.",
            "agent.mcp.launchFailed",
            Some("MCP server returned a nonzero exit status.".to_string()),
            provenance,
            checked_at,
        );
    }

    match decode_mcp_initialize_response(MCP_INITIALIZE_REQUEST_ID, response) {
        Ok(initialize) => {
            provenance.insert("protocolVersion".to_string(), initialize.protocol_version);
            provenance.insert("serverName".to_string(), initialize.server_name);
            provenance.insert("serverVersion".to_string(), initialize.server_version);
            mcp_health(
                SettingsHealthState::Ready,
                "Video Creater MCP initialize handshake succeeded.",
                "agent.mcp.ready",
                None,
                provenance,
                checked_at,
            )
        }
        Err(error @ McpInitializeResponseError::ProtocolMismatch { .. }) => mcp_health(
            SettingsHealthState::ActionRequired,
            "The MCP server uses an incompatible protocol version.",
            "agent.mcp.protocolMismatch",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
        Err(error) => mcp_health(
            SettingsHealthState::ActionRequired,
            "The MCP server returned an incompatible initialize response.",
            "agent.mcp.malformedResponse",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
    }
}

fn mcp_health_from_probe_error(
    error: ProcessProbeError,
    mut provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    record_mcp_probe_error_provenance(&error, &mut provenance);
    match &error {
        ProcessProbeError::Timeout { .. } => mcp_health(
            SettingsHealthState::Failed,
            "The MCP server did not initialize before the timeout.",
            "agent.mcp.timeout",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
        ProcessProbeError::MalformedJson { .. }
        | ProcessProbeError::CaptureTimeout { .. }
        | ProcessProbeError::StdoutLimitExceeded { .. } => mcp_health(
            SettingsHealthState::ActionRequired,
            "The MCP server returned an incompatible initialize response.",
            "agent.mcp.malformedResponse",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
        error => mcp_health(
            SettingsHealthState::Failed,
            "The MCP server could not be launched.",
            "agent.mcp.launchFailed",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
    }
}

fn record_mcp_probe_error_provenance(
    error: &ProcessProbeError,
    provenance: &mut BTreeMap<String, String>,
) {
    provenance.insert("probeError".to_string(), error.to_string());
    match error {
        ProcessProbeError::InvalidRequest { field, detail } => {
            provenance.insert("probeErrorKind".to_string(), "invalidRequest".to_string());
            provenance.insert("requestField".to_string(), (*field).to_string());
            provenance.insert("probeErrorDetail".to_string(), detail.clone());
        }
        ProcessProbeError::UnsupportedPlatform => {
            provenance.insert(
                "probeErrorKind".to_string(),
                "unsupportedPlatform".to_string(),
            );
        }
        ProcessProbeError::LaunchFailed { program, detail } => {
            provenance.insert("probeErrorKind".to_string(), "launchFailed".to_string());
            provenance.insert("launchPath".to_string(), program.display().to_string());
            provenance.insert("launchError".to_string(), detail.clone());
        }
        ProcessProbeError::StdinWriteFailed { detail, cleanup } => {
            provenance.insert("probeErrorKind".to_string(), "stdinWriteFailed".to_string());
            provenance.insert("probeErrorDetail".to_string(), detail.clone());
            record_probe_cleanup(cleanup, provenance);
        }
        ProcessProbeError::StatusFailed { detail, cleanup } => {
            provenance.insert("probeErrorKind".to_string(), "statusFailed".to_string());
            provenance.insert("probeErrorDetail".to_string(), detail.clone());
            record_probe_cleanup(cleanup, provenance);
        }
        ProcessProbeError::OutputReadFailed { stream, detail } => {
            provenance.insert("probeErrorKind".to_string(), "outputReadFailed".to_string());
            provenance.insert("stream".to_string(), (*stream).to_string());
            provenance.insert("probeErrorDetail".to_string(), detail.clone());
        }
        ProcessProbeError::Timeout {
            timeout_millis,
            elapsed_millis,
            stderr,
            stderr_truncated,
            terminated,
            cleanup,
            capture_errors,
        } => {
            provenance.insert("probeErrorKind".to_string(), "timeout".to_string());
            provenance.insert("timeoutMillis".to_string(), timeout_millis.to_string());
            record_probe_runtime(
                *elapsed_millis,
                stderr,
                *stderr_truncated,
                *terminated,
                provenance,
            );
            provenance.insert(
                "captureErrors".to_string(),
                json_string_array(capture_errors),
            );
            record_probe_cleanup(cleanup, provenance);
        }
        ProcessProbeError::CaptureTimeout {
            exit_state,
            elapsed_millis,
            pending_streams,
            stderr,
            stderr_truncated,
            terminated,
            cleanup,
            capture_errors,
        } => {
            provenance.insert("probeErrorKind".to_string(), "captureTimeout".to_string());
            record_probe_exit_state(exit_state, provenance);
            record_probe_runtime(
                *elapsed_millis,
                stderr,
                *stderr_truncated,
                *terminated,
                provenance,
            );
            provenance.insert(
                "pendingStreams".to_string(),
                json_string_array(pending_streams),
            );
            provenance.insert(
                "captureErrors".to_string(),
                json_string_array(capture_errors),
            );
            record_probe_cleanup(cleanup, provenance);
        }
        ProcessProbeError::MalformedJson {
            line_number,
            line,
            detail,
            exit_state,
            stderr,
            stderr_truncated,
            elapsed_millis,
            terminated,
            cleanup,
        } => {
            provenance.insert("probeErrorKind".to_string(), "malformedJson".to_string());
            provenance.insert("responseLineNumber".to_string(), line_number.to_string());
            provenance.insert("responseLine".to_string(), line.clone());
            provenance.insert("responseParseError".to_string(), detail.clone());
            record_probe_exit_state(exit_state, provenance);
            record_probe_runtime(
                *elapsed_millis,
                stderr,
                *stderr_truncated,
                *terminated,
                provenance,
            );
            record_probe_cleanup(cleanup, provenance);
        }
        ProcessProbeError::StdoutLimitExceeded {
            max_bytes,
            elapsed_millis,
            stderr,
            stderr_truncated,
            terminated,
            cleanup,
        } => {
            provenance.insert(
                "probeErrorKind".to_string(),
                "stdoutLimitExceeded".to_string(),
            );
            provenance.insert("stdoutLimitBytes".to_string(), max_bytes.to_string());
            record_probe_runtime(
                *elapsed_millis,
                stderr,
                *stderr_truncated,
                *terminated,
                provenance,
            );
            record_probe_cleanup(cleanup, provenance);
        }
    }
}

fn record_probe_runtime(
    elapsed_millis: u64,
    stderr: &str,
    stderr_truncated: bool,
    terminated: bool,
    provenance: &mut BTreeMap<String, String>,
) {
    provenance.insert("elapsedMillis".to_string(), elapsed_millis.to_string());
    provenance.insert("stderr".to_string(), stderr.to_string());
    provenance.insert("stderrTruncated".to_string(), stderr_truncated.to_string());
    provenance.insert("terminated".to_string(), terminated.to_string());
}

fn record_probe_exit_state(
    exit_state: &ProcessProbeExitState,
    provenance: &mut BTreeMap<String, String>,
) {
    provenance.insert(
        "exitCode".to_string(),
        exit_state
            .code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string()),
    );
    provenance.insert("exitSuccess".to_string(), exit_state.success.to_string());
}

fn record_probe_cleanup(cleanup: &ProcessProbeCleanup, provenance: &mut BTreeMap<String, String>) {
    let termination = match cleanup.termination {
        ProcessProbeTerminationState::NotRequired => "notRequired",
        ProcessProbeTerminationState::Confirmed => "confirmed",
        ProcessProbeTerminationState::Partial => "partial",
    };
    provenance.insert("cleanupTermination".to_string(), termination.to_string());
    provenance.insert(
        "cleanupChildReaped".to_string(),
        cleanup.child_reaped.to_string(),
    );
    provenance.insert(
        "cleanupDescendantTerminationSupported".to_string(),
        cleanup.descendant_termination_supported.to_string(),
    );
    provenance.insert(
        "cleanupDescendantContainmentGuaranteed".to_string(),
        cleanup.descendant_containment_guaranteed.to_string(),
    );
    provenance.insert(
        "cleanupSteps".to_string(),
        json_string_array(&cleanup.steps),
    );
    provenance.insert(
        "cleanupErrors".to_string(),
        json_string_array(&cleanup.errors),
    );
}

fn json_string_array(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn mcp_client_configuration_with(
    executable: &Path,
    active_project_dir: Option<&Path>,
) -> McpClientConfigurationState {
    let Some(project_dir) = active_project_dir else {
        return McpClientConfigurationState {
            action_label: "Open a project".to_string(),
            configuration: None,
            executable: executable.to_path_buf(),
            project_dir: None,
        };
    };
    let configuration = serde_json::to_string_pretty(&json!({
        "mcpServers": {
            "video-creater": {
                "command": executable,
                "args": ["--project-dir", project_dir]
            }
        }
    }))
    .expect("paths serialize to JSON strings");
    McpClientConfigurationState {
        action_label: "Copy client configuration".to_string(),
        configuration: Some(configuration),
        executable: executable.to_path_buf(),
        project_dir: Some(project_dir.to_path_buf()),
    }
}

/// The app's own MCP sidecar, staged next to the running executable.
///
/// The Claude transport points `--mcp-config` at the same binary this module's readiness probe
/// and the copy-a-snippet dialog already name, so there is exactly one answer to "where is the
/// sidecar".
pub fn bundled_mcp_server_executable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .map(|directory| directory.join("video-creater-mcp-server"))
        .unwrap_or_else(|| PathBuf::from("video-creater-mcp-server"))
}

fn mcp_command_provenance(
    executable: &Path,
    fixture_dir: Option<&Path>,
) -> BTreeMap<String, String> {
    let executable = executable.display().to_string();
    let mut provenance = BTreeMap::from([("executable".to_string(), executable.clone())]);
    if let Some(fixture_dir) = fixture_dir {
        provenance.insert(
            "command".to_string(),
            format!(
                "{executable} --project-dir {}",
                fixture_dir.to_string_lossy()
            ),
        );
        provenance.insert(
            "fixtureProjectDir".to_string(),
            fixture_dir.to_string_lossy().into_owned(),
        );
    }
    provenance
}

fn mcp_health(
    state: SettingsHealthState,
    summary: &str,
    diagnostic_code: &str,
    diagnostic_detail: Option<String>,
    provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    SettingsComponentHealth {
        id: MCP_COMPONENT_ID.to_string(),
        label: "Video Creater MCP server".to_string(),
        state,
        summary: summary.to_string(),
        action_id: Some("agent.mcp.selfTest".to_string()),
        action_label: Some("Run MCP self-test".to_string()),
        last_checked_at: checked_at,
        diagnostic_code: Some(diagnostic_code.to_string()),
        diagnostic_detail,
        provenance,
    }
}

fn probe_codex_app_server_with(codex_binary: &Path, timeout: Duration) -> SettingsComponentHealth {
    let checked_at = current_timestamp();
    let requested_binary = codex_binary.display().to_string();
    let command = match resolve_codex_app_server_command(&requested_binary) {
        Ok(command) => command,
        Err(error) => {
            // Codex is bundled but no longer required: Claude answers turns on the user's
            // own subscription. So a missing sidecar is reported as a configuration state
            // whose remediation names the alternative, not as a broken install the user has
            // to repair before the app works.
            return codex_health(
                SettingsHealthState::NotConfigured,
                "The bundled Codex runtime is missing from this app installation. It is optional: turns can run on Claude instead.",
                "agent.codex.missing",
                Some(error.to_string()),
                BTreeMap::from([("requestedExecutable".to_string(), requested_binary)]),
                checked_at,
            );
        }
    };
    let initialize_request = build_app_server_initialize_request(CODEX_INITIALIZE_REQUEST_ID);
    let stdin_line = match serde_json::to_string(&initialize_request) {
        Ok(line) => line,
        Err(error) => {
            return codex_health(
                SettingsHealthState::Failed,
                "Codex app-server could not be launched.",
                "agent.codex.launchFailed",
                Some(error.to_string()),
                command_provenance(&command.program),
                checked_at,
            );
        }
    };

    match run_process_probe(ProcessProbeRequest {
        program: command.program.clone().into(),
        args: command.args,
        stdin_lines: vec![stdin_line],
        timeout,
        max_stdout_bytes: CODEX_PROBE_MAX_STDOUT_BYTES,
        max_stderr_bytes: CODEX_PROBE_MAX_STDERR_BYTES,
        terminate_after_response_id: Some(CODEX_INITIALIZE_REQUEST_ID),
        stdout_format: ProcessProbeStdoutFormat::JsonLines,
    }) {
        Ok(result) => codex_health_from_probe(result, &command.program, checked_at),
        Err(error) => codex_health_from_probe_error(error, &command.program, checked_at),
    }
}

fn codex_health_from_probe(
    result: ProcessProbeResult,
    executable: &str,
    checked_at: String,
) -> SettingsComponentHealth {
    let mut provenance = command_provenance(executable);
    provenance.insert(
        "elapsedMillis".to_string(),
        result.elapsed_millis.to_string(),
    );
    provenance.insert(
        "exitCode".to_string(),
        result
            .exit_state
            .code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string()),
    );
    provenance.insert("terminated".to_string(), result.terminated.to_string());
    if !result.stderr.is_empty() {
        provenance.insert("stderr".to_string(), result.stderr.clone());
    }

    let Some(response) = result
        .response_lines
        .iter()
        .find(|message| message.get("id").and_then(Value::as_u64) == Some(1))
        .cloned()
    else {
        return incompatible_health(
            "Codex app-server did not return the initialize response.",
            provenance,
            checked_at,
        );
    };
    provenance.insert("initializeResponse".to_string(), response.to_string());

    if !result.exit_state.success {
        return incompatible_health(
            "Codex app-server exited unsuccessfully after initialize.",
            provenance,
            checked_at,
        );
    }

    let initialize =
        match decode_app_server_initialize_response(CODEX_INITIALIZE_REQUEST_ID, response) {
            Ok(initialize) => initialize,
            Err(error) => {
                return incompatible_health(&error.to_string(), provenance, checked_at);
            }
        };
    let required_fields = ["userAgent", "codexHome", "platformFamily", "platformOs"];
    if let Some(field) = required_fields.iter().find(|field| {
        initialize
            .get(**field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    }) {
        return incompatible_health(
            &format!("Codex initialize result is missing {field}."),
            provenance,
            checked_at,
        );
    }
    for field in required_fields {
        provenance.insert(
            field.to_string(),
            initialize[field]
                .as_str()
                .expect("validated initialize field")
                .to_string(),
        );
    }

    codex_health(
        SettingsHealthState::Ready,
        "Codex app-server initialize handshake succeeded.",
        "agent.codex.ready",
        None,
        provenance,
        checked_at,
    )
}

fn codex_health_from_probe_error(
    error: ProcessProbeError,
    executable: &str,
    checked_at: String,
) -> SettingsComponentHealth {
    let provenance = command_provenance(executable);
    match error {
        error @ ProcessProbeError::Timeout { .. } => codex_health(
            SettingsHealthState::Failed,
            "Codex app-server did not initialize before the timeout.",
            "agent.codex.timeout",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
        error @ ProcessProbeError::MalformedJson { .. }
        | error @ ProcessProbeError::CaptureTimeout { .. }
        | error @ ProcessProbeError::StdoutLimitExceeded { .. } => codex_health(
            SettingsHealthState::ActionRequired,
            "Codex app-server returned an incompatible initialize response.",
            "agent.codex.incompatible",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
        error => codex_health(
            SettingsHealthState::Failed,
            "Codex app-server could not be launched.",
            "agent.codex.launchFailed",
            Some(error.to_string()),
            provenance,
            checked_at,
        ),
    }
}

fn incompatible_health(
    detail: &str,
    provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    codex_health(
        SettingsHealthState::ActionRequired,
        "Codex app-server returned an incompatible initialize response.",
        "agent.codex.incompatible",
        Some(detail.to_string()),
        provenance,
        checked_at,
    )
}

fn command_provenance(executable: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("executable".to_string(), executable.to_string()),
        (
            "command".to_string(),
            format!("{executable} app-server --stdio"),
        ),
    ])
}

fn codex_health(
    state: SettingsHealthState,
    summary: &str,
    diagnostic_code: &str,
    diagnostic_detail: Option<String>,
    provenance: BTreeMap<String, String>,
    checked_at: String,
) -> SettingsComponentHealth {
    SettingsComponentHealth {
        id: CODEX_COMPONENT_ID.to_string(),
        label: "Codex app-server".to_string(),
        state,
        summary: summary.to_string(),
        action_id: None,
        action_label: None,
        last_checked_at: checked_at,
        diagnostic_code: Some(diagnostic_code.to_string()),
        diagnostic_detail,
        provenance,
    }
}

/// The testable form: `executable` is the binary the requested component should
/// probe, so a self-test can be proved against a fixture instead of an install.
fn run_agent_component_self_test_with(
    component_id: &str,
    executable: &Path,
    timeout: Duration,
) -> SettingsOperation {
    let health = if component_id == CODEX_COMPONENT_ID {
        probe_codex_app_server_with(executable, timeout)
    } else if component_id == CLAUDE_COMPONENT_ID {
        probe_claude_cli(executable.to_str())
    } else if component_id == PROPOSAL_VALIDATOR_COMPONENT_ID {
        probe_proposal_validator()
    } else {
        codex_health(
            SettingsHealthState::Failed,
            "The requested Agent & MCP component is unknown.",
            "agent.component.unknown",
            Some(format!("unknown component id: {component_id}")),
            BTreeMap::new(),
            current_timestamp(),
        )
    };
    settings_operation_for_health(component_id, health)
}

fn settings_operation_for_health(
    component_id: &str,
    health: SettingsComponentHealth,
) -> SettingsOperation {
    let ready = health.state == SettingsHealthState::Ready;
    let now = current_operation_timestamp();
    SettingsOperation {
        id: uuid::Uuid::new_v4().to_string(),
        kind: SettingsOperationKind::HealthCheck,
        target_id: component_id.to_string(),
        phase: if ready { "ready" } else { "failed" }.to_string(),
        state: if ready {
            SettingsOperationState::Succeeded
        } else {
            SettingsOperationState::Failed
        },
        completed_units: 1,
        total_units: Some(1),
        unit: Some("components".to_string()),
        cancellable: false,
        message: health.summary.clone(),
        error: (!ready).then(|| SettingsOperationError {
            code: health
                .diagnostic_code
                .clone()
                .unwrap_or_else(|| "agent.component.failed".to_string()),
            message: health.summary,
            recovery_action: recovery_action_for(health.diagnostic_code.as_deref()),
            detail: health.diagnostic_detail,
        }),
        started_at: now.clone(),
        updated_at: now,
    }
}

fn recovery_action_for(diagnostic_code: Option<&str>) -> Option<String> {
    match diagnostic_code {
        Some("agent.codex.missing") => Some(
            "Use Claude instead, or reinstall Video Creater to restore the bundled Codex runtime."
                .to_string(),
        ),
        Some("agent.codex.incompatible") => Some("Update Video Creater and retry.".to_string()),
        Some("agent.codex.timeout") => Some("Retry the Codex health check.".to_string()),
        Some("agent.codex.launchFailed") => {
            Some("Update or reinstall Video Creater, then retry.".to_string())
        }
        Some("agent.claude.missing") => {
            Some("Install Claude from claude.com, then run the self-test again.".to_string())
        }
        Some("agent.claude.notLoggedIn") => Some(
            "Run `claude` in a terminal and use /login to sign in with your Claude subscription, then run the self-test again."
                .to_string(),
        ),
        Some("agent.claude.launchFailed") | Some("agent.claude.malformedStatus") => {
            Some("Update Claude, then run the self-test again.".to_string())
        }
        Some("agent.mcp.missing") => Some("Reinstall Video Creater.".to_string()),
        Some("agent.mcp.malformedResponse") | Some("agent.mcp.protocolMismatch") => {
            Some("Update or reinstall Video Creater.".to_string())
        }
        Some("agent.mcp.timeout") | Some("agent.mcp.launchFailed") => {
            Some("Retry the MCP self-test.".to_string())
        }
        Some(code) if code.starts_with("agent.proposalValidator.") => {
            Some("Retry the proposal-validator self-test.".to_string())
        }
        Some("agent.component.unknown") => Some("Choose a supported component.".to_string()),
        _ => None,
    }
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn current_operation_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use crate::agent::claude_cli::{ClaudeAuthMode, ClaudeTurnReadiness};
    use crate::settings::health::SettingsHealthState;
    use crate::settings::operations::{SettingsOperationKind, SettingsOperationState};
    use crate::settings::process_probe::{
        ProcessProbeCleanup, ProcessProbeError, ProcessProbeTerminationState,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{
        claude_readiness, mcp_client_configuration_with, mcp_health_from_probe_error,
        probe_claude_cli, probe_claude_cli_with, probe_codex_app_server_with,
        probe_mcp_server_with, probe_proposal_validator, recovery_action_for,
        run_agent_component_self_test_with, with_codex_model_notice, CLAUDE_COMPONENT_ID,
        CLAUDE_SIGN_IN_SUMMARY, PROPOSAL_VALIDATOR_COMPONENT_ID,
    };

    const RESPONSIVE_FIXTURE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Serializes the tests that write an executable fixture and then run it.
    ///
    /// A fork in another test thread inherits the writable descriptor of a fixture being
    /// written, and the exec of that fixture then fails with ETXTBSY ("Text file busy").
    /// Holding this for a fixture's whole lifetime keeps one writer and one exec at a time.
    static EXECUTABLE_FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The lock, ignoring poisoning: a panicking test leaves no shared state behind.
    fn lock_executable_fixtures() -> std::sync::MutexGuard<'static, ()> {
        EXECUTABLE_FIXTURE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Writes `body` as an executable shell script named `name` in a fresh temp directory.
    fn write_executable_fixture(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write fixture");
        let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("fixture permissions");
        (temp, path)
    }
    const MISSING_CLAUDE_PATH: &str = "/tmp/video-creater-missing-claude-cli/claude";

    /// A stand-in `claude` that answers `--version` and `auth status` from a script,
    /// so readiness is proved without the real binary and without spending tokens.
    struct ClaudeCliFixture {
        _guard: std::sync::MutexGuard<'static, ()>,
        _temp: tempfile::TempDir,
        path: PathBuf,
    }

    impl ClaudeCliFixture {
        fn new(body: &str) -> Self {
            let guard = lock_executable_fixtures();
            let (temp, path) = write_executable_fixture("claude", body);
            Self {
                _guard: guard,
                _temp: temp,
                path,
            }
        }

        fn answering(version: &str, auth_status: &str) -> Self {
            Self::new(&format!(
                r#"case "$1" in
  --version) printf '%s\n' '{version}' ;;
  auth) printf '%s\n' '{auth_status}' ;;
  *) exit 64 ;;
esac
"#
            ))
        }
    }

    #[test]
    fn a_subscription_login_is_ready_and_says_no_api_key_is_needed() {
        let fixture = ClaudeCliFixture::answering(
            "2.1.270 (Claude Code)",
            r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max","email":"a@b.c","orgId":"org_1"}"#,
        );

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.id, CLAUDE_COMPONENT_ID);
        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(
            health.summary,
            "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed."
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.ready")
        );
        assert_eq!(
            readiness,
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription)
        );
        assert_eq!(
            health.provenance.get("executable").map(String::as_str),
            Some(fixture.path.to_str().expect("fixture path"))
        );
        assert_eq!(
            health.provenance.get("version").map(String::as_str),
            Some("2.1.270")
        );
        assert_eq!(
            health.provenance.get("authMethod").map(String::as_str),
            Some("claude.ai")
        );
        assert_eq!(
            health
                .provenance
                .get("subscriptionType")
                .map(String::as_str),
            Some("max")
        );
        // The account's identity is the user's data, and the row has no use for it.
        let rendered = format!("{health:?}");
        assert!(!rendered.contains("a@b.c"), "{rendered}");
        assert!(!rendered.contains("org_1"), "{rendered}");
    }

    #[test]
    fn a_subscription_login_without_a_plan_still_names_the_subscription() {
        let fixture = ClaudeCliFixture::answering(
            "2.1.270 (Claude Code)",
            r#"{"loggedIn":true,"authMethod":"claude.ai"}"#,
        );

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(
            health.summary,
            "Claude 2.1.270 is signed in with your Claude subscription. No API key is needed."
        );
        assert_eq!(
            readiness,
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription)
        );
    }

    /// A login the app does not claim to understand — an enterprise route, a stored key, a
    /// gateway. It is ready, it is not described as a subscription, and its `ANTHROPIC_API_KEY`
    /// is left alone because the app cannot tell whether it is the credential.
    #[test]
    fn a_login_that_is_not_a_subscription_is_ready_and_says_so() {
        let fixture = ClaudeCliFixture::answering(
            "2.1.270 (Claude Code)",
            r#"{"loggedIn":true,"authMethod":"apiKey","apiKeySource":"ANTHROPIC_API_KEY"}"#,
        );

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(
            health.summary,
            "Claude 2.1.270 is signed in, but not with a Claude subscription."
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.readyOtherAuth")
        );
        assert_eq!(
            readiness,
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::OtherLogin)
        );
        assert_eq!(
            health.provenance.get("authMethod").map(String::as_str),
            Some("apiKey")
        );
    }

    /// The real CLI pretty-prints `claude auth status` across several lines. A JSON-lines
    /// reader cannot parse that, and a packaged Linux run reported a signed-in 2.1.270
    /// install as "failed / agent.claude.malformedStatus" because of it.
    #[test]
    fn a_pretty_printed_auth_status_is_read_as_one_json_document() {
        let fixture = ClaudeCliFixture::answering(
            "2.1.270 (Claude Code)",
            "{\n  \"loggedIn\": true,\n  \"authMethod\": \"claude.ai\",\n  \"subscriptionType\": \"max\",\n  \"email\": \"a@b.c\"\n}",
        );

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::Ready, "{health:?}");
        assert_eq!(
            readiness,
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription)
        );
        assert_eq!(
            health
                .provenance
                .get("subscriptionType")
                .map(String::as_str),
            Some("max")
        );
    }

    /// The whole point of the copy: a signed-out user is pointed at their subscription login,
    /// not at buying API credit.
    #[test]
    fn a_signed_out_claude_points_at_the_subscription_login() {
        let fixture = ClaudeCliFixture::answering("2.1.270 (Claude Code)", r#"{"loggedIn":false}"#);

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::ActionRequired);
        assert_eq!(health.summary, CLAUDE_SIGN_IN_SUMMARY);
        assert!(health.summary.contains("/login"), "{}", health.summary);
        assert!(
            !health.summary.to_lowercase().contains("api key"),
            "the sign-in instruction must not ask for an API key: {}",
            health.summary
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.notLoggedIn")
        );
        assert_eq!(health.action_id.as_deref(), Some("agent.claude.selfTest"));
        // A signed-out Claude must not take a turn Codex could take.
        assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
    }

    /// An API key is accepted when it is the only credential, and the row says plainly that a
    /// subscription would be used instead — the key is optional, never required.
    #[test]
    fn a_signed_out_claude_with_an_api_key_is_ready_on_the_key() {
        let fixture = ClaudeCliFixture::answering("2.1.270 (Claude Code)", r#"{"loggedIn":false}"#);

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, true);

        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(
            health.summary,
            "Claude 2.1.270 isn't signed in, so turns will use the Anthropic API key from the environment. Sign in with your subscription to use it instead."
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.apiKey")
        );
        assert_eq!(
            readiness,
            ClaudeTurnReadiness::Ready(ClaudeAuthMode::ApiKey)
        );
    }

    #[test]
    fn a_claude_that_is_not_installed_is_not_configured() {
        let (health, readiness) = claude_readiness(Some(MISSING_CLAUDE_PATH));

        assert_eq!(health.state, SettingsHealthState::NotConfigured);
        assert_eq!(
            health.summary,
            "Claude isn't installed. Install it from claude.com, then sign in with your subscription."
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.missing")
        );
        assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
        assert_eq!(
            probe_claude_cli(Some(MISSING_CLAUDE_PATH)).state,
            health.state
        );
    }

    #[test]
    fn a_claude_that_will_not_start_reports_its_stderr() {
        let fixture = ClaudeCliFixture::new(
            r#">&2 printf '%s\n' 'claude: incompatible glibc'
exit 1
"#,
        );

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(health.summary, "Claude is installed but didn't start.");
        assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.launchFailed")
        );
        assert!(
            health
                .diagnostic_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("incompatible glibc")),
            "unexpected detail: {:?}",
            health.diagnostic_detail
        );
    }

    #[test]
    fn a_claude_with_an_unreadable_auth_status_reports_a_malformed_status() {
        let fixture = ClaudeCliFixture::answering("2.1.270 (Claude Code)", "not json at all");

        let (health, readiness) =
            probe_claude_cli_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT, false);

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(
            health.summary,
            "Claude is installed but its sign-in status could not be read."
        );
        assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.claude.malformedStatus")
        );
    }

    #[test]
    fn the_claude_self_test_reruns_both_probes_and_returns_an_operation() {
        let operation = run_agent_component_self_test_with(
            CLAUDE_COMPONENT_ID,
            &PathBuf::from(MISSING_CLAUDE_PATH),
            Duration::from_millis(1),
        );

        assert_eq!(operation.target_id, CLAUDE_COMPONENT_ID);
        assert_eq!(operation.state, SettingsOperationState::Failed);
        assert_eq!(
            operation.error.as_ref().map(|error| error.code.as_str()),
            Some("agent.claude.missing")
        );
        assert_eq!(
            operation
                .error
                .as_ref()
                .and_then(|error| error.recovery_action.as_deref()),
            Some("Install Claude from claude.com, then run the self-test again.")
        );
    }

    #[test]
    fn ready_codex_status_shows_the_model_notice_and_failures_do_not() {
        let notice = "Your Codex config selects a model this app's Codex can't use; using gpt-5.5.";
        let ready = super::codex_health(
            SettingsHealthState::Ready,
            "Codex app-server initialize handshake succeeded.",
            "agent.codex.ready",
            None,
            Default::default(),
            "2026-09-17T00:00:00Z".to_string(),
        );
        let failed = super::codex_health(
            SettingsHealthState::Failed,
            "Codex app-server could not be launched.",
            "agent.codex.launchFailed",
            None,
            Default::default(),
            "2026-09-17T00:00:00Z".to_string(),
        );

        let shown = with_codex_model_notice(ready.clone(), Some(notice.to_string()));
        assert_eq!(
            shown.summary,
            format!("Codex app-server initialize handshake succeeded. {notice}")
        );
        assert_eq!(shown.state, SettingsHealthState::Ready);
        assert_eq!(with_codex_model_notice(ready.clone(), None), ready);
        assert_eq!(
            with_codex_model_notice(failed.clone(), Some(notice.to_string())),
            failed
        );
    }

    #[test]
    fn proposal_validator_self_test_is_ready_with_versioned_pure_fixtures() {
        let health = probe_proposal_validator();

        assert_eq!(health.id, "agent.proposalValidator");
        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.proposalValidator.ready")
        );
        assert_eq!(
            health
                .provenance
                .get("validatorSchemaVersion")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            health.provenance.get("fixtureVersion").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            health
                .provenance
                .get("invalidFixtureIssueCode")
                .map(String::as_str),
            Some("codex.proposal.invalidOverlayTiming")
        );
        assert_eq!(
            health
                .provenance
                .get("projectActionsWritten")
                .map(String::as_str),
            Some("0")
        );
        assert_eq!(
            health
                .provenance
                .get("canonicalProjectUnchanged")
                .map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn proposal_validator_self_test_does_not_depend_on_codex_executable() {
        let operation = run_agent_component_self_test_with(
            PROPOSAL_VALIDATOR_COMPONENT_ID,
            &PathBuf::from("/tmp/video-creater-missing-codex-app-server"),
            Duration::from_millis(1),
        );

        assert_eq!(operation.target_id, "agent.proposalValidator");
        assert_eq!(operation.state, SettingsOperationState::Succeeded);
        assert_eq!(operation.phase, "ready");
    }

    #[test]
    fn ready_initialize_response_reports_codex_ready() {
        let fixture = AppServerFixture::new(
            r#"IFS= read -r request
printf '%s\n' '{"id":1,"result":{"userAgent":"codex-test/1","codexHome":"/tmp/codex","platformFamily":"unix","platformOs":"macos"}}'
"#,
        );

        let health = probe_codex_app_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(health.id, "agent.codex");
        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.codex.ready"));
        assert_eq!(
            health.provenance.get("userAgent").map(String::as_str),
            Some("codex-test/1")
        );
    }

    /// Codex is optional now that Claude answers turns, so an absent sidecar is a
    /// configuration state — the same state an absent `claude` reports — and its copy names
    /// the alternative instead of telling the user to repair the app.
    #[test]
    fn missing_executable_reports_stable_diagnostic() {
        let missing = PathBuf::from("/tmp/video-creater-missing-codex-app-server");

        let health = probe_codex_app_server_with(&missing, Duration::from_secs(1));

        assert_eq!(health.state, SettingsHealthState::NotConfigured);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.codex.missing")
        );
        assert!(
            health
                .summary
                .contains("It is optional: turns can run on Claude instead."),
            "{}",
            health.summary
        );
        assert_eq!(
            recovery_action_for(health.diagnostic_code.as_deref()).as_deref(),
            Some("Use Claude instead, or reinstall Video Creater to restore the bundled Codex runtime.")
        );
    }

    #[test]
    fn incompatible_initialize_protocol_reports_stable_diagnostic() {
        let fixture = AppServerFixture::new(
            r#"IFS= read -r request
printf '%s\n' '{"id":1,"result":{"userAgent":"old-codex"}}'
"#,
        );

        let health = probe_codex_app_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(
            health.state,
            SettingsHealthState::ActionRequired,
            "{health:?}"
        );
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.codex.incompatible")
        );
    }

    #[test]
    fn initialize_timeout_reports_stable_diagnostic() {
        let fixture = AppServerFixture::new(
            r#"IFS= read -r request
sleep 30
"#,
        );

        let health = probe_codex_app_server_with(&fixture.path, Duration::from_millis(50));

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.codex.timeout")
        );
    }

    #[test]
    fn launch_failure_reports_stable_diagnostic() {
        let temp = tempfile::tempdir().expect("temp");
        let fixture = temp.path().join("codex");
        fs::write(&fixture, "#!/bin/sh\nexit 0\n").expect("write fixture");

        let health = probe_codex_app_server_with(&fixture, Duration::from_secs(1));

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.codex.launchFailed")
        );
    }

    #[test]
    fn app_server_process_is_terminated_after_the_probe() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("codex.pid");
        let fixture = AppServerFixture::new(&format!(
            r#"printf '%s' "$$" > '{}'
IFS= read -r request
printf '%s\n' '{{"id":1,"result":{{"userAgent":"codex-test/1","codexHome":"/tmp/codex","platformFamily":"unix","platformOs":"macos"}}}}'
while IFS= read -r extra; do :; done
"#,
            pid_path.display()
        ));

        let health = probe_codex_app_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(health.state, SettingsHealthState::Ready);
        let pid = fs::read_to_string(&pid_path)
            .expect("pid file")
            .parse::<i32>()
            .expect("pid");
        assert!(
            !process_exists(pid),
            "Codex app-server fixture process {pid} survived the probe"
        );
    }

    #[test]
    fn component_self_test_returns_the_shared_settings_operation_shape() {
        let fixture = AppServerFixture::new(
            r#"IFS= read -r request
printf '%s\n' '{"id":1,"result":{"userAgent":"codex-test/1","codexHome":"/tmp/codex","platformFamily":"unix","platformOs":"macos"}}'
"#,
        );

        let operation = run_agent_component_self_test_with(
            "agent.codex",
            &fixture.path,
            RESPONSIVE_FIXTURE_TIMEOUT,
        );

        assert_eq!(operation.kind, SettingsOperationKind::HealthCheck);
        assert_eq!(operation.target_id, "agent.codex");
        assert_eq!(operation.state, SettingsOperationState::Succeeded);
        assert_eq!(operation.phase, "ready");
        assert!(!operation.cancellable);
    }

    #[test]
    fn missing_mcp_binary_reports_stable_diagnostic() {
        let missing = PathBuf::from("/tmp/video-creater-missing-mcp-server");

        let health = probe_mcp_server_with(&missing, Duration::from_secs(1));

        assert_eq!(health.id, "agent.mcpServer");
        assert_eq!(health.state, SettingsHealthState::ActionRequired);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.mcp.missing"));
    }

    #[test]
    fn malformed_mcp_initialize_response_reports_stable_diagnostic() {
        let fixture = AppServerFixture::new(
            r#"test "$1" = "--project-dir"
test -f "$2/video-creater.project.json"
IFS= read -r request
printf '%s\n' 'malformed stderr evidence' >&2
printf '%s\n' 'not-json'
"#,
        );

        let health = probe_mcp_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(health.state, SettingsHealthState::ActionRequired);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.mcp.malformedResponse")
        );
        assert_eq!(
            health.provenance.get("responseLine").map(String::as_str),
            Some("not-json")
        );
        assert_eq!(
            health
                .provenance
                .get("responseLineNumber")
                .map(String::as_str),
            Some("1")
        );
        assert!(health
            .provenance
            .get("responseParseError")
            .is_some_and(|detail| detail.contains("expected ident")));
        assert!(health
            .provenance
            .get("stderr")
            .is_some_and(|stderr| stderr.contains("malformed stderr evidence")));
        assert_eq!(
            health.provenance.get("stderrTruncated").map(String::as_str),
            Some("false")
        );
        assert!(health
            .provenance
            .get("elapsedMillis")
            .is_some_and(|value| value.parse::<u64>().is_ok()));
        assert_eq!(
            health.provenance.get("terminated").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            health
                .provenance
                .get("cleanupTermination")
                .map(String::as_str),
            Some("notRequired")
        );
        assert_eq!(
            health
                .provenance
                .get("cleanupChildReaped")
                .map(String::as_str),
            Some("true")
        );
        assert!(health
            .provenance
            .get("cleanupSteps")
            .and_then(|steps| serde_json::from_str::<Vec<String>>(steps).ok())
            .is_some_and(|steps| !steps.is_empty()));
    }

    #[test]
    fn mismatched_mcp_protocol_reports_stable_diagnostic() {
        let fixture = AppServerFixture::new(
            r#"test "$1" = "--project-dir"
test -f "$2/video-creater.project.json"
IFS= read -r request
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-01-01","serverInfo":{"name":"video-creater","version":"test"},"capabilities":{"resources":{},"tools":{}}}}'
"#,
        );

        let health = probe_mcp_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(health.state, SettingsHealthState::ActionRequired);
        assert_eq!(
            health.diagnostic_code.as_deref(),
            Some("agent.mcp.protocolMismatch")
        );
    }

    #[test]
    fn mcp_initialize_timeout_reports_stable_diagnostic() {
        let fixture = AppServerFixture::new(
            r#"test "$1" = "--project-dir"
test -f "$2/video-creater.project.json"
IFS= read -r request
sleep 30
"#,
        );

        let health = probe_mcp_server_with(&fixture.path, Duration::from_millis(200));

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.mcp.timeout"));
        assert_eq!(
            health.provenance.get("timeoutMillis").map(String::as_str),
            Some("200")
        );
        assert_eq!(
            health.provenance.get("stderrTruncated").map(String::as_str),
            Some("false")
        );
        assert!(health
            .provenance
            .get("elapsedMillis")
            .is_some_and(|value| value.parse::<u64>().is_ok_and(|value| value >= 200)));
        assert!(health
            .provenance
            .get("terminated")
            .is_some_and(|terminated| matches!(terminated.as_str(), "true" | "false")));
        assert!(health
            .provenance
            .get("cleanupTermination")
            .is_some_and(|termination| matches!(termination.as_str(), "confirmed" | "partial")));
        assert_eq!(
            health
                .provenance
                .get("cleanupChildReaped")
                .map(String::as_str),
            Some("true")
        );
        assert!(health
            .provenance
            .get("cleanupSteps")
            .and_then(|steps| serde_json::from_str::<Vec<String>>(steps).ok())
            .is_some_and(|steps| !steps.is_empty()));
    }

    #[test]
    fn mcp_timeout_error_preserves_captured_stderr_provenance() {
        let cleanup = ProcessProbeCleanup {
            termination: ProcessProbeTerminationState::Confirmed,
            child_reaped: true,
            descendant_termination_supported: true,
            descendant_containment_guaranteed: false,
            steps: vec!["sent SIGTERM to process group".to_string()],
            errors: Vec::new(),
        };
        let error = ProcessProbeError::Timeout {
            timeout_millis: 200,
            elapsed_millis: 205,
            stderr: "timeout stderr evidence".to_string(),
            stderr_truncated: false,
            terminated: true,
            cleanup: Box::new(cleanup),
            capture_errors: Vec::new(),
        };

        let health = mcp_health_from_probe_error(
            error,
            std::collections::BTreeMap::new(),
            "2026-07-17T00:00:00.000Z".to_string(),
        );

        assert_eq!(health.state, SettingsHealthState::Failed);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.mcp.timeout"));
        assert_eq!(
            health.provenance.get("stderr").map(String::as_str),
            Some("timeout stderr evidence")
        );
        assert_eq!(
            health.provenance.get("stderrTruncated").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            health.provenance.get("probeErrorKind").map(String::as_str),
            Some("timeout")
        );
        assert_eq!(
            health
                .provenance
                .get("cleanupTermination")
                .map(String::as_str),
            Some("confirmed")
        );
    }

    #[test]
    fn valid_mcp_initialize_response_reports_ready_and_uses_schema_v2_fixture() {
        let fixture = AppServerFixture::new(
            r#"test "$1" = "--project-dir"
test -f "$2/video-creater.project.json"
grep -Eq '"schemaVersion"[[:space:]]*:[[:space:]]*2' "$2/video-creater.project.json"
IFS= read -r request
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","serverInfo":{"name":"video-creater","version":"test"},"capabilities":{"resources":{"listChanged":false},"tools":{"listChanged":false}}}}'
"#,
        );

        let health = probe_mcp_server_with(&fixture.path, RESPONSIVE_FIXTURE_TIMEOUT);

        assert_eq!(health.id, "agent.mcpServer");
        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.mcp.ready"));
        assert_eq!(
            health.provenance.get("protocolVersion").map(String::as_str),
            Some("2025-11-25")
        );
        assert_eq!(
            health
                .provenance
                .get("fixtureSchemaVersion")
                .map(String::as_str),
            Some("2")
        );
    }

    #[test]
    fn prepared_mcp_sidecar_passes_the_schema_v2_probe() {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!(
                "video-creater-mcp-server-{}",
                env!("TAURI_ENV_TARGET_TRIPLE")
            ));

        let health = probe_mcp_server_with(&binary, Duration::from_secs(2));

        assert_eq!(health.state, SettingsHealthState::Ready);
        assert_eq!(health.diagnostic_code.as_deref(), Some("agent.mcp.ready"));
    }

    #[test]
    fn mcp_configuration_requires_an_active_project() {
        let executable = PathBuf::from(
            "/Applications/Video Creater.app/Contents/MacOS/video-creater-mcp-server",
        );

        let state = mcp_client_configuration_with(&executable, None);

        assert_eq!(state.action_label, "Open a project");
        assert_eq!(state.configuration, None);
        assert_eq!(state.project_dir, None);
    }

    #[test]
    fn mcp_configuration_contains_exact_executable_and_project_paths() {
        let executable = PathBuf::from(
            "/Applications/Video Creater.app/Contents/MacOS/video-creater-mcp-server",
        );
        let project_dir = PathBuf::from("/Users/editor/Projects/Exact Project");

        let state = mcp_client_configuration_with(&executable, Some(&project_dir));
        let configuration: serde_json::Value = serde_json::from_str(
            state
                .configuration
                .as_deref()
                .expect("copyable configuration"),
        )
        .expect("configuration JSON");

        assert_eq!(state.action_label, "Copy client configuration");
        assert_eq!(state.project_dir.as_deref(), Some(project_dir.as_path()));
        assert_eq!(
            configuration["mcpServers"]["video-creater"]["command"],
            executable.to_string_lossy().as_ref()
        );
        assert_eq!(
            configuration["mcpServers"]["video-creater"]["args"],
            serde_json::json!(["--project-dir", project_dir])
        );
    }

    #[test]
    fn release_policy_bundles_prepares_and_verifies_mcp_server() {
        let tauri_config: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tauri.conf.json"
        )))
        .expect("tauri config JSON");
        let external_bins = tauri_config["bundle"]["externalBin"]
            .as_array()
            .expect("externalBin array");
        assert_eq!(
            external_bins
                .iter()
                .filter(|value| *value == "binaries/video-creater-mcp-server")
                .count(),
            1,
            "the MCP sidecar must be bundled exactly once"
        );
        assert_eq!(
            tauri_config["build"]["beforeDevCommand"]
                .as_str()
                .expect("beforeDevCommand"),
            "pnpm dev:web-runtime",
            "Tauri must start only the internal Vite runtime after native helper preparation"
        );
        assert!(tauri_config["build"]["beforeBuildCommand"]
            .as_str()
            .expect("beforeBuildCommand")
            .contains("build-macos-release.mjs --prepare-mcp-sidecar"));

        let cargo_toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        assert!(cargo_toml.contains("mcp-server = []"));
        assert!(cargo_toml.contains(
            "[[bin]]\nname = \"video-creater-mcp-server\"\npath = \"src/bin/video-creater-mcp-server.rs\"\nrequired-features = [\"mcp-server\"]"
        ));

        let release_script = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/build-macos-release.mjs"
        ));
        assert!(release_script.contains(
            "const mcpServerPath = join(appPath, \"Contents/MacOS/video-creater-mcp-server\");"
        ));
        assert!(release_script.contains(
            "const mcpServerCodesign = checked(\"codesign\", [\n  \"--verify\",\n  \"--strict\",\n  \"--verbose=2\",\n  mcpServerPath,\n]);"
        ));
        assert!(release_script.contains(
            "assertMatchingDeveloperIdSignature({\n  path: mcpServerPath,\n  signature: mcpServerSignature,\n  expectedAuthority: identity,\n  expectedTeamId: teamId,\n});"
        ));
        assert!(release_script.contains(
            "mcpServer: {\n      ...artifactEvidence(mcpServerPath),\n      codesign: mcpServerCodesign.output.trim(),\n      signature: mcpServerSignature,\n    }"
        ));
    }

    struct AppServerFixture {
        _guard: std::sync::MutexGuard<'static, ()>,
        _temp: tempfile::TempDir,
        path: PathBuf,
    }

    impl AppServerFixture {
        fn new(body: &str) -> Self {
            let guard = lock_executable_fixtures();
            let (temp, path) = write_executable_fixture("codex", body);
            Self {
                _guard: guard,
                _temp: temp,
                path,
            }
        }
    }

    fn process_exists(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        if result == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
