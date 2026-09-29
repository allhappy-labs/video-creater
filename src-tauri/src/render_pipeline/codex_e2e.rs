use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::codex::app_server::{
    build_app_server_initialize_request, build_video_edit_turn_request, build_video_thread_request,
    codex_app_server_command, codex_app_server_deadline, proposal_from_text, proposal_from_value,
    request_codex_app_server_until, run_codex_turn_pump, CodexAppServerError, CodexThreadAction,
    StdioCodexAppServerTransport,
};
use crate::codex::context::{build_video_edit_context, load_project_skill_bundle};
use crate::codex::proposal::validate_codex_edit_proposal;
use crate::codex::proposal::{CodexEditProposal, CodexProposalClip, CodexRenderReview};
use crate::codex::tools::{call_codex_local_tool, CodexLocalToolError};
use crate::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use crate::project::action::ProjectAction;
use crate::project::model::{
    JobStatus, JobSummary, MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTrack, TrackKind, Transcript, TranscriptSegment, TranscriptWord, VideoProject,
};
use crate::project::split::save_split_project;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_REPORT_PATH: &str = "output/e2e-codex/codex-report.json";
const E2E_MEDIA_ID: &str = "codex-e2e-media";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexE2eConfig {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub codex_binary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexE2eReport {
    pub generated_at: String,
    pub thread_id: String,
    pub proposal: CodexEditProposal,
    pub tool_acceptance: CodexE2eToolAcceptance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexE2eToolAcceptance {
    pub project_context: bool,
    pub inspect_timeline: bool,
    pub validate_proposal: bool,
    pub apply_project_actions: bool,
    pub build_codex_edit_start_request: bool,
    pub build_render_start_request: bool,
    pub build_export_start_request: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexE2eRunResult {
    pub report_path: PathBuf,
    pub codex_binary: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexE2eTurnOutput {
    pub turn_response: Value,
    pub assistant_text: String,
}

pub trait CodexE2eAppServerClient {
    fn initialize(&mut self, request: Value) -> PipelineResult<Value>;
    fn start_thread(&mut self, request: Value) -> PipelineResult<Value>;
    fn start_turn(&mut self, request: Value, thread_id: &str)
        -> PipelineResult<CodexE2eTurnOutput>;
}

pub struct StdioCodexE2eAppServerClient {
    transport: StdioCodexAppServerTransport,
    deadline: Instant,
}

#[derive(Debug, Default)]
struct CodexE2eArgs {
    project_root: Option<PathBuf>,
    report_path: Option<PathBuf>,
    codex_binary: Option<String>,
}

pub fn parse_codex_e2e_args<I, S>(args: I) -> PipelineResult<CodexE2eConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tokens = args.into_iter().map(Into::into).collect::<Vec<String>>();
    if tokens.first().is_some_and(|token| !token.starts_with("--")) {
        tokens.remove(0);
    }

    let mut parsed = CodexE2eArgs::default();
    let mut index = 0;
    while index < tokens.len() {
        let flag = &tokens[index];
        if !flag.starts_with("--") {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unexpected positional argument.",
                "Use --project-root, --report, or --codex.",
            )]);
        }
        if !is_codex_e2e_flag(flag) {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unknown Codex e2e argument.",
                "Use --project-root, --report, or --codex.",
            )]);
        }

        let value = tokens
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| {
                vec![arg_error(
                    arg_path(flag),
                    "Missing value for Codex e2e argument.",
                    format!("Pass a value after {flag}."),
                )]
            })?
            .clone();

        match flag.as_str() {
            "--project-root" => parsed.project_root = Some(value.into()),
            "--report" => parsed.report_path = Some(value.into()),
            "--codex" => parsed.codex_binary = Some(value),
            _ => unreachable!("codex e2e flag was checked before value parsing"),
        }

        index += 2;
    }

    Ok(CodexE2eConfig {
        project_root: default_project_root(parsed.project_root)?,
        report_path: parsed
            .report_path
            .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_PATH)),
        codex_binary: parsed.codex_binary.unwrap_or_else(|| {
            env::var("VIDEO_CREATER_CODEX").unwrap_or_else(|_| "codex".to_string())
        }),
    })
}

pub fn run_codex_e2e_cli<I, S>(args: I) -> PipelineResult<CodexE2eRunResult>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let config = parse_codex_e2e_args(args)?;
    run_codex_e2e(&config)
}

pub fn run_codex_e2e_cli_with_client<I, S, C>(
    args: I,
    client: &mut C,
) -> PipelineResult<CodexE2eRunResult>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
    C: CodexE2eAppServerClient,
{
    let config = parse_codex_e2e_args(args)?;
    run_codex_e2e_with_client(&config, client)
}

pub fn run_codex_e2e(config: &CodexE2eConfig) -> PipelineResult<CodexE2eRunResult> {
    if config.codex_binary == "fixture" {
        let mut client = FixtureCodexE2eAppServerClient;
        return run_codex_e2e_with_client(config, &mut client);
    }
    let mut client =
        StdioCodexE2eAppServerClient::spawn(&config.codex_binary, &config.project_root)?;
    run_codex_e2e_with_client(config, &mut client)
}

pub fn run_codex_e2e_with_client<C>(
    config: &CodexE2eConfig,
    client: &mut C,
) -> PipelineResult<CodexE2eRunResult>
where
    C: CodexE2eAppServerClient,
{
    let skills = load_project_skill_bundle(&config.project_root).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "projectRoot",
            "Codex e2e project skills could not be loaded.",
            "Run the Codex e2e from the Video Creater repository root.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let (mut project, edit_request) = build_codex_e2e_fixture();

    client.initialize(build_app_server_initialize_request(1))?;
    let thread_response = client.start_thread(build_video_thread_request(
        2,
        CodexThreadAction::Start,
        &config.project_root.display().to_string(),
        &skills,
    ))?;
    let thread_id = extract_thread_id(&thread_response)?;
    project.codex_thread_id = Some(thread_id.clone());

    let context = build_video_edit_context(&project, &edit_request).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project",
            "Codex e2e fixture could not build a video edit context.",
            "Keep the deterministic Codex e2e fixture media and transcript valid.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let turn_output = client.start_turn(
        build_video_edit_turn_request(3, &thread_id, &context),
        &thread_id,
    )?;
    let proposal = proposal_from_app_server_output(&turn_output)?;
    validate_codex_edit_proposal(&project, &edit_request, &proposal).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::CodexProposalInvalid,
            "proposal",
            "Codex e2e returned a proposal that failed Rust validation.",
            "Return selected sourceIn/sourceOut ranges and required render review criteria for the e2e media.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let tool_acceptance = run_codex_tool_acceptance(config, &project, &edit_request, &proposal)?;

    let report = CodexE2eReport {
        generated_at: Utc::now().to_rfc3339(),
        thread_id: thread_id.clone(),
        proposal,
        tool_acceptance,
    };
    write_codex_e2e_report(&config.report_path, &report)?;

    Ok(CodexE2eRunResult {
        report_path: config.report_path.clone(),
        codex_binary: config.codex_binary.clone(),
        thread_id,
    })
}

fn run_codex_tool_acceptance(
    config: &CodexE2eConfig,
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
) -> PipelineResult<CodexE2eToolAcceptance> {
    let split_project_dir = tempfile::tempdir().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "codexTools.projectDir",
            "Failed to create Codex E2E tool project directory.",
            "Use a writable temp directory for Codex E2E local tool acceptance.",
        )
        .with_detail("error", error.to_string())]
    })?;
    save_split_project(split_project_dir.path(), project).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "codexTools.projectDir",
            "Failed to save Codex E2E split project for local tool acceptance.",
            "Keep the Codex E2E fixture serializable as a split project.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let project_dir = split_project_dir.path().display().to_string();

    let project_context = call_tool(
        project,
        "video_creater.project_context",
        serde_json::json!({ "projectDir": project_dir }),
    )?;
    assert_tool_bool(
        &project_context.payload,
        "/counts/media",
        |value| value.as_u64().is_some_and(|count| count > 0),
        "project context",
    )?;

    let timeline = call_tool(
        project,
        "video_creater.inspect_timeline",
        serde_json::json!({
            "startSeconds": 0.0,
            "endSeconds": request.target_duration_seconds.unwrap_or(20.0),
            "maxItems": 10
        }),
    )?;
    assert_tool_bool(
        &timeline.payload,
        "/activeItems",
        |value| value.as_array().is_some(),
        "timeline inspection",
    )?;

    let validation = call_tool(
        project,
        "video_creater.validate_codex_edit_proposal",
        serde_json::json!({
            "request": request,
            "proposal": proposal
        }),
    )?;
    assert_tool_bool(
        &validation.payload,
        "/valid",
        |value| value.as_bool() == Some(true),
        "proposal validation",
    )?;

    let record_job_action = ProjectAction::RecordJob {
        job: Box::new(JobSummary {
            id: "codex-e2e-tool-job".to_string(),
            kind: "codex_edit".to_string(),
            status: JobStatus::Queued,
            updated_at: "2026-06-17T00:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        }),
    };
    let apply = call_tool(
        project,
        "video_creater.apply_project_actions",
        serde_json::json!({
            "projectDir": split_project_dir.path().display().to_string(),
            "actions": [record_job_action]
        }),
    )?;
    assert_tool_bool(
        &apply.payload,
        "/applied",
        |value| value.as_bool() == Some(true),
        "project action application",
    )?;

    let codex_start = call_tool(
        project,
        "video_creater.build_codex_edit_start_request",
        serde_json::json!({
            "projectRoot": config.project_root.display().to_string(),
            "projectDir": split_project_dir.path().display().to_string(),
            "jobId": "codex-e2e-tool-codex-job",
            "request": request
        }),
    )?;
    assert_tool_bool(
        &codex_start.payload,
        "/startRequest/workflowType",
        |value| value.as_str().is_some_and(|value| value.contains("Codex")),
        "Codex edit start request",
    )?;

    let render_start = call_tool(
        project,
        "video_creater.export_project",
        serde_json::json!({
            "projectDir": split_project_dir.path().display().to_string(),
            "jobId": "codex-e2e-tool-render-job",
            "profile": "draftWebm",
            "outputPath": split_project_dir.path().join("renders/codex-e2e.webm").display().to_string()
        }),
    )?;
    assert_tool_bool(
        &render_start.payload,
        "/startRequest/workflowType",
        |value| value.as_str().is_some_and(|value| value.contains("Render")),
        "render start request",
    )?;

    let export_start = call_tool(
        project,
        "video_creater.export_project",
        serde_json::json!({
            "projectDir": split_project_dir.path().display().to_string(),
            "jobId": "codex-e2e-tool-export-job",
            "profile": "premiereXmeml",
            "outputPath": split_project_dir.path().join("exports/codex-e2e.xml").display().to_string()
        }),
    )?;
    assert_tool_bool(
        &export_start.payload,
        "/startRequest/workflowType",
        |value| value.as_str().is_some_and(|value| value.contains("Export")),
        "export start request",
    )?;

    Ok(CodexE2eToolAcceptance {
        project_context: true,
        inspect_timeline: true,
        validate_proposal: true,
        apply_project_actions: apply.mutates_project,
        build_codex_edit_start_request: true,
        build_render_start_request: true,
        build_export_start_request: true,
    })
}

fn call_tool(
    project: &VideoProject,
    tool_name: &str,
    args: Value,
) -> Result<crate::codex::tools::CodexLocalToolCallResult, Vec<PipelineError>> {
    call_codex_local_tool(project, tool_name, args).map_err(|error| tool_error(tool_name, error))
}

fn tool_error(tool_name: &str, error: CodexLocalToolError) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("codexTools.{tool_name}"),
        "Codex E2E local tool acceptance failed.",
        "Keep the local Codex tool control plane compatible with the Codex E2E fixture.",
    )
    .with_detail("error", error.to_string())]
}

fn assert_tool_bool(
    payload: &Value,
    pointer: &str,
    predicate: impl FnOnce(&Value) -> bool,
    label: &str,
) -> PipelineResult<()> {
    let Some(value) = payload.pointer(pointer) else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("codexTools.{label}"),
            "Codex E2E local tool payload is missing required evidence.",
            "Return stable payload fields from local Codex tools.",
        )
        .with_detail("pointer", pointer.to_string())]);
    };
    if predicate(value) {
        Ok(())
    } else {
        Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("codexTools.{label}"),
            "Codex E2E local tool payload did not satisfy acceptance criteria.",
            "Return stable payload fields from local Codex tools.",
        )
        .with_detail("pointer", pointer.to_string())
        .with_detail("value", value.to_string())])
    }
}

impl StdioCodexE2eAppServerClient {
    pub fn spawn(codex_binary: &str, project_root: &Path) -> PipelineResult<Self> {
        let command = codex_app_server_command(codex_binary);
        let deadline = codex_app_server_deadline();
        let transport =
            StdioCodexAppServerTransport::spawn_until(&command, Some(project_root), deadline, None)
                .map_err(map_app_server_error)?;
        Ok(Self {
            transport,
            deadline,
        })
    }
}

impl CodexE2eAppServerClient for StdioCodexE2eAppServerClient {
    fn initialize(&mut self, request: Value) -> PipelineResult<Value> {
        request_codex_app_server_until(&mut self.transport, request, self.deadline, None)
            .map_err(map_app_server_error)
    }

    fn start_thread(&mut self, request: Value) -> PipelineResult<Value> {
        request_codex_app_server_until(&mut self.transport, request, self.deadline, None)
            .map_err(map_app_server_error)
    }

    fn start_turn(
        &mut self,
        request: Value,
        thread_id: &str,
    ) -> PipelineResult<CodexE2eTurnOutput> {
        let completed =
            run_codex_turn_pump(&mut self.transport, request, thread_id, self.deadline, None)
                .map_err(map_app_server_error)?;
        Ok(CodexE2eTurnOutput {
            turn_response: completed.start_response,
            assistant_text: completed.final_text,
        })
    }
}

fn map_app_server_error(error: CodexAppServerError) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::CodexTransportFailed,
        "codex",
        "Codex app-server request failed.",
        "Check the Codex app-server process and retry.",
    )
    .with_detail("error", error.to_string())]
}

struct FixtureCodexE2eAppServerClient;

impl CodexE2eAppServerClient for FixtureCodexE2eAppServerClient {
    fn initialize(&mut self, _request: Value) -> PipelineResult<Value> {
        Ok(serde_json::json!({}))
    }

    fn start_thread(&mut self, _request: Value) -> PipelineResult<Value> {
        Ok(serde_json::json!({ "thread": { "id": "thread-e2e-fixture" } }))
    }

    fn start_turn(
        &mut self,
        _request: Value,
        _thread_id: &str,
    ) -> PipelineResult<CodexE2eTurnOutput> {
        Ok(CodexE2eTurnOutput {
            turn_response: serde_json::json!({ "turn": { "id": "turn-e2e-fixture" } }),
            assistant_text: serde_json::to_string(&fixture_codex_e2e_proposal()).map_err(
                |error| {
                    vec![PipelineError::new(
                        PipelineErrorCode::PipelineInputInvalid,
                        "codex.fixture.proposal",
                        "Failed to serialize Codex e2e fixture proposal.",
                        "Keep the fixture proposal JSON serializable.",
                    )
                    .with_detail("error", error.to_string())]
                },
            )?,
        })
    }
}

fn fixture_codex_e2e_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: E2E_MEDIA_ID.to_string(),
        clips: vec![
            CodexProposalClip {
                media_id: String::new(),
                source_in: 4.0,
                source_out: 21.0,
                reason: "opening hook with clear setup".to_string(),
            },
            CodexProposalClip {
                media_id: String::new(),
                source_in: 38.0,
                source_out: 58.0,
                reason: "main payoff and product result".to_string(),
            },
        ],
        captions: vec![serde_json::json!({
            "text": "Cut from hook to payoff",
            "startSeconds": 1.0,
            "durationSeconds": 1.8,
            "visualTreatment": "large phone-readable caption with translucent backing",
            "motion": "quick scale pop with underline wipe",
            "safeZone": "inside lower third and 10% side margins",
            "avoid": "full-width opaque black caption slabs"
        })],
        overlays: Vec::new(),
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 37.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}

fn default_project_root(project_root: Option<PathBuf>) -> PipelineResult<PathBuf> {
    if let Some(project_root) = project_root {
        return Ok(project_root);
    }
    std::env::current_dir().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.projectRoot",
            "Failed to resolve default Codex e2e project root.",
            "Run Codex e2e from a readable project directory or pass --project-root.",
        )
        .with_detail("ioError", error.to_string())]
    })
}

fn arg_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> PipelineError {
    PipelineError::new(PipelineErrorCode::PipelineInputInvalid, path, message, fix)
}

fn is_codex_e2e_flag(flag: &str) -> bool {
    matches!(flag, "--project-root" | "--report" | "--codex")
}

fn arg_path(flag: &str) -> String {
    match flag {
        "--project-root" => "args.projectRoot".to_string(),
        "--report" => "args.report".to_string(),
        "--codex" => "args.codex".to_string(),
        _ => {
            let trimmed = flag.trim_start_matches('-');
            format!("args.{}", kebab_to_camel(trimmed))
        }
    }
}

fn kebab_to_camel(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for character in value.chars() {
        if character == '-' || character == '_' {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            output.extend(character.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn build_codex_e2e_fixture() -> (VideoProject, EditJobRequest) {
    let now = "2026-06-17T00:00:00Z".to_string();
    let mut project = VideoProject::new_empty(
        "codex-e2e-project".to_string(),
        "Codex E2E Fixture".to_string(),
        now.clone(),
    );
    project.media.push(MediaAsset {
        id: E2E_MEDIA_ID.to_string(),
        name: None,
        relative_path: "media/codex-e2e-source.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 90.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-codex-e2e".to_string(),
        media_id: E2E_MEDIA_ID.to_string(),
        engine: Some("codex-e2e-fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: vec![
            TranscriptSegment {
                text: "We open with the problem and the hook.".to_string(),
                start_seconds: 4.0,
                end_seconds: 21.0,
            },
            TranscriptSegment {
                text: "The result lands after the workflow reveal.".to_string(),
                start_seconds: 38.0,
                end_seconds: 58.0,
            },
        ],
        words: vec![
            transcript_word("We", 4.0, 4.3),
            transcript_word("open", 4.4, 4.8),
            transcript_word("with", 4.9, 5.2),
            transcript_word("the", 5.3, 5.5),
            transcript_word("problem", 5.6, 6.2),
            transcript_word("and", 6.3, 6.5),
            transcript_word("the", 6.6, 6.8),
            transcript_word("hook", 6.9, 7.4),
            transcript_word("The", 38.0, 38.3),
            transcript_word("workflow", 38.4, 39.0),
            transcript_word("reveal", 39.1, 39.8),
            transcript_word("shows", 39.9, 40.4),
            transcript_word("the", 40.5, 40.7),
            transcript_word("result", 40.8, 41.5),
        ],
    });
    project.timeline.tracks = vec![
        TimelineTrack::empty("track-video", "Video", TrackKind::Video),
        TimelineTrack::empty("track-overlays", "Overlays", TrackKind::Overlay),
        TimelineTrack::empty("track-captions", "Captions", TrackKind::Caption),
        TimelineTrack::empty("track-audio", "Audio", TrackKind::Audio),
    ];
    if let Some(video_track) = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-video")
    {
        video_track.items.push(TimelineItem {
            id: "codex-e2e-source-range".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 12.0,
            source: TimelineSource::Media {
                media_id: E2E_MEDIA_ID.to_string(),
            },
            label: "Codex E2E source range".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(4.0)),
                ("sourceOut".to_string(), serde_json::json!(16.0)),
                (
                    "reason".to_string(),
                    serde_json::json!("render-start acceptance needs a real selected EDL range"),
                ),
            ]),
        });
    }
    project.timeline.duration_seconds = 12.0;

    let request = EditJobRequest {
        media_id: E2E_MEDIA_ID.to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Create a concise trailer cut that proves the app-server returns a real EDL before any visual layers.".to_string(),
        target_duration_seconds: Some(37.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: now,
    };

    (project, request)
}

fn transcript_word(text: &str, start_seconds: f64, end_seconds: f64) -> TranscriptWord {
    TranscriptWord {
        text: text.to_string(),
        start_seconds,
        end_seconds,
        confidence: Some(0.99),
        speaker: Some("speaker-1".to_string()),
    }
}

fn extract_thread_id(response: &Value) -> PipelineResult<String> {
    response
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::CodexTransportFailed,
                "codex.thread.id",
                "Codex app-server thread response did not include thread.id.",
                "Use a Codex app-server version that returns thread identifiers.",
            )]
        })
}

fn proposal_from_app_server_output(
    output: &CodexE2eTurnOutput,
) -> PipelineResult<CodexEditProposal> {
    if let Some(proposal) = proposal_from_text(&output.assistant_text) {
        return Ok(proposal);
    }
    if let Some(proposal) = proposal_from_value(&output.turn_response) {
        return Ok(proposal);
    }
    Err(vec![PipelineError::new(
        PipelineErrorCode::CodexProposalInvalid,
        "proposal",
        "Codex e2e did not receive a valid Codex edit proposal.",
        "Return only a JSON CodexEditProposal or include it in the turn/start result.",
    )])
}

fn write_codex_e2e_report(path: &Path, report: &CodexE2eReport) -> PipelineResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineReportWriteFailed,
                "reportPath",
                "Failed to create Codex e2e report directory.",
                "Choose a writable --report path.",
            )
            .with_detail("error", error.to_string())]
        })?;
    }
    let json = serde_json::to_string_pretty(report).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "report",
            "Failed to serialize Codex e2e report.",
            "Keep Codex e2e report fields JSON serializable.",
        )
        .with_detail("error", error.to_string())]
    })?;
    fs::write(path, format!("{json}\n")).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "reportPath",
            "Failed to write Codex e2e report.",
            "Choose a writable --report path.",
        )
        .with_detail("error", error.to_string())]
    })
}
