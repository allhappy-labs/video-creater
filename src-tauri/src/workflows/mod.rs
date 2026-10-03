#[cfg(test)]
mod bundle_publication_tests;
mod temporal_export_destination;
pub mod temporal_reconcile;

use crate::codex::app_server::{
    bundled_codex_app_server_command, codex_app_server_deadline, proposal_from_value,
    start_codex_video_edit_turn_unpersisted_until, CodexAppServerTransport,
    StdioCodexAppServerTransport,
};
use crate::codex::context::{build_video_edit_context_with_project_dir, load_project_skill_bundle};
use crate::codex::proposal::{
    materialize_codex_edit_proposal_actions, validate_codex_edit_proposal, CodexEditProposal,
};
use crate::codex::tools::resolved_generation_model_payload;
use crate::edit::preset::EditJobRequest;
use crate::edit::render_plan::{
    RenderClip, RenderOutputProfile, RenderPlan, RenderQuality, RenderQualityProfile,
};
use crate::edit::transcript::parse_transcript_artifact;
use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::capabilities::validate_generation_asset_capabilities;
use crate::generation::elevenlabs::{
    build_elevenlabs_generation_submission, run_elevenlabs_generation_submission_with_client,
    run_elevenlabs_generation_submission_with_client_cancellable, ElevenLabsGenerationRun,
    ElevenLabsGenerationSubmission, ElevenLabsGenerationWorkerError, ElevenLabsStatusKind,
    ELEVENLABS_PROVIDER,
};
use crate::generation::fal::{
    build_fal_queue_submission, cancel_fal_queue_request_with_client,
    resume_fal_generation_request_with_client_cancellable,
    run_fal_generation_submission_with_client_after_submit,
    run_fal_generation_submission_with_client_after_submit_cancellable,
    upload_fal_local_file_to_cdn_with_client, FalGenerationRun, FalGenerationRunOptions,
    FalGenerationWorkerError, FalQueueCancelStatus, FalQueueLog, FalQueueMetrics,
    FalQueueStatusKind, FalQueueSubmission, FalResumeRequest, FAL_AURA_SR_MODEL_ID,
    FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
    FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER,
    FAL_REST_API_BASE_URL, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID, FAL_VIDEO_UPSCALER_MODEL_ID,
    FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID,
    FAL_WAN_TEXT_TO_VIDEO_MODEL_ID, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
    VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR,
};
use crate::generation::google::{
    build_google_gemini_tts_generation_submission, build_google_lyria_generation_submission,
    build_google_veo_generation_submission,
    run_google_gemini_tts_generation_submission_with_client,
    run_google_gemini_tts_generation_submission_with_client_cancellable,
    run_google_lyria_generation_submission_with_client,
    run_google_lyria_generation_submission_with_client_cancellable,
    run_google_veo_generation_submission_with_client,
    run_google_veo_generation_submission_with_client_cancellable, GoogleGeminiTtsGenerationRun,
    GoogleGeminiTtsGenerationSubmission, GoogleGenerationWorkerError, GoogleLyriaGenerationRun,
    GoogleLyriaGenerationSubmission, GoogleVeoGenerationRun, GoogleVeoGenerationSubmission,
    GoogleVeoStatusKind, GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_LYRIA_3_PRO_MODEL_ID, GOOGLE_PROVIDER,
    GOOGLE_VEO_31_FAST_MODEL_ID,
};
use crate::generation::minimax::{
    build_minimax_generation_submission, run_minimax_generation_submission_with_client,
    run_minimax_generation_submission_with_client_cancellable, MinimaxGenerationRun,
    MinimaxGenerationSubmission, MinimaxGenerationWorkerError, MinimaxStatusKind, MINIMAX_PROVIDER,
};
use crate::generation::mock::mock_generation_completion_actions;
use crate::generation::openai::{
    build_openai_image_generation_submission, openai_image_data_url_from_path,
    run_openai_image_generation_submission_with_client,
    run_openai_image_generation_submission_with_client_cancellable, OpenAiGenerationWorkerError,
    OpenAiImageGenerationRun, OpenAiImageGenerationSubmission, OpenAiImageStatusKind,
    OPENAI_GPT_IMAGE_EDIT_MODEL_ID, OPENAI_PROVIDER,
};
use crate::generation::replicate::{
    build_replicate_prediction_submission, cancel_replicate_prediction_with_client,
    resume_replicate_generation_request_with_client_cancellable,
    run_replicate_generation_submission_with_client_after_submit,
    run_replicate_generation_submission_with_client_after_submit_cancellable,
    upload_replicate_local_file_with_client, ReplicateGenerationRun, ReplicateGenerationRunOptions,
    ReplicateGenerationWorkerError, ReplicatePredictionStatusKind, ReplicatePredictionSubmission,
    REPLICATE_API_BASE_URL, REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
    REPLICATE_SEEDANCE_20_MODEL_ID, VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR,
};
use crate::generation::xai::{
    build_xai_image_generation_submission, build_xai_video_generation_submission,
    run_xai_image_generation_submission_with_client,
    run_xai_image_generation_submission_with_client_cancellable,
    run_xai_video_generation_submission_with_client,
    run_xai_video_generation_submission_with_client_reporting, XAiGenerationWorkerError,
    XAiImageGenerationRun, XAiImageGenerationSubmission, XAiImageStatusKind,
    XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use crate::generation::GenerationTarget;
use crate::project::action::{ProjectAction, ProjectActionGeneratedAssetReferences};
use crate::project::export_options::ExportRenderOptions;
use crate::project::export_profiles::{mp4_export_profile_availability_report, ExportProfile};
use crate::project::job_progress::JobProgressReporter;
use crate::project::model::{
    GeneratedAsset, GeneratedAssetStatus, JobProviderRequest, JobStatus, JobSummary, MediaAsset,
    MediaKind, ProjectExportArtifact, ProjectExportArtifactKind, TemporalWorkflowMetadata,
    TemporalWorkflowStartRequest, TimelineItem, TimelineItemKind, TimelineSource, TrackKind,
    VideoProject,
};
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::nle_export::{
    export_project_timeline_to_nle_xml, nle_xml_export_artifact, NleXmlExport, NleXmlFormat,
};
use crate::project::split::{
    apply_project_actions_to_split_project, load_split_project,
    record_app_server_conversation_turn, replace_split_project_if_revision,
    resolve_project_relative_path, save_split_project, split_timeline_from_project,
    AppServerConversationTurn, ProjectActionWriteResult,
};
use crate::provider_credentials::resolve_provider_credential;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::gstreamer_backend::{
    probe_media_with_gstreamer, GstreamerGesRenderBackend,
};
use crate::render_pipeline::probe::MediaProbe;
use crate::render_pipeline::process::{ProcessOutput, ProcessRunner, SystemProcessRunner};
use crate::render_pipeline::project_export::{
    build_project_media_render_plan, build_project_media_render_plan_with_options,
    build_project_provider_input_render_plan_for_range, build_project_webm_render_plan,
    render_export_workflow_media_to_split_project_folder, render_webm_to_split_project_folder,
};
#[cfg(not(target_os = "linux"))]
use crate::transcription::fluidaudio::FluidAudioCoreMlRuntime;
use crate::transcription::job::{
    TemporalStoreTranscriptOutput, TemporalTranscribeMediaWorkflowInput,
    TemporalTranscribeProbeOutput, TemporalTranscribeProbeStatus, TemporalTranscribeRunOutput,
};
use crate::transcription::model::{
    transcription_model_catalog_entry, ModelInstallStatus, TranscriptionModelCatalogEntry,
};
use crate::transcription::runtime::{
    InstalledModel, ModelArtifactFormat, ModelModality, NativeTranscriptionOutput,
    RuntimeCapability, TranscriptionBackend, TranscriptionRuntimeJob, TranscriptionRuntimeOutput,
    TranscriptionRuntimeRegistry,
};
#[cfg(target_os = "linux")]
use crate::transcription::sherpa_onnx::SherpaOnnxRuntime;
use crate::transcription::store::{
    default_global_transcription_model_root, ModelStoreError, TranscriptionModelStore,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const VIDEO_CREATER_TEMPORAL_TASK_QUEUE: &str = "video-creater-workflows";
pub const VIDEO_CREATER_TEMPORAL_LOCAL_SERVICE_TARGET: &str = "http://localhost:7233";
pub const VIDEO_CREATER_TEMPORAL_LOCAL_WEB_UI_URL: &str = "http://localhost:8233";
pub const VIDEO_CREATER_TEMPORAL_LOCAL_DEV_COMMAND: &str = "temporal server start-dev";
pub const VIDEO_CREATER_TEMPORAL_FEATURE_NAME: &str = "temporal-worker";
pub const VIDEO_CREATER_TEMPORAL_WORKER_RUN_COMMAND: &str =
    "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker";
pub const VIDEO_CREATER_TEMPORAL_RUST_SDK_CRATES: &[&str] = &[
    "temporalio-client",
    "temporalio-common",
    "temporalio-macros",
    "temporalio-sdk",
    "temporalio-sdk-core",
    "temporalio-workflow",
];
pub const VIDEO_CREATER_TEMPORAL_REQUIRED_TOOLS: &[&str] = &["protoc", "temporal"];
pub const VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS: &[TemporalWorkflowKind] = &[
    TemporalWorkflowKind::GenerateMedia,
    TemporalWorkflowKind::RenderDraft,
    TemporalWorkflowKind::TranscribeMedia,
    TemporalWorkflowKind::CodexEdit,
    TemporalWorkflowKind::ExportMedia,
    TemporalWorkflowKind::ExportNleXml,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalWorkflowKind {
    GenerateMedia,
    RenderDraft,
    TranscribeMedia,
    CodexEdit,
    ExportNleXml,
    ExportMedia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalWorkflowSpec {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub activity_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalWorkerManifest {
    pub task_queue: String,
    pub local_service_target: String,
    pub local_web_ui_url: String,
    pub local_dev_command: String,
    pub worker_run_command: String,
    pub feature_name: String,
    pub sdk_crates: Vec<String>,
    pub required_tools: Vec<String>,
    pub workflow_types: Vec<String>,
    pub activity_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkerRegistrationPlan {
    pub task_queue: String,
    pub workflow_registrations: Vec<String>,
    pub activity_registrations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkerToolStatus {
    pub name: String,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkerEnvironmentReport {
    pub ready: bool,
    pub feature_enabled: bool,
    pub task_queue: String,
    pub local_service_target: String,
    pub local_web_ui_url: String,
    pub local_dev_command: String,
    pub worker_run_command: String,
    pub feature_name: String,
    pub tools: Vec<TemporalWorkerToolStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowStartResult {
    pub status: String,
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub run_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowClientStartPlan {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub input: Value,
    pub search_attributes: Value,
    pub activity_types: Vec<String>,
    pub id_reuse_policy: String,
}

#[cfg(feature = "temporal-worker")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemporalWorkflowStartTarget {
    GenerateMedia,
    RenderDraft,
    TranscribeMedia,
    CodexEdit,
    ExportMedia,
    ExportNleXml,
}

#[cfg(feature = "temporal-worker")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowStartDispatch {
    pub target: TemporalWorkflowStartTarget,
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub input: Value,
}

#[cfg(feature = "temporal-worker")]
pub struct TemporalWorkflowUntypedStartRequest {
    pub workflow: temporalio_client::UntypedWorkflow,
    pub input: temporalio_common::data_converters::RawValue,
    pub options: temporalio_client::WorkflowStartOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalGenerateMediaBrief {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemporalGenerateMediaWorkflowInput {
    pub project_id: String,
    pub project_dir: String,
    pub asset_id: String,
    pub job_id: String,
    pub mock_mode: bool,
    pub name: Option<String>,
    pub placement_intent: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<Value>,
    pub references: Option<Value>,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemporalExportNleXmlWorkflowInput {
    pub project_id: String,
    pub project_dir: String,
    pub job_id: String,
    pub timeline_id: Option<String>,
    pub format: NleXmlFormat,
    pub output_path: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalExportNleXmlBuildActivityOutput {
    pub project_id: String,
    pub project_dir: String,
    pub job_id: String,
    pub format: NleXmlFormat,
    pub filename: String,
    pub output_path: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    pub mime_type: String,
    pub xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalExportNleXmlValidateActivityOutput {
    pub status: String,
    pub project_id: String,
    pub project_dir: String,
    pub job_id: String,
    pub format: NleXmlFormat,
    pub filename: String,
    pub output_path: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    pub mime_type: String,
    pub xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalExportNleXmlWriteArtifactActivityOutput {
    pub project_id: String,
    pub project_dir: String,
    pub job_id: String,
    pub format: NleXmlFormat,
    pub filename: String,
    pub output_path: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    pub mime_type: String,
    pub written_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalRenderBuildPlanActivityOutput {
    pub status: String,
    pub project_id: String,
    pub project_dir: String,
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<RenderQualityProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_profile: Option<ExportProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ExportRenderOptions>,
    pub render_plan: RenderPlan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalRenderMediaActivityInput {
    #[serde(flatten)]
    pub build: TemporalRenderBuildPlanActivityOutput,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalRenderWorkflowActivityPlan {
    pub status: String,
    pub workflow_type: String,
    pub activity_types: Vec<String>,
    pub build_render_plan_input: Value,
    pub render_media_input_from: String,
    pub validate_rendered_media_input_from: String,
    pub attach_render_report_input_from: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalGenerateMediaProviderRunOptions {
    pub max_status_polls: usize,
    pub poll_interval_millis: u64,
}

impl Default for TemporalGenerateMediaProviderRunOptions {
    fn default() -> Self {
        Self {
            max_status_polls: 60,
            poll_interval_millis: 5_000,
        }
    }
}

impl From<TemporalGenerateMediaProviderRunOptions> for FalGenerationRunOptions {
    fn from(options: TemporalGenerateMediaProviderRunOptions) -> Self {
        Self {
            max_status_polls: options.max_status_polls,
            poll_interval: std::time::Duration::from_millis(options.poll_interval_millis),
        }
    }
}

impl From<TemporalGenerateMediaProviderRunOptions> for ReplicateGenerationRunOptions {
    fn from(options: TemporalGenerateMediaProviderRunOptions) -> Self {
        Self {
            max_status_polls: options.max_status_polls,
            poll_interval: std::time::Duration::from_millis(options.poll_interval_millis),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemporalGenerateMediaProviderSubmission {
    ElevenLabs(ElevenLabsGenerationSubmission),
    Fal(FalQueueSubmission),
    GoogleGeminiTts(GoogleGeminiTtsGenerationSubmission),
    GoogleLyria(GoogleLyriaGenerationSubmission),
    Google(GoogleVeoGenerationSubmission),
    Minimax(MinimaxGenerationSubmission),
    OpenAi(OpenAiImageGenerationSubmission),
    Replicate(ReplicatePredictionSubmission),
    XAi(XAiImageGenerationSubmission),
}

impl TemporalGenerateMediaProviderSubmission {
    #[cfg(feature = "temporal-worker")]
    fn provider(&self) -> &'static str {
        match self {
            Self::ElevenLabs(_) => ELEVENLABS_PROVIDER,
            Self::Fal(_) => FAL_PROVIDER,
            Self::GoogleGeminiTts(_) | Self::GoogleLyria(_) | Self::Google(_) => GOOGLE_PROVIDER,
            Self::Minimax(_) => MINIMAX_PROVIDER,
            Self::OpenAi(_) => OPENAI_PROVIDER,
            Self::Replicate(_) => REPLICATE_PROVIDER,
            Self::XAi(_) => XAI_PROVIDER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemporalGenerateMediaProviderStatus {
    ElevenLabs(ElevenLabsStatusKind),
    Fal(FalQueueStatusKind),
    GoogleGeminiTts(GoogleVeoStatusKind),
    GoogleLyria(GoogleVeoStatusKind),
    Google(GoogleVeoStatusKind),
    Minimax(MinimaxStatusKind),
    OpenAi(OpenAiImageStatusKind),
    Replicate(ReplicatePredictionStatusKind),
    XAi(XAiImageStatusKind),
}

impl PartialEq<FalQueueStatusKind> for TemporalGenerateMediaProviderStatus {
    fn eq(&self, other: &FalQueueStatusKind) -> bool {
        matches!(self, Self::Fal(status) if status == other)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalGenerateMediaRunProviderActivityInput {
    pub start_request: TemporalWorkflowStartRequest,
    pub submission: TemporalGenerateMediaProviderSubmission,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default)]
    pub options: TemporalGenerateMediaProviderRunOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalGenerateMediaRunProviderActivityOutput {
    pub request_id: String,
    pub project_id: String,
    pub asset_id: String,
    pub job_status: JobStatus,
    pub provider_status: TemporalGenerateMediaProviderStatus,
    pub queue_position: Option<u64>,
    #[serde(default)]
    pub logs: Vec<FalQueueLog>,
    pub metrics: Option<FalQueueMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub output_path: String,
    pub written_files: Vec<String>,
    pub removed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalGenerateMediaCancelProviderActivityOutput {
    pub request_id: String,
    pub cancel_url: String,
    pub status: FalQueueCancelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemporalStartResultError {
    #[error("workflow job is missing workflow metadata: {0}")]
    MissingWorkflowMetadata(String),
    #[error("workflow job is missing start request: {0}")]
    MissingStartRequest(String),
    #[error("workflow start request does not match workflow metadata: {0}")]
    MismatchedStartRequest(String),
    #[error("Temporal run id must not be blank")]
    BlankRunId,
    #[error("workflow start timestamp must not be blank")]
    BlankUpdatedAt,
    #[error("unsupported Temporal workflow id reuse policy: {0}")]
    UnsupportedIdReusePolicy(String),
    #[error("unsupported Temporal workflow type: {0}")]
    UnsupportedWorkflowType(String),
    #[error("Temporal workflow start failed: {0}")]
    ClientStartFailed(String),
    #[error("Temporal workflow start did not return a run id")]
    MissingTemporalRunId,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemporalWorkflowInputError {
    #[error("generate media workflow input is missing field: {0}")]
    MissingField(String),
    #[error("generate media workflow input field is blank: {0}")]
    BlankField(String),
    #[error("generate media workflow input field has invalid type: {0}")]
    InvalidFieldType(String),
    #[error("generate media workflow input contains forbidden credential field: {0}")]
    ForbiddenCredentialField(String),
    #[error("generate media workflow input does not match worker boundary: {0}")]
    MismatchedInputField(String),
    #[error("generate media provider input upload is unsupported for provider: {0}")]
    UnsupportedProviderInputUpload(String),
    #[error("generate media workflow input requires live mode for fal queue submission")]
    LiveModeRequired,
    #[error("generate media workflow input requires mockMode for mock completion")]
    MockModeRequired,
    #[error("generate media workflow input references missing generated asset: {0}")]
    MissingGeneratedAsset(String),
    #[error("generate media workflow could not load split project: {0}")]
    LoadSplitProject(String),
    #[error("generate media workflow could not save split project: {0}")]
    SaveSplitProject(String),
    #[error("generate media workflow could not build failure actions: {0}")]
    FailureActions(String),
    #[error("generate media workflow could not apply project actions: {0}")]
    ApplyProjectActions(String),
    #[error("generate media activity input could not be decoded: {0}")]
    DecodeActivityInput(String),
    #[error("generate media request violates model capabilities: {0}")]
    GenerationCapability(String),
    #[error("generate media fal queue request failed: {0}")]
    FalGenerationRequest(String),
    #[error("generate media fal provider run failed: {0}")]
    FalGenerationRun(String),
    #[error("generate media generation was cancelled")]
    GenerationCancelled,
    #[error("generate media mock completion failed: {0}")]
    MockGeneration(String),
    #[error("NLE XML export failed: {0}")]
    NleXmlExport(String),
    #[error("render workflow failed: {0}")]
    Render(String),
    #[error("Codex edit proposal validation failed: {0}")]
    CodexProposal(String),
    #[error("Codex edit context collection failed: {0}")]
    CodexContext(String),
    #[error("Codex app-server request failed: {0}")]
    CodexAppServer(String),
    #[error("transcribe media workflow failed: {0}")]
    Transcription(String),
}

impl TemporalWorkflowKind {
    pub fn job_kind(self) -> &'static str {
        match self {
            Self::GenerateMedia => "generate_media",
            Self::RenderDraft => "render_draft",
            Self::TranscribeMedia => "transcribe_media",
            Self::CodexEdit => "codex_edit",
            Self::ExportNleXml => "export_nle_xml",
            Self::ExportMedia => "export_media",
        }
    }

    pub fn id_segment(self) -> &'static str {
        match self {
            Self::GenerateMedia => "generate-media",
            Self::RenderDraft => "render-draft",
            Self::TranscribeMedia => "transcribe-media",
            Self::CodexEdit => "codex-edit",
            Self::ExportNleXml => "export-nle-xml",
            Self::ExportMedia => "export-media",
        }
    }

    pub fn workflow_type(self) -> &'static str {
        match self {
            Self::GenerateMedia => "VideoCreaterGenerateMediaWorkflow",
            Self::RenderDraft => "VideoCreaterRenderDraftWorkflow",
            Self::TranscribeMedia => "VideoCreaterTranscribeMediaWorkflow",
            Self::CodexEdit => "VideoCreaterCodexEditWorkflow",
            Self::ExportNleXml => "VideoCreaterExportNleXmlWorkflow",
            Self::ExportMedia => "VideoCreaterExportMediaWorkflow",
        }
    }

    pub fn activity_types(self) -> &'static [&'static str] {
        match self {
            Self::GenerateMedia => &[
                "PrepareProviderInputs",
                "BuildFalGenerationRequest",
                "RunMediaProviderGeneration",
                "AttachGeneratedAssetFailure",
            ],
            Self::RenderDraft => &[
                "BuildRenderPlan",
                "RenderMedia",
                "ValidateRenderedMedia",
                "AttachRenderReport",
            ],
            Self::TranscribeMedia => &["ProbeMedia", "RunTranscription", "StoreTranscript"],
            Self::CodexEdit => &[
                "CollectProjectContext",
                "RequestCodexProposal",
                "ValidateProjectActions",
                "PersistAcceptedProposal",
                "AttachCodexEditFailure",
            ],
            Self::ExportNleXml => &[
                "BuildNleXml",
                "ValidateNleXml",
                "WriteExportArtifact",
                "AttachExportReport",
            ],
            Self::ExportMedia => &[
                "BuildRenderPlan",
                "ValidateExportProfile",
                "RenderMedia",
                "ValidateRenderedMedia",
                "AttachRenderReport",
                "WriteExportArtifact",
                "AttachExportReport",
            ],
        }
    }
}

pub fn temporal_workflow_spec(
    kind: TemporalWorkflowKind,
    project_id: &str,
    job_id: &str,
) -> TemporalWorkflowSpec {
    TemporalWorkflowSpec {
        workflow_id: format!(
            "video-creater/{}/{}/{}",
            safe_workflow_segment(project_id),
            kind.id_segment(),
            safe_workflow_segment(job_id)
        ),
        workflow_type: kind.workflow_type().to_string(),
        task_queue: VIDEO_CREATER_TEMPORAL_TASK_QUEUE.to_string(),
        activity_types: kind
            .activity_types()
            .iter()
            .map(|activity| (*activity).to_string())
            .collect(),
    }
}

pub fn temporal_job_summary(
    kind: TemporalWorkflowKind,
    project_id: &str,
    job_id: &str,
    status: JobStatus,
    updated_at: &str,
) -> JobSummary {
    let spec = temporal_workflow_spec(kind, project_id, job_id);
    JobSummary {
        id: job_id.to_string(),
        kind: kind.job_kind().to_string(),
        status,
        updated_at: updated_at.to_string(),
        workflow: Some(TemporalWorkflowMetadata {
            workflow_id: spec.workflow_id,
            workflow_type: spec.workflow_type,
            task_queue: spec.task_queue,
            run_id: None,
            activity_types: spec.activity_types,
        }),
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

pub fn temporal_workflow_start_request(
    kind: TemporalWorkflowKind,
    project_id: &str,
    job_id: &str,
    input: Value,
) -> TemporalWorkflowStartRequest {
    let spec = temporal_workflow_spec(kind, project_id, job_id);
    TemporalWorkflowStartRequest {
        workflow_id: spec.workflow_id,
        workflow_type: spec.workflow_type,
        task_queue: spec.task_queue,
        input,
        search_attributes: json!({
            "projectId": project_id,
            "jobId": job_id,
            "workflowKind": kind.job_kind(),
        }),
        activity_types: spec.activity_types,
        id_reuse_policy: "rejectDuplicate".to_string(),
    }
}

pub fn temporal_generate_media_start_request(
    project_id: &str,
    project_dir: &str,
    asset_id: &str,
    job_id: &str,
    mock_mode: bool,
    brief: Option<TemporalGenerateMediaBrief>,
) -> TemporalWorkflowStartRequest {
    let mut input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "assetId": asset_id,
        "jobId": job_id,
        "mockMode": mock_mode,
    });
    if let Some(brief) = brief {
        let input_object = input
            .as_object_mut()
            .expect("generate media workflow input is an object");
        if let Some(name) = brief.name {
            input_object.insert("name".to_string(), json!(name));
        }
        if let Some(target_folder_id) = brief.target_folder_id {
            input_object.insert("targetFolderId".to_string(), json!(target_folder_id));
        }
        if let Some(placement_intent) = brief.placement_intent {
            input_object.insert("placementIntent".to_string(), json!(placement_intent));
        }
        if let Some(prompt) = brief.prompt {
            input_object.insert("prompt".to_string(), json!(prompt));
        }
        if let Some(model) = brief.model {
            input_object.insert("model".to_string(), model);
        }
        if let Some(references) = brief.references {
            input_object.insert("references".to_string(), references);
        }
        if let Some(settings) = brief.settings {
            input_object.insert("settings".to_string(), settings);
        }
    }

    temporal_workflow_start_request(
        TemporalWorkflowKind::GenerateMedia,
        project_id,
        job_id,
        input,
    )
}

pub fn temporal_transcribe_media_start_request(
    project_id: &str,
    project_dir: &str,
    media_id: &str,
    job_id: &str,
    language_mode: &str,
) -> TemporalWorkflowStartRequest {
    temporal_workflow_start_request(
        TemporalWorkflowKind::TranscribeMedia,
        project_id,
        job_id,
        json!(TemporalTranscribeMediaWorkflowInput {
            project_id: project_id.to_string(),
            project_dir: project_dir.to_string(),
            media_id: media_id.to_string(),
            job_id: job_id.to_string(),
            language_mode: language_mode.to_string(),
        }),
    )
}

pub fn temporal_transcribe_probe_media_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let store = TranscriptionModelStore::new(default_global_transcription_model_root());
    let registry = default_transcription_runtime_registry();
    temporal_transcribe_probe_media_activity_value_with_store_and_registry(input, &store, &registry)
}

pub fn temporal_transcribe_probe_media_activity_value_with_store(
    input: Value,
    store: &TranscriptionModelStore,
) -> Result<Value, TemporalWorkflowInputError> {
    let registry = default_transcription_runtime_registry();
    temporal_transcribe_probe_media_activity_value_with_store_and_registry(input, store, &registry)
}

pub fn temporal_transcribe_probe_media_activity_value_with_store_and_registry(
    input: Value,
    store: &TranscriptionModelStore,
    registry: &TranscriptionRuntimeRegistry,
) -> Result<Value, TemporalWorkflowInputError> {
    let input: TemporalTranscribeMediaWorkflowInput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_probe_input(&input)?;

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != input.project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }

    let media = project
        .media
        .iter()
        .find(|media| media.id == input.media_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("mediaId".to_string()))?;
    let relative_path = media.relative_path.trim();
    if relative_path.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "media.relativePath".to_string(),
        ));
    }
    let source_path = resolve_project_relative_path(project_dir, relative_path).map_err(|_| {
        TemporalWorkflowInputError::MismatchedInputField("media.relativePath".to_string())
    })?;
    if !is_safe_transcription_artifact_segment(&input.job_id) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }
    if !fs::metadata(&source_path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "media.relativePath".to_string(),
        ));
    }
    let artifact_path = resolve_project_relative_path(
        project_dir,
        &format!("transcripts/{}-transcript.json", input.job_id),
    )
    .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;

    let model_status = store
        .active_status()
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    if model_status.install_status != ModelInstallStatus::Ready {
        return Err(TemporalWorkflowInputError::Transcription(format!(
            "active transcription model {} is not ready",
            model_status.model_id
        )));
    }
    let entry = transcription_model_catalog_entry(&model_status.model_id).ok_or_else(|| {
        TemporalWorkflowInputError::Transcription(format!(
            "active transcription model {} is not in the catalog",
            model_status.model_id
        ))
    })?;
    let selected_model = select_ready_installed_transcription_model(&entry, store, registry)?;

    serde_json::to_value(TemporalTranscribeProbeOutput {
        status: TemporalTranscribeProbeStatus::Ready,
        project_id: input.project_id,
        project_dir: input.project_dir,
        media_id: input.media_id,
        job_id: input.job_id,
        language_mode: input.language_mode,
        source_path: source_path.display().to_string(),
        artifact_path: artifact_path.display().to_string(),
        media_kind: media.kind.clone(),
        media_relative_path: relative_path.to_string(),
        media_duration_seconds: media.duration_seconds,
        media_width: media.width,
        media_height: media.height,
        media_fps: media.fps,
        model_id: entry.id.to_string(),
        model_path: selected_model.model_dir.display().to_string(),
        runtime_id: selected_model.runtime_family,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_transcribe_run_activity_value_with_backend<B: TranscriptionBackend>(
    input: Value,
    backend: &B,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_run_input(&probe)?;

    let job = TranscriptionRuntimeJob {
        media_id: probe.media_id.clone(),
        source_path: probe.source_path.clone(),
        model_path: probe.model_path.clone(),
        language_mode: probe.language_mode.clone(),
        output_artifact_path: Some(transcribe_output_artifact_path(&probe, artifact_dir)),
    };
    let output = backend
        .transcribe(&job, &probe.model_id)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;

    temporal_transcribe_run_activity_output(probe, job, output, artifact_dir)
}

pub fn temporal_transcribe_run_activity_value_with_runtime_registry(
    input: Value,
    registry: &TranscriptionRuntimeRegistry,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_run_input(&probe)?;

    let installed_model = installed_model_for_transcribe_probe(&probe)?;
    let runtime = registry
        .select(&probe.runtime_id, &installed_model)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let job = TranscriptionRuntimeJob {
        media_id: probe.media_id.clone(),
        source_path: probe.source_path.clone(),
        model_path: probe.model_path.clone(),
        language_mode: probe.language_mode.clone(),
        output_artifact_path: Some(transcribe_output_artifact_path(&probe, artifact_dir)),
    };
    let output = runtime
        .transcribe_installed(&job, &installed_model)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;

    temporal_transcribe_run_activity_output(probe, job, output, artifact_dir)
}

pub fn temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
    input: Value,
    registry: &TranscriptionRuntimeRegistry,
    store: &TranscriptionModelStore,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_run_input(&probe)?;

    let installed_model =
        validated_installed_model_for_transcribe_run(&probe, store, artifact_dir)?;
    let runtime = registry
        .select(&probe.runtime_id, &installed_model)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let job = TranscriptionRuntimeJob {
        media_id: probe.media_id.clone(),
        source_path: canonical_path_string(&probe.source_path, "sourcePath")?,
        model_path: canonical_path_string(&installed_model.model_dir, "modelPath")?,
        language_mode: probe.language_mode.clone(),
        output_artifact_path: Some(transcribe_output_artifact_path(&probe, artifact_dir)),
    };
    let output = runtime
        .transcribe_installed(&job, &installed_model)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;

    temporal_transcribe_run_activity_output(probe, job, output, artifact_dir)
}

#[cfg(not(target_os = "linux"))]
pub fn default_transcription_runtime_registry() -> TranscriptionRuntimeRegistry {
    TranscriptionRuntimeRegistry::new(vec![Box::new(FluidAudioCoreMlRuntime::new(
        FluidAudioCoreMlRuntime::default_helper_path(),
    ))])
}

#[cfg(target_os = "linux")]
pub fn default_transcription_runtime_registry() -> TranscriptionRuntimeRegistry {
    TranscriptionRuntimeRegistry::new(vec![Box::new(SherpaOnnxRuntime::new(
        SherpaOnnxRuntime::default_helper_path(),
    ))])
}

fn select_ready_installed_transcription_model(
    entry: &TranscriptionModelCatalogEntry,
    store: &TranscriptionModelStore,
    registry: &TranscriptionRuntimeRegistry,
) -> Result<InstalledModel, TemporalWorkflowInputError> {
    let mut last_not_ready = None;
    for runtime in registry.iter() {
        let Some(installed_model) =
            installed_model_for_catalog_runtime(entry, store, runtime.runtime_id())?
        else {
            continue;
        };
        if runtime.modality() != ModelModality::Transcription
            || !runtime.supports_installed_model(&installed_model)
        {
            continue;
        }

        match runtime.probe_installed_model(&installed_model) {
            RuntimeCapability::Ready => return Ok(installed_model),
            capability => {
                let diagnostic = runtime.readiness_diagnostic(&installed_model, &capability);
                let mut message = format!(
                    "transcription runtime {} is not ready for model {}: {capability:?}",
                    runtime.runtime_id(),
                    entry.id
                );
                if let Some(diagnostic) = diagnostic {
                    if !diagnostic.trim().is_empty() {
                        message.push_str("; ");
                        message.push_str(&diagnostic);
                    }
                }
                last_not_ready = Some(message);
            }
        }
    }

    Err(TemporalWorkflowInputError::Transcription(
        last_not_ready.unwrap_or_else(|| {
            format!(
                "no transcription runtime supports active model {}",
                entry.id
            )
        }),
    ))
}

fn installed_model_for_catalog_runtime(
    entry: &TranscriptionModelCatalogEntry,
    store: &TranscriptionModelStore,
    runtime_id: &str,
) -> Result<Option<InstalledModel>, TemporalWorkflowInputError> {
    let model_dir = match store.runtime_model_dir(entry.id, runtime_id) {
        Ok(model_dir) => model_dir,
        Err(ModelStoreError::UnsupportedRuntime { .. }) => return Ok(None),
        Err(error) => return Err(TemporalWorkflowInputError::Transcription(error.to_string())),
    };

    Ok(Some(InstalledModel {
        model_id: entry.id.to_string(),
        model_dir,
        artifact_format: ModelArtifactFormat::from(entry.artifact_format),
        modality: ModelModality::Transcription,
        runtime_family: runtime_id.to_string(),
    }))
}

fn installed_model_for_transcribe_probe(
    probe: &TemporalTranscribeProbeOutput,
) -> Result<InstalledModel, TemporalWorkflowInputError> {
    let entry = transcription_model_catalog_entry(&probe.model_id).ok_or_else(|| {
        TemporalWorkflowInputError::Transcription(format!(
            "transcription model {} is not in the catalog",
            probe.model_id
        ))
    })?;

    Ok(InstalledModel {
        model_id: entry.id.to_string(),
        model_dir: PathBuf::from(&probe.model_path),
        artifact_format: ModelArtifactFormat::from(entry.artifact_format),
        modality: ModelModality::Transcription,
        runtime_family: probe.runtime_id.clone(),
    })
}

fn validated_installed_model_for_transcribe_run(
    probe: &TemporalTranscribeProbeOutput,
    store: &TranscriptionModelStore,
    artifact_dir: &Path,
) -> Result<InstalledModel, TemporalWorkflowInputError> {
    let project_dir = Path::new(&probe.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != probe.project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }

    let media = project
        .media
        .iter()
        .find(|media| media.id == probe.media_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("mediaId".to_string()))?;
    if media.relative_path.trim() != probe.media_relative_path {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "mediaRelativePath".to_string(),
        ));
    }
    let expected_source_path =
        resolve_project_relative_path(project_dir, media.relative_path.trim()).map_err(|_| {
            TemporalWorkflowInputError::MismatchedInputField("media.relativePath".to_string())
        })?;
    let expected_source_path = fs::canonicalize(expected_source_path)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("sourcePath".to_string()))?;
    let probe_source_path = fs::canonicalize(&probe.source_path)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("sourcePath".to_string()))?;
    if expected_source_path != probe_source_path
        || !fs::metadata(&probe_source_path)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "sourcePath".to_string(),
        ));
    }

    let entry = transcription_model_catalog_entry(&probe.model_id).ok_or_else(|| {
        TemporalWorkflowInputError::Transcription(format!(
            "transcription model {} is not in the catalog",
            probe.model_id
        ))
    })?;
    let installed_model = installed_model_for_catalog_runtime(&entry, store, &probe.runtime_id)?
        .ok_or_else(|| {
            TemporalWorkflowInputError::Transcription(format!(
                "unsupported transcription runtime {} for model {}",
                probe.runtime_id, entry.id
            ))
        })?;
    let expected_model_path = fs::canonicalize(&installed_model.model_dir)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("modelPath".to_string()))?;
    let probe_model_path = fs::canonicalize(&probe.model_path)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("modelPath".to_string()))?;
    if expected_model_path != probe_model_path {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "modelPath".to_string(),
        ));
    }

    let expected_artifact_dir = transcribe_run_artifact_dir(project_dir, &probe.job_id)?;
    if artifact_dir != expected_artifact_dir {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "artifactPath".to_string(),
        ));
    }

    Ok(installed_model)
}

fn temporal_transcribe_run_activity_output(
    probe: TemporalTranscribeProbeOutput,
    job: TranscriptionRuntimeJob,
    output: TranscriptionRuntimeOutput,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    if output.model_id != probe.model_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "modelId".to_string(),
        ));
    }
    if output.runtime_id.trim().is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "runtimeId".to_string(),
        ));
    }
    if output.runtime_id != probe.runtime_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "runtimeId".to_string(),
        ));
    }
    let token_count = output.tokens.len();
    let artifact_path = artifact_dir.join(format!("{}-transcript.json", probe.job_id));
    let artifact = json!({
        "schemaVersion": 1,
        "mediaId": probe.media_id,
        "engine": output.runtime_id,
        "modelId": output.model_id,
        "runtimeId": output.runtime_id,
        "languageMode": probe.language_mode,
        "tokens": output.tokens,
    });
    let transcript = parse_transcript_artifact(&artifact)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let native_output = NativeTranscriptionOutput {
        engine: output.runtime_id.clone(),
        raw_artifact_path: artifact_path.clone(),
        words: transcript.words,
        segments: transcript.segments,
    };

    let _project_lease = acquire_split_project_mutation_lease(Path::new(&probe.project_dir))
        .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
    fs::create_dir_all(artifact_dir)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let artifact_bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    fs::write(&artifact_path, artifact_bytes)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;

    serde_json::to_value(TemporalTranscribeRunOutput {
        project_id: probe.project_id,
        project_dir: probe.project_dir,
        media_id: job.media_id,
        job_id: probe.job_id,
        language_mode: job.language_mode,
        model_id: artifact["modelId"].as_str().unwrap_or_default().to_string(),
        runtime_id: artifact["runtimeId"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        artifact_path: artifact_path.display().to_string(),
        token_count,
        native_output,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

fn transcribe_output_artifact_path(
    probe: &TemporalTranscribeProbeOutput,
    artifact_dir: &Path,
) -> String {
    artifact_dir
        .join(format!("{}-transcript.json", probe.job_id))
        .display()
        .to_string()
}

pub fn temporal_transcribe_run_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput = serde_json::from_value(input.clone())
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_run_input(&probe)?;
    let artifact_dir = transcribe_run_artifact_dir(Path::new(&probe.project_dir), &probe.job_id)?;

    let registry = default_transcription_runtime_registry();
    let store = TranscriptionModelStore::new(default_global_transcription_model_root());
    temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
        input,
        &registry,
        &store,
        &artifact_dir,
    )
}

pub fn temporal_transcribe_store_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let updated_at = optional_workflow_string(&input, "updatedAt")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let run: TemporalTranscribeRunOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_store_input(&run)?;

    let project_dir = Path::new(&run.project_dir);
    let _project_lease = acquire_split_project_mutation_lease(project_dir)
        .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
    let artifact_path = transcription_artifact_path(project_dir, &run.artifact_path)?;
    let artifact_text = fs::read_to_string(&artifact_path)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let artifact_json: Value = serde_json::from_str(&artifact_text)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let mut transcript = parse_transcript_artifact(&artifact_json)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    validate_transcribe_store_artifact_metadata(&run, &artifact_json)?;
    if transcript.media_id != run.media_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "mediaId".to_string(),
        ));
    }

    let mut project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != run.project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }
    if !project
        .media
        .iter()
        .any(|media| media.id == transcript.media_id)
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "mediaId".to_string(),
        ));
    }
    transcript.raw_artifact_path =
        Some(project_relative_artifact_path(project_dir, &artifact_path)?);

    project
        .transcripts
        .retain(|existing| existing.media_id != transcript.media_id);
    let transcript_id = transcript.id.clone();
    let segment_count = transcript.segments.len();
    let word_count = transcript.words.len();
    project.transcripts.push(transcript);
    if let Some(updated_at) = updated_at {
        complete_transcription_job(&mut project, &run.job_id, updated_at, run_id)?;
    }
    save_split_project(project_dir, &project)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;

    serde_json::to_value(TemporalStoreTranscriptOutput {
        project_id: run.project_id,
        project_dir: run.project_dir,
        media_id: run.media_id,
        job_id: run.job_id,
        transcript_id,
        segment_count,
        word_count,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

fn complete_transcription_job(
    project: &mut VideoProject,
    job_id: &str,
    updated_at: String,
    run_id: Option<String>,
) -> Result<(), TemporalWorkflowInputError> {
    let job = project
        .jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
    if run_id.is_some() && job.workflow.is_none() {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "workflow".to_string(),
        ));
    }

    job.status = JobStatus::Completed;
    job.updated_at = updated_at;
    if let Some(run_id) = run_id {
        if let Some(workflow) = &mut job.workflow {
            workflow.run_id = Some(run_id);
        }
    }

    Ok(())
}

pub fn temporal_transcribe_workflow_activity_plan_value(
    input: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let decoded: TemporalTranscribeMediaWorkflowInput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_probe_input(&decoded)?;
    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "updatedAt".to_string(),
        ));
    }
    let run_id = run_id
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .map(str::to_string);

    Ok(json!({
        "status": "planned",
        "workflowType": "VideoCreaterTranscribeMediaWorkflow",
        "activityTypes": TemporalWorkflowKind::TranscribeMedia.activity_types(),
        "probeMediaInput": {
            "projectId": decoded.project_id,
            "projectDir": decoded.project_dir,
            "mediaId": decoded.media_id,
            "jobId": decoded.job_id,
            "languageMode": decoded.language_mode,
        },
        "runTranscriptionInputFrom": "ProbeMedia",
        "storeTranscriptInputFrom": "RunTranscription",
        "updatedAt": updated_at,
        "runId": run_id,
    }))
}

fn validate_transcribe_probe_input(
    input: &TemporalTranscribeMediaWorkflowInput,
) -> Result<(), TemporalWorkflowInputError> {
    for (field, value) in [
        ("projectId", input.project_id.as_str()),
        ("projectDir", input.project_dir.as_str()),
        ("mediaId", input.media_id.as_str()),
        ("jobId", input.job_id.as_str()),
        ("languageMode", input.language_mode.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(TemporalWorkflowInputError::BlankField(field.to_string()));
        }
    }

    Ok(())
}

fn validate_transcribe_run_input(
    input: &TemporalTranscribeProbeOutput,
) -> Result<(), TemporalWorkflowInputError> {
    for (field, value) in [
        ("projectId", input.project_id.as_str()),
        ("projectDir", input.project_dir.as_str()),
        ("mediaId", input.media_id.as_str()),
        ("jobId", input.job_id.as_str()),
        ("languageMode", input.language_mode.as_str()),
        ("sourcePath", input.source_path.as_str()),
        ("modelId", input.model_id.as_str()),
        ("modelPath", input.model_path.as_str()),
        ("runtimeId", input.runtime_id.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(TemporalWorkflowInputError::BlankField(field.to_string()));
        }
    }
    if !is_safe_transcription_artifact_segment(&input.job_id) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    Ok(())
}

fn transcribe_run_artifact_dir(
    project_dir: &Path,
    job_id: &str,
) -> Result<PathBuf, TemporalWorkflowInputError> {
    if !is_safe_transcription_artifact_segment(job_id) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    resolve_project_relative_path(project_dir, &format!("workflow-artifacts/{job_id}"))
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))
}

fn canonical_path_string<P: AsRef<Path>>(
    path: P,
    field: &str,
) -> Result<String, TemporalWorkflowInputError> {
    fs::canonicalize(path.as_ref())
        .map(|path| path.display().to_string())
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField(field.to_string()))
}

fn validate_transcribe_store_input(
    input: &TemporalTranscribeRunOutput,
) -> Result<(), TemporalWorkflowInputError> {
    for (field, value) in [
        ("projectId", input.project_id.as_str()),
        ("projectDir", input.project_dir.as_str()),
        ("mediaId", input.media_id.as_str()),
        ("jobId", input.job_id.as_str()),
        ("languageMode", input.language_mode.as_str()),
        ("modelId", input.model_id.as_str()),
        ("runtimeId", input.runtime_id.as_str()),
        ("artifactPath", input.artifact_path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(TemporalWorkflowInputError::BlankField(field.to_string()));
        }
    }

    Ok(())
}

fn validate_transcribe_store_artifact_metadata(
    run: &TemporalTranscribeRunOutput,
    artifact: &Value,
) -> Result<(), TemporalWorkflowInputError> {
    for (field, expected) in [
        ("mediaId", run.media_id.as_str()),
        ("modelId", run.model_id.as_str()),
        ("runtimeId", run.runtime_id.as_str()),
        ("engine", run.runtime_id.as_str()),
        ("languageMode", run.language_mode.as_str()),
    ] {
        let actual = required_workflow_string(artifact, field)?;
        if actual != expected {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                field.to_string(),
            ));
        }
    }
    let token_count = artifact
        .get("tokens")
        .and_then(Value::as_array)
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType("tokens".to_string()))?
        .len();
    if token_count != run.token_count {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "tokenCount".to_string(),
        ));
    }

    Ok(())
}

fn transcription_artifact_path(
    project_dir: &Path,
    artifact_path: &str,
) -> Result<PathBuf, TemporalWorkflowInputError> {
    let path = Path::new(artifact_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        resolve_project_relative_path(project_dir, artifact_path).map_err(|_| {
            TemporalWorkflowInputError::MismatchedInputField("artifactPath".to_string())
        })?
    };
    let project_dir = fs::canonicalize(project_dir)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let resolved = fs::canonicalize(resolved)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    if !resolved.starts_with(&project_dir) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "artifactPath".to_string(),
        ));
    }

    Ok(resolved)
}

fn project_relative_artifact_path(
    project_dir: &Path,
    artifact_path: &Path,
) -> Result<String, TemporalWorkflowInputError> {
    let project_dir = fs::canonicalize(project_dir)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    artifact_path
        .strip_prefix(&project_dir)
        .map(|path| path.display().to_string())
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("artifactPath".to_string()))
}

fn is_safe_transcription_artifact_segment(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

pub fn temporal_codex_edit_start_request(
    project_id: &str,
    project_root: &str,
    project_dir: &str,
    job_id: &str,
    request: EditJobRequest,
) -> TemporalWorkflowStartRequest {
    temporal_workflow_start_request(
        TemporalWorkflowKind::CodexEdit,
        project_id,
        job_id,
        json!({
            "projectId": project_id,
            "projectRoot": project_root,
            "projectDir": project_dir,
            "jobId": job_id,
            "request": request,
        }),
    )
}

pub fn temporal_codex_edit_proposal_activity_input_value(
    runtime_input: &Value,
    proposal_output: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(runtime_input, "projectId")?;
    let project_root = required_workflow_string(runtime_input, "projectRoot")?;
    let project_dir = required_workflow_string(runtime_input, "projectDir")?;
    let job_id = required_workflow_string(runtime_input, "jobId")?;
    let request_value = runtime_input
        .get("request")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("request".to_string()))?
        .clone();
    let _: EditJobRequest = serde_json::from_value(request_value.clone())
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let proposal = proposal_from_value(&proposal_output)
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("proposal".to_string()))?;
    let mut input = json!({
        "projectId": project_id,
        "projectRoot": project_root,
        "projectDir": project_dir,
        "jobId": job_id,
        "request": request_value,
        "proposal": proposal,
        "updatedAt": updated_at,
    });
    if let Some(run_id) = run_id {
        input
            .as_object_mut()
            .expect("codex edit proposal input is an object")
            .insert("runId".to_string(), json!(run_id));
    }

    Ok(input)
}

pub fn temporal_codex_edit_failure_activity_input_value(
    runtime_input: &Value,
    updated_at: &str,
    run_id: Option<&str>,
    error: &str,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(runtime_input, "projectId")?;
    let project_root = required_workflow_string(runtime_input, "projectRoot")?;
    let project_dir = required_workflow_string(runtime_input, "projectDir")?;
    let job_id = required_workflow_string(runtime_input, "jobId")?;
    let request_value = runtime_input
        .get("request")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("request".to_string()))?
        .clone();
    let request: EditJobRequest = serde_json::from_value(request_value)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let start_request = temporal_codex_edit_start_request(
        &project_id,
        &project_root,
        &project_dir,
        &job_id,
        request.clone(),
    );
    validate_temporal_start_request(&start_request, TemporalWorkflowKind::CodexEdit)?;
    let mut input = json!({
        "startRequest": start_request,
        "updatedAt": updated_at,
        "error": error,
    });
    if let Some(run_id) = run_id {
        input
            .as_object_mut()
            .expect("codex edit failure input is an object")
            .insert("runId".to_string(), json!(run_id));
    }

    Ok(input)
}

pub fn temporal_codex_edit_collect_project_context_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_root = required_workflow_string(&input, "projectRoot")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let request_value = input
        .get("request")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("request".to_string()))?
        .clone();
    let request: EditJobRequest = serde_json::from_value(request_value.clone())
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let project_dir_path = Path::new(&project_dir);
    let project = load_split_project(project_dir_path)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }
    let context = build_video_edit_context_with_project_dir(&project, &request, project_dir_path)
        .map_err(|error| TemporalWorkflowInputError::CodexContext(error.to_string()))?;

    Ok(json!({
        "status": "collected",
        "projectId": project_id,
        "projectRoot": project_root,
        "projectDir": project_dir,
        "jobId": job_id,
        "request": request_value,
        "context": {
            "projectId": context.project_id,
            "projectName": context.project_name,
            "mediaLibrarySummary": context.media_library_summary,
            "timelineSummary": context.timeline_summary,
            "generatedAssetsSummary": context.generated_assets_summary,
            "templateOverridesSummary": context.template_overrides_summary,
            "renderReportsSummary": context.render_reports_summary,
            "workflowJobsSummary": context.workflow_jobs_summary,
            "exportArtifactsSummary": context.export_artifacts_summary,
            "exportCapabilitiesSummary": context.export_capabilities_summary,
            "projectFilesSummary": context.project_files_summary,
            "mediaRelativePath": context.media_relative_path,
            "mediaDurationSeconds": context.media_duration_seconds,
            "mediaWidth": context.media_width,
            "mediaHeight": context.media_height,
            "mediaFps": context.media_fps,
            "transcriptExcerpt": context.transcript_excerpt,
        },
    }))
}

pub fn temporal_codex_edit_request_proposal_activity_with_transport<T: CodexAppServerTransport>(
    transport: &mut T,
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    temporal_codex_edit_request_proposal_activity_with_transport_until(
        transport,
        input,
        codex_app_server_deadline(),
    )
}

fn temporal_codex_edit_request_proposal_activity_with_transport_until<
    T: CodexAppServerTransport,
>(
    transport: &mut T,
    input: Value,
    deadline: Instant,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_root = required_workflow_string(&input, "projectRoot")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    input
        .get("context")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("context".to_string()))?;
    let request_value = input
        .get("request")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("request".to_string()))?
        .clone();
    let request: EditJobRequest = serde_json::from_value(request_value.clone())
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let project_dir_path = Path::new(&project_dir);
    let (mut project, expected_revision) = {
        let _project_lease = acquire_split_project_mutation_lease(project_dir_path)
            .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
        let project = load_split_project(project_dir_path)
            .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
        let revision = project.content_revision;
        (project, revision)
    };
    if project.id != project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }
    let skills = load_project_skill_bundle(Path::new(&project_root))
        .map_err(|error| TemporalWorkflowInputError::CodexContext(error.to_string()))?;
    let result = start_codex_video_edit_turn_unpersisted_until(
        transport,
        1,
        &project_root,
        &mut project,
        request.clone(),
        &skills,
        Some(project_dir_path),
        deadline,
        None,
    )
    .map_err(|error| TemporalWorkflowInputError::CodexAppServer(error.to_string()))?;
    let proposal_validation_issues =
        result.proposal_validation_issues.as_ref().ok_or_else(|| {
            TemporalWorkflowInputError::MissingField("proposalValidationIssues".to_string())
        })?;
    if !proposal_validation_issues.is_empty() {
        let message = proposal_validation_issues
            .iter()
            .map(|issue| format!("{}: {}", issue.path, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(TemporalWorkflowInputError::CodexProposal(message));
    }
    let has_proposal = result.proposal.is_some();
    let proposal = result
        .proposal
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("proposal".to_string()))?;
    {
        let _project_lease = acquire_split_project_mutation_lease(project_dir_path)
            .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
        let mut canonical = load_split_project(project_dir_path)
            .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
        if canonical.content_revision != expected_revision {
            return Err(TemporalWorkflowInputError::CodexAppServer(format!(
                "project revision conflict: expected {expected_revision}, but canonical revision is {}",
                canonical.content_revision
            )));
        }
        canonical.codex_thread_id = Some(result.thread_id.clone());
        project = replace_split_project_if_revision(project_dir_path, canonical, expected_revision)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?
            .project;
        record_app_server_conversation_turn(
            project_dir_path,
            &project.id,
            &result.thread_id,
            AppServerConversationTurn {
                turn_id: result
                    .turn_response
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                turn_status: result
                    .turn_response
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                prompt: request.prompt.clone(),
                created_at: request.created_at.clone(),
                request: serde_json::to_value(&request).map_err(|error| {
                    TemporalWorkflowInputError::SaveSplitProject(error.to_string())
                })?,
                thread_response: result.thread_response.clone(),
                turn_response: result.turn_response.clone(),
                has_proposal,
                provider: None,
                provider_session_id: None,
            },
        )
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    }

    Ok(json!({
        "status": "completed",
        "projectId": project_id,
        "projectRoot": project_root,
        "projectDir": project_dir,
        "jobId": job_id,
        "request": request_value,
        "threadId": result.thread_id,
        "threadResponse": result.thread_response,
        "turnResponse": result.turn_response,
        "proposal": proposal,
        "proposalValidationIssues": proposal_validation_issues,
    }))
}

pub fn temporal_codex_edit_request_proposal_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let deadline = codex_app_server_deadline();
    let command = bundled_codex_app_server_command()
        .map_err(|error| TemporalWorkflowInputError::CodexAppServer(error.to_string()))?;
    let mut transport =
        StdioCodexAppServerTransport::spawn_until(&command, None, deadline, None)
            .map_err(|error| TemporalWorkflowInputError::CodexAppServer(error.to_string()))?;
    temporal_codex_edit_request_proposal_activity_with_transport_until(
        &mut transport,
        input,
        deadline,
    )
}

pub fn temporal_generate_media_workflow_input(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<TemporalGenerateMediaWorkflowInput, TemporalWorkflowInputError> {
    for field in [
        concat!("providerCredential", "EnvVar"),
        concat!("credential", "EnvVar"),
        concat!("auth", "EnvVar"),
    ] {
        if start_request.input.get(field).is_some() {
            return Err(TemporalWorkflowInputError::ForbiddenCredentialField(
                field.to_string(),
            ));
        }
    }
    validate_generate_media_start_request(start_request)?;
    let input = TemporalGenerateMediaWorkflowInput {
        project_id: required_workflow_string(&start_request.input, "projectId")?,
        project_dir: required_workflow_string(&start_request.input, "projectDir")?,
        asset_id: required_workflow_string(&start_request.input, "assetId")?,
        job_id: required_workflow_string(&start_request.input, "jobId")?,
        mock_mode: required_workflow_bool(&start_request.input, "mockMode")?,
        name: optional_workflow_string(&start_request.input, "name")?,
        placement_intent: optional_workflow_string(&start_request.input, "placementIntent")?,
        prompt: optional_workflow_string(&start_request.input, "prompt")?,
        model: optional_workflow_value(&start_request.input, "model"),
        references: optional_workflow_value(&start_request.input, "references"),
        settings: optional_workflow_value(&start_request.input, "settings"),
    };
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::GenerateMedia,
        &input.project_id,
        &input.job_id,
    );
    validate_temporal_search_attribute(
        &start_request.search_attributes,
        "projectId",
        &input.project_id,
    )?;
    validate_temporal_search_attribute(&start_request.search_attributes, "jobId", &input.job_id)?;
    validate_temporal_search_attribute(
        &start_request.search_attributes,
        "workflowKind",
        TemporalWorkflowKind::GenerateMedia.job_kind(),
    )?;
    if start_request.workflow_id != spec.workflow_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "workflowId".to_string(),
        ));
    }

    Ok(input)
}

pub fn temporal_export_nle_xml_workflow_input(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<TemporalExportNleXmlWorkflowInput, TemporalWorkflowInputError> {
    validate_temporal_start_request(start_request, TemporalWorkflowKind::ExportNleXml)?;
    let format = start_request
        .input
        .get("format")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("format".to_string()))
        .and_then(|format| {
            serde_json::from_value(format.clone())
                .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
        })?;
    let input = TemporalExportNleXmlWorkflowInput {
        project_id: required_workflow_string(&start_request.input, "projectId")?,
        project_dir: required_workflow_string(&start_request.input, "projectDir")?,
        job_id: required_workflow_string(&start_request.input, "jobId")?,
        timeline_id: optional_workflow_string(&start_request.input, "timelineId")?,
        format,
        output_path: required_workflow_string(&start_request.input, "outputPath")?,
        overwrite: optional_workflow_bool(&start_request.input, "overwrite")?.unwrap_or(true),
    };
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::ExportNleXml,
        &input.project_id,
        &input.job_id,
    );
    validate_temporal_search_attribute(
        &start_request.search_attributes,
        "projectId",
        &input.project_id,
    )?;
    validate_temporal_search_attribute(&start_request.search_attributes, "jobId", &input.job_id)?;
    validate_temporal_search_attribute(
        &start_request.search_attributes,
        "workflowKind",
        TemporalWorkflowKind::ExportNleXml.job_kind(),
    )?;
    if start_request.workflow_id != spec.workflow_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "workflowId".to_string(),
        ));
    }

    Ok(input)
}

fn validate_generate_media_start_request(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<(), TemporalWorkflowInputError> {
    validate_temporal_start_request(start_request, TemporalWorkflowKind::GenerateMedia)
}

fn validate_temporal_start_request(
    start_request: &TemporalWorkflowStartRequest,
    kind: TemporalWorkflowKind,
) -> Result<(), TemporalWorkflowInputError> {
    let spec = temporal_workflow_spec(
        kind,
        start_request
            .search_attributes
            .get("projectId")
            .and_then(Value::as_str)
            .unwrap_or("project"),
        start_request
            .search_attributes
            .get("jobId")
            .and_then(Value::as_str)
            .unwrap_or("job"),
    );
    if start_request.workflow_type != spec.workflow_type {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "workflowType".to_string(),
        ));
    }
    if start_request.task_queue != spec.task_queue {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "taskQueue".to_string(),
        ));
    }
    if start_request.activity_types != spec.activity_types {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "activityTypes".to_string(),
        ));
    }

    Ok(())
}

fn validate_temporal_search_attribute(
    search_attributes: &Value,
    field: &'static str,
    expected: &str,
) -> Result<(), TemporalWorkflowInputError> {
    match search_attributes.get(field).and_then(Value::as_str) {
        Some(actual) if actual.trim() == expected => Ok(()),
        _ => Err(TemporalWorkflowInputError::MismatchedInputField(format!(
            "searchAttributes.{field}"
        ))),
    }
}

pub fn temporal_generate_media_mock_completion_actions(
    project: &VideoProject,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if !input.mock_mode {
        return Err(TemporalWorkflowInputError::MockModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });
    let replacement_item_id = replacement_item_id.or(placement_replacement_item_id);

    let mut actions = mock_generation_completion_actions(asset, updated_at, replacement_item_id)
        .map_err(|error| TemporalWorkflowInputError::MockGeneration(error.to_string()))?;
    append_timeline_audio_insert_action(project, asset, &mut actions);
    append_timeline_visual_insert_action(project, asset, &mut actions);
    Ok(actions)
}

fn append_timeline_audio_insert_action(
    project: &VideoProject,
    asset: &GeneratedAsset,
    actions: &mut Vec<ProjectAction>,
) {
    if asset.placement_intent.as_deref() != Some("timeline") {
        return;
    }
    let Some(timeline_start_seconds) = asset.settings.timeline_start_seconds else {
        return;
    };
    let Some(audio_track_id) = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .map(|track| track.id.clone())
    else {
        return;
    };
    let Some((generated_output_media_id, output_duration_seconds, _relative_path)) =
        completed_generated_output_for_actions(actions)
    else {
        return;
    };
    if !is_audio_output_relative_path(&_relative_path) {
        return;
    }
    let duration_seconds = asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(output_duration_seconds)
        .max(0.001);
    let item_id = format!("{}-timeline-audio", asset.id);
    if project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == item_id)
        || actions.iter().any(|action| {
            matches!(
                action,
                ProjectAction::InsertItems { items, .. }
                    if items.iter().any(|item| item.id == item_id)
            )
        })
    {
        return;
    }

    actions.insert(
        actions.len().saturating_sub(1),
        ProjectAction::InsertItems {
            target_track_id: audio_track_id,
            insert_seconds: timeline_start_seconds,
            items: vec![TimelineItem {
                id: item_id,
                kind: TimelineItemKind::AudioClip,
                start_seconds: timeline_start_seconds,
                duration_seconds,
                source: TimelineSource::Media {
                    media_id: generated_output_media_id.clone(),
                },
                label: asset
                    .name
                    .clone()
                    .unwrap_or_else(|| "Generated audio".to_string()),
                properties: BTreeMap::from([
                    ("generatedAssetId".to_string(), json!(asset.id)),
                    (
                        "generatedOutputMediaId".to_string(),
                        json!(generated_output_media_id),
                    ),
                    ("sourceIn".to_string(), json!(0.0)),
                    ("sourceOut".to_string(), json!(duration_seconds)),
                ]),
            }],
        },
    );
}

fn append_timeline_visual_insert_action(
    project: &VideoProject,
    asset: &GeneratedAsset,
    actions: &mut Vec<ProjectAction>,
) {
    if asset.kind == MediaKind::Audio {
        return;
    }
    let Some(timeline_start_seconds) = asset.settings.timeline_start_seconds else {
        return;
    };
    let Some(video_track_id) = visual_insert_track_id(project, asset.placement_intent.as_deref())
    else {
        return;
    };
    let Some((generated_output_media_id, output_duration_seconds, relative_path)) =
        completed_generated_output_for_actions(actions)
    else {
        return;
    };
    if is_audio_output_relative_path(&relative_path) {
        return;
    }
    let duration_seconds = output_duration_seconds.max(0.001);
    let item_id = format!("{}-timeline-visual", asset.id);
    if project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .any(|item| item.id == item_id)
        || actions.iter().any(|action| {
            matches!(
                action,
                ProjectAction::InsertItems { items, .. }
                    if items.iter().any(|item| item.id == item_id)
            )
        })
    {
        return;
    }
    let item_kind = if relative_path.ends_with(".mp4")
        || relative_path.ends_with(".mov")
        || relative_path.ends_with(".webm")
    {
        TimelineItemKind::VideoClip
    } else {
        TimelineItemKind::ImageClip
    };

    actions.insert(
        actions.len().saturating_sub(1),
        ProjectAction::InsertItems {
            target_track_id: video_track_id,
            insert_seconds: timeline_start_seconds,
            items: vec![TimelineItem {
                id: item_id,
                kind: item_kind,
                start_seconds: timeline_start_seconds,
                duration_seconds,
                source: TimelineSource::Media {
                    media_id: generated_output_media_id.clone(),
                },
                label: asset
                    .name
                    .clone()
                    .unwrap_or_else(|| "Generated visual".to_string()),
                properties: BTreeMap::from([
                    ("generatedAssetId".to_string(), json!(asset.id)),
                    (
                        "generatedOutputMediaId".to_string(),
                        json!(generated_output_media_id),
                    ),
                    ("sourceIn".to_string(), json!(0.0)),
                    ("sourceOut".to_string(), json!(duration_seconds)),
                ]),
            }],
        },
    );
}

fn is_audio_output_relative_path(relative_path: &str) -> bool {
    relative_path.ends_with(".m4a")
        || relative_path.ends_with(".mp3")
        || relative_path.ends_with(".wav")
        || relative_path.ends_with(".aac")
}

fn visual_insert_track_id(
    project: &VideoProject,
    placement_intent: Option<&str>,
) -> Option<String> {
    match placement_intent {
        Some("timeline") => project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video && !track.locked)
            .map(|track| track.id.clone()),
        Some(intent) => intent
            .strip_prefix("insert-video:")
            .filter(|track_id| {
                project.timeline.tracks.iter().any(|track| {
                    track.id == *track_id && track.kind == TrackKind::Video && !track.locked
                })
            })
            .map(str::to_string),
        None => None,
    }
}

fn completed_generated_output_for_actions(
    actions: &[ProjectAction],
) -> Option<(String, f64, String)> {
    actions.iter().find_map(|action| {
        if let ProjectAction::CompleteGeneratedAsset { outputs, .. } = action {
            (outputs.len() == 1).then(|| {
                let output = &outputs[0];
                (
                    output.media_id.clone(),
                    output.duration_seconds.max(0.001),
                    output.relative_path.clone(),
                )
            })
        } else {
            None
        }
    })
}

pub fn temporal_generate_media_fal_queue_submission(
    project: &VideoProject,
    start_request: &TemporalWorkflowStartRequest,
) -> Result<FalQueueSubmission, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let submission = build_fal_queue_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;

    Ok(submission)
}

pub fn temporal_generate_media_fal_queue_submission_from_project_dir(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<FalQueueSubmission, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project = load_split_project(Path::new(&input.project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;

    temporal_generate_media_fal_queue_submission(&project, start_request)
}

pub fn temporal_generate_media_provider_submission_from_project_dir(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<TemporalGenerateMediaProviderSubmission, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project = load_split_project(Path::new(&input.project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;

    if asset.model.provider == FAL_PROVIDER {
        let submission = build_fal_queue_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::Fal(submission));
    }

    if asset.model.provider == ELEVENLABS_PROVIDER {
        let submission = build_elevenlabs_generation_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::ElevenLabs(
            submission,
        ));
    }

    if asset.model.provider == MINIMAX_PROVIDER {
        let submission = build_minimax_generation_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::Minimax(submission));
    }

    if asset.model.provider == GOOGLE_PROVIDER {
        if asset.model.id == GOOGLE_GEMINI_TTS_MODEL_ID {
            let submission =
                build_google_gemini_tts_generation_submission(asset).map_err(|error| {
                    TemporalWorkflowInputError::FalGenerationRequest(error.to_string())
                })?;
            return Ok(TemporalGenerateMediaProviderSubmission::GoogleGeminiTts(
                submission,
            ));
        }
        if asset.model.id == GOOGLE_LYRIA_3_PRO_MODEL_ID {
            let submission = build_google_lyria_generation_submission(asset).map_err(|error| {
                TemporalWorkflowInputError::FalGenerationRequest(error.to_string())
            })?;
            return Ok(TemporalGenerateMediaProviderSubmission::GoogleLyria(
                submission,
            ));
        }
        let submission = build_google_veo_generation_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::Google(submission));
    }

    if asset.model.provider == REPLICATE_PROVIDER {
        let submission = build_replicate_prediction_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::Replicate(
            submission,
        ));
    }

    if asset.model.provider == OPENAI_PROVIDER {
        let submission = build_openai_image_generation_submission(asset)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::OpenAi(submission));
    }

    if asset.model.provider == XAI_PROVIDER {
        let submission = if asset.model.id == XAI_GROK_VIDEO_MODEL_ID {
            build_xai_video_generation_submission(asset)
        } else {
            build_xai_image_generation_submission(asset)
        }
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
        return Ok(TemporalGenerateMediaProviderSubmission::XAi(submission));
    }

    Err(TemporalWorkflowInputError::FalGenerationRequest(format!(
        "unsupported generation provider: {}",
        asset.model.provider
    )))
}

fn validate_generation_asset_against_current_catalog(
    project: &VideoProject,
    asset: &GeneratedAsset,
) -> Result<(), TemporalWorkflowInputError> {
    let model =
        resolved_generation_model_payload(asset.model.provider.trim(), asset.model.id.trim())
            .map_err(|error| TemporalWorkflowInputError::GenerationCapability(error.to_string()))?
            .ok_or_else(|| {
                TemporalWorkflowInputError::GenerationCapability(format!(
                    "model is not available in the resolved catalog: {}:{}",
                    asset.model.provider, asset.model.id
                ))
            })?;
    validate_generation_asset_capabilities(project, asset, &model)
        .map_err(|error| TemporalWorkflowInputError::GenerationCapability(error.to_string()))
}

fn validate_generation_start_request_against_current_catalog(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<(), TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project = load_split_project(Path::new(&input.project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    validate_generation_asset_against_current_catalog(&project, asset)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderInputUploadRoute {
    DataUrl,
    Replicate,
    Fal,
}

fn provider_input_upload_route(
    provider: &str,
) -> Result<ProviderInputUploadRoute, TemporalWorkflowInputError> {
    match provider.trim() {
        OPENAI_PROVIDER | XAI_PROVIDER | GOOGLE_PROVIDER => Ok(ProviderInputUploadRoute::DataUrl),
        REPLICATE_PROVIDER => Ok(ProviderInputUploadRoute::Replicate),
        FAL_PROVIDER => Ok(ProviderInputUploadRoute::Fal),
        provider => Err(TemporalWorkflowInputError::UnsupportedProviderInputUpload(
            provider.to_string(),
        )),
    }
}

pub fn temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
    start_request: &TemporalWorkflowStartRequest,
    _updated_at: &str,
    runner: &dyn ProcessRunner,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let provider_input_renderer = GstreamerProviderInputVideoRenderer { runner };
    let client = reqwest::blocking::Client::new();
    temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
        start_request,
        |provider, source_path, credential_provider| match provider_input_upload_route(provider)? {
            ProviderInputUploadRoute::DataUrl => openai_image_data_url_from_path(source_path)
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string())),
            ProviderInputUploadRoute::Replicate => {
                let credential = resolve_provider_credential(credential_provider)
                    .map_err(|error| {
                        TemporalWorkflowInputError::FalGenerationRun(error.to_string())
                    })?
                    .into_secret();
                let api_base_url = std::env::var(VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR)
                    .unwrap_or_else(|_| REPLICATE_API_BASE_URL.to_string());
                upload_replicate_local_file_with_client(
                    &client,
                    &api_base_url,
                    source_path,
                    &credential,
                )
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
            }
            ProviderInputUploadRoute::Fal => {
                let credential = resolve_provider_credential(credential_provider)
                    .map_err(|error| {
                        TemporalWorkflowInputError::FalGenerationRun(error.to_string())
                    })?
                    .into_secret();
                let rest_api_base_url = std::env::var(VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR)
                    .unwrap_or_else(|_| FAL_REST_API_BASE_URL.to_string());
                upload_fal_local_file_to_cdn_with_client(
                    &client,
                    &rest_api_base_url,
                    source_path,
                    &credential,
                )
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
            }
        },
        |source_path, output_path, source_in, source_out| {
            trim_provider_input_video_with_renderer(
                &provider_input_renderer,
                source_path,
                output_path,
                source_in,
                source_out,
            )
        },
        |source_path, output_path| {
            compress_provider_input_video_with_renderer(
                &provider_input_renderer,
                source_path,
                output_path,
            )
        },
        |project_dir, project, asset, output_path, start_seconds, end_seconds| {
            render_timeline_provider_input_video_with_runner(
                runner,
                project_dir,
                project,
                asset,
                output_path,
                start_seconds,
                end_seconds,
            )
        },
    )
}

fn temporal_generate_media_prepare_provider_inputs_from_project_dir_with_credential(
    start_request: &TemporalWorkflowStartRequest,
    client: &reqwest::blocking::Client,
    credential: &str,
    runner: &dyn ProcessRunner,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let provider_input_renderer = GstreamerProviderInputVideoRenderer { runner };
    temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
        start_request,
        |provider, source_path, _credential_provider| match provider_input_upload_route(provider)? {
            ProviderInputUploadRoute::DataUrl => openai_image_data_url_from_path(source_path)
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string())),
            ProviderInputUploadRoute::Replicate => {
                let api_base_url = std::env::var(VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR)
                    .unwrap_or_else(|_| REPLICATE_API_BASE_URL.to_string());
                upload_replicate_local_file_with_client(
                    client,
                    &api_base_url,
                    source_path,
                    credential,
                )
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
            }
            ProviderInputUploadRoute::Fal => {
                let rest_api_base_url = std::env::var(VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR)
                    .unwrap_or_else(|_| FAL_REST_API_BASE_URL.to_string());
                upload_fal_local_file_to_cdn_with_client(
                    client,
                    &rest_api_base_url,
                    source_path,
                    credential,
                )
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
            }
        },
        |source_path, output_path, source_in, source_out| {
            trim_provider_input_video_with_renderer(
                &provider_input_renderer,
                source_path,
                output_path,
                source_in,
                source_out,
            )
        },
        |source_path, output_path| {
            compress_provider_input_video_with_renderer(
                &provider_input_renderer,
                source_path,
                output_path,
            )
        },
        |project_dir, project, asset, output_path, start_seconds, end_seconds| {
            render_timeline_provider_input_video_with_runner(
                runner,
                project_dir,
                project,
                asset,
                output_path,
                start_seconds,
                end_seconds,
            )
        },
    )
}

#[cfg(test)]
fn temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
    start_request: &TemporalWorkflowStartRequest,
    upload_provider_input: impl FnMut(&str, &Path, &str) -> Result<String, TemporalWorkflowInputError>,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
        start_request,
        upload_provider_input,
        |_source_path, _output_path, _source_in, _source_out| {
            Err(TemporalWorkflowInputError::FalGenerationRun(
                "trimmed provider input requires a process runner".to_string(),
            ))
        },
        |_source_path, output_path| {
            fs::write(output_path, b"compressed-reference-video")
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
        },
        |_project_dir, _project, _asset, output_path, _start_seconds, _end_seconds| {
            fs::write(output_path, b"rendered-timeline-provider-input")
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
        },
    )
}

fn temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
    start_request: &TemporalWorkflowStartRequest,
    mut upload_provider_input: impl FnMut(
        &str,
        &Path,
        &str,
    ) -> Result<String, TemporalWorkflowInputError>,
    mut trim_provider_input_video: impl FnMut(
        &Path,
        &Path,
        f64,
        f64,
    ) -> Result<(), TemporalWorkflowInputError>,
    mut compress_provider_input_video: impl FnMut(
        &Path,
        &Path,
    ) -> Result<(), TemporalWorkflowInputError>,
    mut render_timeline_provider_input_video: impl FnMut(
        &Path,
        &VideoProject,
        &GeneratedAsset,
        &Path,
        f64,
        f64,
    ) -> Result<(), TemporalWorkflowInputError>,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    validate_generation_asset_against_current_catalog(&project, asset)?;

    let Some(provider_input_media_ids) = provider_input_media_ids(asset) else {
        return apply_project_actions_to_split_project(project_dir, Vec::new())
            .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()));
    };
    if asset
        .references
        .provider_input_urls
        .iter()
        .any(|url| !url.trim().is_empty())
    {
        return apply_project_actions_to_split_project(project_dir, Vec::new())
            .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()));
    }
    if provider_input_media_ids.is_empty() {
        if let Some(range) = provider_input_timeline_render_range(asset) {
            let dir = tempfile::tempdir()
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;
            let output_path = dir.path().join(format!(
                "provider-input-{}-timeline.mp4",
                safe_workflow_segment(&asset.id)
            ));
            render_timeline_provider_input_video(
                project_dir,
                &project,
                asset,
                &output_path,
                range.source_in,
                range.source_out,
            )?;
            if !output_path.is_file() {
                return Err(TemporalWorkflowInputError::FalGenerationRun(
                    "timeline provider input render was not written".to_string(),
                ));
            }
            let provider_input_urls = vec![upload_provider_input(
                &asset.model.provider,
                &output_path,
                &asset.model.provider,
            )?];
            drop(dir);
            return update_generated_asset_provider_input_urls(
                project_dir,
                &input.asset_id,
                asset,
                provider_input_urls,
            );
        }
        return apply_project_actions_to_split_project(project_dir, Vec::new())
            .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()));
    }

    let mut provider_input_urls = Vec::new();
    for media_id in provider_input_media_ids {
        let source_media = project
            .media
            .iter()
            .find(|media| media.id == media_id)
            .ok_or_else(|| {
                TemporalWorkflowInputError::MismatchedInputField(format!("references.{media_id}"))
            })?;
        let source_path = resolve_project_relative_path(project_dir, &source_media.relative_path)
            .map_err(|_| {
            TemporalWorkflowInputError::MismatchedInputField("media.relativePath".to_string())
        })?;
        if !source_path.is_file() {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "media.relativePath".to_string(),
            ));
        }
        let mut temp_dirs = Vec::new();
        let mut upload_path = if let Some(range) =
            provider_input_trim_range(&input, asset, source_media)
        {
            let dir = tempfile::tempdir()
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;
            let output_path = dir.path().join(format!(
                "provider-input-{}-{}.mp4",
                safe_workflow_segment(&asset.id),
                safe_workflow_segment(&media_id)
            ));
            trim_provider_input_video(
                &source_path,
                &output_path,
                range.source_in,
                range.source_out,
            )?;
            if !output_path.is_file() {
                return Err(TemporalWorkflowInputError::FalGenerationRun(
                    "trimmed provider input was not written".to_string(),
                ));
            }
            temp_dirs.push(dir);
            output_path
        } else {
            source_path
        };
        if should_compress_provider_reference_video(asset, source_media) {
            let dir = tempfile::tempdir()
                .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;
            let output_path = dir.path().join(format!(
                "provider-input-{}-{}-compressed.mp4",
                safe_workflow_segment(&asset.id),
                safe_workflow_segment(&media_id)
            ));
            compress_provider_input_video(&upload_path, &output_path)?;
            if !output_path.is_file() {
                return Err(TemporalWorkflowInputError::FalGenerationRun(
                    "compressed provider input was not written".to_string(),
                ));
            }
            temp_dirs.push(dir);
            upload_path = output_path;
        }
        provider_input_urls.push(upload_provider_input(
            &asset.model.provider,
            &upload_path,
            &asset.model.provider,
        )?);
        drop(temp_dirs);
    }

    update_generated_asset_provider_input_urls(
        project_dir,
        &input.asset_id,
        asset,
        provider_input_urls,
    )
}

fn update_generated_asset_provider_input_urls(
    project_dir: &Path,
    asset_id: &str,
    asset: &GeneratedAsset,
    provider_input_urls: Vec<String>,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let mut references = asset.references.clone();
    references.provider_input_urls = provider_input_urls;
    let action_references = ProjectActionGeneratedAssetReferences {
        media_ids: references.media_ids,
        source_video_media_ref: references.source_video_media_ref,
        first_frame_media_id: references.first_frame_media_id,
        last_frame_media_id: references.last_frame_media_id,
        reference_image_media_refs: references.reference_image_media_refs,
        reference_video_media_refs: references.reference_video_media_refs,
        reference_audio_media_refs: references.reference_audio_media_refs,
        provider_input_urls: references.provider_input_urls,
    };

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateGeneratedAssetReferences {
            asset_id: asset_id.to_string(),
            references: action_references,
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))
}

const PROVIDER_REFERENCE_VIDEO_MAX_LONG_SIDE: u32 = 1100;

fn should_compress_provider_reference_video(asset: &GeneratedAsset, media: &MediaAsset) -> bool {
    if media.kind != MediaKind::Video {
        return false;
    }
    if !asset
        .references
        .reference_video_media_refs
        .iter()
        .any(|media_id| media_id.trim() == media.id)
    {
        return false;
    }
    media
        .width
        .zip(media.height)
        .map(|(width, height)| width.max(height) > PROVIDER_REFERENCE_VIDEO_MAX_LONG_SIDE)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ProviderInputTrimRange {
    source_in: f64,
    source_out: f64,
}

fn provider_input_trim_range(
    input: &TemporalGenerateMediaWorkflowInput,
    asset: &GeneratedAsset,
    media: &MediaAsset,
) -> Option<ProviderInputTrimRange> {
    if media.kind != MediaKind::Video {
        return None;
    }

    let brief_range = input
        .settings
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|settings| {
            if settings
                .get("sourceClipId")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_none_or(str::is_empty)
            {
                return None;
            }
            let source_in = settings.get("sourceIn").and_then(Value::as_f64)?;
            let source_out = settings.get("sourceOut").and_then(Value::as_f64)?;
            Some((source_in, source_out))
        });
    let settings_range = asset
        .settings
        .video_source_start_seconds
        .zip(asset.settings.video_source_end_seconds);
    let (source_in, source_out) = brief_range.or(settings_range)?;
    if !source_in.is_finite() || !source_out.is_finite() || source_in < 0.0 {
        return None;
    }
    if source_out <= source_in {
        return None;
    }
    let media_duration = media.duration_seconds;
    if media_duration.is_finite()
        && media_duration > 0.0
        && source_in <= 0.001
        && source_out >= media_duration - 0.001
    {
        return None;
    }

    Some(ProviderInputTrimRange {
        source_in,
        source_out,
    })
}

fn provider_input_timeline_render_range(asset: &GeneratedAsset) -> Option<ProviderInputTrimRange> {
    if asset.model.provider != FAL_PROVIDER
        || !matches!(
            asset.model.id.as_str(),
            FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID
        )
    {
        return None;
    }
    let (source_in, source_out) = asset
        .settings
        .video_source_start_seconds
        .zip(asset.settings.video_source_end_seconds)?;
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
    {
        return None;
    }
    Some(ProviderInputTrimRange {
        source_in,
        source_out,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct ProviderInputVideoJob {
    source_path: PathBuf,
    output_path: PathBuf,
    source_in: f64,
    source_out: f64,
    max_dimension: Option<u32>,
}

trait ProviderInputVideoRenderer {
    fn render(
        &self,
        job: &ProviderInputVideoJob,
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput>;
}

struct GstreamerProviderInputVideoRenderer<'a> {
    runner: &'a dyn ProcessRunner,
}

impl ProviderInputVideoRenderer for GstreamerProviderInputVideoRenderer<'_> {
    fn render(
        &self,
        job: &ProviderInputVideoJob,
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput> {
        let (probe, _) = probe_media_with_gstreamer(
            &job.source_path,
            Duration::from_secs(30),
            "providerInput.source",
        )?;
        let plan = provider_input_render_plan(job, &probe)?;
        GstreamerGesRenderBackend::new().render_cancellable(self.runner, &plan, &[], timeout, None)
    }
}

fn trim_provider_input_video_with_renderer(
    renderer: &dyn ProviderInputVideoRenderer,
    source_path: &Path,
    output_path: &Path,
    source_in: f64,
    source_out: f64,
) -> Result<(), TemporalWorkflowInputError> {
    let duration = source_out - source_in;
    if !duration.is_finite() || duration <= 0.0 {
        return Err(TemporalWorkflowInputError::FalGenerationRun(
            "trimmed provider input duration must be positive".to_string(),
        ));
    }

    render_provider_input_video(
        renderer,
        ProviderInputVideoJob {
            source_path: source_path.to_path_buf(),
            output_path: output_path.to_path_buf(),
            source_in,
            source_out,
            max_dimension: None,
        },
        Duration::from_secs(120),
        "trim",
    )
}

fn compress_provider_input_video_with_renderer(
    renderer: &dyn ProviderInputVideoRenderer,
    source_path: &Path,
    output_path: &Path,
) -> Result<(), TemporalWorkflowInputError> {
    render_provider_input_video(
        renderer,
        ProviderInputVideoJob {
            source_path: source_path.to_path_buf(),
            output_path: output_path.to_path_buf(),
            source_in: 0.0,
            source_out: f64::INFINITY,
            max_dimension: Some(960),
        },
        Duration::from_secs(300),
        "compress",
    )
}

fn render_provider_input_video(
    renderer: &dyn ProviderInputVideoRenderer,
    job: ProviderInputVideoJob,
    timeout: Duration,
    operation: &str,
) -> Result<(), TemporalWorkflowInputError> {
    let output = renderer.render(&job, timeout).map_err(|errors| {
        TemporalWorkflowInputError::FalGenerationRun(pipeline_errors_message(errors))
    })?;
    if output.status_code != Some(0) {
        return Err(TemporalWorkflowInputError::FalGenerationRun(format!(
            "{operation} provider input failed with status {:?}: {}",
            output.status_code,
            output.stderr.trim()
        )));
    }
    if !job.output_path.is_file() {
        return Err(TemporalWorkflowInputError::FalGenerationRun(format!(
            "{operation} provider input did not produce an output file"
        )));
    }
    Ok(())
}

fn provider_input_render_plan(
    job: &ProviderInputVideoJob,
    probe: &MediaProbe,
) -> PipelineResult<RenderPlan> {
    let video = probe.video.as_ref().ok_or_else(|| {
        vec![provider_input_pipeline_error(
            "providerInput.source.video",
            "Provider input preparation requires a video stream.",
            "Choose a readable source video and retry.",
        )]
    })?;
    let source_width = video.width.filter(|value| *value > 0).ok_or_else(|| {
        vec![provider_input_pipeline_error(
            "providerInput.source.width",
            "Provider input video width is unavailable.",
            "Re-import the source so GStreamer can inspect its dimensions.",
        )]
    })?;
    let source_height = video.height.filter(|value| *value > 0).ok_or_else(|| {
        vec![provider_input_pipeline_error(
            "providerInput.source.height",
            "Provider input video height is unavailable.",
            "Re-import the source so GStreamer can inspect its dimensions.",
        )]
    })?;
    let source_duration = probe
        .duration_seconds
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            vec![provider_input_pipeline_error(
                "providerInput.source.duration",
                "Provider input video duration is unavailable.",
                "Re-import the source so GStreamer can inspect its duration.",
            )]
        })?;
    let source_out = if job.source_out.is_infinite() {
        source_duration
    } else {
        job.source_out
    };
    if !job.source_in.is_finite()
        || job.source_in < 0.0
        || !source_out.is_finite()
        || source_out <= job.source_in
        || source_out > source_duration + 0.05
    {
        return Err(vec![provider_input_pipeline_error(
            "providerInput.source.range",
            "Provider input video range is invalid.",
            "Choose a finite range inside the source duration.",
        )]);
    }
    let (width, height) =
        fit_provider_input_dimensions(source_width, source_height, job.max_dimension);
    let clip = RenderClip {
        source_path: Some(job.source_path.display().to_string()),
        timeline_start_seconds: Some(0.0),
        properties: BTreeMap::from([(
            "timelineDurationSeconds".to_string(),
            json!(source_out - job.source_in),
        )]),
        timeline_track_index: 0,
        source_in: job.source_in,
        source_out,
    };
    Ok(RenderPlan {
        input_path: job.source_path.display().to_string(),
        output_path: job.output_path.display().to_string(),
        width,
        height,
        fps: video
            .fps
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(30.0),
        quality: RenderQuality::Final,
        output_profile: RenderOutputProfile::Mp4Primary,
        encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
        clips: vec![clip.clone()],
        audio_clips: probe.audio.as_ref().map(|_| vec![clip]).unwrap_or_default(),
        transitions: Vec::new(),
        audio_transitions: Vec::new(),
    })
}

fn fit_provider_input_dimensions(
    width: u32,
    height: u32,
    max_dimension: Option<u32>,
) -> (u32, u32) {
    let Some(max_dimension) = max_dimension.filter(|value| *value >= 2) else {
        return (even_dimension(width), even_dimension(height));
    };
    let scale = (max_dimension as f64 / width.max(height) as f64).min(1.0);
    (
        even_dimension((width as f64 * scale).floor() as u32),
        even_dimension((height as f64 * scale).floor() as u32),
    )
}

fn even_dimension(value: u32) -> u32 {
    value.max(2) & !1
}

fn provider_input_pipeline_error(path: &str, summary: &str, recovery: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        path,
        summary,
        recovery,
    )
}

fn render_timeline_provider_input_video_with_runner(
    runner: &dyn ProcessRunner,
    project_dir: &Path,
    project: &VideoProject,
    asset: &GeneratedAsset,
    output_path: &Path,
    start_seconds: f64,
    end_seconds: f64,
) -> Result<(), TemporalWorkflowInputError> {
    let plan = build_project_provider_input_render_plan_for_range(
        project_dir,
        project,
        output_path.to_path_buf(),
        start_seconds,
        end_seconds,
    )
    .map_err(|errors| TemporalWorkflowInputError::Render(pipeline_errors_message(errors)))?;
    let backend = GstreamerGesRenderBackend::new();
    let render_output = backend
        .render_cancellable(runner, &plan, &[], Duration::from_secs(300), None)
        .map_err(|errors| TemporalWorkflowInputError::Render(pipeline_errors_message(errors)))?;
    if render_output.status_code != Some(0) {
        return Err(TemporalWorkflowInputError::Render(format!(
            "timeline provider input render failed for {} with status {:?}: {}",
            asset.id,
            render_output.status_code,
            render_output.stderr.trim()
        )));
    }
    if !output_path.is_file() {
        return Err(TemporalWorkflowInputError::Render(format!(
            "timeline provider input render did not produce {}",
            output_path.display()
        )));
    }
    Ok(())
}

fn provider_input_media_ids(asset: &GeneratedAsset) -> Option<Vec<String>> {
    match (asset.model.provider.as_str(), asset.model.id.as_str()) {
        (provider, FAL_AURA_SR_MODEL_ID) if provider == FAL_PROVIDER => asset
            .references
            .media_ids
            .first()
            .cloned()
            .map(|id| vec![id]),
        (provider, FAL_VIDEO_UPSCALER_MODEL_ID) if provider == FAL_PROVIDER => Some(
            asset
                .references
                .source_video_media_ref
                .as_deref()
                .map(str::trim)
                .filter(|media_id| !media_id.is_empty())
                .map(|media_id| vec![media_id.to_string()])
                .unwrap_or_else(|| {
                    asset
                        .references
                        .media_ids
                        .iter()
                        .map(|media_id| media_id.trim())
                        .find(|media_id| !media_id.is_empty())
                        .map(|media_id| vec![media_id.to_string()])
                        .unwrap_or_default()
                }),
        ),
        (provider, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(image_edit_provider_input_media_ids(asset))
        }
        (provider, OPENAI_GPT_IMAGE_EDIT_MODEL_ID) if provider == OPENAI_PROVIDER => {
            Some(image_edit_provider_input_media_ids(asset))
        }
        (provider, crate::generation::xai::XAI_GROK_IMAGE_QUALITY_MODEL_ID)
            if provider == XAI_PROVIDER =>
        {
            Some(image_edit_provider_input_media_ids(asset))
        }
        (provider, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(wan_image_to_video_provider_input_media_ids(asset))
        }
        (provider, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(wan_reference_to_video_provider_input_media_ids(asset))
        }
        (provider, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(video_to_video_provider_input_media_ids(asset))
        }
        (provider, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(kling_image_to_video_provider_input_media_ids(asset))
        }
        (provider, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID) if provider == FAL_PROVIDER => {
            Some(kling_motion_control_provider_input_media_ids(asset))
        }
        (provider, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID) if provider == FAL_PROVIDER => Some(
            asset
                .references
                .reference_audio_media_refs
                .iter()
                .map(|media_id| media_id.trim())
                .find(|media_id| !media_id.is_empty())
                .map(|media_id| vec![media_id.to_string()])
                .unwrap_or_default(),
        ),
        (provider, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID)
            if provider == FAL_PROVIDER =>
        {
            Some(video_to_audio_provider_input_media_ids(asset))
        }
        (provider, REPLICATE_SEEDANCE_20_MODEL_ID | REPLICATE_SEEDANCE_20_FAST_MODEL_ID)
            if provider == REPLICATE_PROVIDER =>
        {
            Some(seedance_provider_input_media_ids(asset))
        }
        (provider, GOOGLE_VEO_31_FAST_MODEL_ID) if provider == GOOGLE_PROVIDER => {
            Some(google_veo_provider_input_media_ids(asset))
        }
        (provider, XAI_GROK_VIDEO_MODEL_ID) if provider == XAI_PROVIDER => {
            Some(xai_video_provider_input_media_ids(asset))
        }
        _ => None,
    }
}

fn video_to_audio_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
        .map(|media_id| vec![media_id.to_string()])
        .unwrap_or_else(|| {
            asset
                .references
                .reference_video_media_refs
                .iter()
                .map(|media_id| media_id.trim())
                .find(|media_id| !media_id.is_empty())
                .map(|media_id| vec![media_id.to_string()])
                .unwrap_or_default()
        })
}

fn video_to_video_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
        .map(|media_id| vec![media_id.to_string()])
        .unwrap_or_else(|| {
            asset
                .references
                .reference_video_media_refs
                .iter()
                .map(|media_id| media_id.trim())
                .find(|media_id| !media_id.is_empty())
                .map(|media_id| vec![media_id.to_string()])
                .unwrap_or_else(|| {
                    non_empty_media_ids(&asset.references.media_ids)
                        .take(1)
                        .collect()
                })
        })
}

fn kling_motion_control_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    asset
        .references
        .source_video_media_ref
        .iter()
        .chain(asset.references.reference_image_media_refs.iter())
        .map(|media_id| media_id.trim())
        .filter(|media_id| !media_id.is_empty())
        .map(|media_id| media_id.to_string())
        .collect()
}

fn image_edit_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    let reference_image_refs =
        non_empty_media_ids(&asset.references.reference_image_media_refs).collect::<Vec<_>>();
    if !reference_image_refs.is_empty() {
        return reference_image_refs;
    }

    non_empty_media_ids(&asset.references.media_ids).collect()
}

fn wan_image_to_video_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    let mut media_ids = Vec::new();
    if let Some(source_video_media_ref) = asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        media_ids.push(source_video_media_ref.to_string());
    } else {
        if let Some(first_frame_media_id) = asset
            .references
            .first_frame_media_id
            .as_deref()
            .map(str::trim)
            .filter(|media_id| !media_id.is_empty())
        {
            media_ids.push(first_frame_media_id.to_string());
        }
        if let Some(last_frame_media_id) = asset
            .references
            .last_frame_media_id
            .as_deref()
            .map(str::trim)
            .filter(|media_id| !media_id.is_empty())
        {
            media_ids.push(last_frame_media_id.to_string());
        }
    }
    if let Some(audio_media_id) = asset
        .references
        .reference_audio_media_refs
        .iter()
        .map(|media_id| media_id.trim())
        .find(|media_id| !media_id.is_empty())
    {
        media_ids.push(audio_media_id.to_string());
    }
    media_ids
}

fn wan_reference_to_video_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    non_empty_media_ids(&asset.references.reference_image_media_refs)
        .chain(non_empty_media_ids(
            &asset.references.reference_video_media_refs,
        ))
        .collect()
}

fn kling_image_to_video_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    [
        asset.references.first_frame_media_id.as_deref(),
        asset.references.last_frame_media_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|media_id| !media_id.is_empty())
    .map(str::to_string)
    .collect()
}

fn seedance_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    let mut media_ids = Vec::new();
    if let Some(source_video_media_ref) = asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        media_ids.push(source_video_media_ref.to_string());
    }
    if let Some(first_frame_media_id) = asset
        .references
        .first_frame_media_id
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        media_ids.push(first_frame_media_id.to_string());
    }
    if let Some(last_frame_media_id) = asset
        .references
        .last_frame_media_id
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        media_ids.push(last_frame_media_id.to_string());
    }
    media_ids.extend(non_empty_media_ids(
        &asset.references.reference_image_media_refs,
    ));
    media_ids.extend(non_empty_media_ids(
        &asset.references.reference_video_media_refs,
    ));
    media_ids.extend(non_empty_media_ids(
        &asset.references.reference_audio_media_refs,
    ));
    media_ids
}

fn xai_video_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    if let Some(source_video_media_ref) = asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        return vec![source_video_media_ref.to_string()];
    }
    if let Some(first_frame_media_id) = asset
        .references
        .first_frame_media_id
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        return vec![first_frame_media_id.to_string()];
    }
    non_empty_media_ids(&asset.references.reference_image_media_refs).collect()
}

fn google_veo_provider_input_media_ids(asset: &GeneratedAsset) -> Vec<String> {
    [
        asset.references.first_frame_media_id.as_deref(),
        asset.references.last_frame_media_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|media_id| !media_id.is_empty())
    .map(str::to_string)
    .chain(non_empty_media_ids(
        &asset.references.reference_image_media_refs,
    ))
    .collect()
}

fn non_empty_media_ids(media_ids: &[String]) -> impl Iterator<Item = String> + '_ {
    media_ids
        .iter()
        .map(|media_id| media_id.trim())
        .filter(|media_id| !media_id.is_empty())
        .map(str::to_string)
}

pub fn temporal_generate_media_prepare_provider_inputs_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_generate_media_start_request_from_runtime_input(&input)?;
    let updated_at = required_workflow_string(&input, "updatedAt")?;
    let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
        &start_request,
        &updated_at,
        &SystemProcessRunner,
    )?;
    let workflow_input = temporal_generate_media_workflow_input(&start_request)?;

    Ok(json!({
        "projectId": write.project.id,
        "jobId": workflow_input.job_id,
        "assetId": workflow_input.asset_id,
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

pub fn temporal_generate_media_build_fal_generation_request_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_generate_media_start_request_from_runtime_input(&input)?;
    let submission = temporal_generate_media_provider_submission_from_project_dir(&start_request)?;

    serde_json::to_value(submission)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

fn temporal_generate_media_start_request_from_runtime_input(
    input: &Value,
) -> Result<TemporalWorkflowStartRequest, TemporalWorkflowInputError> {
    temporal_start_request_from_runtime_input(input)
}

fn temporal_start_request_from_runtime_input(
    input: &Value,
) -> Result<TemporalWorkflowStartRequest, TemporalWorkflowInputError> {
    let start_request = input
        .get("startRequest")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("startRequest".to_string()))?
        .clone();

    serde_json::from_value(start_request)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_export_nle_xml_build_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_start_request_from_runtime_input(&input)?;
    let workflow_input = temporal_export_nle_xml_workflow_input(&start_request)?;
    let project = load_split_project(Path::new(&workflow_input.project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != workflow_input.project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }

    let project = match workflow_input.timeline_id.as_deref() {
        Some(timeline_id) => project.projected_for_timeline(timeline_id).ok_or_else(|| {
            TemporalWorkflowInputError::MismatchedInputField("timelineId".to_string())
        })?,
        None => project,
    };
    let export = export_project_timeline_to_nle_xml(&project, workflow_input.format)
        .map_err(|error| TemporalWorkflowInputError::NleXmlExport(error.to_string()))?;
    let expected_output_path = format!("exports/{}", export.filename);
    if workflow_input.output_path != expected_output_path {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }

    serde_json::to_value(TemporalExportNleXmlBuildActivityOutput {
        project_id: workflow_input.project_id,
        project_dir: workflow_input.project_dir,
        job_id: workflow_input.job_id,
        format: workflow_input.format,
        filename: export.filename,
        output_path: workflow_input.output_path,
        overwrite: workflow_input.overwrite,
        mime_type: export.mime_type,
        xml: export.xml,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_export_nle_xml_validate_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let output: TemporalExportNleXmlBuildActivityOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_nle_xml_activity_output(&output)?;

    serde_json::to_value(TemporalExportNleXmlValidateActivityOutput {
        status: "validated".to_string(),
        project_id: output.project_id,
        project_dir: output.project_dir,
        job_id: output.job_id,
        format: output.format,
        filename: output.filename,
        output_path: output.output_path,
        overwrite: output.overwrite,
        mime_type: output.mime_type,
        xml: output.xml,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_export_nle_xml_write_artifact_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let output: TemporalExportNleXmlBuildActivityOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_nle_xml_activity_output(&output)?;
    let export = NleXmlExport {
        filename: output.filename.clone(),
        mime_type: output.mime_type.clone(),
        xml: output.xml.clone(),
    };
    let output_absolute = Path::new(&output.project_dir).join(&output.output_path);
    let _project_lease = acquire_split_project_mutation_lease(Path::new(&output.project_dir))
        .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
    ensure_export_output_can_be_written(&output_absolute, output.overwrite)?;
    let written_path = crate::project::nle_export::write_nle_xml_export_with_overwrite(
        Path::new(&output.project_dir),
        &export,
        output.overwrite,
    )
    .map_err(|error| TemporalWorkflowInputError::NleXmlExport(error.to_string()))?;

    serde_json::to_value(TemporalExportNleXmlWriteArtifactActivityOutput {
        project_id: output.project_id,
        project_dir: output.project_dir,
        job_id: output.job_id,
        format: output.format,
        filename: output.filename,
        output_path: output.output_path,
        overwrite: output.overwrite,
        mime_type: output.mime_type,
        written_path: written_path.display().to_string(),
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_export_nle_xml_attach_export_report_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let write_output = input
        .get("writeOutput")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("writeOutput".to_string()))?
        .clone();
    let write_output: TemporalExportNleXmlWriteArtifactActivityOutput =
        serde_json::from_value(write_output)
            .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let created_at = required_workflow_string(&input, "createdAt")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    if write_output.output_path != format!("exports/{}", write_output.filename) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    if write_output.mime_type != "application/xml" {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "mimeType".to_string(),
        ));
    }
    let export = NleXmlExport {
        filename: write_output.filename.clone(),
        mime_type: write_output.mime_type.clone(),
        xml: String::new(),
    };
    let artifact = nle_xml_export_artifact(
        &export,
        write_output.format,
        &write_output.job_id,
        &created_at,
    );
    if artifact.path != write_output.output_path {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "artifact.path".to_string(),
        ));
    }
    let write = apply_project_actions_to_split_project(
        Path::new(&write_output.project_dir),
        vec![
            ProjectAction::UpdateJobStatus {
                job_id: write_output.job_id.clone(),
                status: JobStatus::Completed,
                updated_at: created_at,
                run_id,
            },
            ProjectAction::RecordExportArtifact { artifact },
        ],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(json!({
        "status": "attached",
        "projectId": write.project.id,
        "jobId": write_output.job_id,
        "format": write_output.format,
        "artifactPath": write_output.output_path,
        "writtenPath": write_output.written_path,
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

pub fn temporal_export_project_bundle_write_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let profile = required_workflow_string(&input, "profile")?;
    let output_path = required_workflow_string(&input, "outputPath")?;
    let created_at = required_workflow_string(&input, "createdAt")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let overwrite = optional_workflow_bool(&input, "overwrite")?.unwrap_or(true);

    if profile != "palmierProject" {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "profile".to_string(),
        ));
    }
    if Path::new(&output_path).extension().and_then(OsStr::to_str) != Some("palmier") {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    if !project_bundle_output_path_stays_under_exports(&output_path) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }

    let project_dir_path = Path::new(&project_dir);
    let _project_lease = acquire_split_project_mutation_lease(project_dir_path)
        .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
    let project = load_split_project(project_dir_path)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }

    let output_absolute = resolve_project_relative_path(project_dir_path, &output_path)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("outputPath".to_string()))?;
    write_project_bundle_package(project_dir_path, &output_absolute, &job_id, overwrite)?;

    let artifact = ProjectExportArtifact {
        schema_version: 1,
        id: job_id.clone(),
        kind: ProjectExportArtifactKind::ProjectBundle,
        format: "palmierProject".to_string(),
        path: output_path.clone(),
        mime_type: "application/vnd.video-creater.project".to_string(),
        job_id: Some(job_id.clone()),
        created_at: created_at.clone(),
    };
    let write = apply_project_actions_to_split_project(
        project_dir_path,
        vec![
            ProjectAction::UpdateJobStatus {
                job_id: job_id.clone(),
                status: JobStatus::Completed,
                updated_at: created_at,
                run_id,
            },
            ProjectAction::RecordExportArtifact { artifact },
        ],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(json!({
        "status": "exported",
        "projectId": write.project.id,
        "jobId": job_id,
        "profile": "palmierProject",
        "artifactPath": output_path,
        "writtenPath": output_absolute.display().to_string(),
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

pub fn temporal_export_media_write_artifact_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let profile_text = required_workflow_string(&input, "profile")?;
    let output_path = required_workflow_string(&input, "outputPath")?;
    let created_at = required_workflow_string(&input, "createdAt")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let overwrite = optional_workflow_bool(&input, "overwrite")?.unwrap_or(true);
    let validation = input
        .get("validation")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("validation".to_string()))?;
    let validation_output = input
        .get("validationOutput")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("validationOutput".to_string()))?;
    if validation_output.get("status").and_then(Value::as_str) != Some("validated") {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "validationOutput.status".to_string(),
        ));
    }
    let profile: ExportProfile = serde_json::from_value(json!(profile_text.clone()))
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let quality = input
        .get("quality")
        .map(|value| serde_json::from_value::<RenderQuality>(value.clone()))
        .transpose()
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?
        .unwrap_or(RenderQuality::Final);

    if profile == ExportProfile::PalmierProject {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "profile".to_string(),
        ));
    }
    if !export_profile_validation_matches(profile, validation) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "validation".to_string(),
        ));
    }
    if !project_bundle_output_path_stays_under_exports(&output_path) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    if Path::new(&output_path).extension().and_then(OsStr::to_str)
        != validation.get("extension").and_then(Value::as_str)
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    let project_dir_path = Path::new(&project_dir);
    let _project_lease = acquire_split_project_mutation_lease(project_dir_path)
        .map_err(TemporalWorkflowInputError::SaveSplitProject)?;
    let project = load_split_project(project_dir_path)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }

    let rendered_output_path = required_workflow_nested_string(
        validation_output,
        &["renderReport", "summary", "outputPath"],
        "validationOutput.renderReport.summary.outputPath",
    )?;
    let rendered_output_absolute =
        resolve_project_relative_path(project_dir_path, &rendered_output_path).map_err(|_| {
            TemporalWorkflowInputError::MismatchedInputField(
                "validationOutput.renderReport.summary.outputPath".to_string(),
            )
        })?;
    if !rendered_output_absolute.is_file() {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "validationOutput.renderReport.summary.outputPath".to_string(),
        ));
    }

    if let Some(destination) = temporal_export_destination::export_destination_from_input(&input)? {
        let written = temporal_export_destination::write_export_to_destination(
            project_dir_path,
            profile,
            &destination,
            &rendered_output_absolute,
            &job_id,
            &created_at,
            files_have_identical_bytes,
        )?;
        let artifact_path = written.artifact.path.clone();
        let write = apply_project_actions_to_split_project(
            project_dir_path,
            vec![
                ProjectAction::UpdateJobStatus {
                    job_id: job_id.clone(),
                    status: JobStatus::Completed,
                    updated_at: created_at,
                    run_id,
                },
                ProjectAction::RecordExportArtifact {
                    artifact: written.artifact,
                },
            ],
        )
        .map_err(|error| {
            let _ = fs::remove_file(&written.written_path);
            TemporalWorkflowInputError::ApplyProjectActions(error.to_string())
        })?;
        return Ok(json!({
            "status": "exported",
            "projectId": write.project.id,
            "jobId": job_id,
            "profile": profile_text,
            "artifactPath": artifact_path,
            "sourcePath": rendered_output_path,
            "writtenPath": written.written_path,
            "writtenBytes": written.written_bytes,
            "quality": quality,
            "materialization": {
                "kind": "noClobberLink",
                "byteIdentical": true,
            },
            "writtenFiles": write.report.written_files,
            "removedFiles": write.report.removed_files,
        }));
    }

    let output_absolute = resolve_project_relative_path(project_dir_path, &output_path)
        .map_err(|_| TemporalWorkflowInputError::MismatchedInputField("outputPath".to_string()))?;
    ensure_export_output_can_be_written(&output_absolute, overwrite)?;
    if let Some(parent) = output_absolute.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    }
    let output_name = output_absolute
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| {
            TemporalWorkflowInputError::MismatchedInputField("outputPath".to_string())
        })?;
    let staging_path = output_absolute.with_file_name(format!(
        ".{output_name}.{}.tmp",
        safe_workflow_segment(&job_id)
    ));
    if staging_path.exists() {
        remove_existing_export_output(&staging_path)?;
    }
    let copied_bytes = match fs::copy(&rendered_output_absolute, &staging_path) {
        Ok(copied_bytes) => copied_bytes,
        Err(error) => {
            let _ = fs::remove_file(&staging_path);
            return Err(TemporalWorkflowInputError::SaveSplitProject(
                error.to_string(),
            ));
        }
    };
    let staged_matches = match files_have_identical_bytes(&rendered_output_absolute, &staging_path)
    {
        Ok(matches) => matches,
        Err(error) => {
            let _ = fs::remove_file(&staging_path);
            return Err(error);
        }
    };
    if copied_bytes == 0 || !staged_matches {
        let _ = fs::remove_file(&staging_path);
        return Err(TemporalWorkflowInputError::Render(format!(
            "{profile_text} staged export did not match the validated render"
        )));
    }
    remove_existing_export_output(&output_absolute)?;
    if let Err(error) = fs::rename(&staging_path, &output_absolute) {
        let _ = fs::remove_file(&staging_path);
        return Err(TemporalWorkflowInputError::SaveSplitProject(
            error.to_string(),
        ));
    }
    let output_matches =
        match files_have_identical_bytes(&rendered_output_absolute, &output_absolute) {
            Ok(matches) => matches,
            Err(error) => {
                let _ = fs::remove_file(&output_absolute);
                return Err(error);
            }
        };
    if !output_matches {
        let _ = fs::remove_file(&output_absolute);
        return Err(TemporalWorkflowInputError::Render(format!(
            "{profile_text} export artifact did not match the validated render"
        )));
    }
    let output_size = fs::metadata(&output_absolute)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?
        .len();
    if output_size == 0 {
        return Err(TemporalWorkflowInputError::Render(format!(
            "{} materialized an empty export artifact",
            profile_text
        )));
    }

    let artifact = ProjectExportArtifact {
        schema_version: 1,
        id: job_id.clone(),
        kind: export_artifact_kind_for_profile(profile)?,
        format: profile_text.clone(),
        path: output_path.clone(),
        mime_type: required_workflow_nested_string(
            validation,
            &["mimeType"],
            "validation.mimeType",
        )?,
        job_id: Some(job_id.clone()),
        created_at: created_at.clone(),
    };
    let write = apply_project_actions_to_split_project(
        project_dir_path,
        vec![
            ProjectAction::UpdateJobStatus {
                job_id: job_id.clone(),
                status: JobStatus::Completed,
                updated_at: created_at,
                run_id,
            },
            ProjectAction::RecordExportArtifact { artifact },
        ],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(json!({
        "status": "exported",
        "projectId": write.project.id,
        "jobId": job_id,
        "profile": profile_text,
        "artifactPath": output_path,
        "sourcePath": rendered_output_path,
        "writtenPath": output_absolute.display().to_string(),
        "writtenBytes": output_size,
        "quality": quality,
        "materialization": {
            "kind": "atomicCopy",
            "byteIdentical": true,
        },
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

fn ensure_export_profile_quality_available(
    availability: &crate::project::export_profiles::ExportProfileAvailability,
    profile_text: &str,
    quality: RenderQuality,
) -> Result<(), TemporalWorkflowInputError> {
    if availability.quality_available(quality) {
        return Ok(());
    }

    let quality_name = match quality {
        RenderQuality::Draft => "Draft",
        RenderQuality::Final => "Final",
    };
    Err(TemporalWorkflowInputError::Render(format!(
        "{quality_name} {profile_text} export is unavailable: {}",
        availability
            .quality_unavailable_reason(quality)
            .or(availability.unavailable_reason.as_deref())
            .unwrap_or("approved encoder runtime is unavailable")
    )))
}

fn export_artifact_kind_for_profile(
    profile: ExportProfile,
) -> Result<ProjectExportArtifactKind, TemporalWorkflowInputError> {
    match profile {
        ExportProfile::Webm => Ok(ProjectExportArtifactKind::Webm),
        ExportProfile::Mp4H264 | ExportProfile::Mp4H265 => Ok(ProjectExportArtifactKind::Mp4),
        ExportProfile::ProResMov => Ok(ProjectExportArtifactKind::Mov),
        ExportProfile::PalmierProject => Err(TemporalWorkflowInputError::MismatchedInputField(
            "profile".to_string(),
        )),
    }
}

pub fn temporal_export_project_bundle_attach_export_report_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let write_output = input
        .get("writeOutput")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("writeOutput".to_string()))?;
    if write_output.get("status").and_then(Value::as_str) != Some("exported") {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "writeOutput.status".to_string(),
        ));
    }
    if write_output.get("profile").and_then(Value::as_str) != Some("palmierProject") {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "writeOutput.profile".to_string(),
        ));
    }
    let project_id = required_workflow_string(write_output, "projectId")?;
    let job_id = required_workflow_string(write_output, "jobId")?;
    let artifact_path = required_workflow_string(write_output, "artifactPath")?;
    let written_path = required_workflow_string(write_output, "writtenPath")?;

    Ok(json!({
        "status": "attached",
        "projectId": project_id,
        "jobId": job_id,
        "profile": "palmierProject",
        "artifactPath": artifact_path,
        "writtenPath": written_path,
        "writtenFiles": write_output.get("writtenFiles").cloned().unwrap_or_else(|| json!([])),
        "removedFiles": write_output.get("removedFiles").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub fn temporal_export_media_attach_export_report_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let write_output = input
        .get("writeOutput")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("writeOutput".to_string()))?;
    if write_output.get("status").and_then(Value::as_str) != Some("exported") {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "writeOutput.status".to_string(),
        ));
    }
    let profile_text = required_workflow_nested_string(
        &input,
        &["writeOutput", "profile"],
        "writeOutput.profile",
    )?;
    let profile: ExportProfile = serde_json::from_value(json!(profile_text.clone()))
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    if profile == ExportProfile::PalmierProject {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "writeOutput.profile".to_string(),
        ));
    }
    let project_id = required_workflow_nested_string(
        &input,
        &["writeOutput", "projectId"],
        "writeOutput.projectId",
    )?;
    let job_id =
        required_workflow_nested_string(&input, &["writeOutput", "jobId"], "writeOutput.jobId")?;
    let artifact_path = required_workflow_nested_string(
        &input,
        &["writeOutput", "artifactPath"],
        "writeOutput.artifactPath",
    )?;
    let written_path = required_workflow_nested_string(
        &input,
        &["writeOutput", "writtenPath"],
        "writeOutput.writtenPath",
    )?;

    Ok(json!({
        "status": "attached",
        "projectId": project_id,
        "jobId": job_id,
        "profile": profile_text,
        "artifactPath": artifact_path,
        "writtenPath": written_path,
        "writtenFiles": write_output.get("writtenFiles").cloned().unwrap_or_else(|| json!([])),
        "removedFiles": write_output.get("removedFiles").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub fn temporal_export_media_workflow_activity_plan_value(
    input: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let profile = required_workflow_string(&input, "profile")?;
    let output_path = required_workflow_string(&input, "outputPath")?;
    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "updatedAt".to_string(),
        ));
    }
    let run_id = run_id
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .map(str::to_string);

    let validation = input
        .get("validation")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("validation".to_string()))?;
    let profile_value: ExportProfile = serde_json::from_value(json!(profile.clone()))
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let overwrite = optional_workflow_bool(&input, "overwrite")?.unwrap_or(true);

    if profile_value == ExportProfile::PalmierProject {
        if Path::new(&output_path).extension().and_then(OsStr::to_str) != Some("palmier") {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "outputPath".to_string(),
            ));
        }
        if !project_bundle_output_path_stays_under_exports(&output_path) {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "outputPath".to_string(),
            ));
        }
        if !export_profile_validation_matches(profile_value, validation) {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "validation".to_string(),
            ));
        }

        let mut write_input = json!({
            "projectId": project_id,
            "projectDir": project_dir,
            "jobId": job_id,
            "profile": "palmierProject",
            "outputPath": output_path,
            "overwrite": overwrite,
            "createdAt": updated_at,
        });
        if let Some(run_id) = run_id.as_deref() {
            if let Some(object) = write_input.as_object_mut() {
                object.insert("runId".to_string(), json!(run_id));
            }
        }

        return Ok(json!({
            "status": "planned",
            "workflowType": TemporalWorkflowKind::ExportMedia.workflow_type(),
            "profile": "palmierProject",
            "activityTypes": ["WriteExportArtifact", "AttachExportReport"],
            "writeExportArtifactInput": write_input,
            "attachExportReportInputFrom": "WriteExportArtifact",
            "updatedAt": updated_at,
            "runId": run_id,
        }));
    }

    if !project_bundle_output_path_stays_under_exports(&output_path) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    if !export_profile_validation_matches(profile_value, validation) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "validation".to_string(),
        ));
    }
    if Path::new(&output_path).extension().and_then(OsStr::to_str)
        != validation.get("extension").and_then(Value::as_str)
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    let options = temporal_render_options_from_input(&input)?;
    if options.profile != profile_value {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "profile".to_string(),
        ));
    }
    let destination = temporal_export_destination::export_destination_from_input(&input)?;
    if let Some(destination) = &destination {
        let extension = validation
            .get("extension")
            .and_then(Value::as_str)
            .unwrap_or_default();
        crate::project::export_destination::validate_export_output_request(destination, extension)
            .map_err(|error| TemporalWorkflowInputError::Render(error.to_string()))?;
    }
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == profile_value)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("profile".to_string()))?;
    ensure_export_profile_quality_available(&availability, &profile, options.quality)?;

    let mut render_media_input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": options.profile,
        "quality": options.quality,
        "width": options.width,
        "height": options.height,
        "updatedAt": updated_at,
        "runId": run_id,
    });
    if run_id.is_none() {
        if let Some(object) = render_media_input.as_object_mut() {
            object.remove("runId");
        }
    }
    let mut write_export_artifact_base_input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": options.profile,
        "quality": options.quality,
        "width": options.width,
        "height": options.height,
        "outputPath": output_path,
        "overwrite": overwrite,
        "createdAt": updated_at,
        "runId": run_id,
        "validation": validation,
    });
    if run_id.is_none() {
        if let Some(object) = write_export_artifact_base_input.as_object_mut() {
            object.remove("runId");
        }
    }
    if let Some(destination) = &destination {
        write_export_artifact_base_input["destination"] = json!(destination);
    }
    insert_export_encode_settings(&mut render_media_input, &options);
    insert_export_encode_settings(&mut write_export_artifact_base_input, &options);
    let mut build_render_plan_input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": options.profile,
        "quality": options.quality,
        "width": options.width,
        "height": options.height
    });
    insert_export_encode_settings(&mut build_render_plan_input, &options);
    let mut validate_rendered_media_input = json!({
        "profile": profile,
        "quality": options.quality,
        "width": options.width,
        "height": options.height,
        "outputPath": output_path,
        "validation": validation
    });
    insert_export_encode_settings(&mut validate_rendered_media_input, &options);

    Ok(json!({
        "status": "planned",
        "workflowType": TemporalWorkflowKind::ExportMedia.workflow_type(),
        "profile": profile,
        "activityTypes": [
            "BuildRenderPlan",
            "ValidateExportProfile",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
            "WriteExportArtifact",
            "AttachExportReport"
        ],
        "buildRenderPlanInput": build_render_plan_input,
        "validateExportProfileInput": {
            "projectId": project_id,
            "jobId": job_id,
            "profile": profile,
            "quality": options.quality,
            "width": options.width,
            "height": options.height,
            "outputPath": output_path,
            "validation": validation
        },
        "renderMediaInput": render_media_input,
        "renderMediaInputFrom": "BuildRenderPlan",
        "validateRenderedMediaInput": validate_rendered_media_input,
        "validateRenderedMediaInputFrom": "RenderMedia",
        "attachRenderReportInputFrom": "ValidateRenderedMedia",
        "writeExportArtifactBaseInput": write_export_artifact_base_input,
        "updatedAt": updated_at,
        "runId": run_id,
    }))
}

fn export_profile_validation_matches(profile: ExportProfile, validation: &Value) -> bool {
    *validation == export_profile_validation_payload(profile)
}

fn write_project_bundle_package(
    project_dir: &Path,
    output_path: &Path,
    job_id: &str,
    overwrite: bool,
) -> Result<(), TemporalWorkflowInputError> {
    let exports_dir = project_dir.join("exports");
    fs::create_dir_all(&exports_dir)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    ensure_export_output_can_be_written(output_path, overwrite)?;
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    // Only recorded project-package exports are omitted. Authored folders and loose
    // media/export artifacts retain their contents, including folders ending in .palmier.
    let excluded_bundles: Vec<_> = project
        .export_artifacts
        .iter()
        .filter(|artifact| artifact.kind == ProjectExportArtifactKind::ProjectBundle)
        .filter_map(|artifact| {
            let mut components = Path::new(&artifact.path).components();
            if components.next() != Some(std::path::Component::Normal(OsStr::new("exports")))
                || components.clone().count() == 0
                || !components.all(|component| matches!(component, std::path::Component::Normal(_)))
            {
                return None;
            }
            let path = project_dir.join(&artifact.path);
            path.is_dir().then_some(path)
        })
        .collect();
    // Prepare the complete replacement before altering an existing user export.
    let staging = tempfile::Builder::new()
        .prefix(&format!(
            ".{}-project-bundle-",
            safe_workflow_segment(job_id)
        ))
        .tempdir_in(&exports_dir)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    copy_project_bundle_contents(
        project_dir,
        staging.path(),
        staging.path(),
        output_path,
        &excluded_bundles,
    )?;
    write_palmier_compatibility_package_files(project_dir, staging.path())?;
    publish_project_bundle(staging.path(), output_path, overwrite)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))
}

fn publish_project_bundle(staging: &Path, output: &Path, overwrite: bool) -> std::io::Result<()> {
    // Exchanging directories publishes a replacement atomically; the old bundle is then
    // owned by the staging guard and removed only after successful publication.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::ffi::OsStrExt;
        let source = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
        let destination = std::ffi::CString::new(output.as_os_str().as_bytes())?;
        let replacing = std::fs::symlink_metadata(output).is_ok();
        let result = unsafe {
            #[cfg(target_os = "linux")]
            {
                libc::renameat2(
                    libc::AT_FDCWD,
                    source.as_ptr(),
                    libc::AT_FDCWD,
                    destination.as_ptr(),
                    if replacing && overwrite {
                        libc::RENAME_EXCHANGE
                    } else {
                        libc::RENAME_NOREPLACE
                    },
                )
            }
            #[cfg(target_os = "macos")]
            {
                libc::renamex_np(
                    source.as_ptr(),
                    destination.as_ptr(),
                    if replacing && overwrite {
                        libc::RENAME_SWAP
                    } else {
                        libc::RENAME_EXCL
                    },
                )
            }
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (staging, output, overwrite);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic project bundle publication is unavailable on this platform",
        ))
    }
}

fn write_palmier_compatibility_package_files(
    project_dir: &Path,
    package_dir: &Path,
) -> Result<(), TemporalWorkflowInputError> {
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    write_project_bundle_json_file(
        &package_dir.join("project.json"),
        &split_timeline_from_project(&project),
    )?;
    write_project_bundle_json_file(
        &package_dir.join("media.json"),
        &palmier_media_manifest_from_project(&project),
    )?;
    write_project_bundle_json_file(
        &package_dir.join("generation-log.json"),
        &palmier_generation_log_from_project(&project),
    )
}

fn palmier_generation_log_from_project(project: &VideoProject) -> Value {
    let entries = project
        .generated_assets
        .iter()
        .map(palmier_generation_log_entry_from_generated_asset)
        .collect::<Vec<_>>();
    json!({
        "version": 1,
        "entries": entries,
    })
}

fn palmier_generation_log_entry_from_generated_asset(asset: &GeneratedAsset) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("id".to_string(), json!(&asset.id));
    entry.insert("model".to_string(), json!(&asset.model.id));
    if !asset.created_at.trim().is_empty() {
        entry.insert("createdAt".to_string(), json!(&asset.created_at));
    }
    Value::Object(entry)
}

fn palmier_media_manifest_from_project(project: &VideoProject) -> Value {
    let entries = project
        .media
        .iter()
        .map(|asset| {
            let mut entry = serde_json::Map::new();
            entry.insert("id".to_string(), json!(&asset.id));
            entry.insert(
                "name".to_string(),
                json!(asset.name.as_deref().unwrap_or(asset.id.as_str())),
            );
            entry.insert("type".to_string(), json!(palmier_clip_type(&asset.kind)));
            entry.insert(
                "source".to_string(),
                json!({ "project": { "relativePath": &asset.relative_path } }),
            );
            entry.insert("duration".to_string(), json!(asset.duration_seconds));
            if let Some(width) = asset.width {
                entry.insert("sourceWidth".to_string(), json!(width));
            }
            if let Some(height) = asset.height {
                entry.insert("sourceHeight".to_string(), json!(height));
            }
            if let Some(fps) = asset.fps {
                entry.insert("sourceFPS".to_string(), json!(fps));
            }
            if let Some(folder_id) = &asset.folder_id {
                entry.insert("folderId".to_string(), json!(folder_id));
            }
            if let Some(generation_input) = palmier_generation_input_for_media(project, &asset.id) {
                entry.insert("generationInput".to_string(), generation_input);
            }
            Value::Object(entry)
        })
        .collect::<Vec<_>>();

    json!({
        "version": 2,
        "entries": entries,
        "folders": palmier_media_folders_from_project(project),
    })
}

fn palmier_generation_input_for_media(project: &VideoProject, media_id: &str) -> Option<Value> {
    project.generated_assets.iter().find_map(|asset| {
        asset
            .outputs
            .iter()
            .enumerate()
            .find(|(_, output)| output.media_id == media_id)
            .map(|(output_index, output)| {
                let mut input = serde_json::Map::new();
                input.insert("prompt".to_string(), json!(&asset.prompt));
                input.insert("model".to_string(), json!(&asset.model.id));
                input.insert(
                    "duration".to_string(),
                    json!(palmier_generation_input_duration(asset, output)),
                );
                input.insert(
                    "aspectRatio".to_string(),
                    json!(asset.settings.aspect_ratio.as_deref().unwrap_or_default()),
                );
                if let Some(resolution) = asset.settings.resolution.as_deref() {
                    input.insert("resolution".to_string(), json!(resolution));
                }
                if let Some(quality) = asset.settings.quality.as_deref() {
                    input.insert("quality".to_string(), json!(quality));
                }
                if let Some(num_images) = asset.settings.num_images {
                    input.insert("numImages".to_string(), json!(num_images));
                }
                if let Some(voice) = asset.settings.voice.as_deref() {
                    input.insert("voice".to_string(), json!(voice));
                }
                if let Some(lyrics) = asset.settings.lyrics.as_deref() {
                    input.insert("lyrics".to_string(), json!(lyrics));
                }
                if let Some(style) = asset.settings.style_instructions.as_deref() {
                    input.insert("styleInstructions".to_string(), json!(style));
                }
                if let Some(instrumental) = asset.settings.instrumental {
                    input.insert("instrumental".to_string(), json!(instrumental));
                }
                if let Some(generate_audio) = asset.settings.generate_audio {
                    input.insert("generateAudio".to_string(), json!(generate_audio));
                }
                if !asset.created_at.trim().is_empty() {
                    input.insert("createdAt".to_string(), json!(&asset.created_at));
                }
                input.insert("outputIndex".to_string(), json!(output_index));
                let result_urls = asset
                    .outputs
                    .iter()
                    .filter_map(|output| output.source_url.as_deref())
                    .filter(|url| !url.trim().is_empty())
                    .collect::<Vec<_>>();
                if !result_urls.is_empty() {
                    input.insert("resultURLs".to_string(), json!(result_urls));
                }
                if !asset.references.reference_image_media_refs.is_empty() {
                    input.insert(
                        "referenceImageAssetIds".to_string(),
                        json!(&asset.references.reference_image_media_refs),
                    );
                }
                let reference_video_asset_ids = asset
                    .references
                    .source_video_media_ref
                    .iter()
                    .chain(asset.references.reference_video_media_refs.iter())
                    .collect::<Vec<_>>();
                if !reference_video_asset_ids.is_empty() {
                    input.insert(
                        "referenceVideoAssetIds".to_string(),
                        json!(reference_video_asset_ids),
                    );
                }
                if !asset.references.reference_audio_media_refs.is_empty() {
                    input.insert(
                        "referenceAudioAssetIds".to_string(),
                        json!(&asset.references.reference_audio_media_refs),
                    );
                }
                Value::Object(input)
            })
    })
}

fn palmier_generation_input_duration(
    asset: &GeneratedAsset,
    output: &crate::project::model::GeneratedAssetOutput,
) -> u64 {
    let duration = asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(output.duration_seconds);
    if duration.is_finite() && duration > 0.0 {
        duration.round() as u64
    } else {
        0
    }
}

fn palmier_media_folders_from_project(project: &VideoProject) -> Vec<Value> {
    project
        .media_folders
        .iter()
        .map(|folder| {
            let mut value = serde_json::Map::new();
            value.insert("id".to_string(), json!(&folder.id));
            value.insert("name".to_string(), json!(&folder.name));
            if let Some(parent_id) = &folder.parent_id {
                value.insert("parentFolderId".to_string(), json!(parent_id));
            }
            Value::Object(value)
        })
        .collect()
}

fn palmier_clip_type(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
        MediaKind::Lottie => "lottie",
        MediaKind::Video | MediaKind::Generated => "video",
    }
}

fn write_project_bundle_json_file<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), TemporalWorkflowInputError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    fs::write(path, bytes)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))
}

fn copy_project_bundle_contents(
    source_dir: &Path,
    destination_dir: &Path,
    staging_dir: &Path,
    final_output_dir: &Path,
    excluded_bundles: &[PathBuf],
) -> Result<(), TemporalWorkflowInputError> {
    for entry in fs::read_dir(source_dir)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?
    {
        let entry = entry
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
        let source_path = entry.path();
        if path_is_or_is_inside(&source_path, staging_dir)
            || path_is_or_is_inside(&source_path, final_output_dir)
            || excluded_bundles
                .iter()
                .any(|path| path_is_or_is_inside(&source_path, path))
        {
            continue;
        }

        let destination_path = destination_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
        if file_type.is_dir() {
            fs::create_dir_all(&destination_path)
                .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
            copy_project_bundle_contents(
                &source_path,
                &destination_path,
                staging_dir,
                final_output_dir,
                excluded_bundles,
            )?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
        }
    }
    Ok(())
}

fn path_is_or_is_inside(path: &Path, parent: &Path) -> bool {
    path == parent || path.starts_with(parent)
}

fn project_bundle_output_path_stays_under_exports(value: &str) -> bool {
    let path = Path::new(value);
    let mut components = path.components();
    let starts_with_exports = matches!(
        components.next(),
        Some(std::path::Component::Normal(component)) if component == "exports"
    );

    starts_with_exports && components.next().is_some()
}

pub fn temporal_render_options_from_input(
    input: &Value,
) -> Result<ExportRenderOptions, TemporalWorkflowInputError> {
    if input.get("quality").is_some()
        || input.get("width").is_some()
        || input.get("height").is_some()
    {
        let profile: ExportProfile = input
            .get("profile")
            .ok_or_else(|| TemporalWorkflowInputError::MissingField("profile".to_string()))
            .and_then(|value| {
                serde_json::from_value(value.clone()).map_err(|error| {
                    TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
                })
            })?;
        let quality: RenderQuality = input
            .get("quality")
            .ok_or_else(|| TemporalWorkflowInputError::MissingField("quality".to_string()))
            .and_then(|value| {
                serde_json::from_value(value.clone()).map_err(|error| {
                    TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
                })
            })?;
        let width = required_workflow_u32(input, "width")?;
        let height = required_workflow_u32(input, "height")?;
        let fps =
            match input.get("fps") {
                None | Some(Value::Null) => None,
                Some(value) => Some(value.as_f64().ok_or_else(|| {
                    TemporalWorkflowInputError::InvalidFieldType("fps".to_string())
                })?),
            };
        let encode_tier: crate::edit::render_plan::ExportEncodeTier = match input.get("encodeTier")
        {
            None | Some(Value::Null) => Default::default(),
            Some(value) => serde_json::from_value(value.clone()).map_err(|error| {
                TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
            })?,
        };
        return ExportRenderOptions::new(profile, quality, width, height)
            .and_then(|options| options.with_fps(fps))
            .and_then(|options| options.with_encode_tier(encode_tier))
            .map_err(|error| TemporalWorkflowInputError::Render(error.to_string()));
    }

    Err(TemporalWorkflowInputError::MissingField(
        "quality".to_string(),
    ))
}

pub fn temporal_render_build_plan_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_id = required_workflow_string(&input, "projectId")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let explicit_options = (input.get("quality").is_some()
        || input.get("width").is_some()
        || input.get("height").is_some())
    .then(|| temporal_render_options_from_input(&input))
    .transpose()?;
    let profile = if explicit_options.is_none() {
        Some(
            input
                .get("profile")
                .ok_or_else(|| TemporalWorkflowInputError::MissingField("profile".to_string()))
                .and_then(|profile| {
                    serde_json::from_value(profile.clone()).map_err(|error| {
                        TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
                    })
                })?,
        )
    } else {
        None
    };
    let export_profile: Option<ExportProfile> = input
        .get("exportProfile")
        .map(|profile| {
            serde_json::from_value(profile.clone())
                .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
        })
        .transpose()?;
    let timeline_id = optional_workflow_string(&input, "timelineId")?;
    let project = load_split_project(Path::new(&project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    if project.id != project_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "projectId".to_string(),
        ));
    }
    let project = match timeline_id.as_deref() {
        Some(timeline_id) => project.projected_for_timeline(timeline_id).ok_or_else(|| {
            TemporalWorkflowInputError::MismatchedInputField("timelineId".to_string())
        })?,
        None => project,
    };
    let render_plan = if let Some(options) = explicit_options {
        build_project_media_render_plan_with_options(
            Path::new(&project_dir),
            &project,
            &job_id,
            options,
        )
    } else {
        match export_profile {
            Some(export_profile) => build_project_media_render_plan(
                Path::new(&project_dir),
                &project,
                &job_id,
                crate::edit::render_plan::render_quality_from_legacy(
                    profile.clone().expect("legacy render profile"),
                ),
                export_profile,
            ),
            None => build_project_webm_render_plan(
                Path::new(&project_dir),
                &project,
                &job_id,
                profile.clone().expect("legacy render profile"),
            ),
        }
    }
    .map_err(|errors| TemporalWorkflowInputError::Render(pipeline_errors_message(errors)))?;

    serde_json::to_value(TemporalRenderBuildPlanActivityOutput {
        status: "built".to_string(),
        project_id,
        project_dir,
        job_id,
        profile,
        export_profile: explicit_options
            .map(|options| options.profile)
            .or(export_profile),
        options: explicit_options,
        render_plan,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_render_workflow_activity_plan(
    kind: TemporalWorkflowKind,
    input: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalRenderWorkflowActivityPlan, TemporalWorkflowInputError> {
    if kind != TemporalWorkflowKind::RenderDraft {
        return Err(TemporalWorkflowInputError::Render(format!(
            "{} is not supported by the WebM render workflow yet",
            kind.workflow_type()
        )));
    }

    let project_id = required_workflow_string(&input, "projectId")?;
    let project_dir = required_workflow_string(&input, "projectDir")?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let profile: RenderQualityProfile = input
        .get("profile")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("profile".to_string()))
        .and_then(|profile| {
            serde_json::from_value(profile.clone())
                .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
        })?;
    let timeline_id = optional_workflow_string(&input, "timelineId")?;
    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "updatedAt".to_string(),
        ));
    }
    let run_id = run_id
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .map(str::to_string);

    let mut build_render_plan_input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": profile,
    });
    if let Some(timeline_id) = timeline_id {
        build_render_plan_input["timelineId"] = json!(timeline_id);
    }

    Ok(TemporalRenderWorkflowActivityPlan {
        status: "planned".to_string(),
        workflow_type: kind.workflow_type().to_string(),
        activity_types: kind
            .activity_types()
            .iter()
            .map(|activity| (*activity).to_string())
            .collect(),
        build_render_plan_input,
        render_media_input_from: "BuildRenderPlan".to_string(),
        validate_rendered_media_input_from: "RenderMedia".to_string(),
        attach_render_report_input_from: "ValidateRenderedMedia".to_string(),
        updated_at: updated_at.to_string(),
        run_id,
    })
}

pub fn temporal_render_media_activity_input_value(
    build_output: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let build: TemporalRenderBuildPlanActivityOutput = serde_json::from_value(build_output)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "updatedAt".to_string(),
        ));
    }
    let run_id = run_id
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .map(str::to_string);

    serde_json::to_value(TemporalRenderMediaActivityInput {
        build,
        updated_at: updated_at.to_string(),
        run_id,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_render_media_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let input: TemporalRenderMediaActivityInput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let build = input.build;
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        &build.project_id,
        &build.job_id,
        JobStatus::Queued,
        &input.updated_at,
    );
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = input.run_id.clone();
    }
    let result = if let Some(options) = build.options {
        // Only the export workflow sends render options; its WriteExportArtifact step completes
        // the job once the export file exists.
        let result = render_export_workflow_media_to_split_project_folder(
            Path::new(&build.project_dir),
            &build.project_id,
            options,
            job,
            &input.updated_at,
            input.run_id,
        )
        .map_err(|errors| TemporalWorkflowInputError::Render(pipeline_errors_message(errors)))?;
        let mut render_report = serde_json::to_value(result.render_report)
            .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
        if let Some(summary) = render_report
            .get_mut("summary")
            .and_then(Value::as_object_mut)
        {
            summary.insert("quality".to_string(), json!(options.quality));
            summary.insert("requestedWidth".to_string(), json!(options.width));
            summary.insert("requestedHeight".to_string(), json!(options.height));
            summary.insert("actualWidth".to_string(), json!(build.render_plan.width));
            summary.insert("actualHeight".to_string(), json!(build.render_plan.height));
        }
        json!({
            "project": result.project,
            "projectRenderReport": result.project_render_report,
            "renderReport": render_report,
            "outputPath": result.output_path,
        })
    } else if build.export_profile.is_some() {
        return Err(TemporalWorkflowInputError::MissingField(
            "quality,width,height".to_string(),
        ));
    } else {
        let result = render_webm_to_split_project_folder(
            Path::new(&build.project_dir),
            &build.project_id,
            build
                .profile
                .ok_or_else(|| TemporalWorkflowInputError::MissingField("profile".to_string()))?,
            job,
            &input.updated_at,
            input.run_id,
            None,
        )
        .map_err(|errors| TemporalWorkflowInputError::Render(pipeline_errors_message(errors)))?;
        json!({
            "project": result.project,
            "projectRenderReport": result.project_render_report,
            "renderReport": result.render_report,
            "outputPath": result.output_path,
        })
    };

    Ok(json!({
        "status": "rendered",
        "projectId": result["project"]["id"].clone(),
        "jobId": result["projectRenderReport"]["id"].clone(),
        "outputPath": result["outputPath"].clone(),
        "renderReport": result["renderReport"].clone(),
        "projectRenderReport": result["projectRenderReport"].clone(),
    }))
}

pub fn temporal_validate_rendered_media_activity_input_value(
    render_output: Value,
    validation_input: Option<&Value>,
) -> Result<Value, TemporalWorkflowInputError> {
    let Some(validation_input) = validation_input else {
        return Ok(render_output);
    };
    let mut input = validation_input.clone();
    let object = input.as_object_mut().ok_or_else(|| {
        TemporalWorkflowInputError::InvalidFieldType("validateRenderedMediaInput".to_string())
    })?;
    object.insert(
        "renderReport".to_string(),
        render_output
            .get("renderReport")
            .ok_or_else(|| TemporalWorkflowInputError::MissingField("renderReport".to_string()))?
            .clone(),
    );
    object.insert(
        "projectRenderReport".to_string(),
        render_output
            .get("projectRenderReport")
            .ok_or_else(|| {
                TemporalWorkflowInputError::MissingField("projectRenderReport".to_string())
            })?
            .clone(),
    );
    if let Some(output_path) = render_output.get("outputPath") {
        object.insert("renderedOutputPath".to_string(), output_path.clone());
    }

    Ok(input)
}

pub fn temporal_validate_rendered_media_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    input
        .get("renderReport")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("renderReport".to_string()))?;
    input.get("projectRenderReport").ok_or_else(|| {
        TemporalWorkflowInputError::MissingField("projectRenderReport".to_string())
    })?;
    let mut export_metadata = None;
    if input.get("profile").is_some()
        || input.get("validation").is_some()
        || input.get("outputPath").is_some()
    {
        let profile_text = required_workflow_string(&input, "profile")?;
        let profile: ExportProfile = serde_json::from_value(json!(profile_text.clone()))
            .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
        let validation = input
            .get("validation")
            .ok_or_else(|| TemporalWorkflowInputError::MissingField("validation".to_string()))?;
        if !export_profile_validation_matches(profile, validation) {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "validation".to_string(),
            ));
        }
        let expected_extension = validation
            .get("extension")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                TemporalWorkflowInputError::MissingField("validation.extension".to_string())
            })?;
        let requested_output_path = required_workflow_string(&input, "outputPath")?;
        if Path::new(&requested_output_path)
            .extension()
            .and_then(OsStr::to_str)
            != Some(expected_extension)
        {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "outputPath".to_string(),
            ));
        }
        let rendered_output_path = required_workflow_nested_string(
            &input,
            &["renderReport", "summary", "outputPath"],
            "renderReport.summary.outputPath",
        )?;
        if Path::new(&rendered_output_path)
            .extension()
            .and_then(OsStr::to_str)
            != Some(expected_extension)
        {
            return Err(TemporalWorkflowInputError::MismatchedInputField(
                "renderReport.summary.outputPath".to_string(),
            ));
        }
        if let Some(rendered_output_path) = input.get("renderedOutputPath").and_then(Value::as_str)
        {
            if Path::new(rendered_output_path)
                .extension()
                .and_then(OsStr::to_str)
                != Some(expected_extension)
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "renderedOutputPath".to_string(),
                ));
            }
        }
        if input.get("quality").is_some()
            || input.get("width").is_some()
            || input.get("height").is_some()
        {
            let options = temporal_render_options_from_input(&input)?;
            let summary = input.pointer("/renderReport/summary").ok_or_else(|| {
                TemporalWorkflowInputError::MissingField("renderReport.summary".to_string())
            })?;
            let actual_width = required_workflow_u32(summary, "actualWidth")?;
            let actual_height = required_workflow_u32(summary, "actualHeight")?;
            if summary.get("quality") != Some(&json!(options.quality))
                || summary.get("requestedWidth") != Some(&json!(options.width))
                || summary.get("requestedHeight") != Some(&json!(options.height))
                || actual_width != options.width
                || actual_height != options.height
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "renderReport.summary.exportOptions".to_string(),
                ));
            }
            if summary.get("container") != validation.get("container")
                || summary.get("videoCodec") != validation.get("videoCodec")
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "renderReport.summary.exportProfile".to_string(),
                ));
            }
            if input
                .pointer("/projectRenderReport/streams/video")
                .and_then(Value::as_bool)
                != Some(true)
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "projectRenderReport.streams.video".to_string(),
                ));
            }
            if input
                .pointer("/projectRenderReport/streams/audio")
                .and_then(Value::as_bool)
                == Some(true)
                && summary.get("audioCodec") != validation.get("audioCodec")
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "renderReport.summary.audioCodec".to_string(),
                ));
            }
            export_metadata = Some((options, actual_width, actual_height));
        }
    }

    let mut output = json!({
        "status": "validated",
        "renderReport": input["renderReport"].clone(),
        "projectRenderReport": input["projectRenderReport"].clone(),
    });
    if let Some((options, actual_width, actual_height)) = export_metadata {
        let object = output.as_object_mut().expect("validation output object");
        object.insert("quality".to_string(), json!(options.quality));
        object.insert("requestedWidth".to_string(), json!(options.width));
        object.insert("requestedHeight".to_string(), json!(options.height));
        object.insert("actualWidth".to_string(), json!(actual_width));
        object.insert("actualHeight".to_string(), json!(actual_height));
    }
    Ok(output)
}

pub fn temporal_attach_render_report_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let project_report = input.get("projectRenderReport").ok_or_else(|| {
        TemporalWorkflowInputError::MissingField("projectRenderReport".to_string())
    })?;

    let mut output = json!({
        "status": "attached",
        "projectRenderReport": project_report,
    });
    for field in [
        "quality",
        "requestedWidth",
        "requestedHeight",
        "actualWidth",
        "actualHeight",
    ] {
        if let Some(value) = input.get(field) {
            output[field] = value.clone();
        }
    }
    Ok(output)
}

fn validate_nle_xml_activity_output(
    output: &TemporalExportNleXmlBuildActivityOutput,
) -> Result<(), TemporalWorkflowInputError> {
    if output.project_id.trim().is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "projectId".to_string(),
        ));
    }
    if output.job_id.trim().is_empty() {
        return Err(TemporalWorkflowInputError::BlankField("jobId".to_string()));
    }
    if output.mime_type != "application/xml" {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "mimeType".to_string(),
        ));
    }
    if output.output_path != format!("exports/{}", output.filename) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "outputPath".to_string(),
        ));
    }
    match output.format {
        NleXmlFormat::PremiereXmeml => {
            if !output.filename.ends_with("-premiere.xml")
                || !output.xml.contains("<xmeml version=\"5\">")
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "xml".to_string(),
                ));
            }
        }
        NleXmlFormat::DavinciFcpxml => {
            if !output.filename.ends_with("-davinci.fcpxml")
                || !output.xml.contains("<fcpxml version=\"1.10\">")
            {
                return Err(TemporalWorkflowInputError::MismatchedInputField(
                    "xml".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn pipeline_errors_message(errors: Vec<crate::render_pipeline::error::PipelineError>) -> String {
    errors
        .into_iter()
        .map(|error| format!("{}: {}", error.path, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn temporal_codex_edit_validate_project_actions_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let (project_dir, request, proposal) = temporal_codex_edit_request_and_proposal(&input)?;
    let project = load_split_project(Path::new(&project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let edl = validate_codex_edit_proposal(&project, &request, &proposal)
        .map_err(|error| TemporalWorkflowInputError::CodexProposal(error.to_string()))?;
    let actions = materialize_codex_edit_proposal_actions(&project, &request, &proposal)
        .map_err(|error| TemporalWorkflowInputError::CodexProposal(error.to_string()))?;

    Ok(json!({
        "status": "validated",
        "projectId": project.id,
        "mediaId": proposal.media_id,
        "clipCount": proposal.clips.len(),
        "projectActionCount": actions.len(),
        "durationSeconds": edl.duration_seconds(),
    }))
}

pub fn temporal_codex_edit_persist_accepted_proposal_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let (project_dir, request, proposal) = temporal_codex_edit_request_and_proposal(&input)?;
    let job_id = required_workflow_string(&input, "jobId")?;
    let updated_at = required_workflow_string(&input, "updatedAt")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let project = load_split_project(Path::new(&project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let mut actions = materialize_codex_edit_proposal_actions(&project, &request, &proposal)
        .map_err(|error| TemporalWorkflowInputError::CodexProposal(error.to_string()))?;

    let project_action_count = actions.len();
    actions.push(ProjectAction::UpdateJobStatus {
        job_id: job_id.clone(),
        status: JobStatus::Completed,
        updated_at,
        run_id,
    });
    let write = apply_project_actions_to_split_project(Path::new(&project_dir), actions)
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(json!({
        "status": "persisted",
        "projectId": write.project.id,
        "jobId": job_id,
        "projectActionCount": project_action_count,
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

pub fn temporal_codex_edit_attach_failure_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_start_request_from_runtime_input(&input)?;
    validate_temporal_start_request(&start_request, TemporalWorkflowKind::CodexEdit)?;
    let project_dir = required_workflow_string(&start_request.input, "projectDir")?;
    let job_id = required_workflow_string(&start_request.input, "jobId")?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let updated_at = required_workflow_string(&input, "updatedAt")?;
    let error = optional_workflow_string(&input, "error")?;
    let project = load_split_project(Path::new(&project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
    let actions = temporal_codex_edit_failure_actions(job, run_id.as_deref(), &updated_at)
        .map_err(|error| TemporalWorkflowInputError::FailureActions(error.to_string()))?;
    let write = apply_project_actions_to_split_project(Path::new(&project_dir), actions)
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(json!({
        "status": "failed",
        "projectId": write.project.id,
        "jobId": job_id,
        "runId": run_id,
        "error": error,
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

fn temporal_codex_edit_request_and_proposal(
    input: &Value,
) -> Result<(String, EditJobRequest, CodexEditProposal), TemporalWorkflowInputError> {
    let project_dir = required_workflow_string(input, "projectDir")?;
    let request: EditJobRequest = input
        .get("request")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("request".to_string()))
        .and_then(|request| {
            serde_json::from_value(request.clone())
                .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
        })?;
    let proposal: CodexEditProposal = input
        .get("proposal")
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("proposal".to_string()))
        .and_then(|proposal| {
            serde_json::from_value(proposal.clone())
                .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
        })?;

    Ok((project_dir, request, proposal))
}

pub fn temporal_generate_media_run_provider_activity_input_value(
    runtime_input: &Value,
    submission: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_generate_media_start_request_from_runtime_input(runtime_input)?;
    let submission: TemporalGenerateMediaProviderSubmission = serde_json::from_value(submission)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let input = TemporalGenerateMediaRunProviderActivityInput {
        start_request,
        submission,
        updated_at: updated_at.to_string(),
        run_id: run_id.map(str::to_string),
        options: TemporalGenerateMediaProviderRunOptions::default(),
    };

    serde_json::to_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

pub fn temporal_generate_media_fal_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &FalQueueSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
    options: FalGenerationRunOptions,
) -> Result<FalGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;

    temporal_generate_media_fal_run_submission_with_client(
        client,
        project_dir,
        &project,
        start_request,
        submission,
        FalWorkflowExecution {
            credential,
            updated_at,
            run_id,
            options,
        },
    )
}

pub fn temporal_generate_media_attach_generated_asset_result_to_project_dir(
    start_request: &TemporalWorkflowStartRequest,
    mut actions: Vec<ProjectAction>,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    append_timeline_audio_insert_action(&project, asset, &mut actions);
    append_timeline_visual_insert_action(&project, asset, &mut actions);

    apply_project_actions_to_split_project(project_dir, actions)
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))
}

pub fn temporal_generate_media_attach_generated_asset_failure_to_project_dir(
    start_request: &TemporalWorkflowStartRequest,
    run_id: Option<&str>,
    updated_at: &str,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == input.job_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
    let actions = temporal_generate_media_failure_actions(job, &input.asset_id, run_id, updated_at)
        .map_err(|error| TemporalWorkflowInputError::FailureActions(error.to_string()))?;

    apply_project_actions_to_split_project(project_dir, actions)
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))
}

pub fn temporal_generate_media_attach_failure_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let start_request = temporal_generate_media_start_request_from_runtime_input(&input)?;
    let run_id = optional_workflow_string(&input, "runId")?;
    let updated_at = required_workflow_string(&input, "updatedAt")?;
    let workflow_input = temporal_generate_media_workflow_input(&start_request)?;
    let write = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
        &start_request,
        run_id.as_deref(),
        &updated_at,
    )?;

    Ok(json!({
        "projectId": write.project.id,
        "jobId": workflow_input.job_id,
        "assetId": workflow_input.asset_id,
        "status": "failed",
        "runId": run_id,
        "writtenFiles": write.report.written_files,
        "removedFiles": write.report.removed_files,
    }))
}

#[derive(Debug)]
pub struct TemporalGenerateMediaFalRunAndAttachResult {
    pub run: FalGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaReplicateRunAndAttachResult {
    pub run: ReplicateGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaElevenLabsRunAndAttachResult {
    pub run: ElevenLabsGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaMinimaxRunAndAttachResult {
    pub run: MinimaxGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaGoogleRunAndAttachResult {
    pub run: GoogleVeoGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaGoogleGeminiTtsRunAndAttachResult {
    pub run: GoogleGeminiTtsGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaGoogleLyriaRunAndAttachResult {
    pub run: GoogleLyriaGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaOpenAiRunAndAttachResult {
    pub run: OpenAiImageGenerationRun,
    pub write: ProjectActionWriteResult,
}

#[derive(Debug)]
pub struct TemporalGenerateMediaXAiRunAndAttachResult {
    pub run: XAiImageGenerationRun,
    pub write: ProjectActionWriteResult,
}

pub fn temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &FalQueueSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
    options: FalGenerationRunOptions,
) -> Result<TemporalGenerateMediaFalRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_fal_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
        options,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaFalRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_replicate_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
    options: ReplicateGenerationRunOptions,
) -> Result<ReplicateGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_replicate_prediction_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.version != expected_submission.version
        || submission.method != expected_submission.method
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });
    let job_id = input.job_id.clone();

    run_replicate_generation_submission_with_client_after_submit(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
        options,
        |submit_response| {
            apply_project_actions_to_split_project(
                project_dir,
                vec![ProjectAction::UpdateJobProviderRequest {
                    job_id: job_id.clone(),
                    provider_request: JobProviderRequest {
                        provider: REPLICATE_PROVIDER.to_string(),
                        request_id: submit_response.id.clone(),
                        status_url: submit_response.urls.get.clone(),
                        response_url: submit_response.urls.get.clone(),
                        cancel_url: submit_response.urls.cancel.clone(),
                        submitted_at: updated_at.to_string(),
                    },
                }],
            )
            .map(|_| ())
            .map_err(|error| ReplicateGenerationWorkerError::SubmitHookFailed {
                message: error.to_string(),
            })
        },
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
}

pub fn temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
    options: ReplicateGenerationRunOptions,
) -> Result<TemporalGenerateMediaReplicateRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_replicate_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
        options,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaReplicateRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_elevenlabs_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<ElevenLabsGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_elevenlabs_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.method != expected_submission.method
        || submission.url != expected_submission.url
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_elevenlabs_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: ELEVENLABS_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_elevenlabs_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaElevenLabsRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_elevenlabs_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaElevenLabsRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_minimax_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<MinimaxGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_minimax_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.api_model != expected_submission.api_model
        || submission.method != expected_submission.method
        || submission.url != expected_submission.url
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_minimax_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: MINIMAX_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_minimax_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaMinimaxRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_minimax_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaMinimaxRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_google_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<GoogleVeoGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_google_veo_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.api_model != expected_submission.api_model
        || submission.method != expected_submission.method
        || submission.url != expected_submission.url
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_google_veo_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: GOOGLE_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_google_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaGoogleRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_google_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaGoogleRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_google_gemini_tts_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleGeminiTtsGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<GoogleGeminiTtsGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_google_gemini_tts_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.method != expected_submission.method
        || submission.url != expected_submission.url
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_google_gemini_tts_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: GOOGLE_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_google_gemini_tts_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleGeminiTtsGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaGoogleGeminiTtsRunAndAttachResult, TemporalWorkflowInputError> {
    let run =
        temporal_generate_media_google_gemini_tts_run_submission_from_project_dir_with_client(
            client,
            start_request,
            submission,
            credential,
            updated_at,
            run_id,
        )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaGoogleGeminiTtsRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_google_lyria_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleLyriaGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<GoogleLyriaGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_google_lyria_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.method != expected_submission.method
        || submission.url != expected_submission.url
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_google_lyria_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: GOOGLE_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_google_lyria_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &GoogleLyriaGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaGoogleLyriaRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_google_lyria_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaGoogleLyriaRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_openai_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<OpenAiImageGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_openai_image_generation_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.method != expected_submission.method
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = run_openai_image_generation_submission_with_client(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: OPENAI_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaOpenAiRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_openai_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaOpenAiRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_xai_run_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<XAiImageGenerationRun, TemporalWorkflowInputError> {
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let project_dir = Path::new(&input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let is_xai_video = asset.model.id == XAI_GROK_VIDEO_MODEL_ID;
    let expected_submission = if is_xai_video {
        build_xai_video_generation_submission(asset)
    } else {
        build_xai_image_generation_submission(asset)
    }
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.model != expected_submission.model
        || submission.method != expected_submission.method
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    let run = if is_xai_video {
        run_xai_video_generation_submission_with_client(
            client,
            GenerationTarget::new(
                project_dir,
                asset,
                updated_at,
                run_id,
                placement_replacement_item_id,
            ),
            submission,
            credential,
            60,
            Duration::from_secs(5),
        )
    } else {
        run_xai_image_generation_submission_with_client(
            client,
            GenerationTarget::new(
                project_dir,
                asset,
                updated_at,
                run_id,
                placement_replacement_item_id,
            ),
            submission,
            credential,
        )
    }
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;

    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request: JobProviderRequest {
                provider: XAI_PROVIDER.to_string(),
                request_id: run.request_id.clone(),
                status_url: submission.url.clone(),
                response_url: submission.url.clone(),
                cancel_url: submission.url.clone(),
                submitted_at: updated_at.to_string(),
            },
        }],
    )
    .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;

    Ok(run)
}

pub fn temporal_generate_media_xai_run_and_attach_submission_from_project_dir_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<TemporalGenerateMediaXAiRunAndAttachResult, TemporalWorkflowInputError> {
    let run = temporal_generate_media_xai_run_submission_from_project_dir_with_client(
        client,
        start_request,
        submission,
        credential,
        updated_at,
        run_id,
    )?;
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        run.completion.actions.clone(),
    )?;

    Ok(TemporalGenerateMediaXAiRunAndAttachResult { run, write })
}

pub fn temporal_generate_media_run_provider_activity_with_client(
    client: &reqwest::blocking::Client,
    input: &TemporalGenerateMediaRunProviderActivityInput,
    credential: &str,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    let workflow_input = temporal_generate_media_workflow_input(&input.start_request)?;
    validate_generation_start_request_against_current_catalog(&input.start_request)?;
    match &input.submission {
        TemporalGenerateMediaProviderSubmission::Fal(submission) => {
            let result =
                temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                    input.options.into(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::Fal(result.run.status.status),
                queue_position: result.run.status.queue_position,
                logs: result.run.status.logs,
                metrics: result.run.status.metrics,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::ElevenLabs(submission) => {
            let result =
                temporal_generate_media_elevenlabs_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::ElevenLabs(result.run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::Minimax(submission) => {
            let result =
                temporal_generate_media_minimax_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::Minimax(result.run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::Google(submission) => {
            let result =
                temporal_generate_media_google_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::Google(result.run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::GoogleGeminiTts(submission) => {
            let result =
                temporal_generate_media_google_gemini_tts_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::GoogleGeminiTts(
                    result.run.status,
                ),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::GoogleLyria(submission) => {
            let result =
                temporal_generate_media_google_lyria_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::GoogleLyria(
                    result.run.status,
                ),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::Replicate(submission) => {
            let result =
                temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client(
            client,
            &input.start_request,
                    submission,
            credential,
            &input.updated_at,
            input.run_id.as_deref(),
            input.options.into(),
        )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.prediction_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::Replicate(
                    result.run.status.status,
                ),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::OpenAi(submission) => {
            let result =
                temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::OpenAi(result.run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
        TemporalGenerateMediaProviderSubmission::XAi(submission) => {
            let result =
                temporal_generate_media_xai_run_and_attach_submission_from_project_dir_with_client(
                    client,
                    &input.start_request,
                    submission,
                    credential,
                    &input.updated_at,
                    input.run_id.as_deref(),
                )?;
            let job = result
                .write
                .project
                .jobs
                .iter()
                .find(|job| job.id == workflow_input.job_id)
                .ok_or_else(|| {
                    TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
                })?;
            let job_status = job.status.clone();
            let run_id = job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| input.run_id.clone());

            Ok(TemporalGenerateMediaRunProviderActivityOutput {
                request_id: result.run.request_id,
                project_id: result.write.project.id,
                asset_id: workflow_input.asset_id,
                job_status,
                provider_status: TemporalGenerateMediaProviderStatus::XAi(result.run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                run_id,
                output_path: result.run.completion.output_path.display().to_string(),
                written_files: result.write.report.written_files,
                removed_files: result.write.report.removed_files,
            })
        }
    }
}

struct CancellableProviderRun {
    request_id: String,
    provider_status: TemporalGenerateMediaProviderStatus,
    queue_position: Option<u64>,
    logs: Vec<FalQueueLog>,
    metrics: Option<FalQueueMetrics>,
    output_path: PathBuf,
    completion_actions: Vec<ProjectAction>,
}

fn generation_cancelled_if_requested(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), TemporalWorkflowInputError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(TemporalWorkflowInputError::GenerationCancelled)
    } else {
        Ok(())
    }
}

fn remove_cancelled_generation_outputs(project_dir: &Path, actions: &[ProjectAction]) {
    for action in actions {
        if let ProjectAction::CompleteGeneratedAsset { outputs, .. } = action {
            for output in outputs {
                if let Ok(path) = resolve_project_relative_path(project_dir, &output.relative_path)
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

fn provider_request_for_completed_run(
    provider: &str,
    request_id: &str,
    url: &str,
    updated_at: &str,
) -> JobProviderRequest {
    JobProviderRequest {
        provider: provider.to_string(),
        request_id: request_id.to_string(),
        status_url: url.to_string(),
        response_url: url.to_string(),
        cancel_url: url.to_string(),
        submitted_at: updated_at.to_string(),
    }
}

fn persist_completed_provider_request(
    project_dir: &Path,
    job_id: &str,
    provider_request: JobProviderRequest,
    completion_actions: &[ProjectAction],
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), TemporalWorkflowInputError> {
    match apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::UpdateJobProviderRequest {
            job_id: job_id.to_string(),
            provider_request,
        }],
    ) {
        Ok(_) => Ok(()),
        Err(_) if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) => {
            remove_cancelled_generation_outputs(project_dir, completion_actions);
            Err(TemporalWorkflowInputError::GenerationCancelled)
        }
        Err(error) => Err(TemporalWorkflowInputError::ApplyProjectActions(
            error.to_string(),
        )),
    }
}

fn provider_submission_matches_expected(
    actual: &TemporalGenerateMediaProviderSubmission,
    expected: &TemporalGenerateMediaProviderSubmission,
) -> bool {
    match (actual, expected) {
        (
            TemporalGenerateMediaProviderSubmission::Fal(actual),
            TemporalGenerateMediaProviderSubmission::Fal(expected),
        ) => {
            actual.endpoint == expected.endpoint
                && actual.method == expected.method
                && actual.input == expected.input
                && actual.provider == expected.provider
        }
        (
            TemporalGenerateMediaProviderSubmission::Replicate(actual),
            TemporalGenerateMediaProviderSubmission::Replicate(expected),
        ) => {
            actual.version == expected.version
                && actual.method == expected.method
                && actual.input == expected.input
                && actual.provider == expected.provider
        }
        _ => actual == expected,
    }
}

/// Cancellation-aware in-process provider activity. Provider I/O observes the
/// supplied token, and the token's completion CAS is acquired before any
/// completion actions are attached to canonical project state.
pub fn temporal_generate_media_run_provider_activity_with_client_cancellable(
    client: &reqwest::blocking::Client,
    input: &TemporalGenerateMediaRunProviderActivityInput,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    generation_cancelled_if_requested(cancellation)?;
    let workflow_input = temporal_generate_media_workflow_input(&input.start_request)?;
    validate_generation_start_request_against_current_catalog(&input.start_request)?;
    let expected =
        temporal_generate_media_provider_submission_from_project_dir(&input.start_request)?;
    if !provider_submission_matches_expected(&input.submission, &expected) {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let project_dir = Path::new(&workflow_input.project_dir);
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == workflow_input.asset_id)
        .ok_or_else(|| {
            TemporalWorkflowInputError::MissingGeneratedAsset(workflow_input.asset_id.clone())
        })?;
    let replacement_item_id = workflow_input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });
    let updated_at = input.updated_at.as_str();
    let run_id = input.run_id.as_deref();
    let job_id = workflow_input.job_id.clone();
    let target = GenerationTarget::new(project_dir, asset, updated_at, run_id, replacement_item_id);

    let run = match &input.submission {
        TemporalGenerateMediaProviderSubmission::Fal(submission) => {
            let run = run_fal_generation_submission_with_client_after_submit_cancellable(
                client,
                target,
                submission,
                credential,
                input.options.into(),
                cancellation,
                |submitted| {
                    apply_project_actions_to_split_project(
                        project_dir,
                        vec![ProjectAction::UpdateJobProviderRequest {
                            job_id: job_id.clone(),
                            provider_request: JobProviderRequest {
                                provider: FAL_PROVIDER.to_string(),
                                request_id: submitted.request_id.clone(),
                                status_url: submitted.status_url.clone(),
                                response_url: submitted.response_url.clone(),
                                cancel_url: submitted.cancel_url.clone(),
                                submitted_at: updated_at.to_string(),
                            },
                        }],
                    )
                    .map(|_| ())
                    .map_err(|error| {
                        FalGenerationWorkerError::SubmitHookFailed {
                            message: error.to_string(),
                        }
                    })
                },
            )
            .map_err(|error| match error {
                FalGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            CancellableProviderRun {
                request_id: run.request_id,
                provider_status: TemporalGenerateMediaProviderStatus::Fal(run.status.status),
                queue_position: run.status.queue_position,
                logs: run.status.logs,
                metrics: run.status.metrics,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::Replicate(submission) => {
            let run = run_replicate_generation_submission_with_client_after_submit_cancellable(
                client,
                target,
                submission,
                credential,
                input.options.into(),
                cancellation,
                |submitted| {
                    apply_project_actions_to_split_project(
                        project_dir,
                        vec![ProjectAction::UpdateJobProviderRequest {
                            job_id: job_id.clone(),
                            provider_request: JobProviderRequest {
                                provider: REPLICATE_PROVIDER.to_string(),
                                request_id: submitted.id.clone(),
                                status_url: submitted.urls.get.clone(),
                                response_url: submitted.urls.get.clone(),
                                cancel_url: submitted.urls.cancel.clone(),
                                submitted_at: updated_at.to_string(),
                            },
                        }],
                    )
                    .map(|_| ())
                    .map_err(|error| {
                        ReplicateGenerationWorkerError::SubmitHookFailed {
                            message: error.to_string(),
                        }
                    })
                },
            )
            .map_err(|error| match error {
                ReplicateGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            CancellableProviderRun {
                request_id: run.prediction_id,
                provider_status: TemporalGenerateMediaProviderStatus::Replicate(run.status.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::ElevenLabs(submission) => {
            let run = run_elevenlabs_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                ElevenLabsGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    ELEVENLABS_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::ElevenLabs(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::Minimax(submission) => {
            let run = run_minimax_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                MinimaxGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    MINIMAX_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::Minimax(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::Google(submission) => {
            let run = run_google_veo_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                GoogleGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    GOOGLE_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::Google(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::GoogleGeminiTts(submission) => {
            let run = run_google_gemini_tts_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                GoogleGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    GOOGLE_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::GoogleGeminiTts(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::GoogleLyria(submission) => {
            let run = run_google_lyria_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                GoogleGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    GOOGLE_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::GoogleLyria(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::OpenAi(submission) => {
            let run = run_openai_image_generation_submission_with_client_cancellable(
                client,
                target,
                submission,
                credential,
                cancellation,
            )
            .map_err(|error| match error {
                OpenAiGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    OPENAI_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::OpenAi(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
        TemporalGenerateMediaProviderSubmission::XAi(submission) => {
            let provider_run = if asset.model.id == XAI_GROK_VIDEO_MODEL_ID {
                let progress = JobProgressReporter::new(project_dir, &job_id);
                let run = run_xai_video_generation_submission_with_client_reporting(
                    client,
                    target,
                    submission,
                    credential,
                    input.options.max_status_polls,
                    Duration::from_millis(input.options.poll_interval_millis),
                    cancellation,
                    Some(&progress),
                );
                progress.clear();
                run
            } else {
                run_xai_image_generation_submission_with_client_cancellable(
                    client,
                    target,
                    submission,
                    credential,
                    cancellation,
                )
            };
            let run = provider_run.map_err(|error| match error {
                XAiGenerationWorkerError::Cancelled => {
                    TemporalWorkflowInputError::GenerationCancelled
                }
                other => TemporalWorkflowInputError::FalGenerationRun(other.to_string()),
            })?;
            let request_id = run.request_id.clone();
            persist_completed_provider_request(
                project_dir,
                &job_id,
                provider_request_for_completed_run(
                    XAI_PROVIDER,
                    &request_id,
                    &submission.url,
                    updated_at,
                ),
                &run.completion.actions,
                cancellation,
            )?;
            CancellableProviderRun {
                request_id,
                provider_status: TemporalGenerateMediaProviderStatus::XAi(run.status),
                queue_position: None,
                logs: Vec::new(),
                metrics: None,
                output_path: run.completion.output_path,
                completion_actions: run.completion.actions,
            }
        }
    };

    if cancellation.is_some_and(|token| !token.begin_completion()) {
        remove_cancelled_generation_outputs(project_dir, &run.completion_actions);
        return Err(TemporalWorkflowInputError::GenerationCancelled);
    }
    let write = temporal_generate_media_attach_generated_asset_result_to_project_dir(
        &input.start_request,
        run.completion_actions,
    )?;
    if let Some(token) = cancellation {
        token.mark_terminal();
    }
    let job = write
        .project
        .jobs
        .iter()
        .find(|job| job.id == workflow_input.job_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
    let durable_run_id = job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.clone())
        .or_else(|| input.run_id.clone());

    Ok(TemporalGenerateMediaRunProviderActivityOutput {
        request_id: run.request_id,
        project_id: write.project.id,
        asset_id: workflow_input.asset_id,
        job_status: job.status.clone(),
        provider_status: run.provider_status,
        queue_position: run.queue_position,
        logs: run.logs,
        metrics: run.metrics,
        run_id: durable_run_id,
        output_path: run.output_path.display().to_string(),
        written_files: write.report.written_files,
        removed_files: write.report.removed_files,
    })
}

pub fn temporal_generate_media_run_provider_activity_value_with_client(
    client: &reqwest::blocking::Client,
    input: Value,
    credential: &str,
) -> Result<Value, TemporalWorkflowInputError> {
    let input: TemporalGenerateMediaRunProviderActivityInput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    let output =
        temporal_generate_media_run_provider_activity_with_client(client, &input, credential)?;

    serde_json::to_value(output)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

/// Runs the GenerateMedia activity chain in the current process.
///
/// The caller is expected to execute this blocking function away from an async
/// runtime thread. Provider credentials are resolved here and are never part of
/// the returned value. Any failure is durably attached to the project before
/// the error is returned, matching the Temporal workflow's failure branch.
pub fn run_generate_media_in_process_with_client(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    run_id: Option<&str>,
    options: TemporalGenerateMediaProviderRunOptions,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    let workflow_input = temporal_generate_media_workflow_input(start_request)?;
    let credential = if workflow_input.mock_mode {
        String::new()
    } else {
        let project = load_split_project(Path::new(&workflow_input.project_dir))
            .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
        let provider = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == workflow_input.asset_id)
            .map(|asset| asset.model.provider.trim())
            .filter(|provider| !provider.is_empty())
            .ok_or_else(|| {
                TemporalWorkflowInputError::MissingGeneratedAsset(workflow_input.asset_id.clone())
            })?;
        match resolve_provider_credential(provider) {
            Ok(credential) => credential.into_secret(),
            Err(error) => {
                let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                    start_request,
                    run_id,
                    updated_at,
                );
                return Err(TemporalWorkflowInputError::FalGenerationRun(
                    error.to_string(),
                ));
            }
        }
    };
    run_generate_media_in_process_with_client_and_credential(
        client,
        start_request,
        updated_at,
        run_id,
        options,
        &credential,
    )
}

/// Runs the in-process GenerateMedia chain with a credential supplied by the
/// trusted desktop boundary (for example, macOS Keychain). The credential is
/// used only for provider I/O and is never serialized into workflow input,
/// project state, logs, or the returned activity output.
pub fn run_generate_media_in_process_with_client_and_credential(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    run_id: Option<&str>,
    options: TemporalGenerateMediaProviderRunOptions,
    credential: &str,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    run_generate_media_in_process_with_client_and_credential_cancellable(
        client,
        start_request,
        updated_at,
        run_id,
        options,
        credential,
        None,
    )
}

/// Cancellation-aware variant used by the desktop in-process runner.
pub fn run_generate_media_in_process_with_client_and_credential_cancellable(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    run_id: Option<&str>,
    options: TemporalGenerateMediaProviderRunOptions,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    generation_cancelled_if_requested(cancellation)?;
    let workflow_input = temporal_generate_media_workflow_input(start_request)?;
    if workflow_input.mock_mode {
        let project_dir = Path::new(&workflow_input.project_dir);
        let project = load_split_project(project_dir)
            .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
        let actions = temporal_generate_media_mock_completion_actions(
            &project,
            start_request,
            updated_at,
            None,
        )?;
        let output_path = completed_generated_output_for_actions(&actions)
            .map(|(_, _, path)| path)
            .ok_or_else(|| {
                TemporalWorkflowInputError::MissingGeneratedAsset(workflow_input.asset_id.clone())
            })?;
        let write = apply_project_actions_to_split_project(project_dir, actions)
            .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;
        let job = write
            .project
            .jobs
            .iter()
            .find(|job| job.id == workflow_input.job_id)
            .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
        return Ok(TemporalGenerateMediaRunProviderActivityOutput {
            request_id: format!("mock-{}", workflow_input.asset_id),
            project_id: write.project.id.clone(),
            asset_id: workflow_input.asset_id,
            job_status: job.status.clone(),
            provider_status: TemporalGenerateMediaProviderStatus::Fal(
                FalQueueStatusKind::Completed,
            ),
            queue_position: None,
            logs: Vec::new(),
            metrics: None,
            run_id: job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone())
                .or_else(|| run_id.map(str::to_string)),
            output_path,
            written_files: write.report.written_files,
            removed_files: write.report.removed_files,
        });
    }
    if credential.trim().is_empty() {
        if !cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
            let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                start_request,
                run_id,
                updated_at,
            );
        }
        return Err(TemporalWorkflowInputError::FalGenerationRun(
            "provider credential cannot be empty".to_string(),
        ));
    }
    run_generate_media_in_process_with_submission_builder_cancellable(
        client,
        start_request,
        GenerateMediaRunContext {
            updated_at,
            run_id,
            options,
            credential,
            cancellation,
        },
        || temporal_generate_media_provider_submission_from_project_dir(start_request),
    )
}

pub fn resume_interrupted_generate_media_job_with_client_and_credential_cancellable(
    client: &reqwest::blocking::Client,
    project_dir: &Path,
    job_id: &str,
    updated_at: &str,
    credential: &str,
    options: TemporalGenerateMediaProviderRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<ProjectActionWriteResult, TemporalWorkflowInputError> {
    generation_cancelled_if_requested(cancellation)?;
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("jobId".to_string()))?;
    if job.kind != TemporalWorkflowKind::GenerateMedia.job_kind()
        || !matches!(
            job.status,
            JobStatus::Queued | JobStatus::Running | JobStatus::Progress
        )
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobStatus".to_string(),
        ));
    }
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("job.startRequest".to_string()))?;
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.job_id != job.id || Path::new(&input.project_dir) != project_dir {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "job.startRequest".to_string(),
        ));
    }
    let provider_request = job.provider_request.as_ref().ok_or_else(|| {
        TemporalWorkflowInputError::MissingField("job.providerRequest".to_string())
    })?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    if asset.model.provider.trim() != provider_request.provider.trim() {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "job.providerRequest.provider".to_string(),
        ));
    }
    let replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });
    let run_id = job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref());
    let target = GenerationTarget::new(project_dir, asset, updated_at, run_id, replacement_item_id);

    let completion_actions = match provider_request.provider.as_str() {
        FAL_PROVIDER => {
            validate_restart_recovery_url(FAL_PROVIDER, &provider_request.status_url)?;
            validate_restart_recovery_url(FAL_PROVIDER, &provider_request.response_url)?;
            resume_fal_generation_request_with_client_cancellable(
                client,
                target,
                FalResumeRequest {
                    request_id: &provider_request.request_id,
                    status_url: &provider_request.status_url,
                    response_url: &provider_request.response_url,
                },
                credential,
                options.into(),
                cancellation,
            )
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?
            .completion
            .actions
        }
        REPLICATE_PROVIDER => {
            validate_restart_recovery_url(REPLICATE_PROVIDER, &provider_request.status_url)?;
            resume_replicate_generation_request_with_client_cancellable(
                client,
                target,
                &provider_request.request_id,
                &provider_request.status_url,
                credential,
                options.into(),
                cancellation,
            )
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?
            .completion
            .actions
        }
        provider => {
            return Err(TemporalWorkflowInputError::FalGenerationRun(format!(
                "provider restart recovery is unsupported for {provider}; the persisted request will not be resubmitted"
            )))
        }
    };

    generation_cancelled_if_requested(cancellation)?;
    temporal_generate_media_attach_generated_asset_result_to_project_dir(
        start_request,
        completion_actions,
    )
}

fn validate_restart_recovery_url(
    provider: &str,
    value: &str,
) -> Result<(), TemporalWorkflowInputError> {
    let url = Url::parse(value).map_err(|_| {
        TemporalWorkflowInputError::MismatchedInputField("job.providerRequest.url".to_string())
    })?;
    let host = url.host_str().unwrap_or_default();
    let configured_host = match provider {
        FAL_PROVIDER => std::env::var(VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR).ok(),
        REPLICATE_PROVIDER => std::env::var(VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR).ok(),
        _ => None,
    }
    .and_then(|base| Url::parse(&base).ok())
    .and_then(|base| base.host_str().map(str::to_string));
    let provider_host = match provider {
        FAL_PROVIDER => host == "fal.run" || host.ends_with(".fal.run"),
        REPLICATE_PROVIDER => host == "api.replicate.com",
        _ => false,
    };
    let configured = configured_host.as_deref() == Some(host);
    #[cfg(test)]
    let test_loopback = url.scheme() == "http" && matches!(host, "127.0.0.1" | "localhost");
    #[cfg(not(test))]
    let test_loopback = false;
    if (url.scheme() == "https" && (provider_host || configured)) || test_loopback {
        Ok(())
    } else {
        Err(TemporalWorkflowInputError::MismatchedInputField(
            "job.providerRequest.url".to_string(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptedGenerationResumeCandidate {
    pub job_id: String,
    pub provider: String,
}

static APP_SESSION_STARTED_AT: OnceLock<chrono::DateTime<chrono::Utc>> = OnceLock::new();

/// When this app process started. `run` records it at startup; project-open recovery treats a
/// generation that was already queued before then as left behind by an earlier session.
pub fn app_session_started_at() -> chrono::DateTime<chrono::Utc> {
    *APP_SESSION_STARTED_AT.get_or_init(chrono::Utc::now)
}

/// The run id prefix the desktop process gives generations it runs itself. Any other run id belongs
/// to a Temporal workflow, which keeps running in the worker and is reconciled by polling.
const IN_PROCESS_GENERATION_RUN_ID_PREFIX: &str = "in-process/";

/// Whether an unfinished generation job has no runner left: it is not registered in this process,
/// and it either ran in process or was still waiting for its start before this app session began.
fn generation_job_left_by_earlier_session(project_id: &str, job: &JobSummary) -> bool {
    if crate::generation::cancel::is_generation_job_active(project_id, &job.id) {
        return false;
    }
    match job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref())
    {
        Some(run_id) => run_id.starts_with(IN_PROCESS_GENERATION_RUN_ID_PREFIX),
        // A queued job recorded in this session may still be started by the editor.
        None if job.status == JobStatus::Queued => {
            chrono::DateTime::parse_from_rfc3339(&job.updated_at).map_or(true, |recorded_at| {
                recorded_at.with_timezone(&chrono::Utc) < app_session_started_at()
            })
        }
        None => true,
    }
}

#[derive(Debug)]
pub struct InterruptedGenerationRecoveryReport {
    pub project: VideoProject,
    pub resume_candidates: Vec<InterruptedGenerationResumeCandidate>,
    pub failed_job_ids: Vec<String>,
}

pub fn reconcile_interrupted_generation_jobs_on_project_open(
    project_dir: &Path,
    updated_at: &str,
) -> Result<InterruptedGenerationRecoveryReport, TemporalWorkflowInputError> {
    let project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let mut resume_candidates = Vec::new();
    let mut failed_job_ids = Vec::new();
    let mut failure_actions = Vec::new();

    for job in project.jobs.iter().filter(|job| {
        job.kind == TemporalWorkflowKind::GenerateMedia.job_kind()
            && matches!(
                job.status,
                JobStatus::Queued | JobStatus::Running | JobStatus::Progress
            )
            && generation_job_left_by_earlier_session(&project.id, job)
    }) {
        let input = job
            .start_request
            .as_ref()
            .and_then(|request| temporal_generate_media_workflow_input(request).ok());
        let asset_id = input
            .as_ref()
            .map(|input| input.asset_id.as_str())
            .unwrap_or(job.id.as_str());
        let asset = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == asset_id);
        let provider_request = job.provider_request.as_ref();
        let resumable = input.as_ref().is_some_and(|input| {
            input.job_id == job.id
                && Path::new(&input.project_dir) == project_dir
                && input.project_id == project.id
        }) && asset.is_some_and(|asset| {
            matches!(
                asset.status,
                GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
            ) && provider_request.is_some_and(|request| {
                request.provider == asset.model.provider
                    && matches!(request.provider.as_str(), FAL_PROVIDER | REPLICATE_PROVIDER)
            })
        });

        if resumable {
            let provider = provider_request
                .expect("resumable provider request")
                .provider
                .clone();
            resume_candidates.push(InterruptedGenerationResumeCandidate {
                job_id: job.id.clone(),
                provider,
            });
            continue;
        }

        failed_job_ids.push(job.id.clone());
        failure_actions.push(ProjectAction::UpdateJobStatus {
            job_id: job.id.clone(),
            status: JobStatus::Failed,
            updated_at: updated_at.to_string(),
            run_id: job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.clone()),
        });
        if asset.is_some_and(|asset| {
            matches!(
                asset.status,
                GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
            )
        }) {
            failure_actions.push(ProjectAction::UpdateGeneratedAssetStatus {
                asset_id: asset_id.to_string(),
                status: GeneratedAssetStatus::Failed,
            });
        }
    }

    let project = if failure_actions.is_empty() {
        project
    } else {
        apply_project_actions_to_split_project(project_dir, failure_actions)
            .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?
            .project
    };

    Ok(InterruptedGenerationRecoveryReport {
        project,
        resume_candidates,
        failed_job_ids,
    })
}

pub fn run_generate_media_in_process_with_submission_builder(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    run_id: Option<&str>,
    options: TemporalGenerateMediaProviderRunOptions,
    credential: &str,
    build_submission: impl FnOnce() -> Result<
        TemporalGenerateMediaProviderSubmission,
        TemporalWorkflowInputError,
    >,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    run_generate_media_in_process_with_submission_builder_cancellable(
        client,
        start_request,
        GenerateMediaRunContext {
            updated_at,
            run_id,
            options,
            credential,
            cancellation: None,
        },
        build_submission,
    )
}

pub struct GenerateMediaRunContext<'a> {
    pub updated_at: &'a str,
    pub run_id: Option<&'a str>,
    pub options: TemporalGenerateMediaProviderRunOptions,
    pub credential: &'a str,
    pub cancellation: Option<&'a GenerationCancellationToken>,
}

pub fn run_generate_media_in_process_with_submission_builder_cancellable(
    client: &reqwest::blocking::Client,
    start_request: &TemporalWorkflowStartRequest,
    context: GenerateMediaRunContext<'_>,
    build_submission: impl FnOnce() -> Result<
        TemporalGenerateMediaProviderSubmission,
        TemporalWorkflowInputError,
    >,
) -> Result<TemporalGenerateMediaRunProviderActivityOutput, TemporalWorkflowInputError> {
    let GenerateMediaRunContext {
        updated_at,
        run_id,
        options,
        credential,
        cancellation,
    } = context;
    let run = || {
        generation_cancelled_if_requested(cancellation)?;
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_credential(
            start_request,
            client,
            credential,
            &SystemProcessRunner,
        )?;
        generation_cancelled_if_requested(cancellation)?;
        let submission = build_submission()?;
        generation_cancelled_if_requested(cancellation)?;
        temporal_generate_media_run_provider_activity_with_client_cancellable(
            client,
            &TemporalGenerateMediaRunProviderActivityInput {
                start_request: start_request.clone(),
                submission,
                updated_at: updated_at.to_string(),
                run_id: run_id.map(str::to_string),
                options,
            },
            credential,
            cancellation,
        )
    };

    match run() {
        Ok(output) => Ok(output),
        Err(error) => {
            if matches!(error, TemporalWorkflowInputError::GenerationCancelled)
                || cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
            {
                return Err(TemporalWorkflowInputError::GenerationCancelled);
            }
            // Best effort: preserve the original provider/setup error while
            // ensuring the project is terminal and retryable whenever possible.
            let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                start_request,
                run_id,
                updated_at,
            );
            Err(error)
        }
    }
}

pub fn temporal_generate_media_cancel_provider_activity_with_client(
    client: &reqwest::blocking::Client,
    cancel_url: &str,
    credential: &str,
) -> Result<TemporalGenerateMediaCancelProviderActivityOutput, TemporalWorkflowInputError> {
    let cancel_url = cancel_url.trim();
    if cancel_url.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "cancelUrl".to_string(),
        ));
    }
    let status = if is_replicate_cancel_url(cancel_url) {
        cancel_replicate_prediction_with_client(client, cancel_url, credential)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?;
        FalQueueCancelStatus::CancellationRequested
    } else {
        cancel_fal_queue_request_with_client(client, cancel_url, credential)
            .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))?
            .status
    };

    Ok(TemporalGenerateMediaCancelProviderActivityOutput {
        request_id: request_id_from_fal_cancel_url(cancel_url),
        cancel_url: cancel_url.to_string(),
        status,
    })
}

pub fn temporal_generate_media_cancel_provider_activity_value_with_client(
    client: &reqwest::blocking::Client,
    input: Value,
    credential: &str,
) -> Result<Value, TemporalWorkflowInputError> {
    let cancel_url = required_workflow_string(&input, "cancelUrl")?;
    let output = temporal_generate_media_cancel_provider_activity_with_client(
        client,
        &cancel_url,
        credential,
    )?;

    serde_json::to_value(output)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

fn request_id_from_fal_cancel_url(cancel_url: &str) -> String {
    let mut parts = cancel_url
        .trim_end_matches('/')
        .rsplit('/')
        .filter(|part| !part.trim().is_empty());
    let last = parts.next().unwrap_or("unknown");
    if last == "cancel" {
        parts.next()
    } else {
        Some(last)
    }
    .filter(|request_id| !request_id.trim().is_empty())
    .unwrap_or("unknown")
    .to_string()
}

fn is_replicate_cancel_url(cancel_url: &str) -> bool {
    let path = cancel_url
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/').map(|(_, path)| path))
        .unwrap_or(cancel_url);
    path.contains("v1/predictions/") && path.trim_end_matches('/').ends_with("/cancel")
}

#[derive(Clone, Copy)]
pub struct FalWorkflowExecution<'a> {
    pub credential: &'a str,
    pub updated_at: &'a str,
    pub run_id: Option<&'a str>,
    pub options: FalGenerationRunOptions,
}

pub fn temporal_generate_media_fal_run_submission_with_client(
    client: &reqwest::blocking::Client,
    project_dir: &Path,
    project: &VideoProject,
    start_request: &TemporalWorkflowStartRequest,
    submission: &FalQueueSubmission,
    execution: FalWorkflowExecution<'_>,
) -> Result<FalGenerationRun, TemporalWorkflowInputError> {
    let FalWorkflowExecution {
        credential,
        updated_at,
        run_id,
        options,
    } = execution;
    let input = temporal_generate_media_workflow_input(start_request)?;
    if input.mock_mode {
        return Err(TemporalWorkflowInputError::LiveModeRequired);
    }
    if input.job_id != input.asset_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "jobId".to_string(),
        ));
    }

    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == input.asset_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingGeneratedAsset(input.asset_id.clone()))?;
    let expected_submission = build_fal_queue_submission(asset)
        .map_err(|error| TemporalWorkflowInputError::FalGenerationRequest(error.to_string()))?;
    if submission.endpoint != expected_submission.endpoint
        || submission.method != expected_submission.method
        || submission.input != expected_submission.input
    {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "submission".to_string(),
        ));
    }

    let placement_replacement_item_id = input
        .placement_intent
        .as_deref()
        .and_then(replacement_item_id_from_placement_intent)
        .or_else(|| {
            asset
                .placement_intent
                .as_deref()
                .and_then(replacement_item_id_from_placement_intent)
        });

    let job_id = input.job_id.clone();
    run_fal_generation_submission_with_client_after_submit(
        client,
        GenerationTarget::new(
            project_dir,
            asset,
            updated_at,
            run_id,
            placement_replacement_item_id,
        ),
        submission,
        credential,
        options,
        |submit_response| {
            apply_project_actions_to_split_project(
                project_dir,
                vec![ProjectAction::UpdateJobProviderRequest {
                    job_id: job_id.clone(),
                    provider_request: JobProviderRequest {
                        provider: FAL_PROVIDER.to_string(),
                        request_id: submit_response.request_id.clone(),
                        status_url: submit_response.status_url.clone(),
                        response_url: submit_response.response_url.clone(),
                        cancel_url: submit_response.cancel_url.clone(),
                        submitted_at: updated_at.to_string(),
                    },
                }],
            )
            .map(|_| ())
            .map_err(|error| FalGenerationWorkerError::SubmitHookFailed {
                message: error.to_string(),
            })
        },
    )
    .map_err(|error| TemporalWorkflowInputError::FalGenerationRun(error.to_string()))
}

pub fn temporal_export_media_start_request(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    options: ExportRenderOptions,
    output_path: &str,
) -> TemporalWorkflowStartRequest {
    temporal_export_media_start_request_with_options_and_overwrite(
        project_id,
        project_dir,
        job_id,
        options,
        output_path,
        true,
    )
}

pub fn temporal_export_media_start_request_with_options(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    options: ExportRenderOptions,
    output_path: &str,
) -> TemporalWorkflowStartRequest {
    temporal_export_media_start_request_with_options_and_overwrite(
        project_id,
        project_dir,
        job_id,
        options,
        output_path,
        true,
    )
}

/// An export start request that saves under `output`'s name and folder
/// instead of overwriting `output_path`.
pub fn temporal_export_media_start_request_with_output(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    options: ExportRenderOptions,
    output_path: &str,
    output: Option<&crate::project::export_destination::ExportOutputRequest>,
) -> TemporalWorkflowStartRequest {
    let mut request = temporal_export_media_start_request_with_options_and_overwrite(
        project_id,
        project_dir,
        job_id,
        options,
        output_path,
        true,
    );
    if let Some(output) = output {
        request.input["destination"] = json!(output);
    }
    request
}

pub fn temporal_export_media_start_request_with_options_and_overwrite(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    options: ExportRenderOptions,
    output_path: &str,
    overwrite: bool,
) -> TemporalWorkflowStartRequest {
    let validation = export_profile_validation_payload(options.profile);
    let mut input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": options.profile,
        "quality": options.quality,
        "width": options.width,
        "height": options.height,
        "outputPath": output_path,
        "validation": validation,
    });
    if !overwrite {
        input["overwrite"] = json!(false);
    }
    insert_export_encode_settings(&mut input, &options);

    let mut request = temporal_workflow_start_request(
        TemporalWorkflowKind::ExportMedia,
        project_id,
        job_id,
        input,
    );
    request.activity_types = export_media_activity_types(options.profile)
        .iter()
        .map(|activity| (*activity).to_string())
        .collect();
    request
}

/// Adds `fps` and `encodeTier` to an export input object only when they differ
/// from the defaults, so existing payloads stay unchanged.
fn insert_export_encode_settings(input: &mut Value, options: &ExportRenderOptions) {
    let Some(object) = input.as_object_mut() else {
        return;
    };
    if let Some(fps) = options.fps {
        object.insert("fps".to_string(), json!(fps));
    }
    if !options.encode_tier.is_standard() {
        object.insert("encodeTier".to_string(), json!(options.encode_tier));
    }
}

pub fn temporal_export_project_bundle_start_request(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    output_path: &str,
    overwrite: bool,
) -> TemporalWorkflowStartRequest {
    let profile = ExportProfile::PalmierProject;
    let mut input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": profile,
        "outputPath": output_path,
        "validation": export_profile_validation_payload(profile),
    });
    if !overwrite {
        input["overwrite"] = json!(false);
    }
    let mut request = temporal_workflow_start_request(
        TemporalWorkflowKind::ExportMedia,
        project_id,
        job_id,
        input,
    );
    request.activity_types = export_media_activity_types(profile)
        .iter()
        .map(|activity| (*activity).to_string())
        .collect();
    request
}

pub(crate) fn export_media_activity_types(profile: ExportProfile) -> &'static [&'static str] {
    match profile {
        ExportProfile::PalmierProject => &["WriteExportArtifact", "AttachExportReport"],
        ExportProfile::Webm
        | ExportProfile::Mp4H264
        | ExportProfile::Mp4H265
        | ExportProfile::ProResMov => &[
            "BuildRenderPlan",
            "ValidateExportProfile",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
        ],
    }
}

fn export_profile_validation_payload(profile: ExportProfile) -> Value {
    if profile == ExportProfile::PalmierProject {
        return json!({
            "container": "palmier",
            "extension": "palmier",
            "mimeType": "application/vnd.video-creater.project",
            "videoCodec": null,
            "audioCodec": null,
            "requireVideoStream": false,
            "requireAudioStreamWhenTimelineHasAudio": false,
            "packageKind": "splitProjectBundle",
        });
    }

    let profile_report = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == profile)
        .expect("media export profile report must include every codec profile");

    json!({
        "container": profile_report.container,
        "extension": profile_report.extension,
        "mimeType": profile_report.mime_type,
        "videoCodec": profile_report.video_codec,
        "audioCodec": profile_report.audio_codec,
        "requireVideoStream": true,
        "requireAudioStreamWhenTimelineHasAudio": true,
    })
}

pub fn temporal_render_draft_start_request(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    profile: RenderQualityProfile,
) -> TemporalWorkflowStartRequest {
    temporal_workflow_start_request(
        TemporalWorkflowKind::RenderDraft,
        project_id,
        job_id,
        json!({
            "projectId": project_id,
            "projectDir": project_dir,
            "jobId": job_id,
            "profile": profile,
        }),
    )
}

pub fn temporal_export_nle_xml_start_request(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    format: NleXmlFormat,
    output_path: &str,
) -> TemporalWorkflowStartRequest {
    temporal_export_nle_xml_start_request_with_overwrite(
        project_id,
        project_dir,
        job_id,
        format,
        output_path,
        true,
    )
}

pub fn temporal_export_nle_xml_start_request_with_overwrite(
    project_id: &str,
    project_dir: &str,
    job_id: &str,
    format: NleXmlFormat,
    output_path: &str,
    overwrite: bool,
) -> TemporalWorkflowStartRequest {
    let mut input = json!({
        "projectId": project_id,
        "projectDir": project_dir,
        "jobId": job_id,
        "format": format,
        "outputPath": output_path,
    });
    if !overwrite {
        input["overwrite"] = json!(false);
    }

    temporal_workflow_start_request(
        TemporalWorkflowKind::ExportNleXml,
        project_id,
        job_id,
        input,
    )
}

pub fn temporal_start_result_action(
    job: &JobSummary,
    run_id: &str,
    updated_at: &str,
) -> Result<ProjectAction, TemporalStartResultError> {
    let workflow = job
        .workflow
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingWorkflowMetadata(job.id.clone()))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingStartRequest(job.id.clone()))?;
    validate_start_request_matches_workflow(start_request, workflow)?;

    let run_id = run_id.trim();
    if run_id.is_empty() {
        return Err(TemporalStartResultError::BlankRunId);
    }

    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalStartResultError::BlankUpdatedAt);
    }

    Ok(ProjectAction::UpdateJobStatus {
        job_id: job.id.clone(),
        status: JobStatus::Running,
        updated_at: updated_at.to_string(),
        run_id: Some(run_id.to_string()),
    })
}

pub fn temporal_workflow_client_start_plan(
    job: &JobSummary,
) -> Result<TemporalWorkflowClientStartPlan, TemporalStartResultError> {
    let workflow = job
        .workflow
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingWorkflowMetadata(job.id.clone()))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingStartRequest(job.id.clone()))?;
    validate_start_request_matches_workflow(start_request, workflow)?;

    if start_request.id_reuse_policy != "rejectDuplicate" {
        return Err(TemporalStartResultError::UnsupportedIdReusePolicy(
            start_request.id_reuse_policy.clone(),
        ));
    }

    Ok(TemporalWorkflowClientStartPlan {
        workflow_id: start_request.workflow_id.clone(),
        workflow_type: start_request.workflow_type.clone(),
        task_queue: start_request.task_queue.clone(),
        input: temporal_workflow_client_runtime_input(start_request),
        search_attributes: start_request.search_attributes.clone(),
        activity_types: start_request.activity_types.clone(),
        id_reuse_policy: start_request.id_reuse_policy.clone(),
    })
}

fn temporal_workflow_client_runtime_input(start_request: &TemporalWorkflowStartRequest) -> Value {
    if start_request.workflow_type == TemporalWorkflowKind::GenerateMedia.workflow_type() {
        json!({ "startRequest": start_request })
    } else {
        start_request.input.clone()
    }
}

#[cfg(feature = "temporal-worker")]
pub fn temporal_workflow_start_options(
    plan: &TemporalWorkflowClientStartPlan,
) -> temporalio_client::WorkflowStartOptions {
    use temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy;

    temporalio_client::WorkflowStartOptions::new(&plan.task_queue, &plan.workflow_id)
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .build()
}

#[cfg(feature = "temporal-worker")]
pub fn temporal_workflow_start_dispatch(
    plan: &TemporalWorkflowClientStartPlan,
) -> Result<TemporalWorkflowStartDispatch, TemporalStartResultError> {
    let target = match plan.workflow_type.as_str() {
        "VideoCreaterGenerateMediaWorkflow" => TemporalWorkflowStartTarget::GenerateMedia,
        "VideoCreaterRenderDraftWorkflow" => TemporalWorkflowStartTarget::RenderDraft,
        "VideoCreaterTranscribeMediaWorkflow" => TemporalWorkflowStartTarget::TranscribeMedia,
        "VideoCreaterCodexEditWorkflow" => TemporalWorkflowStartTarget::CodexEdit,
        "VideoCreaterExportMediaWorkflow" => TemporalWorkflowStartTarget::ExportMedia,
        "VideoCreaterExportNleXmlWorkflow" => TemporalWorkflowStartTarget::ExportNleXml,
        unsupported => {
            return Err(TemporalStartResultError::UnsupportedWorkflowType(
                unsupported.to_string(),
            ));
        }
    };

    Ok(TemporalWorkflowStartDispatch {
        target,
        workflow_id: plan.workflow_id.clone(),
        workflow_type: plan.workflow_type.clone(),
        task_queue: plan.task_queue.clone(),
        input: plan.input.clone(),
    })
}

#[cfg(feature = "temporal-worker")]
pub fn temporal_workflow_untyped_start_request(
    plan: &TemporalWorkflowClientStartPlan,
) -> Result<TemporalWorkflowUntypedStartRequest, TemporalStartResultError> {
    use temporalio_common::data_converters::{PayloadConverter, RawValue};

    let dispatch = temporal_workflow_start_dispatch(plan)?;
    let payload_converter = PayloadConverter::default();

    Ok(TemporalWorkflowUntypedStartRequest {
        workflow: temporalio_client::UntypedWorkflow::new(dispatch.workflow_type),
        input: RawValue::from_value(&dispatch.input, &payload_converter),
        options: temporal_workflow_start_options(plan),
    })
}

pub fn temporal_workflow_started_start_result(
    plan: &TemporalWorkflowClientStartPlan,
    run_id: &str,
) -> Result<TemporalWorkflowStartResult, TemporalStartResultError> {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return Err(TemporalStartResultError::BlankRunId);
    }

    Ok(TemporalWorkflowStartResult {
        status: "started".to_string(),
        workflow_id: plan.workflow_id.clone(),
        workflow_type: plan.workflow_type.clone(),
        task_queue: plan.task_queue.clone(),
        run_id: Some(run_id.to_string()),
        message: format!(
            "Temporal workflow started on task queue {}.",
            plan.task_queue
        ),
    })
}

#[cfg(feature = "temporal-worker")]
pub async fn temporal_start_workflow_with_client(
    client: &temporalio_client::Client,
    job: &JobSummary,
) -> Result<TemporalWorkflowStartResult, TemporalStartResultError> {
    let plan = temporal_workflow_client_start_plan(job)?;
    let request = temporal_workflow_untyped_start_request(&plan)?;
    let handle = client
        .start_workflow(request.workflow, request.input, request.options)
        .await
        .map_err(|error| TemporalStartResultError::ClientStartFailed(error.to_string()))?;
    let run_id = handle
        .run_id()
        .ok_or(TemporalStartResultError::MissingTemporalRunId)?;

    temporal_workflow_started_start_result(&plan, run_id)
}

pub fn temporal_workflow_unavailable_start_result(
    job: &JobSummary,
) -> Result<TemporalWorkflowStartResult, TemporalStartResultError> {
    let workflow = job
        .workflow
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingWorkflowMetadata(job.id.clone()))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingStartRequest(job.id.clone()))?;
    validate_start_request_matches_workflow(start_request, workflow)?;

    Ok(TemporalWorkflowStartResult {
        status: "unavailable".to_string(),
        workflow_id: start_request.workflow_id.clone(),
        workflow_type: start_request.workflow_type.clone(),
        task_queue: start_request.task_queue.clone(),
        run_id: None,
        message: format!(
            "Temporal runtime is unavailable in this build. Rebuild with default features or explicitly pass --features temporal-worker, run `{}`, and start the worker before dispatching workflow executions.",
            VIDEO_CREATER_TEMPORAL_LOCAL_DEV_COMMAND
        ),
    })
}

pub fn temporal_generate_media_failure_actions(
    job: &JobSummary,
    asset_id: &str,
    run_id: Option<&str>,
    updated_at: &str,
) -> Result<Vec<ProjectAction>, TemporalStartResultError> {
    let workflow = job
        .workflow
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingWorkflowMetadata(job.id.clone()))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingStartRequest(job.id.clone()))?;
    validate_start_request_matches_workflow(start_request, workflow)?;

    if workflow.workflow_type != TemporalWorkflowKind::GenerateMedia.workflow_type() {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "workflowType".to_string(),
        ));
    }

    if start_request.input.get("assetId").and_then(Value::as_str) != Some(asset_id) {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "input.assetId".to_string(),
        ));
    }

    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalStartResultError::BlankUpdatedAt);
    }

    let run_id = match run_id {
        Some(run_id) => {
            let run_id = run_id.trim();
            if run_id.is_empty() {
                return Err(TemporalStartResultError::BlankRunId);
            }
            Some(run_id.to_string())
        }
        None => None,
    };

    Ok(vec![
        ProjectAction::UpdateJobStatus {
            job_id: job.id.clone(),
            status: JobStatus::Failed,
            updated_at: updated_at.to_string(),
            run_id,
        },
        ProjectAction::UpdateGeneratedAssetStatus {
            asset_id: asset_id.to_string(),
            status: GeneratedAssetStatus::Failed,
        },
    ])
}

pub fn temporal_generate_media_cancellation_actions(
    job: &JobSummary,
    asset_id: &str,
    run_id: Option<&str>,
    updated_at: &str,
) -> Result<Vec<ProjectAction>, TemporalStartResultError> {
    let mut actions = temporal_generate_media_failure_actions(job, asset_id, run_id, updated_at)?;
    for action in &mut actions {
        match action {
            ProjectAction::UpdateJobStatus { status, .. } => *status = JobStatus::Cancelled,
            ProjectAction::UpdateGeneratedAssetStatus { status, .. } => {
                *status = GeneratedAssetStatus::Cancelled;
            }
            _ => {}
        }
    }
    Ok(actions)
}

pub fn temporal_codex_edit_failure_actions(
    job: &JobSummary,
    run_id: Option<&str>,
    updated_at: &str,
) -> Result<Vec<ProjectAction>, TemporalStartResultError> {
    let workflow = job
        .workflow
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingWorkflowMetadata(job.id.clone()))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| TemporalStartResultError::MissingStartRequest(job.id.clone()))?;
    validate_start_request_matches_workflow(start_request, workflow)?;

    if workflow.workflow_type != TemporalWorkflowKind::CodexEdit.workflow_type() {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "workflowType".to_string(),
        ));
    }

    if start_request.input.get("jobId").and_then(Value::as_str) != Some(job.id.as_str()) {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "input.jobId".to_string(),
        ));
    }

    let updated_at = updated_at.trim();
    if updated_at.is_empty() {
        return Err(TemporalStartResultError::BlankUpdatedAt);
    }

    let run_id = match run_id {
        Some(run_id) => {
            let run_id = run_id.trim();
            if run_id.is_empty() {
                return Err(TemporalStartResultError::BlankRunId);
            }
            Some(run_id.to_string())
        }
        None => None,
    };

    Ok(vec![ProjectAction::UpdateJobStatus {
        job_id: job.id.clone(),
        status: JobStatus::Failed,
        updated_at: updated_at.to_string(),
        run_id,
    }])
}

fn validate_start_request_matches_workflow(
    start_request: &TemporalWorkflowStartRequest,
    workflow: &TemporalWorkflowMetadata,
) -> Result<(), TemporalStartResultError> {
    if start_request.workflow_id != workflow.workflow_id {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "workflowId".to_string(),
        ));
    }
    if start_request.workflow_type != workflow.workflow_type {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "workflowType".to_string(),
        ));
    }
    if start_request.task_queue != workflow.task_queue {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "taskQueue".to_string(),
        ));
    }
    if start_request.activity_types != workflow.activity_types {
        return Err(TemporalStartResultError::MismatchedStartRequest(
            "activityTypes".to_string(),
        ));
    }

    Ok(())
}

fn required_workflow_string(
    input: &Value,
    field: &'static str,
) -> Result<String, TemporalWorkflowInputError> {
    let value = input
        .get(field)
        .ok_or_else(|| TemporalWorkflowInputError::MissingField(field.to_string()))?;
    let text = value
        .as_str()
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))?
        .trim();
    if text.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(field.to_string()));
    }

    Ok(text.to_string())
}

fn required_workflow_u32(
    input: &Value,
    field: &'static str,
) -> Result<u32, TemporalWorkflowInputError> {
    input
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))
}

fn required_workflow_nested_string(
    input: &Value,
    path: &[&str],
    field: &'static str,
) -> Result<String, TemporalWorkflowInputError> {
    let mut value = input;
    for segment in path {
        value = value
            .get(segment)
            .ok_or_else(|| TemporalWorkflowInputError::MissingField(field.to_string()))?;
    }
    let text = value
        .as_str()
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))?
        .trim();
    if text.is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(field.to_string()));
    }

    Ok(text.to_string())
}

fn optional_workflow_string(
    input: &Value,
    field: &'static str,
) -> Result<Option<String>, TemporalWorkflowInputError> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let text = value
        .as_str()
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))?
        .trim();
    if text.is_empty() {
        return Ok(None);
    }

    Ok(Some(text.to_string()))
}

fn optional_workflow_bool(
    input: &Value,
    field: &'static str,
) -> Result<Option<bool>, TemporalWorkflowInputError> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))
}

fn required_workflow_bool(
    input: &Value,
    field: &'static str,
) -> Result<bool, TemporalWorkflowInputError> {
    input
        .get(field)
        .ok_or_else(|| TemporalWorkflowInputError::MissingField(field.to_string()))?
        .as_bool()
        .ok_or_else(|| TemporalWorkflowInputError::InvalidFieldType(field.to_string()))
}

fn default_true() -> bool {
    true
}

fn ensure_export_output_can_be_written(
    output_path: &Path,
    overwrite: bool,
) -> Result<(), TemporalWorkflowInputError> {
    if output_path.exists() && !overwrite {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "overwrite".to_string(),
        ));
    }
    Ok(())
}

fn remove_existing_export_output(output_path: &Path) -> Result<(), TemporalWorkflowInputError> {
    if !output_path.exists() {
        return Ok(());
    }
    if output_path.is_dir() {
        fs::remove_dir_all(output_path)
    } else {
        fs::remove_file(output_path)
    }
    .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))
}

fn files_have_identical_bytes(
    left_path: &Path,
    right_path: &Path,
) -> Result<bool, TemporalWorkflowInputError> {
    let left_metadata = fs::metadata(left_path)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    let right_metadata = fs::metadata(right_path)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let mut left = BufReader::new(
        fs::File::open(left_path)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?,
    );
    let mut right = BufReader::new(
        fs::File::open(right_path)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?,
    );
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_read = left
            .read(&mut left_buffer)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
        let right_read = right
            .read(&mut right_buffer)
            .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn optional_workflow_value(input: &Value, field: &'static str) -> Option<Value> {
    input.get(field).filter(|value| !value.is_null()).cloned()
}

fn replacement_item_id_from_placement_intent(intent: &str) -> Option<&str> {
    let item_id = intent.trim().strip_prefix("replace:")?.trim();
    if item_id.is_empty() {
        None
    } else {
        Some(item_id)
    }
}

pub fn temporal_worker_manifest() -> TemporalWorkerManifest {
    let mut activity_types = Vec::new();
    for kind in VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS {
        for activity_type in kind.activity_types() {
            if !activity_types.contains(activity_type) {
                activity_types.push(*activity_type);
            }
        }
    }

    TemporalWorkerManifest {
        task_queue: VIDEO_CREATER_TEMPORAL_TASK_QUEUE.to_string(),
        local_service_target: VIDEO_CREATER_TEMPORAL_LOCAL_SERVICE_TARGET.to_string(),
        local_web_ui_url: VIDEO_CREATER_TEMPORAL_LOCAL_WEB_UI_URL.to_string(),
        local_dev_command: VIDEO_CREATER_TEMPORAL_LOCAL_DEV_COMMAND.to_string(),
        worker_run_command: VIDEO_CREATER_TEMPORAL_WORKER_RUN_COMMAND.to_string(),
        feature_name: VIDEO_CREATER_TEMPORAL_FEATURE_NAME.to_string(),
        sdk_crates: VIDEO_CREATER_TEMPORAL_RUST_SDK_CRATES
            .iter()
            .map(|crate_name| (*crate_name).to_string())
            .collect(),
        required_tools: VIDEO_CREATER_TEMPORAL_REQUIRED_TOOLS
            .iter()
            .map(|tool_name| (*tool_name).to_string())
            .collect(),
        workflow_types: VIDEO_CREATER_TEMPORAL_WORKFLOW_KINDS
            .iter()
            .map(|kind| kind.workflow_type().to_string())
            .collect(),
        activity_types: activity_types
            .into_iter()
            .map(|activity_type| activity_type.to_string())
            .collect(),
    }
}

pub fn temporal_worker_registration_plan() -> TemporalWorkerRegistrationPlan {
    let manifest = temporal_worker_manifest();

    TemporalWorkerRegistrationPlan {
        task_queue: manifest.task_queue,
        workflow_registrations: manifest.workflow_types,
        activity_registrations: manifest.activity_types,
    }
}

#[cfg(feature = "temporal-worker")]
pub fn temporal_worker_options(
) -> Result<temporalio_sdk::WorkerOptions, temporalio_sdk::WorkflowRegistrationError> {
    temporal_worker_runtime::worker_options()
}

#[cfg(feature = "temporal-worker")]
mod temporal_worker_runtime {
    #![allow(dead_code)]

    use chrono::{DateTime, SecondsFormat, Utc};
    use serde_json::{json, Value};
    use std::time::{Duration, SystemTime};
    use temporalio_macros::{activities, workflow, workflow_methods};
    use temporalio_sdk::{
        activities::{ActivityContext, ActivityError},
        ActivityOptions, WorkerOptions, WorkflowContext, WorkflowResult,
    };

    use super::VIDEO_CREATER_TEMPORAL_TASK_QUEUE;

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterGenerateMediaWorkflow;

    #[workflow_methods]
    impl VideoCreaterGenerateMediaWorkflow {
        #[run(name = "VideoCreaterGenerateMediaWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let prepare_input = json!({
                "startRequest": input["startRequest"].clone(),
                "updatedAt": updated_at.clone(),
            });
            let prepare_result = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::prepare_provider_inputs,
                    prepare_input,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(300)),
                )
                .await;

            let prepare_output = match prepare_result {
                Ok(output) => output,
                Err(error) => {
                    let failure_attach =
                        attach_generate_media_failure(ctx, input.clone(), &updated_at).await;
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterGenerateMediaWorkflow",
                        "error": format!("{error:?}"),
                        "failureAttach": failure_attach,
                        "input": input,
                    }));
                }
            };
            let build_result = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::build_fal_generation_request,
                    input.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await;

            match build_result {
                Ok(submission) => {
                    let provider_input =
                        match super::temporal_generate_media_run_provider_activity_input_value(
                            &input,
                            submission.clone(),
                            &updated_at,
                            Some(ctx.run_id()),
                        ) {
                            Ok(provider_input) => provider_input,
                            Err(error) => {
                                let failure_attach =
                                    attach_generate_media_failure(ctx, input.clone(), &updated_at)
                                        .await;
                                return Ok(json!({
                                    "status": "failed",
                                    "workflowType": "VideoCreaterGenerateMediaWorkflow",
                                    "error": error.to_string(),
                                    "failureAttach": failure_attach,
                                    "submission": submission,
                                }));
                            }
                        };
                    let provider_result = ctx
                        .start_activity(
                            VideoCreaterTemporalActivities::run_media_provider_generation,
                            provider_input,
                            ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
                        )
                        .await;

                    match provider_result {
                        Ok(output) => Ok(json!({
                            "status": "completed",
                            "workflowType": "VideoCreaterGenerateMediaWorkflow",
                            "prepare": prepare_output,
                            "output": output,
                        })),
                        Err(error) => {
                            let failure_attach =
                                attach_generate_media_failure(ctx, input.clone(), &updated_at)
                                    .await;
                            Ok(json!({
                                "status": "failed",
                                "workflowType": "VideoCreaterGenerateMediaWorkflow",
                                "error": format!("{error:?}"),
                                "failureAttach": failure_attach,
                                "prepare": prepare_output,
                                "submission": submission,
                            }))
                        }
                    }
                }
                Err(error) => {
                    let failure_attach =
                        attach_generate_media_failure(ctx, input.clone(), &updated_at).await;
                    Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterGenerateMediaWorkflow",
                        "error": format!("{error:?}"),
                        "failureAttach": failure_attach,
                        "prepare": prepare_output,
                        "input": input,
                    }))
                }
            }
        }
    }

    async fn attach_generate_media_failure(
        ctx: &mut WorkflowContext<VideoCreaterGenerateMediaWorkflow>,
        input: Value,
        updated_at: &str,
    ) -> Value {
        let failure_input = json!({
            "startRequest": input.get("startRequest").cloned().unwrap_or(Value::Null),
            "runId": ctx.run_id(),
            "updatedAt": updated_at,
        });

        match ctx
            .start_activity(
                VideoCreaterTemporalActivities::attach_generated_asset_failure,
                failure_input,
                ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
            )
            .await
        {
            Ok(output) => json!({
                "status": "attached",
                "output": output,
            }),
            Err(error) => json!({
                "status": "failed",
                "error": format!("{error:?}"),
            }),
        }
    }

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterExportNleXmlWorkflow;

    #[workflow_methods]
    impl VideoCreaterExportNleXmlWorkflow {
        #[run(name = "VideoCreaterExportNleXmlWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let build_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::build_nle_xml,
                    input.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let validation_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::validate_nle_xml,
                    build_output.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let write_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::write_export_artifact,
                    build_output,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let attach_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::attach_export_report,
                    json!({
                        "writeOutput": write_output,
                        "createdAt": updated_at,
                        "runId": ctx.run_id(),
                    }),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;

            Ok(json!({
                "status": "completed",
                "workflowType": "VideoCreaterExportNleXmlWorkflow",
                "validation": validation_output,
                "attach": attach_output,
            }))
        }
    }

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterRenderDraftWorkflow;

    #[workflow_methods]
    impl VideoCreaterRenderDraftWorkflow {
        #[run(name = "VideoCreaterRenderDraftWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let plan = match super::temporal_render_workflow_activity_plan(
                super::TemporalWorkflowKind::RenderDraft,
                input.clone(),
                &updated_at,
                Some(ctx.run_id()),
            ) {
                Ok(plan) => plan,
                Err(error) => {
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterRenderDraftWorkflow",
                        "error": error.to_string(),
                        "input": input,
                    }));
                }
            };
            let build_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::build_render_plan,
                    plan.build_render_plan_input.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let render_input = match super::temporal_render_media_activity_input_value(
                build_output,
                &updated_at,
                Some(ctx.run_id()),
            ) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterRenderDraftWorkflow",
                        "error": error.to_string(),
                        "plan": plan,
                    }));
                }
            };
            let render_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::render_media,
                    render_input,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
                )
                .await?;
            let validation_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::validate_rendered_media,
                    render_output,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let attach_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::attach_render_report,
                    validation_output.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;

            Ok(json!({
                "status": "completed",
                "workflowType": "VideoCreaterRenderDraftWorkflow",
                "plan": plan,
                "validation": validation_output,
                "attach": attach_output,
            }))
        }
    }

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterTranscribeMediaWorkflow;

    #[workflow_methods]
    impl VideoCreaterTranscribeMediaWorkflow {
        #[run(name = "VideoCreaterTranscribeMediaWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let plan = match super::temporal_transcribe_workflow_activity_plan_value(
                input.clone(),
                &updated_at,
                Some(ctx.run_id()),
            ) {
                Ok(plan) => plan,
                Err(error) => {
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterTranscribeMediaWorkflow",
                        "error": error.to_string(),
                        "input": input,
                    }));
                }
            };

            let probe_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::probe_media,
                    plan["probeMediaInput"].clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let run_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::run_transcription,
                    probe_output,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
                )
                .await?;
            let mut store_input = run_output;
            let Some(store_input_object) = store_input.as_object_mut() else {
                return Ok(json!({
                    "status": "failed",
                    "workflowType": "VideoCreaterTranscribeMediaWorkflow",
                    "error": "RunTranscription output must be a JSON object",
                    "plan": plan,
                }));
            };
            store_input_object.insert("updatedAt".to_string(), plan["updatedAt"].clone());
            if let Some(run_id) = plan.get("runId").and_then(Value::as_str) {
                store_input_object.insert("runId".to_string(), json!(run_id));
            }
            let store_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::store_transcript,
                    store_input,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;

            Ok(json!({
                "status": "completed",
                "workflowType": "VideoCreaterTranscribeMediaWorkflow",
                "plan": plan,
                "store": store_output,
            }))
        }
    }

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterCodexEditWorkflow;

    #[workflow_methods]
    impl VideoCreaterCodexEditWorkflow {
        #[run(name = "VideoCreaterCodexEditWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let context_output = match ctx
                .start_activity(
                    VideoCreaterTemporalActivities::collect_project_context,
                    input.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    return codex_edit_stage_failure(
                        ctx,
                        &input,
                        &updated_at,
                        "CollectProjectContext",
                        format!("{error:?}"),
                    )
                    .await;
                }
            };
            let proposal_output = match ctx
                .start_activity(
                    VideoCreaterTemporalActivities::request_codex_proposal,
                    context_output,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(600)),
                )
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    return codex_edit_stage_failure(
                        ctx,
                        &input,
                        &updated_at,
                        "RequestCodexProposal",
                        format!("{error:?}"),
                    )
                    .await;
                }
            };
            let proposal_input = match super::temporal_codex_edit_proposal_activity_input_value(
                &input,
                proposal_output,
                &updated_at,
                Some(ctx.run_id()),
            )
            .map_err(|error| error.to_string())
            {
                Ok(input) => input,
                Err(error) => {
                    return codex_edit_stage_failure(
                        ctx,
                        &input,
                        &updated_at,
                        "BuildProposalActivityInput",
                        error,
                    )
                    .await;
                }
            };
            let validation_output = match ctx
                .start_activity(
                    VideoCreaterTemporalActivities::validate_project_actions,
                    proposal_input.clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    return codex_edit_stage_failure(
                        ctx,
                        &input,
                        &updated_at,
                        "ValidateProjectActions",
                        format!("{error:?}"),
                    )
                    .await;
                }
            };
            let persist_output = match ctx
                .start_activity(
                    VideoCreaterTemporalActivities::persist_accepted_proposal,
                    proposal_input,
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    return codex_edit_stage_failure(
                        ctx,
                        &input,
                        &updated_at,
                        "PersistAcceptedProposal",
                        format!("{error:?}"),
                    )
                    .await;
                }
            };

            Ok(json!({
                "status": "completed",
                "workflowType": "VideoCreaterCodexEditWorkflow",
                "validation": validation_output,
                "persist": persist_output,
            }))
        }
    }

    async fn codex_edit_stage_failure(
        ctx: &mut WorkflowContext<VideoCreaterCodexEditWorkflow>,
        input: &Value,
        updated_at: &str,
        stage: &str,
        error: String,
    ) -> WorkflowResult<Value> {
        let failure_attach = attach_codex_edit_failure(ctx, input, updated_at, stage, &error).await;

        Ok(json!({
            "status": "failed",
            "workflowType": "VideoCreaterCodexEditWorkflow",
            "stage": stage,
            "error": error,
            "failureAttach": failure_attach,
        }))
    }

    async fn attach_codex_edit_failure(
        ctx: &mut WorkflowContext<VideoCreaterCodexEditWorkflow>,
        input: &Value,
        updated_at: &str,
        stage: &str,
        error: &str,
    ) -> Value {
        let error = format!("{stage} failed: {error}");
        let failure_input = match super::temporal_codex_edit_failure_activity_input_value(
            input,
            updated_at,
            Some(ctx.run_id()),
            &error,
        ) {
            Ok(input) => input,
            Err(error) => {
                return json!({
                    "status": "failed",
                    "error": format!("{error:?}"),
                });
            }
        };

        match ctx
            .start_activity(
                VideoCreaterTemporalActivities::attach_codex_edit_failure,
                failure_input,
                ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
            )
            .await
        {
            Ok(output) => json!({
                "status": "attached",
                "output": output,
            }),
            Err(error) => json!({
                "status": "failed",
                "error": format!("{error:?}"),
            }),
        }
    }

    #[workflow]
    #[derive(Default)]
    pub struct VideoCreaterExportMediaWorkflow;

    #[workflow_methods]
    impl VideoCreaterExportMediaWorkflow {
        #[run(name = "VideoCreaterExportMediaWorkflow")]
        pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
            let updated_at = workflow_start_time_rfc3339(
                ctx.workflow_initial_info()
                    .start_time
                    .and_then(|timestamp| timestamp.try_into().ok()),
            );
            let plan = match super::temporal_export_media_workflow_activity_plan_value(
                input.clone(),
                &updated_at,
                Some(ctx.run_id()),
            ) {
                Ok(plan) => plan,
                Err(super::TemporalWorkflowInputError::Render(reason)) => {
                    return Ok(json!({
                        "status": "unsupported",
                        "workflowType": "VideoCreaterExportMediaWorkflow",
                        "reason": reason,
                        "input": input,
                    }));
                }
                Err(error) => {
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterExportMediaWorkflow",
                        "error": error.to_string(),
                        "input": input,
                    }));
                }
            };
            if plan.get("buildRenderPlanInput").is_some() {
                let build_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::build_render_plan,
                        plan["buildRenderPlanInput"].clone(),
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                    )
                    .await?;
                let render_input = match super::temporal_render_media_activity_input_value(
                    build_output,
                    &updated_at,
                    Some(ctx.run_id()),
                ) {
                    Ok(input) => input,
                    Err(error) => {
                        return Ok(json!({
                            "status": "failed",
                            "workflowType": "VideoCreaterExportMediaWorkflow",
                            "error": error.to_string(),
                            "plan": plan,
                        }));
                    }
                };
                let _profile_validation = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::validate_export_profile,
                        plan["validateExportProfileInput"].clone(),
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                    )
                    .await?;
                let render_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::render_media,
                        render_input,
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
                    )
                    .await?;
                let validation_input =
                    match super::temporal_validate_rendered_media_activity_input_value(
                        render_output,
                        plan.get("validateRenderedMediaInput"),
                    ) {
                        Ok(input) => input,
                        Err(error) => {
                            return Ok(json!({
                                "status": "failed",
                                "workflowType": "VideoCreaterExportMediaWorkflow",
                                "error": error.to_string(),
                                "plan": plan,
                            }));
                        }
                    };
                let validation_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::validate_rendered_media,
                        validation_input,
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                    )
                    .await?;
                let render_attach_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::attach_render_report,
                        validation_output.clone(),
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                    )
                    .await?;
                if plan.get("writeExportArtifactBaseInput").is_none() {
                    return Ok(json!({
                        "status": "completed",
                        "workflowType": "VideoCreaterExportMediaWorkflow",
                        "plan": plan,
                        "renderAttach": render_attach_output,
                    }));
                }
                let mut write_input = plan["writeExportArtifactBaseInput"].clone();
                let Some(write_object) = write_input.as_object_mut() else {
                    return Ok(json!({
                        "status": "failed",
                        "workflowType": "VideoCreaterExportMediaWorkflow",
                        "error": "writeExportArtifactBaseInput must be an object",
                        "plan": plan,
                    }));
                };
                write_object.insert("validationOutput".to_string(), validation_output);
                let write_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::write_export_artifact,
                        write_input,
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
                    )
                    .await?;
                let attach_output = ctx
                    .start_activity(
                        VideoCreaterTemporalActivities::attach_export_report,
                        json!({
                            "writeOutput": write_output,
                            "createdAt": updated_at,
                            "runId": ctx.run_id(),
                        }),
                        ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                    )
                    .await?;

                return Ok(json!({
                    "status": "completed",
                    "workflowType": "VideoCreaterExportMediaWorkflow",
                    "plan": plan,
                    "renderAttach": render_attach_output,
                    "attach": attach_output,
                }));
            }
            let write_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::write_export_artifact,
                    plan["writeExportArtifactInput"].clone(),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;
            let attach_output = ctx
                .start_activity(
                    VideoCreaterTemporalActivities::attach_export_report,
                    json!({
                        "writeOutput": write_output,
                        "createdAt": updated_at,
                        "runId": ctx.run_id(),
                    }),
                    ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
                )
                .await?;

            Ok(json!({
                "status": "completed",
                "workflowType": "VideoCreaterExportMediaWorkflow",
                "plan": plan,
                "attach": attach_output,
            }))
        }
    }

    pub struct VideoCreaterTemporalActivities;

    #[activities]
    #[allow(dead_code)]
    impl VideoCreaterTemporalActivities {
        #[activity(name = "PrepareProviderInputs")]
        pub async fn prepare_provider_inputs(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_generate_media_prepare_provider_inputs_activity_value(input)
                .map_err(activity_error)
        }

        #[activity(name = "BuildFalGenerationRequest")]
        pub async fn build_fal_generation_request(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_generate_media_build_fal_generation_request_activity_value(input)
                .map_err(activity_error)
        }

        #[activity(name = "RunMediaProviderGeneration")]
        pub async fn run_media_provider_generation(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            let activity_input: super::TemporalGenerateMediaRunProviderActivityInput =
                serde_json::from_value(input).map_err(activity_error)?;
            let workflow_input =
                super::temporal_generate_media_workflow_input(&activity_input.start_request)
                    .map_err(activity_error)?;
            let credential = if workflow_input.mock_mode {
                String::new()
            } else {
                super::resolve_provider_credential(activity_input.submission.provider())
                    .map_err(activity_error)?
                    .into_secret()
            };
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(activity_error)?;

            super::temporal_generate_media_run_provider_activity_with_client(
                &client,
                &activity_input,
                &credential,
            )
            .and_then(|output| {
                serde_json::to_value(output).map_err(|error| {
                    super::TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
                })
            })
            .map_err(activity_error)
        }

        #[activity(name = "CancelMediaProviderGeneration")]
        pub async fn cancel_media_provider_generation(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            let provider =
                super::required_workflow_string(&input, "provider").map_err(activity_error)?;
            let credential = super::resolve_provider_credential(&provider)
                .map_err(activity_error)?
                .into_secret();
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(activity_error)?;

            super::temporal_generate_media_cancel_provider_activity_value_with_client(
                &client,
                input,
                &credential,
            )
            .map_err(activity_error)
        }

        #[activity(name = "AttachGeneratedAssetFailure")]
        pub async fn attach_generated_asset_failure(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_generate_media_attach_failure_activity_value(input)
                .map_err(activity_error)
        }

        #[activity(name = "BuildRenderPlan")]
        pub async fn build_render_plan(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_render_build_plan_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "RenderMedia")]
        pub async fn render_media(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_render_media_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "ValidateRenderedMedia")]
        pub async fn validate_rendered_media(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_validate_rendered_media_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "AttachRenderReport")]
        pub async fn attach_render_report(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_attach_render_report_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "ProbeMedia")]
        pub async fn probe_media(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_transcribe_probe_media_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "RunTranscription")]
        pub async fn run_transcription(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_transcribe_run_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "StoreTranscript")]
        pub async fn store_transcript(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_transcribe_store_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "CollectProjectContext")]
        pub async fn collect_project_context(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input.get("projectDir").is_some() && input.get("request").is_some() {
                return super::temporal_codex_edit_collect_project_context_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("CollectProjectContext", input))
        }

        #[activity(name = "RequestCodexProposal")]
        pub async fn request_codex_proposal(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input.get("projectRoot").is_some()
                && input.get("projectDir").is_some()
                && input.get("request").is_some()
                && input.get("context").is_some()
            {
                return super::temporal_codex_edit_request_proposal_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("RequestCodexProposal", input))
        }

        #[activity(name = "ValidateProjectActions")]
        pub async fn validate_project_actions(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input.get("projectDir").is_some()
                && input.get("request").is_some()
                && input.get("proposal").is_some()
            {
                return super::temporal_codex_edit_validate_project_actions_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("ValidateProjectActions", input))
        }

        #[activity(name = "PersistAcceptedProposal")]
        pub async fn persist_accepted_proposal(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input.get("projectDir").is_some()
                && input.get("request").is_some()
                && input.get("proposal").is_some()
            {
                return super::temporal_codex_edit_persist_accepted_proposal_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("PersistAcceptedProposal", input))
        }

        #[activity(name = "AttachCodexEditFailure")]
        pub async fn attach_codex_edit_failure(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_codex_edit_attach_failure_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "ValidateExportProfile")]
        pub async fn validate_export_profile(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            Ok(activity_registered("ValidateExportProfile", input))
        }

        #[activity(name = "WriteExportArtifact")]
        pub async fn write_export_artifact(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input.get("profile").and_then(Value::as_str) == Some("palmierProject") {
                return super::temporal_export_project_bundle_write_activity_value(input)
                    .map_err(activity_error);
            }
            if input.get("validationOutput").is_some()
                && input.get("profile").is_some()
                && input.get("outputPath").is_some()
            {
                return super::temporal_export_media_write_artifact_activity_value(input)
                    .map_err(activity_error);
            }
            if input.get("xml").is_some() && input.get("format").is_some() {
                return super::temporal_export_nle_xml_write_artifact_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("WriteExportArtifact", input))
        }

        #[activity(name = "AttachExportReport")]
        pub async fn attach_export_report(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            if input
                .get("writeOutput")
                .and_then(|write_output| write_output.get("profile"))
                .and_then(Value::as_str)
                == Some("palmierProject")
            {
                return super::temporal_export_project_bundle_attach_export_report_activity_value(
                    input,
                )
                .map_err(activity_error);
            }
            if input
                .get("writeOutput")
                .and_then(|write_output| write_output.get("profile"))
                .and_then(Value::as_str)
                .is_some_and(|profile| matches!(profile, "mp4H264" | "mp4H265" | "proResMov"))
            {
                return super::temporal_export_media_attach_export_report_activity_value(input)
                    .map_err(activity_error);
            }
            if input.get("writeOutput").is_some() {
                return super::temporal_export_nle_xml_attach_export_report_activity_value(input)
                    .map_err(activity_error);
            }
            Ok(activity_registered("AttachExportReport", input))
        }

        #[activity(name = "BuildNleXml")]
        pub async fn build_nle_xml(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_export_nle_xml_build_activity_value(input).map_err(activity_error)
        }

        #[activity(name = "ValidateNleXml")]
        pub async fn validate_nle_xml(
            _ctx: ActivityContext,
            input: Value,
        ) -> Result<Value, ActivityError> {
            super::temporal_export_nle_xml_validate_activity_value(input).map_err(activity_error)
        }
    }

    pub fn worker_options() -> Result<WorkerOptions, temporalio_sdk::WorkflowRegistrationError> {
        Ok(WorkerOptions::new(VIDEO_CREATER_TEMPORAL_TASK_QUEUE)
            .register_workflow::<VideoCreaterGenerateMediaWorkflow>()?
            .register_workflow::<VideoCreaterRenderDraftWorkflow>()?
            .register_workflow::<VideoCreaterTranscribeMediaWorkflow>()?
            .register_workflow::<VideoCreaterCodexEditWorkflow>()?
            .register_workflow::<VideoCreaterExportMediaWorkflow>()?
            .register_workflow::<VideoCreaterExportNleXmlWorkflow>()?
            .register_activities(VideoCreaterTemporalActivities)
            .build())
    }

    fn activity_registered(activity_type: &'static str, input: Value) -> Value {
        json!({
            "status": "registered",
            "activityType": activity_type,
            "input": input,
        })
    }

    fn activity_error(error: impl std::fmt::Display) -> ActivityError {
        ActivityError::from(anyhow::anyhow!(error.to_string()))
    }

    fn workflow_start_time_rfc3339(start_time: Option<SystemTime>) -> String {
        let start_time = start_time.unwrap_or(SystemTime::UNIX_EPOCH);
        let timestamp: DateTime<Utc> = start_time.into();
        timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

pub fn temporal_worker_environment_report() -> TemporalWorkerEnvironmentReport {
    temporal_worker_environment_report_for_path_and_protoc(
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PROTOC").as_deref(),
    )
}

pub fn temporal_worker_environment_report_for_path(
    path_value: Option<&OsStr>,
) -> TemporalWorkerEnvironmentReport {
    temporal_worker_environment_report_for_path_and_protoc(path_value, None)
}

pub fn temporal_worker_environment_report_for_path_and_protoc(
    path_value: Option<&OsStr>,
    protoc_value: Option<&OsStr>,
) -> TemporalWorkerEnvironmentReport {
    let manifest = temporal_worker_manifest();
    let tools: Vec<TemporalWorkerToolStatus> = manifest
        .required_tools
        .iter()
        .map(|tool_name| {
            let path = find_temporal_worker_tool(tool_name, path_value, protoc_value);
            TemporalWorkerToolStatus {
                name: tool_name.clone(),
                available: path.is_some(),
                path: path.map(|path| path.display().to_string()),
                install_hint: None,
            }
        })
        .collect();
    let feature_enabled = cfg!(feature = "temporal-worker");
    let ready = feature_enabled && tools.iter().all(|tool| tool.available);

    TemporalWorkerEnvironmentReport {
        ready,
        feature_enabled,
        task_queue: manifest.task_queue,
        local_service_target: manifest.local_service_target,
        local_web_ui_url: manifest.local_web_ui_url,
        local_dev_command: manifest.local_dev_command,
        worker_run_command: manifest.worker_run_command,
        feature_name: manifest.feature_name,
        tools,
    }
}

fn find_temporal_worker_tool(
    tool_name: &str,
    path_value: Option<&OsStr>,
    protoc_value: Option<&OsStr>,
) -> Option<PathBuf> {
    if tool_name == "protoc" {
        if let Some(path) = protoc_value
            .map(PathBuf::from)
            .filter(|path| is_executable_file(path))
        {
            return Some(path);
        }
    }

    find_executable_on_path(tool_name, path_value)
}

fn find_executable_on_path(tool_name: &str, path_value: Option<&OsStr>) -> Option<PathBuf> {
    let path_value = path_value?;
    for directory in std::env::split_paths(path_value) {
        if directory.as_os_str().is_empty() {
            continue;
        }
        let candidate = directory.join(tool_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn safe_workflow_segment(value: &str) -> String {
    let mut segment = String::new();
    let mut last_was_separator = false;

    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            segment.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !segment.is_empty() {
            segment.push('-');
            last_was_separator = true;
        }
    }

    while segment.ends_with('-') {
        segment.pop();
    }

    if segment.is_empty() {
        "job".to_string()
    } else {
        segment
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use std::collections::BTreeMap;
    #[cfg(feature = "ges-render")]
    use std::collections::BTreeSet;
    use std::ffi::OsStr;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use serde_json::json;

    use super::{
        completed_generated_output_for_actions, compress_provider_input_video_with_renderer,
        provider_input_render_plan, reconcile_interrupted_generation_jobs_on_project_open,
        resume_interrupted_generate_media_job_with_client_and_credential_cancellable,
        run_generate_media_in_process_with_client_and_credential,
        run_generate_media_in_process_with_submission_builder,
        temporal_codex_edit_persist_accepted_proposal_activity_value,
        temporal_generate_media_attach_generated_asset_result_to_project_dir,
        temporal_generate_media_build_fal_generation_request_activity_value,
        temporal_generate_media_cancel_provider_activity_with_client,
        temporal_generate_media_fal_queue_submission_from_project_dir,
        temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client,
        temporal_generate_media_fal_run_submission_from_project_dir_with_client,
        temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client,
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner,
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload,
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim,
        temporal_generate_media_provider_submission_from_project_dir,
        temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client,
        temporal_generate_media_run_provider_activity_input_value,
        temporal_generate_media_start_request, temporal_job_summary,
        temporal_worker_environment_report_for_path,
        temporal_worker_environment_report_for_path_and_protoc, temporal_worker_manifest,
        temporal_workflow_client_start_plan, trim_provider_input_video_with_renderer,
        FalGenerationRunOptions, ProviderInputVideoJob, ProviderInputVideoRenderer,
        TemporalGenerateMediaBrief, TemporalGenerateMediaProviderRunOptions,
        TemporalGenerateMediaProviderSubmission, TemporalWorkflowInputError, TemporalWorkflowKind,
        VIDEO_CREATER_TEMPORAL_LOCAL_DEV_COMMAND, VIDEO_CREATER_TEMPORAL_LOCAL_SERVICE_TARGET,
        VIDEO_CREATER_TEMPORAL_TASK_QUEUE,
    };
    use crate::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
    #[cfg(feature = "ges-render")]
    use crate::edit::render_plan::generate_spoken_semantic_multi_source_edit_timeline;
    use crate::edit::render_plan::{RenderOutputProfile, RenderQuality};
    #[cfg(feature = "ges-render")]
    use crate::generation::fal::FalQueueStatusKind;
    use crate::generation::fal::{
        FalQueueCancelStatus, FAL_AURA_SR_MODEL_ID, FAL_FLUX_SCHNELL_MODEL_ID,
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
        FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
        FAL_WAN_TEXT_TO_VIDEO_MODEL_ID, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
    };
    use crate::generation::google::{GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID};
    use crate::generation::openai::{
        OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID, OPENAI_PROVIDER,
    };
    use crate::generation::replicate::{
        ReplicateGenerationRunOptions, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
        REPLICATE_SEEDANCE_20_MODEL_ID,
    };
    use crate::generation::xai::{
        XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
    };
    #[cfg(feature = "ges-render")]
    use crate::project::action::ProjectActionReplaceGeneratedOutput;
    use crate::project::action::{ProjectAction, ProjectActionGeneratedAssetOutput};
    #[cfg(feature = "ges-render")]
    use crate::project::export_options::ExportRenderOptions;
    use crate::project::export_profiles::ExportProfile;
    #[cfg(feature = "ges-render")]
    use crate::project::import::import_media_files;
    use crate::project::model::{
        GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
        GenerationModel, JobProviderRequest, JobStatus, MediaAsset, MediaKind, TimelineItem,
        TimelineItemKind, TimelineSource, VideoProject,
    };
    #[cfg(feature = "ges-render")]
    use crate::project::model::{TrackKind, Transcript, TranscriptSegment, TranscriptWord};
    #[cfg(feature = "ges-render")]
    use crate::project::split::apply_project_action_to_split_project;
    use crate::project::split::{
        load_split_project, save_split_project, split_workflow_job_index_from_project,
    };
    use crate::render_pipeline::error::PipelineResult;
    use crate::render_pipeline::probe::{AudioProbe, MediaProbe, VideoProbe};
    #[cfg(feature = "ges-render")]
    use crate::render_pipeline::process::SystemProcessRunner;
    use crate::render_pipeline::process::{CommandSpec, ProcessOutput, ProcessRunner};
    #[cfg(feature = "ges-render")]
    use crate::render_pipeline::project_export::render_media_to_split_project_folder;
    #[cfg(feature = "ges-render")]
    use crate::search::semantic_visual::SemanticVisualHit;

    struct NoopProcessRunner;

    impl ProcessRunner for NoopProcessRunner {
        fn run(&self, _spec: &CommandSpec, _timeout: Duration) -> PipelineResult<ProcessOutput> {
            panic!("NoopProcessRunner should not run a process in this test")
        }
    }

    #[test]
    fn multi_output_completion_requires_explicit_timeline_selection() {
        let output = |suffix: &str| ProjectActionGeneratedAssetOutput {
            media_id: format!("generated-output-{suffix}"),
            relative_path: format!("generated/asset/output-{suffix}.png"),
            source_url: None,
            width: 1024,
            height: 1024,
            duration_seconds: 4.0,
            fps: 1.0,
        };
        let actions = vec![ProjectAction::CompleteGeneratedAsset {
            asset_id: "asset".to_string(),
            outputs: vec![output("one"), output("two")],
            completion: None,
            replacement: None,
        }];

        assert_eq!(completed_generated_output_for_actions(&actions), None);
    }

    #[test]
    fn temporal_worker_manifest_includes_runbook_and_required_tools() {
        let manifest = temporal_worker_manifest();

        assert_eq!(manifest.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
        assert_eq!(
            manifest.local_service_target,
            VIDEO_CREATER_TEMPORAL_LOCAL_SERVICE_TARGET
        );
        assert_eq!(
            manifest.local_dev_command,
            VIDEO_CREATER_TEMPORAL_LOCAL_DEV_COMMAND
        );
        assert_eq!(manifest.local_web_ui_url, "http://localhost:8233");
        assert_eq!(
            manifest.worker_run_command,
            "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker"
        );
        assert_eq!(manifest.feature_name, "temporal-worker");
        assert!(manifest.required_tools.contains(&"protoc".to_string()));
        assert!(manifest.required_tools.contains(&"temporal".to_string()));
        assert!(!manifest.required_tools.contains(&"fal".to_string()));
    }

    #[test]
    fn codex_edit_persist_materializes_proposal_clips_without_project_actions() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-07-08T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-codex-edit".to_string(),
            "Codex Edit".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "media-1".to_string(),
            name: Some("Source interview".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 120.0,
            width: Some(1920),
            height: Some(1080),
            fps: Some(24.0),
            folder_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::CodexEdit,
            &project.id,
            "job-codex-edit",
            JobStatus::Queued,
            "2026-07-08T12:00:00Z",
        ));
        save_split_project(project_dir.path(), &project).expect("save split project");

        let request = EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Build a concise hook edit".to_string(),
            target_duration_seconds: Some(45.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-07-08T12:00:00Z".to_string(),
        };
        let input = json!({
            "projectDir": project_dir.path(),
            "jobId": "job-codex-edit",
            "updatedAt": "2026-07-08T12:05:00Z",
            "request": request,
            "proposal": {
                "mediaId": "media-1",
                "clips": [
                    { "sourceIn": 1.0, "sourceOut": 46.0, "reason": "strong opening hook" }
                ],
                "captions": [],
                "overlays": [
                    {
                        "kind": "lower_third",
                        "templateId": "kinetic-lower-third-v1",
                        "startSeconds": 0.5,
                        "durationSeconds": 2.0,
                        "fields": { "headline": "Opening Hook", "subline": "Best moment" },
                        "visualTreatment": "compact lower-third block with translucent backing",
                        "motion": "slide-and-fade in over 8 frames",
                        "safeZone": "keep essential text inside 10% margins",
                        "avoid": "full-width opaque black slabs"
                    }
                ],
                "hyperframes": [],
                "gpuVisuals": [],
                "projectActions": [],
                "renderReview": {
                    "durationSeconds": 45.0,
                    "streamCheckRequired": true,
                    "captionAlignmentRequired": true,
                    "overlayTimingRequired": true,
                    "visualFrameEvidenceRequired": true,
                    "artifactPathsRequired": true,
                    "logReferenceRequired": true
                }
            }
        });

        let result = temporal_codex_edit_persist_accepted_proposal_activity_value(input)
            .expect("persist accepted proposal");
        assert_eq!(result["projectActionCount"], json!(3));

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let video_track = reloaded
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-video")
            .expect("video track");
        let audio_track = reloaded
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-audio")
            .expect("audio track");
        let overlay_track = reloaded
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-overlays")
            .expect("overlay track");

        assert_eq!(video_track.items.len(), 1);
        assert_eq!(
            video_track.items[0].source,
            TimelineSource::Media {
                media_id: "media-1".to_string()
            }
        );
        assert_eq!(video_track.items[0].properties["sourceIn"], json!(1.0));
        assert_eq!(video_track.items[0].properties["sourceOut"], json!(46.0));
        assert_eq!(audio_track.items.len(), 1);
        assert_eq!(overlay_track.items.len(), 1);
        assert_eq!(
            overlay_track.items[0].properties["templateId"],
            json!("kinetic-lower-third-v1")
        );
        assert_eq!(reloaded.timeline.duration_seconds, 45.0);
    }

    #[test]
    fn generate_media_workflow_declares_executed_activity_sequence() {
        assert_eq!(
            TemporalWorkflowKind::GenerateMedia.activity_types(),
            &[
                "PrepareProviderInputs",
                "BuildFalGenerationRequest",
                "RunMediaProviderGeneration",
                "AttachGeneratedAssetFailure",
            ]
        );
    }

    #[test]
    fn temporal_worker_environment_report_marks_missing_required_tools() {
        let report = temporal_worker_environment_report_for_path(Some(OsStr::new("")));

        assert_eq!(report.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
        assert_eq!(
            report.worker_run_command,
            temporal_worker_manifest().worker_run_command
        );
        assert!(!report.ready);
        assert_eq!(report.tools.len(), 2);
        assert!(report.tools.iter().all(|tool| !tool.available));
        assert!(report.tools.iter().all(|tool| tool.install_hint.is_none()));
        assert!(!report.tools.iter().any(|tool| tool.name == "fal"));
    }

    #[test]
    fn temporal_worker_environment_report_requires_feature_enabled_for_readiness() {
        let tools_dir = tempfile::tempdir().expect("temp tools dir");
        write_executable_stub(&tools_dir.path().join("protoc"));
        write_executable_stub(&tools_dir.path().join("temporal"));

        let report =
            temporal_worker_environment_report_for_path(Some(tools_dir.path().as_os_str()));
        let report_json = serde_json::to_value(&report).expect("serialize report");

        assert_eq!(
            report_json["featureEnabled"],
            serde_json::json!(cfg!(feature = "temporal-worker"))
        );
        if !cfg!(feature = "temporal-worker") {
            assert!(!report.ready);
        }
    }

    #[test]
    fn temporal_worker_environment_report_accepts_protoc_environment_override() {
        let tools_dir = tempfile::tempdir().expect("temp tools dir");
        let protoc_path = tools_dir.path().join("project-protoc");
        write_executable_stub(&protoc_path);

        let report = temporal_worker_environment_report_for_path_and_protoc(
            Some(OsStr::new("")),
            Some(protoc_path.as_os_str()),
        );

        let protoc = report
            .tools
            .iter()
            .find(|tool| tool.name == "protoc")
            .expect("protoc tool status");
        let temporal = report
            .tools
            .iter()
            .find(|tool| tool.name == "temporal")
            .expect("temporal tool status");

        assert!(protoc.available);
        assert_eq!(protoc.path.as_deref(), Some(protoc_path.to_str().unwrap()));
        assert!(!temporal.available);
        assert!(!report.ready);
    }

    #[test]
    fn provider_request_credential_contract_excludes_environment_fields() {
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            "project-1",
            "generated-shot-1",
            JobStatus::Queued,
            "2026-06-25T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-shot-1",
            "generated-shot-1",
            false,
            Some(TemporalGenerateMediaBrief {
                model: Some(serde_json::json!({
                    "provider": FAL_PROVIDER,
                    "id": FAL_FLUX_SCHNELL_MODEL_ID,
                })),
                ..TemporalGenerateMediaBrief::default()
            }),
        ));

        let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");
        let runtime_start_request = &plan.input["startRequest"];

        assert_eq!(
            runtime_start_request["workflowId"],
            serde_json::json!("video-creater/project-1/generate-media/generated-shot-1")
        );
        assert_eq!(
            runtime_start_request["workflowType"],
            serde_json::json!("VideoCreaterGenerateMediaWorkflow")
        );
        assert_eq!(
            runtime_start_request["taskQueue"],
            serde_json::json!(VIDEO_CREATER_TEMPORAL_TASK_QUEUE)
        );
        assert_eq!(
            runtime_start_request["activityTypes"],
            serde_json::json!(TemporalWorkflowKind::GenerateMedia.activity_types())
        );
        assert_eq!(
            runtime_start_request["input"]["model"]["provider"],
            FAL_PROVIDER
        );
        assert!(runtime_start_request["input"]
            .get(concat!("providerCredential", "EnvVar"))
            .is_none());
        assert_eq!(
            runtime_start_request["searchAttributes"]["jobId"],
            serde_json::json!("generated-shot-1")
        );
        assert_eq!(
            runtime_start_request["input"]["assetId"],
            serde_json::json!("generated-shot-1")
        );
        assert!(!serde_json::to_string(&plan.input)
            .expect("serialize runtime input")
            .contains("test-fal-key"));

        let job_index = serde_json::to_value(split_workflow_job_index_from_project(&[job]))
            .expect("serialize workflow job index");
        assert_eq!(job_index["jobs"][0]["jobId"], "generated-shot-1");
        assert!(job_index["jobs"][0].get("credentialEnvVar").is_none());
    }

    #[test]
    fn provider_input_upload_routes_only_explicit_supported_providers() {
        assert_eq!(
            super::provider_input_upload_route(FAL_PROVIDER),
            Ok(super::ProviderInputUploadRoute::Fal)
        );
        assert_eq!(
            super::provider_input_upload_route(REPLICATE_PROVIDER),
            Ok(super::ProviderInputUploadRoute::Replicate)
        );
        for provider in [OPENAI_PROVIDER, XAI_PROVIDER, GOOGLE_PROVIDER] {
            assert_eq!(
                super::provider_input_upload_route(provider),
                Ok(super::ProviderInputUploadRoute::DataUrl)
            );
        }
    }

    #[test]
    fn provider_input_upload_rejects_unsupported_provider_before_io() {
        assert_eq!(
            super::provider_input_upload_route("future-file-provider"),
            Err(TemporalWorkflowInputError::UnsupportedProviderInputUpload(
                "future-file-provider".to_string()
            ))
        );
    }

    #[test]
    fn mock_generation_completes_without_a_provider_credential() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-07-19T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-mock".to_string(),
            "Mock Project".to_string(),
            now.clone(),
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-mock".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Mock still".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "offline mock still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: Vec::new(),
            created_at: now.clone(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-mock",
            JobStatus::Queued,
            &now,
        ));
        save_split_project(project_dir.path(), &project).expect("save mock project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-mock",
            "generated-mock",
            true,
            None,
        );

        let output = run_generate_media_in_process_with_client_and_credential(
            &reqwest::blocking::Client::new(),
            &start_request,
            "2026-07-19T12:01:00Z",
            None,
            TemporalGenerateMediaProviderRunOptions::default(),
            "",
        )
        .expect("mock generation without credential");

        assert_eq!(output.job_status, JobStatus::Completed);
        assert_eq!(output.request_id, "mock-generated-mock");
        assert_eq!(
            output.output_path,
            "generated/generated-mock/mock-output.png"
        );
    }

    #[test]
    fn generate_media_build_fal_request_activity_loads_split_project_from_runtime_input() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Queued,
            "2026-06-25T12:00:00Z",
        ));
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );

        let output = temporal_generate_media_build_fal_generation_request_activity_value(
            serde_json::json!({ "startRequest": start_request }),
        )
        .expect("build fal request activity output");

        assert_eq!(output["endpoint"], FAL_FLUX_SCHNELL_MODEL_ID);
        assert_eq!(output["provider"], FAL_PROVIDER);
        assert!(output.get("authEnvVar").is_none());
        assert_eq!(output["input"]["prompt"], "soft studio product still");
        assert_eq!(output["input"]["image_size"]["width"], 1024);
        assert_eq!(output["input"]["image_size"]["height"], 768);
        assert!(!serde_json::to_string(&output)
            .expect("serialize activity output")
            .contains("test-fal-key"));
    }

    #[test]
    fn generate_media_provider_submission_loads_replicate_asset_from_split_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-replicate".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Replicate still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: REPLICATE_PROVIDER.to_string(),
                id: REPLICATE_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-replicate",
            "generated-replicate",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("replicate provider submission");
        let output = serde_json::to_value(&submission).expect("serialize replicate submission");

        assert_eq!(output["version"], REPLICATE_FLUX_SCHNELL_MODEL_ID);
        assert_eq!(output["provider"], REPLICATE_PROVIDER);
        assert!(output.get("authEnvVar").is_none());
        assert_eq!(output["input"]["prompt"], "soft studio product still");
        assert_eq!(output["input"]["width"], 1024);
        assert_eq!(output["input"]["height"], 768);
        assert!(!serde_json::to_string(&output)
            .expect("serialize activity output")
            .contains("test-replicate-token"));
    }

    #[test]
    fn generate_media_provider_submission_loads_openai_image_asset_from_split_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-openai".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("OpenAI still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: OPENAI_PROVIDER.to_string(),
                id: OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1536),
                height: Some(864),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("1536x864".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-openai",
            "generated-openai",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("openai provider submission");
        let output = serde_json::to_value(&submission).expect("serialize openai submission");

        assert_eq!(output["model"], OPENAI_GPT_IMAGE_2_MODEL_ID);
        assert_eq!(output["provider"], OPENAI_PROVIDER);
        assert!(output.get("authEnvVar").is_none());
        assert_eq!(output["input"]["prompt"], "soft studio product still");
        assert_eq!(output["input"]["size"], "1536x864");
        assert_eq!(output["input"]["output_format"], "png");
        assert!(!serde_json::to_string(&output)
            .expect("serialize activity output")
            .contains("test-openai-token"));
    }

    #[test]
    fn generate_media_provider_submission_loads_xai_image_asset_from_split_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-xai".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Grok still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "fast product still iteration".to_string(),
            model: GenerationModel {
                provider: XAI_PROVIDER.to_string(),
                id: XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                aspect_ratio: Some("16:9".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-xai",
            "generated-xai",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("xai provider submission");
        let output = serde_json::to_value(&submission).expect("serialize xai submission");

        assert_eq!(output["model"], XAI_GROK_IMAGE_QUALITY_MODEL_ID);
        assert_eq!(output["provider"], XAI_PROVIDER);
        assert!(output.get("authEnvVar").is_none());
        assert_eq!(output["url"], "https://api.x.ai/v1/images/generations");
        assert_eq!(output["input"]["prompt"], "fast product still iteration");
        assert_eq!(output["input"]["aspect_ratio"], "16:9");
        assert_eq!(output["input"]["response_format"], "url");
        assert!(!serde_json::to_string(&output)
            .expect("serialize activity output")
            .contains("test-xai-token"));
    }

    #[test]
    fn generate_media_prepare_provider_inputs_encodes_xai_image_edit_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(
            project_dir.path().join("media/product.png"),
            b"fake-product",
        )
        .expect("product image");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "product-ref".to_string(),
            name: Some("Product reference".to_string()),
            relative_path: "media/product.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-xai-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Grok edited product".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Turn this into a fast pencil sketch".to_string(),
            model: GenerationModel {
                provider: XAI_PROVIDER.to_string(),
                id: XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["product-ref".to_string()],
                reference_image_media_refs: vec!["product-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                aspect_ratio: Some("1:1".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-xai-edit",
            "generated-xai-edit",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
            &start_request,
            "2026-06-25T12:02:00Z",
            &NoopProcessRunner,
        )
        .expect("prepare xAI edit inputs");

        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("xAI edit provider submission");
        let output = serde_json::to_value(&submission).expect("serialize xAI edit submission");

        assert_eq!(output["model"], XAI_GROK_IMAGE_QUALITY_MODEL_ID);
        assert_eq!(output["url"], "https://api.x.ai/v1/images/edits");
        assert_eq!(
            output["input"]["image"]["url"],
            "data:image/png;base64,ZmFrZS1wcm9kdWN0"
        );
        assert_eq!(output["input"]["image"]["type"], "image_url");
        assert_eq!(output["input"]["aspect_ratio"], "1:1");
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_xai_video_first_frame() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/first.png"), b"fake-first").expect("first frame");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "first-frame".to_string(),
            name: Some("First frame".to_string()),
            relative_path: "media/first.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-xai-video".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Grok video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate the product hero frame".to_string(),
            model: GenerationModel {
                provider: XAI_PROVIDER.to_string(),
                id: XAI_GROK_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["first-frame".to_string()],
                first_frame_media_id: Some("first-frame".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(6.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: Some(true),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-xai-video",
            "generated-xai-video",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);

        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, XAI_PROVIDER);
                assert_eq!(credential_provider, XAI_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://api.x.ai/v1/files/generated-xai-video/{file_name}"
                ))
            },
        )
        .expect("prepare xAI video inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-xai-video")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://api.x.ai/v1/files/generated-xai-video/first.png".to_string()]
        );
        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("xAI video provider submission");
        let output = serde_json::to_value(&submission).expect("serialize xAI video submission");

        assert_eq!(output["model"], XAI_GROK_VIDEO_MODEL_ID);
        assert_eq!(output["url"], "https://api.x.ai/v1/videos/generations");
        assert_eq!(
            output["input"]["image"]["url"],
            "https://api.x.ai/v1/files/generated-xai-video/first.png"
        );
        assert_eq!(output["input"]["image"]["type"], "image_url");
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-xai-video/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["first.png".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_xai_video_source_edit() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(
            project_dir.path().join("media/source.mp4"),
            b"fake-source-video",
        )
        .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 6.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-xai-video-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Grok video edit".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Give the source clip a clean studio background".to_string(),
            model: GenerationModel {
                provider: XAI_PROVIDER.to_string(),
                id: XAI_GROK_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(6.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: Some(true),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-xai-video-edit",
            "generated-xai-video-edit",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);

        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, XAI_PROVIDER);
                assert_eq!(credential_provider, XAI_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://api.x.ai/v1/files/generated-xai-video-edit/{file_name}"
                ))
            },
        )
        .expect("prepare xAI video edit inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-xai-video-edit")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://api.x.ai/v1/files/generated-xai-video-edit/source.mp4".to_string()]
        );
        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("xAI video edit provider submission");
        let output =
            serde_json::to_value(&submission).expect("serialize xAI video edit submission");

        assert_eq!(output["model"], XAI_GROK_VIDEO_MODEL_ID);
        assert_eq!(output["url"], "https://api.x.ai/v1/videos/edits");
        assert_eq!(
            output["input"],
            json!({
                "prompt": "Give the source clip a clean studio background",
                "video": {
                    "url": "https://api.x.ai/v1/files/generated-xai-video-edit/source.mp4"
                }
            })
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-xai-video-edit/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["source.mp4".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_encodes_google_veo_image_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/first.png"), b"fake-first").expect("first frame");
        fs::write(project_dir.path().join("media/last.png"), b"fake-last").expect("last frame");
        fs::write(project_dir.path().join("media/style.png"), b"fake-style").expect("style ref");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "first-frame".to_string(),
            name: Some("First frame".to_string()),
            relative_path: "media/first.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "last-frame".to_string(),
            name: Some("Last frame".to_string()),
            relative_path: "media/last.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "style-ref".to_string(),
            name: Some("Style reference".to_string()),
            relative_path: "media/style.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-google-veo".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Veo reference video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate the product between keyframes with style reference".to_string(),
            model: GenerationModel {
                provider: GOOGLE_PROVIDER.to_string(),
                id: GOOGLE_VEO_31_FAST_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec![
                    "first-frame".to_string(),
                    "last-frame".to_string(),
                    "style-ref".to_string(),
                ],
                first_frame_media_id: Some("first-frame".to_string()),
                last_frame_media_id: Some("last-frame".to_string()),
                reference_image_media_refs: vec!["style-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(8.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: Some(true),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-google-veo",
            "generated-google-veo",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
            &start_request,
            "2026-06-25T12:02:00Z",
            &NoopProcessRunner,
        )
        .expect("prepare Google Veo inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-google-veo")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "data:image/png;base64,ZmFrZS1maXJzdA==".to_string(),
                "data:image/png;base64,ZmFrZS1sYXN0".to_string(),
                "data:image/png;base64,ZmFrZS1zdHlsZQ==".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("Google Veo provider submission");
        let output = serde_json::to_value(&submission).expect("serialize Google Veo submission");

        assert_eq!(output["model"], GOOGLE_VEO_31_FAST_MODEL_ID);
        assert_eq!(
            output["input"]["instances"][0]["image"]["inlineData"]["data"],
            "ZmFrZS1maXJzdA=="
        );
        assert_eq!(
            output["input"]["instances"][0]["lastFrame"]["image"]["inlineData"]["data"],
            "ZmFrZS1sYXN0"
        );
        assert_eq!(
            output["input"]["instances"][0]["referenceImages"][0]["image"]["inlineData"]["data"],
            "ZmFrZS1zdHlsZQ=="
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_encodes_openai_image_edit_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/style.png"), b"fake-style").expect("style image");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "style-ref".to_string(),
            name: Some("Style reference".to_string()),
            relative_path: "media/style.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-openai-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Edited product image".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Use the reference scene but keep the product".to_string(),
            model: GenerationModel {
                provider: OPENAI_PROVIDER.to_string(),
                id: OPENAI_GPT_IMAGE_EDIT_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["style-ref".to_string()],
                reference_image_media_refs: vec!["style-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: Some("1024x1024".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-openai-edit",
            "generated-openai-edit",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );

        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
            &start_request,
            "2026-06-25T12:02:00Z",
            &NoopProcessRunner,
        )
        .expect("prepare OpenAI edit inputs");

        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("openai edit provider submission");
        let output = serde_json::to_value(&submission).expect("serialize openai edit submission");

        assert_eq!(output["model"], OPENAI_GPT_IMAGE_EDIT_MODEL_ID);
        assert_eq!(output["url"], "https://api.openai.com/v1/images/edits");
        assert_eq!(
            output["input"]["images"][0]["image_url"],
            "data:image/png;base64,ZmFrZS1zdHlsZQ=="
        );
        assert_eq!(output["input"]["size"], "1024x1024");
        assert!(!serde_json::to_string(&output)
            .expect("serialize activity output")
            .contains("test-openai-token"));
    }

    #[test]
    fn generate_media_openai_run_and_attach_persists_provider_request_from_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-openai".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("OpenAI still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: OPENAI_PROVIDER.to_string(),
                id: OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: Some("1024x1024".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-openai",
            "generated-openai",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-openai",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(start_request.clone());
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let mut submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("project-dir openai submission");
        let Some(server) = FakeOpenAiServer::start() else {
            return;
        };
        let request_url = format!("{}/v1/images/generations", server.base_url);
        if let TemporalGenerateMediaProviderSubmission::OpenAi(openai_submission) = &mut submission
        {
            openai_submission.url = request_url.clone();
        } else {
            panic!("expected openai submission");
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let result =
            temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client(
                &client,
                &start_request,
                match &submission {
                    TemporalGenerateMediaProviderSubmission::OpenAi(submission) => submission,
                    _ => panic!("expected openai submission"),
                },
                "test-openai-token",
                "2026-06-25T12:05:00Z",
                Some("temporal-run-openai-1"),
            )
            .expect("run and attach openai result");
        let requests = server.join();

        assert_eq!(result.run.request_id, "img-123");
        assert_eq!(
            result.run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-openai/openai-output.png")
        );
        assert_eq!(
            fs::read(&result.run.completion.output_path).expect("downloaded openai output"),
            b"fake-png"
        );
        let generated_asset = result
            .write
            .project
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-openai")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(generated_asset.outputs.len(), 1);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-openai/openai-output.png"
        );
        let job = result
            .write
            .project
            .jobs
            .iter()
            .find(|job| job.id == "generated-openai")
            .expect("job");
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("temporal-run-openai-1")
        );
        let provider_request = job
            .provider_request
            .as_ref()
            .expect("persist provider request");
        assert_eq!(provider_request.provider, OPENAI_PROVIDER);
        assert_eq!(provider_request.request_id, "img-123");
        assert_eq!(provider_request.status_url, request_url);
        assert_eq!(provider_request.response_url, request_url);
        assert_eq!(provider_request.cancel_url, request_url);
        assert_eq!(provider_request.submitted_at, "2026-06-25T12:05:00Z");
        assert!(!serde_json::to_string(&result.write.project)
            .expect("serialize written project")
            .contains("test-openai-token"));

        let submit = requests
            .iter()
            .find(|request| request.path == "/v1/images/generations")
            .expect("submit request");
        assert_eq!(submit.method, "POST");
        assert_eq!(
            submit.authorization.as_deref(),
            Some("Bearer test-openai-token")
        );
        assert!(submit.body.contains("soft studio product still"));
        assert!(submit.body.contains("\"model\":\"gpt-image-2\""));
        assert!(!submit.body.contains("test-openai-token"));
    }

    #[test]
    fn generate_media_run_provider_activity_input_uses_workflow_runtime_context() {
        let start_request = temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-shot-1",
            "generated-shot-1",
            false,
            None,
        );
        let submission = serde_json::json!({
            "method": "POST",
            "url": "https://queue.fal.run/fal-ai/flux/schnell",
            "endpoint": FAL_FLUX_SCHNELL_MODEL_ID,
            "provider": FAL_PROVIDER,
            "input": {
                "prompt": "soft studio product still",
                "image_size": {
                    "width": 1024,
                    "height": 768
                }
            }
        });

        let activity_input = temporal_generate_media_run_provider_activity_input_value(
            &serde_json::json!({ "startRequest": start_request }),
            submission,
            "2026-06-25T12:05:00Z",
            Some("temporal-run-1"),
        )
        .expect("run provider input");

        assert_eq!(
            activity_input["startRequest"]["workflowType"],
            serde_json::json!("VideoCreaterGenerateMediaWorkflow")
        );
        assert_eq!(
            activity_input["startRequest"]["input"]["assetId"],
            serde_json::json!("generated-shot-1")
        );
        assert_eq!(
            activity_input["submission"]["endpoint"],
            serde_json::json!(FAL_FLUX_SCHNELL_MODEL_ID)
        );
        assert_eq!(
            activity_input["submission"]["provider"],
            serde_json::json!(FAL_PROVIDER)
        );
        assert!(activity_input["submission"].get("authEnvVar").is_none());
        assert_eq!(
            activity_input["updatedAt"],
            serde_json::json!("2026-06-25T12:05:00Z")
        );
        assert_eq!(activity_input["runId"], serde_json::json!("temporal-run-1"));
        assert_eq!(activity_input["options"]["maxStatusPolls"], 60);
        assert_eq!(activity_input["options"]["pollIntervalMillis"], 5000);
        assert!(!serde_json::to_string(&activity_input)
            .expect("serialize activity input")
            .contains("test-fal-key"));
    }

    #[test]
    fn generate_media_run_provider_activity_input_accepts_replicate_submission() {
        let start_request = temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-replicate",
            "generated-replicate",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let submission = serde_json::json!({
            "method": "POST",
            "url": "https://api.replicate.com/v1/predictions",
            "version": REPLICATE_FLUX_SCHNELL_MODEL_ID,
            "provider": REPLICATE_PROVIDER,
            "input": {
                "prompt": "soft studio product still",
                "width": 1024,
                "height": 768,
                "num_outputs": 1,
                "output_format": "png"
            }
        });

        let activity_input = temporal_generate_media_run_provider_activity_input_value(
            &serde_json::json!({ "startRequest": start_request }),
            submission,
            "2026-06-25T12:05:00Z",
            Some("temporal-run-1"),
        )
        .expect("run provider input");

        assert_eq!(
            activity_input["submission"]["version"],
            serde_json::json!(REPLICATE_FLUX_SCHNELL_MODEL_ID)
        );
        assert_eq!(
            activity_input["submission"]["provider"],
            serde_json::json!(REPLICATE_PROVIDER)
        );
        assert!(activity_input["submission"].get("authEnvVar").is_none());
        assert_eq!(
            activity_input["updatedAt"],
            serde_json::json!("2026-06-25T12:05:00Z")
        );
        assert!(!serde_json::to_string(&activity_input)
            .expect("serialize activity input")
            .contains("test-replicate-token"));
    }

    #[test]
    fn generate_media_fal_submission_loads_asset_from_split_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );

        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");

        assert_eq!(submission.endpoint, FAL_FLUX_SCHNELL_MODEL_ID);
        assert_eq!(submission.provider, FAL_PROVIDER);
        assert_eq!(submission.input["prompt"], "soft studio product still");
        assert_eq!(submission.input["image_size"]["width"], 1024);
        assert_eq!(submission.input["image_size"]["height"], 768);
        assert!(!serde_json::to_string(&submission)
            .expect("serialize submission")
            .contains("Authorization"));
    }

    #[test]
    fn generate_media_fal_run_loads_project_dir_and_returns_completion_actions() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Queued,
            "2026-06-25T12:00:00Z",
        ));
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );
        let mut submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");
        let Some(server) = FakeFalServer::start() else {
            return;
        };
        submission.url = format!("{}/{}", server.base_url, FAL_FLUX_SCHNELL_MODEL_ID);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let run = temporal_generate_media_fal_run_submission_from_project_dir_with_client(
            &client,
            &start_request,
            &submission,
            "test-fal-key",
            "2026-06-25T12:05:00Z",
            Some("temporal-run-1"),
            FalGenerationRunOptions {
                max_status_polls: 2,
                poll_interval: std::time::Duration::from_millis(0),
            },
        )
        .expect("project-dir fal run");
        let requests = server.join();

        assert_eq!(run.request_id, "req-1");
        assert_eq!(
            run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-hero/fal-output.png")
        );
        assert_eq!(
            fs::read(&run.completion.output_path).expect("downloaded fal output"),
            tiny_png_fixture()
        );
        assert_eq!(run.completion.actions.len(), 2);
        let actions_json =
            serde_json::to_string(&run.completion.actions).expect("serialize actions");
        assert!(actions_json.contains("completeGeneratedAsset"));
        assert!(actions_json.contains("generated/generated-hero/fal-output.png"));
        assert!(actions_json.contains("temporal-run-1"));
        assert!(!actions_json.contains("test-fal-key"));

        let submit = requests
            .iter()
            .find(|request| request.path == "/fal-ai/flux/schnell")
            .expect("submit request");
        assert_eq!(submit.method, "POST");
        assert!(submit.authorization.as_deref() == Some("Key test-fal-key"));
        assert!(submit.body.contains("soft studio product still"));
        assert!(!submit.body.contains("test-fal-key"));

        let media = requests
            .iter()
            .find(|request| request.path == "/media/fal-output.png")
            .expect("media request");
        assert!(media.authorization.is_none());
    }

    #[test]
    fn attach_generated_asset_result_applies_completion_actions_to_split_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );
        let actions = vec![
            ProjectAction::CompleteGeneratedAsset {
                asset_id: "generated-hero".to_string(),
                outputs: vec![ProjectActionGeneratedAssetOutput {
                    media_id: "generated-hero-fal-output".to_string(),
                    relative_path: "generated/generated-hero/fal-output.png".to_string(),
                    source_url: None,
                    width: 1024,
                    height: 768,
                    duration_seconds: 1.0,
                    fps: 1.0,
                }],
                completion: None,
                replacement: None,
            },
            ProjectAction::UpdateJobStatus {
                job_id: "generated-hero".to_string(),
                status: JobStatus::Completed,
                updated_at: "2026-06-25T12:05:00Z".to_string(),
                run_id: Some("temporal-run-1".to_string()),
            },
        ];

        let result = temporal_generate_media_attach_generated_asset_result_to_project_dir(
            &start_request,
            actions,
        )
        .expect("attach generated asset result");
        let reloaded = load_split_project(project_dir.path()).expect("reload split project");

        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-hero")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(generated_asset.outputs.len(), 1);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-hero/fal-output.png"
        );
        assert!(reloaded
            .media
            .iter()
            .any(|media| media.id == "generated-hero-fal-output"));
        let job = reloaded
            .jobs
            .iter()
            .find(|job| job.id == "generated-hero")
            .expect("job");
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("temporal-run-1")
        );
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-hero/asset.json")));
    }

    #[test]
    fn attach_generated_asset_result_inserts_timeline_visual_span() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-video-insert".to_string(),
            "Visual Insert Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Generated video".to_string()),
            target_folder_id: None,
            placement_intent: Some("insert-video:track-video".to_string()),
            prompt: "five second product launch insert".to_string(),
            model: GenerationModel {
                provider: REPLICATE_PROVIDER.to_string(),
                id: REPLICATE_SEEDANCE_20_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(5.0),
                fps: Some(24.0),
                timeline_start_seconds: Some(6.0),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video",
            "generated-video",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video",
            "generated-video",
            false,
            None,
        );
        let actions = vec![
            ProjectAction::CompleteGeneratedAsset {
                asset_id: "generated-video".to_string(),
                outputs: vec![ProjectActionGeneratedAssetOutput {
                    media_id: "generated-video-replicate-output".to_string(),
                    relative_path: "generated/generated-video/replicate-output.mp4".to_string(),
                    source_url: Some(
                        "https://replicate.delivery/provider-e2e/generated-video.mp4".to_string(),
                    ),
                    width: 1280,
                    height: 720,
                    duration_seconds: 5.0,
                    fps: 24.0,
                }],
                completion: None,
                replacement: None,
            },
            ProjectAction::UpdateJobStatus {
                job_id: "generated-video".to_string(),
                status: JobStatus::Completed,
                updated_at: "2026-06-25T12:05:00Z".to_string(),
                run_id: Some("temporal-run-video-1".to_string()),
            },
        ];

        temporal_generate_media_attach_generated_asset_result_to_project_dir(
            &start_request,
            actions,
        )
        .expect("attach generated visual result");
        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let video_track = reloaded
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-video")
            .expect("video track");
        let inserted = video_track
            .items
            .iter()
            .find(|item| item.id == "generated-video-timeline-visual")
            .expect("inserted generated visual item");

        assert_eq!(inserted.kind, TimelineItemKind::VideoClip);
        assert_eq!(inserted.start_seconds, 6.0);
        assert_eq!(inserted.duration_seconds, 5.0);
        assert_eq!(
            inserted.source,
            TimelineSource::Media {
                media_id: "generated-video-replicate-output".to_string()
            }
        );
        assert_eq!(
            inserted.properties.get("generatedAssetId"),
            Some(&json!("generated-video"))
        );
        assert_eq!(
            inserted.properties.get("generatedOutputMediaId"),
            Some(&json!("generated-video-replicate-output"))
        );
        assert_eq!(inserted.properties.get("sourceIn"), Some(&json!(0.0)));
        assert_eq!(inserted.properties.get("sourceOut"), Some(&json!(5.0)));
    }

    #[test]
    fn in_process_generate_media_failure_is_durable_and_retryable() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );

        let client = reqwest::blocking::Client::new();
        let error = run_generate_media_in_process_with_submission_builder(
            &client,
            &start_request,
            "2026-06-25T12:07:00Z",
            Some("in-process-run-1"),
            TemporalGenerateMediaProviderRunOptions::default(),
            "test-fal-key",
            || {
                Err(super::TemporalWorkflowInputError::BlankField(
                    "fixture".to_string(),
                ))
            },
        )
        .expect_err("fixture provider failure");
        assert!(error.to_string().contains("fixture"));
        let reloaded = load_split_project(project_dir.path()).expect("reload split project");

        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-hero")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Failed);
        assert!(generated_asset.outputs.is_empty());
        let job = reloaded
            .jobs
            .iter()
            .find(|job| job.id == "generated-hero")
            .expect("job");
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.updated_at, "2026-06-25T12:07:00Z");
        assert_eq!(
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("in-process-run-1")
        );
        assert!(
            job.start_request.is_some(),
            "failed jobs retain their typed retry request"
        );
    }

    #[test]
    fn generate_media_fal_run_and_attach_completes_split_project_from_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );
        let mut submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");
        let Some(server) = FakeFalServer::start() else {
            return;
        };
        submission.url = format!("{}/{}", server.base_url, FAL_FLUX_SCHNELL_MODEL_ID);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let result =
            temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
                &client,
                &start_request,
                &submission,
                "test-fal-key",
                "2026-06-25T12:05:00Z",
                Some("temporal-run-1"),
                FalGenerationRunOptions {
                    max_status_polls: 2,
                    poll_interval: std::time::Duration::from_millis(0),
                },
            )
            .expect("run and attach fal result");
        let requests = server.join();

        assert_eq!(result.run.request_id, "req-1");
        assert_eq!(
            result.run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-hero/fal-output.png")
        );
        assert_eq!(
            fs::read(&result.run.completion.output_path).expect("downloaded fal output"),
            tiny_png_fixture()
        );
        let generated_asset = result
            .write
            .project
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-hero")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(generated_asset.outputs.len(), 1);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-hero/fal-output.png"
        );
        assert!(result
            .write
            .project
            .media
            .iter()
            .any(|media| media.id == "generated-hero-fal-output"));
        let job = result
            .write
            .project
            .jobs
            .iter()
            .find(|job| job.id == "generated-hero")
            .expect("job");
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("temporal-run-1")
        );
        assert!(!serde_json::to_string(&result.write.project)
            .expect("serialize written project")
            .contains("test-fal-key"));

        let submit = requests
            .iter()
            .find(|request| request.path == "/fal-ai/flux/schnell")
            .expect("submit request");
        assert_eq!(submit.method, "POST");
        assert!(submit.authorization.as_deref() == Some("Key test-fal-key"));
        assert!(submit.body.contains("soft studio product still"));
        assert!(!submit.body.contains("test-fal-key"));
    }

    #[test]
    fn generate_media_fal_run_and_attach_inserts_timeline_audio_span() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-audio-span".to_string(),
            "Audio Span Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-audio".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Video-synced music".to_string()),
            target_folder_id: None,
            placement_intent: Some("timeline".to_string()),
            prompt: "score the selected source-video span".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: vec![
                    "https://v3.fal.media/files/generated-video-audio/source.mp4".to_string(),
                ],
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                duration_seconds: Some(3.5),
                video_source_start_seconds: Some(2.0),
                video_source_end_seconds: Some(5.5),
                timeline_start_seconds: Some(8.25),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-audio",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-audio",
            "generated-video-audio",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-audio",
            "generated-video-audio",
            false,
            None,
        );
        let mut submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");
        let Some(server) = FakeFalServer::start() else {
            return;
        };
        submission.url = format!("{}/{}", server.base_url, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let result =
            temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
                &client,
                &start_request,
                &submission,
                "test-fal-key",
                "2026-06-25T12:05:00Z",
                Some("temporal-run-audio-1"),
                FalGenerationRunOptions {
                    max_status_polls: 2,
                    poll_interval: std::time::Duration::from_millis(0),
                },
            )
            .expect("run and attach fal audio result");
        let requests = server.join();

        assert_eq!(result.run.request_id, "audio-req-1");
        assert_eq!(
            result.run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-video-audio/fal-output.m4a")
        );
        assert_eq!(
            fs::read(&result.run.completion.output_path).expect("downloaded fal audio output"),
            b"fake-m4a"
        );
        let generated_asset = result
            .write
            .project
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-audio")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-video-audio/fal-output.m4a"
        );
        let audio_track = result
            .write
            .project
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-audio")
            .expect("audio track");
        let inserted = audio_track
            .items
            .iter()
            .find(|item| item.id == "generated-video-audio-timeline-audio")
            .expect("inserted generated audio item");
        assert_eq!(inserted.kind, TimelineItemKind::AudioClip);
        assert_eq!(inserted.start_seconds, 8.25);
        assert_eq!(inserted.duration_seconds, 3.5);
        assert_eq!(
            inserted.source,
            TimelineSource::Media {
                media_id: "generated-video-audio-fal-output".to_string()
            }
        );
        assert_eq!(
            inserted.properties.get("generatedAssetId"),
            Some(&json!("generated-video-audio"))
        );
        assert_eq!(
            inserted.properties.get("generatedOutputMediaId"),
            Some(&json!("generated-video-audio-fal-output"))
        );
        assert_eq!(inserted.properties.get("sourceIn"), Some(&json!(0.0)));
        assert_eq!(inserted.properties.get("sourceOut"), Some(&json!(3.5)));

        let submit = requests
            .iter()
            .find(|request| request.path == "/sonilo/v1.1/video-to-music")
            .expect("audio submit request");
        assert_eq!(submit.method, "POST");
        assert!(submit
            .body
            .contains("https://v3.fal.media/files/generated-video-audio/source.mp4"));
        assert!(!submit.body.contains("test-fal-key"));
    }

    #[test]
    fn generate_media_replicate_run_and_attach_completes_split_project_from_project_dir() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-replicate".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Replicate still".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: REPLICATE_PROVIDER.to_string(),
                id: REPLICATE_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-replicate",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-replicate",
            "generated-replicate",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-replicate",
            "generated-replicate",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let mut submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("project-dir replicate submission");
        let Some(server) = FakeReplicateServer::start() else {
            return;
        };
        if let TemporalGenerateMediaProviderSubmission::Replicate(replicate_submission) =
            &mut submission
        {
            replicate_submission.url = format!("{}/v1/predictions", server.base_url);
        } else {
            panic!("expected replicate submission");
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let result =
            temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client(
                &client,
                &start_request,
                match &submission {
                    TemporalGenerateMediaProviderSubmission::Replicate(submission) => submission,
                    _ => panic!("expected replicate submission"),
                },
                "test-replicate-token",
                "2026-06-25T12:05:00Z",
                Some("temporal-run-replicate-1"),
                ReplicateGenerationRunOptions {
                    max_status_polls: 2,
                    poll_interval: std::time::Duration::from_millis(0),
                },
            )
            .expect("run and attach replicate result");
        let requests = server.join();

        assert_eq!(result.run.prediction_id, "pred-123");
        assert_eq!(
            result.run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-replicate/replicate-output.png")
        );
        assert_eq!(
            fs::read(&result.run.completion.output_path).expect("downloaded replicate output"),
            b"fake-png"
        );
        let generated_asset = result
            .write
            .project
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-replicate")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(generated_asset.outputs.len(), 1);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-replicate/replicate-output.png"
        );
        assert!(result
            .write
            .project
            .media
            .iter()
            .any(|media| media.id == "generated-replicate-replicate-output"));
        let job = result
            .write
            .project
            .jobs
            .iter()
            .find(|job| job.id == "generated-replicate")
            .expect("job");
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("temporal-run-replicate-1")
        );
        let project_json =
            serde_json::to_string(&result.write.project).expect("serialize written project");
        assert!(!project_json.contains("test-replicate-token"));

        let submit = requests
            .iter()
            .find(|request| request.path == "/v1/predictions")
            .expect("submit request");
        assert_eq!(submit.method, "POST");
        assert_eq!(
            submit.authorization.as_deref(),
            Some("Bearer test-replicate-token")
        );
        assert!(submit.body.contains("soft studio product still"));
        assert!(!submit.body.contains("test-replicate-token"));
        assert!(requests
            .iter()
            .any(|request| request.path == "/v1/predictions/pred-123"));
        assert!(requests
            .iter()
            .any(|request| request.path == "/media/replicate-output.png"));
    }

    #[test]
    fn generate_media_fal_run_and_attach_completes_aura_sr_image_upscale() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-image".to_string(),
            name: Some("Source image".to_string()),
            relative_path: "media/source.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(768),
            fps: None,
            folder_id: None,
        });
        project.timeline.duration_seconds = 4.0;
        project.timeline.tracks[0].items.push(TimelineItem {
            id: "item-upscale-target".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "source-image".to_string(),
            },
            label: "Source image".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(0.0)),
                ("sourceOut".to_string(), serde_json::json!(4.0)),
            ]),
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-upscale".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Upscaled source image".to_string()),
            target_folder_id: None,
            placement_intent: Some("replace:item-upscale-target".to_string()),
            prompt: "Upscale Source image".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_AURA_SR_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-image".to_string()],
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: vec!["https://fal.media/uploads/source.png".to_string()],
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(2048),
                height: Some(1536),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-upscale",
            JobStatus::Running,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-upscale",
            "generated-upscale",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-upscale",
            "generated-upscale",
            false,
            None,
        );
        let mut submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");
        let Some(server) = FakeFalServer::start() else {
            return;
        };
        let base_url = server.base_url.clone();
        submission.url = format!("{}/{}", server.base_url, FAL_AURA_SR_MODEL_ID);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");

        let result =
            temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
                &client,
                &start_request,
                &submission,
                "test-fal-key",
                "2026-06-25T12:05:00Z",
                Some("temporal-run-1"),
                FalGenerationRunOptions {
                    max_status_polls: 2,
                    poll_interval: std::time::Duration::from_millis(0),
                },
            )
            .expect("run and attach aura sr result");
        let requests = server.join();

        assert_eq!(result.run.request_id, "req-1");
        assert_eq!(
            result.run.completion.output_path,
            project_dir
                .path()
                .join("generated/generated-upscale/fal-output.png")
        );
        assert_eq!(
            fs::read(&result.run.completion.output_path).expect("downloaded fal output"),
            tiny_png_fixture()
        );
        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-upscale")
            .expect("generated asset");
        assert_eq!(generated_asset.status, GeneratedAssetStatus::Completed);
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://fal.media/uploads/source.png".to_string()]
        );
        assert_eq!(generated_asset.outputs.len(), 1);
        assert_eq!(generated_asset.outputs[0].width, 2048);
        assert_eq!(generated_asset.outputs[0].height, 1536);
        assert_eq!(
            generated_asset.outputs[0].relative_path,
            "generated/generated-upscale/fal-output.png"
        );
        let replaced_item = reloaded.timeline.tracks[0]
            .items
            .iter()
            .find(|item| item.id == "item-upscale-target")
            .expect("replacement target item");
        assert_eq!(replaced_item.duration_seconds, 4.0);
        assert_eq!(
            replaced_item.source,
            TimelineSource::Media {
                media_id: generated_asset.outputs[0].media_id.clone()
            }
        );
        assert_eq!(
            replaced_item.properties["sourceOut"],
            serde_json::json!(4.0)
        );
        assert_eq!(
            replaced_item.properties["generatedOutputMediaId"],
            serde_json::json!(generated_asset.outputs[0].media_id.clone())
        );
        let provider_request = reloaded
            .jobs
            .iter()
            .find(|job| job.id == "generated-upscale")
            .and_then(|job| job.provider_request.as_ref())
            .expect("persist provider request");
        assert_eq!(provider_request.provider, FAL_PROVIDER);
        assert_eq!(
            provider_request.response_url,
            format!("{base_url}/result/req-1")
        );

        let submit = requests
            .iter()
            .find(|request| request.path == "/fal-ai/aura-sr")
            .expect("aura sr submit request");
        assert_eq!(submit.method, "POST");
        assert!(submit.authorization.as_deref() == Some("Key test-fal-key"));
        assert!(submit
            .body
            .contains("\"image_url\":\"https://fal.media/uploads/source.png\""));
        assert!(submit.body.contains("\"upscale_factor\":4"));
        assert!(!submit.body.contains("test-fal-key"));
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_aura_sr_local_media() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.png"), b"fake-png").expect("source image");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-image".to_string(),
            name: Some("Source image".to_string()),
            relative_path: "media/source.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(768),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-upscale".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Upscaled source image".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Upscale Source image".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_AURA_SR_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-image".to_string()],
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(2048),
                height: Some(1536),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-upscale",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-upscale",
            "generated-upscale",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-upscale",
            "generated-upscale",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(source_path.display().to_string());
                Ok("https://v3.fal.media/files/generated-upscale/source.png".to_string())
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-upscale")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-upscale/source.png".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_AURA_SR_MODEL_ID);
        assert_eq!(
            submission.input["image_url"],
            "https://v3.fal.media/files/generated-upscale/source.png"
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-upscale/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &[project_dir
                .path()
                .join("media/source.png")
                .display()
                .to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_fal_video_upscale_source() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-upscale".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Upscaled source video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Upscale Source video".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_VIDEO_UPSCALER_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(2560),
                height: Some(1440),
                duration_seconds: Some(10.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-upscale",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-upscale",
            "generated-video-upscale",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-upscale",
            "generated-video-upscale",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-video-upscale/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-upscale")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-video-upscale/source.mp4".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_VIDEO_UPSCALER_MODEL_ID);
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-video-upscale/source.mp4"
        );
        assert_eq!(submission.input["scale"], json!(2));
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-video-upscale/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["source.mp4".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_trims_fal_video_upscale_source_from_asset_settings() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-upscale".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Upscaled trimmed source video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Upscale trimmed Source video".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_VIDEO_UPSCALER_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(2560),
                height: Some(1440),
                duration_seconds: Some(2.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                video_source_start_seconds: Some(1.25),
                video_source_end_seconds: Some(3.25),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-upscale",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-upscale",
            "generated-video-upscale",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-upscale",
            "generated-video-upscale",
            false,
            None,
        );
        let trim_calls = Arc::new(Mutex::new(Vec::new()));
        let trim_calls_for_closure = Arc::clone(&trim_calls);
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write =
            temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
                &start_request,
                |provider: &str, source_path: &Path, credential_provider: &str| {
                    assert_eq!(provider, FAL_PROVIDER);
                    assert_eq!(credential_provider, FAL_PROVIDER);
                    uploaded_sources_for_closure
                        .lock()
                        .expect("uploaded sources")
                        .push(source_path.display().to_string());
                    Ok(
                        "https://v3.fal.media/files/generated-video-upscale/trimmed-source.mp4"
                            .to_string(),
                    )
                },
                |source_path: &Path, output_path: &Path, source_in: f64, source_out: f64| {
                    trim_calls_for_closure.lock().expect("trim calls").push((
                        source_path.display().to_string(),
                        output_path.display().to_string(),
                        source_in,
                        source_out,
                    ));
                    fs::write(output_path, b"trimmed-video").expect("trimmed output");
                    Ok(())
                },
                |_source_path: &Path, output_path: &Path| {
                    fs::write(output_path, b"compressed-video").expect("compressed output");
                    Ok(())
                },
                |_project_dir: &Path,
                 _project: &VideoProject,
                 _asset: &GeneratedAsset,
                 output_path: &Path,
                 _start_seconds: f64,
                 _end_seconds: f64| {
                    fs::write(output_path, b"rendered-timeline-provider-input")
                        .expect("rendered timeline input");
                    Ok(())
                },
            )
            .expect("prepare provider inputs");

        let trim_calls = trim_calls.lock().expect("trim calls");
        assert_eq!(trim_calls.len(), 1);
        assert!(trim_calls[0].0.ends_with("media/source.mp4"));
        assert_eq!(trim_calls[0].2, 1.25);
        assert_eq!(trim_calls[0].3, 3.25);
        let uploaded_sources = uploaded_sources.lock().expect("uploaded sources");
        assert_eq!(uploaded_sources.len(), 1);
        assert_ne!(uploaded_sources[0], trim_calls[0].0);
        assert!(uploaded_sources[0].ends_with(".mp4"));

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-upscale")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://v3.fal.media/files/generated-video-upscale/trimmed-source.mp4".to_string()
            ]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-video-upscale/trimmed-source.mp4"
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-video-upscale/asset.json")));
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_nano_banana_image_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/style.png"), b"fake-style").expect("style image");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "style-ref".to_string(),
            name: Some("Style reference".to_string()),
            relative_path: "media/style.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-nano-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Edited product image".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Place the product in the reference scene".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_NANO_BANANA_PRO_EDIT_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["style-ref".to_string()],
                reference_image_media_refs: vec!["style-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: Some("1K".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-nano-edit",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-nano-edit",
            "generated-nano-edit",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-nano-edit",
            "generated-nano-edit",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-nano-edit/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-nano-edit")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-nano-edit/style.png".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID);
        assert_eq!(
            submission.input["image_urls"],
            json!(["https://v3.fal.media/files/generated-nano-edit/style.png"])
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-nano-edit/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["style.png".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_wan_image_video_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/first.png"), b"fake-first").expect("first frame");
        fs::write(project_dir.path().join("media/last.png"), b"fake-last").expect("last frame");
        fs::write(project_dir.path().join("media/audio.wav"), b"fake-audio").expect("audio ref");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "first-frame".to_string(),
            name: Some("First frame".to_string()),
            relative_path: "media/first.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "last-frame".to_string(),
            name: Some("Last frame".to_string()),
            relative_path: "media/last.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "audio-ref".to_string(),
            name: Some("Audio reference".to_string()),
            relative_path: "media/audio.wav".to_string(),
            kind: MediaKind::Audio,
            duration_seconds: 4.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-wan-i2v".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Animated reference".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate the first frame into the last frame".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec![
                    "first-frame".to_string(),
                    "last-frame".to_string(),
                    "audio-ref".to_string(),
                ],
                first_frame_media_id: Some("first-frame".to_string()),
                last_frame_media_id: Some("last-frame".to_string()),
                reference_audio_media_refs: vec!["audio-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(5.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-wan-i2v",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-i2v",
            "generated-wan-i2v",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-i2v",
            "generated-wan-i2v",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-wan-i2v/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-wan-i2v")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://v3.fal.media/files/generated-wan-i2v/first.png".to_string(),
                "https://v3.fal.media/files/generated-wan-i2v/last.png".to_string(),
                "https://v3.fal.media/files/generated-wan-i2v/audio.wav".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID);
        assert_eq!(
            submission.input["image_url"],
            "https://v3.fal.media/files/generated-wan-i2v/first.png"
        );
        assert_eq!(
            submission.input["end_image_url"],
            "https://v3.fal.media/files/generated-wan-i2v/last.png"
        );
        assert_eq!(
            submission.input["audio_url"],
            "https://v3.fal.media/files/generated-wan-i2v/audio.wav"
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-wan-i2v/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &[
                "first.png".to_string(),
                "last.png".to_string(),
                "audio.wav".to_string()
            ]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_wan_reference_video_refs() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/style.png"), b"fake-style").expect("style ref");
        fs::write(project_dir.path().join("media/motion.mp4"), b"fake-motion").expect("motion ref");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "style-ref".to_string(),
            name: Some("Style reference".to_string()),
            relative_path: "media/style.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "motion-ref".to_string(),
            name: Some("Motion reference".to_string()),
            relative_path: "media/motion.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 4.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-wan-r2v".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Reference-guided video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate a product shot using the supplied references".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: "fal-ai/wan/v2.7/reference-to-video".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["style-ref".to_string(), "motion-ref".to_string()],
                reference_image_media_refs: vec!["style-ref".to_string()],
                reference_video_media_refs: vec!["motion-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(6.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-wan-r2v",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-r2v",
            "generated-wan-r2v",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-r2v",
            "generated-wan-r2v",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-wan-r2v/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let generated_asset = load_split_project(project_dir.path())
            .expect("reload split project")
            .generated_assets
            .into_iter()
            .find(|asset| asset.id == "generated-wan-r2v")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://v3.fal.media/files/generated-wan-r2v/style.png".to_string(),
                "https://v3.fal.media/files/generated-wan-r2v/provider-input-generated-wan-r2v-motion-ref-compressed.mp4".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, "fal-ai/wan/v2.7/reference-to-video");
        assert_eq!(
            submission.input["reference_image_urls"],
            json!(["https://v3.fal.media/files/generated-wan-r2v/style.png"])
        );
        assert_eq!(
            submission.input["reference_video_urls"],
            json!(["https://v3.fal.media/files/generated-wan-r2v/provider-input-generated-wan-r2v-motion-ref-compressed.mp4"])
        );
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &[
                "style.png".to_string(),
                "provider-input-generated-wan-r2v-motion-ref-compressed.mp4".to_string()
            ]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_compresses_oversized_reference_videos() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(
            project_dir.path().join("media/motion-4k.mp4"),
            b"fake-motion",
        )
        .expect("motion ref");
        fs::write(project_dir.path().join("media/style.png"), b"fake-style").expect("style ref");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "motion-ref".to_string(),
            name: Some("Oversized motion reference".to_string()),
            relative_path: "media/motion-4k.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 4.0,
            width: Some(3840),
            height: Some(2160),
            fps: Some(24.0),
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "style-ref".to_string(),
            name: Some("Style reference".to_string()),
            relative_path: "media/style.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-wan-r2v".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Reference-guided video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Use the oversized motion reference".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: "fal-ai/wan/v2.7/reference-to-video".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["motion-ref".to_string(), "style-ref".to_string()],
                reference_image_media_refs: vec!["style-ref".to_string()],
                reference_video_media_refs: vec!["motion-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(6.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-wan-r2v",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-r2v",
            "generated-wan-r2v",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-wan-r2v",
            "generated-wan-r2v",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(source_path.display().to_string());
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                Ok(format!(
                    "https://v3.fal.media/files/generated-wan-r2v/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let uploaded_sources = uploaded_sources.lock().expect("uploaded sources");
        assert_eq!(uploaded_sources.len(), 2);
        let compressed_video = uploaded_sources
            .iter()
            .find(|path| path.ends_with(".mp4"))
            .expect("compressed video upload");
        assert!(!compressed_video.ends_with("media/motion-4k.mp4"));
        assert!(uploaded_sources
            .iter()
            .any(|path| path.ends_with("media/style.png")));
        let generated_asset = load_split_project(project_dir.path())
            .expect("reload split project")
            .generated_assets
            .into_iter()
            .find(|asset| asset.id == "generated-wan-r2v")
            .expect("generated asset");
        assert!(generated_asset
            .references
            .provider_input_urls
            .iter()
            .any(|url| url.contains("provider-input-generated-wan-r2v-motion-ref-compressed.mp4")));
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_kling_image_video_keyframes() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/first.png"), b"fake-first").expect("first frame");
        fs::write(project_dir.path().join("media/last.png"), b"fake-last").expect("last frame");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "first-frame".to_string(),
            name: Some("First frame".to_string()),
            relative_path: "media/first.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "last-frame".to_string(),
            name: Some("Last frame".to_string()),
            relative_path: "media/last.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-kling-i2v".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Kling keyframed video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate the first frame into the last frame".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["first-frame".to_string(), "last-frame".to_string()],
                first_frame_media_id: Some("first-frame".to_string()),
                last_frame_media_id: Some("last-frame".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(5.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: Some(true),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-kling-i2v",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-kling-i2v",
            "generated-kling-i2v",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-kling-i2v",
            "generated-kling-i2v",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-kling-i2v/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-kling-i2v")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://v3.fal.media/files/generated-kling-i2v/first.png".to_string(),
                "https://v3.fal.media/files/generated-kling-i2v/last.png".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(
            submission.endpoint,
            FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID
        );
        assert_eq!(
            submission.input["start_image_url"],
            "https://v3.fal.media/files/generated-kling-i2v/first.png"
        );
        assert_eq!(
            submission.input["end_image_url"],
            "https://v3.fal.media/files/generated-kling-i2v/last.png"
        );
        assert_eq!(submission.input["generate_audio"], json!(true));
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-kling-i2v/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["first.png".to_string(), "last.png".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_video_to_audio_source() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-audio".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Video-synced music".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Score the video with warm cinematic music".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: None,
                height: None,
                duration_seconds: Some(10.0),
                fps: None,
                aspect_ratio: None,
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-audio",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-audio",
            "generated-video-audio",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-audio",
            "generated-video-audio",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-video-audio/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-audio")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-video-audio/source.mp4".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID);
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-video-audio/source.mp4"
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-video-audio/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["source.mp4".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_renders_timeline_span_for_video_to_music() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 12.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.timeline.duration_seconds = 8.0;
        project.timeline.tracks[0].items.push(TimelineItem {
            id: "item-source-video".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 8.0,
            source: TimelineSource::Media {
                media_id: "source-video".to_string(),
            },
            label: "Timeline source".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(2.0)),
                ("sourceOut".to_string(), serde_json::json!(10.0)),
            ]),
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-timeline-music".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Timeline music".to_string()),
            target_folder_id: None,
            placement_intent: Some("timeline".to_string()),
            prompt: "".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                duration_seconds: Some(3.0),
                video_source_start_seconds: Some(1.0),
                video_source_end_seconds: Some(4.0),
                timeline_start_seconds: Some(1.0),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-timeline-music",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-timeline-music",
            "generated-timeline-music",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-timeline-music",
            "generated-timeline-music",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push((
                        file_name.to_string(),
                        fs::read(source_path).expect("rendered source"),
                    ));
                Ok(format!(
                    "https://v3.fal.media/files/generated-timeline-music/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let uploaded_sources = uploaded_sources.lock().expect("uploaded sources");
        assert_eq!(uploaded_sources.len(), 1);
        assert!(uploaded_sources[0].0.ends_with(".mp4"));
        assert_eq!(uploaded_sources[0].1, b"rendered-timeline-provider-input");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-timeline-music")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![format!(
                "https://v3.fal.media/files/generated-timeline-music/{}",
                uploaded_sources[0].0
            )]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID);
        assert_eq!(
            submission.input["video_url"],
            generated_asset.references.provider_input_urls[0]
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-timeline-music/asset.json")));
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_wan_video_to_video_source() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 4.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Prompt-guided source video edit".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Turn the source video into a warm product reveal".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(4.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-edit",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-edit",
            "generated-video-edit",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-edit",
            "generated-video-edit",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-video-edit/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-edit")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-video-edit/source.mp4".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(submission.endpoint, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID);
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-video-edit/source.mp4"
        );
        assert_eq!(submission.input["num_frames"], 96);
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-video-edit/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["source.mp4".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_kling_motion_control_refs() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        fs::write(
            project_dir.path().join("media/character.png"),
            b"fake-image",
        )
        .expect("reference image");
        let now = "2026-07-08T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 8.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "character-ref".to_string(),
            name: Some("Character reference".to_string()),
            relative_path: "media/character.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(768),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-motion-control".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Motion-controlled character video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Make the character follow the source motion".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string(), "character-ref".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                reference_image_media_refs: vec!["character-ref".to_string()],
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                duration_seconds: Some(8.0),
                generate_audio: Some(false),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-07-08T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-motion-control",
            JobStatus::Queued,
            "2026-07-08T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-motion-control",
            "generated-motion-control",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-motion-control",
            "generated-motion-control",
            false,
            None,
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, FAL_PROVIDER);
                assert_eq!(credential_provider, FAL_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://v3.fal.media/files/generated-motion-control/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-motion-control")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://v3.fal.media/files/generated-motion-control/source.mp4".to_string(),
                "https://v3.fal.media/files/generated-motion-control/character.png".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(
            submission.endpoint,
            FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        );
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-motion-control/source.mp4"
        );
        assert_eq!(
            submission.input["image_url"],
            "https://v3.fal.media/files/generated-motion-control/character.png"
        );
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["source.mp4".to_string(), "character.png".to_string()]
        );
    }

    #[test]
    fn generate_media_prepare_provider_inputs_trims_source_clip_before_upload() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/source.mp4"), b"fake-video")
            .expect("source video");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "source-video".to_string(),
            name: Some("Source video".to_string()),
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(1280),
            height: Some(720),
            fps: Some(24.0),
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-video-edit".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Trimmed source video edit".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Turn the trimmed range into a warm product reveal".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["source-video".to_string()],
                source_video_media_ref: Some("source-video".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(2.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-video-edit",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-edit",
            "generated-video-edit",
            false,
            Some(TemporalGenerateMediaBrief {
                settings: Some(json!({
                    "sourceClipId": "item-trimmed",
                    "sourceIn": 1.5,
                    "sourceOut": 3.5,
                    "durationSeconds": 2.0
                })),
                ..TemporalGenerateMediaBrief::default()
            }),
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-video-edit",
            "generated-video-edit",
            false,
            Some(TemporalGenerateMediaBrief {
                settings: Some(json!({
                    "sourceClipId": "item-trimmed",
                    "sourceIn": 1.5,
                    "sourceOut": 3.5,
                    "durationSeconds": 2.0
                })),
                ..TemporalGenerateMediaBrief::default()
            }),
        );
        let trim_calls = Arc::new(Mutex::new(Vec::new()));
        let trim_calls_for_closure = Arc::clone(&trim_calls);
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write =
            temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload_and_trim(
                &start_request,
                |provider: &str, source_path: &Path, credential_provider: &str| {
                    assert_eq!(provider, FAL_PROVIDER);
                    assert_eq!(credential_provider, FAL_PROVIDER);
                    uploaded_sources_for_closure
                        .lock()
                        .expect("uploaded sources")
                        .push(source_path.display().to_string());
                    Ok(
                        "https://v3.fal.media/files/generated-video-edit/trimmed-source.mp4"
                            .to_string(),
                    )
                },
                |source_path: &Path, output_path: &Path, source_in: f64, source_out: f64| {
                    trim_calls_for_closure.lock().expect("trim calls").push((
                        source_path.display().to_string(),
                        output_path.display().to_string(),
                        source_in,
                        source_out,
                    ));
                    fs::write(output_path, b"trimmed-video").expect("trimmed output");
                    Ok(())
                },
                |_source_path: &Path, output_path: &Path| {
                    fs::write(output_path, b"compressed-video").expect("compressed output");
                    Ok(())
                },
                |_project_dir: &Path,
                 _project: &VideoProject,
                 _asset: &GeneratedAsset,
                 output_path: &Path,
                 _start_seconds: f64,
                 _end_seconds: f64| {
                    fs::write(output_path, b"rendered-timeline-provider-input")
                        .expect("rendered timeline input");
                    Ok(())
                },
            )
            .expect("prepare provider inputs");

        let trim_calls = trim_calls.lock().expect("trim calls");
        assert_eq!(trim_calls.len(), 1);
        assert!(trim_calls[0].0.ends_with("media/source.mp4"));
        assert!(trim_calls[0].1.ends_with(".mp4"));
        assert_eq!(trim_calls[0].2, 1.5);
        assert_eq!(trim_calls[0].3, 3.5);
        let uploaded_sources = uploaded_sources.lock().expect("uploaded sources");
        assert_eq!(uploaded_sources.len(), 1);
        assert_ne!(uploaded_sources[0], trim_calls[0].0);
        assert!(uploaded_sources[0].ends_with(".mp4"));

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-video-edit")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec!["https://v3.fal.media/files/generated-video-edit/trimmed-source.mp4".to_string()]
        );
        let submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        assert_eq!(
            submission.input["video_url"],
            "https://v3.fal.media/files/generated-video-edit/trimmed-source.mp4"
        );
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-video-edit/asset.json")));
    }

    #[derive(Default)]
    struct RecordingProviderInputRenderer {
        jobs: Mutex<Vec<ProviderInputVideoJob>>,
    }

    impl ProviderInputVideoRenderer for RecordingProviderInputRenderer {
        fn render(
            &self,
            job: &ProviderInputVideoJob,
            _timeout: Duration,
        ) -> PipelineResult<ProcessOutput> {
            self.jobs.lock().expect("jobs").push(job.clone());
            fs::write(&job.output_path, b"prepared-video").expect("write prepared output");
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn trim_provider_input_video_uses_gstreamer_render_job() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("source.mp4");
        let output_path = temp.path().join("trimmed.mp4");
        fs::write(&source_path, b"source-video").expect("source video");
        let renderer = RecordingProviderInputRenderer::default();

        trim_provider_input_video_with_renderer(&renderer, &source_path, &output_path, 1.5, 3.5)
            .expect("trim provider input");

        let jobs = renderer.jobs.lock().expect("jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source_path, source_path);
        assert_eq!(jobs[0].output_path, output_path);
        assert_eq!(jobs[0].source_in, 1.5);
        assert_eq!(jobs[0].source_out, 3.5);
        assert_eq!(jobs[0].max_dimension, None);
        assert!(output_path.is_file());
    }

    #[test]
    fn compress_provider_input_video_uses_bounded_gstreamer_render_job() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("source.mp4");
        let output_path = temp.path().join("compressed.mp4");
        fs::write(&source_path, b"source-video").expect("source video");
        let renderer = RecordingProviderInputRenderer::default();

        compress_provider_input_video_with_renderer(&renderer, &source_path, &output_path)
            .expect("compress provider input");

        let jobs = renderer.jobs.lock().expect("jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source_path, source_path);
        assert_eq!(jobs[0].output_path, output_path);
        assert_eq!(jobs[0].source_in, 0.0);
        assert!(jobs[0].source_out.is_infinite());
        assert_eq!(jobs[0].max_dimension, Some(960));
        assert!(output_path.is_file());
    }

    #[test]
    fn provider_input_render_plan_preserves_audio_and_fits_inside_upload_bounds() {
        let job = ProviderInputVideoJob {
            source_path: "/tmp/source.mp4".into(),
            output_path: "/tmp/prepared.mp4".into(),
            source_in: 0.0,
            source_out: f64::INFINITY,
            max_dimension: Some(960),
        };
        let probe = MediaProbe {
            container_name: Some("mp4".to_string()),
            duration_seconds: Some(12.5),
            size_bytes: Some(1024),
            video: Some(VideoProbe {
                codec_name: Some("h264".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(29.97),
            }),
            audio: Some(AudioProbe {
                codec_name: Some("aac".to_string()),
            }),
        };

        let plan = provider_input_render_plan(&job, &probe).expect("provider input plan");

        assert_eq!((plan.width, plan.height), (960, 540));
        assert_eq!(plan.output_profile, RenderOutputProfile::Mp4Primary);
        assert_eq!(plan.quality, RenderQuality::Final);
        assert_eq!(plan.clips.len(), 1);
        assert_eq!(plan.audio_clips.len(), 1);
        assert_eq!(plan.clips[0].source_in, 0.0);
        assert_eq!(plan.clips[0].source_out, 12.5);
        assert_eq!(
            plan.clips[0].properties["timelineDurationSeconds"],
            json!(12.5)
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn gstreamer_provider_input_renderer_trims_real_speech_fixture() {
        crate::render_runtime::start_render_process_runtime()
            .expect("initialize curated render runtime");
        let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/edison-speech-1920s-30s.mp4");
        let output_dir = tempfile::tempdir().expect("output dir");
        let output_path = output_dir.path().join("trimmed.mp4");
        let renderer = super::GstreamerProviderInputVideoRenderer {
            runner: &SystemProcessRunner,
        };

        trim_provider_input_video_with_renderer(&renderer, &source_path, &output_path, 0.5, 2.5)
            .expect("trim real provider input fixture");

        let metadata = fs::metadata(&output_path).expect("trimmed output metadata");
        assert!(metadata.len() > 0);
        let (probe, _) = crate::render_pipeline::gstreamer_backend::probe_media_with_gstreamer(
            &output_path,
            Duration::from_secs(30),
            "providerInput.output",
        )
        .expect("probe trimmed provider input");
        assert!(probe.video.is_some());
        assert!(probe.audio.is_some());
        assert!((probe.duration_seconds.expect("duration") - 2.0).abs() <= 0.1);
    }

    #[test]
    fn generate_media_prepare_provider_inputs_uploads_replicate_seedance_references() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        fs::create_dir_all(project_dir.path().join("media")).expect("media dir");
        fs::write(project_dir.path().join("media/first.png"), b"fake-first").expect("first frame");
        fs::write(project_dir.path().join("media/last.png"), b"fake-last").expect("last frame");
        fs::write(project_dir.path().join("media/audio.wav"), b"fake-audio").expect("audio ref");
        let now = "2026-06-25T12:00:00Z".to_string();
        let mut project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        project.media.push(MediaAsset {
            id: "first-frame".to_string(),
            name: Some("First frame".to_string()),
            relative_path: "media/first.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "last-frame".to_string(),
            name: Some("Last frame".to_string()),
            relative_path: "media/last.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "audio-ref".to_string(),
            name: Some("Audio reference".to_string()),
            relative_path: "media/audio.wav".to_string(),
            kind: MediaKind::Audio,
            duration_seconds: 4.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-seedance".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Seedance reference video".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "Animate the product between keyframes with matching music".to_string(),
            model: GenerationModel {
                provider: REPLICATE_PROVIDER.to_string(),
                id: REPLICATE_SEEDANCE_20_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: vec!["first-frame".to_string(), "last-frame".to_string()],
                first_frame_media_id: Some("first-frame".to_string()),
                last_frame_media_id: Some("last-frame".to_string()),
                provider_input_urls: Vec::new(),
                ..GeneratedAssetReferences::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(5.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                generate_audio: Some(true),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-seedance",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-seedance",
            "generated-seedance",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-seedance",
            "generated-seedance",
            false,
            Some(TemporalGenerateMediaBrief {
                ..Default::default()
            }),
        );
        let uploaded_sources = Arc::new(Mutex::new(Vec::new()));
        let uploaded_sources_for_closure = Arc::clone(&uploaded_sources);
        let write = temporal_generate_media_prepare_provider_inputs_from_project_dir_with_upload(
            &start_request,
            |provider: &str, source_path: &Path, credential_provider: &str| {
                assert_eq!(provider, REPLICATE_PROVIDER);
                assert_eq!(credential_provider, REPLICATE_PROVIDER);
                let file_name = source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("source filename");
                uploaded_sources_for_closure
                    .lock()
                    .expect("uploaded sources")
                    .push(file_name.to_string());
                Ok(format!(
                    "https://api.replicate.com/v1/files/generated-seedance/{file_name}"
                ))
            },
        )
        .expect("prepare provider inputs");

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        let generated_asset = reloaded
            .generated_assets
            .iter()
            .find(|asset| asset.id == "generated-seedance")
            .expect("generated asset");
        assert_eq!(
            generated_asset.references.provider_input_urls,
            vec![
                "https://api.replicate.com/v1/files/generated-seedance/first.png".to_string(),
                "https://api.replicate.com/v1/files/generated-seedance/last.png".to_string(),
            ]
        );
        let submission =
            temporal_generate_media_provider_submission_from_project_dir(&start_request)
                .expect("submission after upload prep");
        let TemporalGenerateMediaProviderSubmission::Replicate(submission) = submission else {
            panic!("expected replicate submission");
        };
        assert_eq!(submission.version, REPLICATE_SEEDANCE_20_MODEL_ID);
        assert_eq!(
            submission.input["first_frame_url"],
            "https://api.replicate.com/v1/files/generated-seedance/first.png"
        );
        assert_eq!(
            submission.input["last_frame_url"],
            "https://api.replicate.com/v1/files/generated-seedance/last.png"
        );
        assert!(submission.input.get("reference_audio_urls").is_none());
        assert_eq!(submission.input["generate_audio"], json!(true));
        assert!(write
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-seedance/asset.json")));
        assert_eq!(
            uploaded_sources
                .lock()
                .expect("uploaded sources")
                .as_slice(),
            &["first.png".to_string(), "last.png".to_string(),]
        );
    }

    #[test]
    fn in_process_generation_rejects_invalid_capabilities_before_submission() {
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let mut project = VideoProject::new_empty(
            "project-capability-gate".to_string(),
            "Capability Gate".to_string(),
            "2026-07-12T00:00:00Z".to_string(),
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-invalid-duration".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "A deliberately invalid seven-second generation".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_TEXT_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                duration_seconds: Some(7.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: Some("720p".to_string()),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-invalid-duration",
            JobStatus::Queued,
            "2026-07-12T00:00:00Z",
        );
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-invalid-duration",
            "generated-invalid-duration",
            false,
            None,
        );
        job.start_request = Some(start_request.clone());
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save project");

        let submission_called = Arc::new(AtomicBool::new(false));
        let submission_called_for_builder = Arc::clone(&submission_called);
        let client = reqwest::blocking::Client::new();
        let error = run_generate_media_in_process_with_submission_builder(
            &client,
            &start_request,
            "2026-07-12T00:00:01Z",
            Some("invalid-capability-run"),
            TemporalGenerateMediaProviderRunOptions::default(),
            "test-secret",
            || {
                submission_called_for_builder.store(true, Ordering::SeqCst);
                panic!("submission builder must not run for invalid capabilities")
            },
        )
        .expect_err("invalid duration must fail before submission");

        assert!(matches!(
            error,
            TemporalWorkflowInputError::GenerationCapability(_)
        ));
        assert!(!submission_called.load(Ordering::SeqCst));
        let reloaded = load_split_project(project_dir.path()).expect("reload project");
        assert_eq!(
            reloaded
                .generated_assets
                .iter()
                .find(|asset| asset.id == "generated-invalid-duration")
                .expect("generated asset")
                .status,
            GeneratedAssetStatus::Failed
        );
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn in_process_generate_media_runs_activity_chain_and_persists_output_without_secret() {
        // The activity chain ends with a native GES export of the generated timeline output.
        crate::render_runtime::start_render_process_runtime()
            .expect("initialize curated render runtime");
        let project_dir = tempfile::tempdir().expect("temp project dir");
        let now = "2026-06-25T12:00:00Z".to_string();
        let project = VideoProject::new_empty(
            "project-split".to_string(),
            "Split Project".to_string(),
            now,
        );
        let source_dir = tempfile::tempdir().expect("source fixture dir");
        let reference_path = source_dir.path().join("composer-reference.png");
        fs::write(
            &reference_path,
            base64::engine::general_purpose::STANDARD
                .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
                .expect("reference PNG"),
        )
        .expect("write reference PNG");
        let imported = import_media_files(
            project_dir.path(),
            project,
            std::slice::from_ref(&reference_path),
        )
        .expect("composer reference import");
        let reference_media_id = imported.imported[0].id.clone();
        let mut project = imported.project;
        project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.kind == TrackKind::Video)
            .expect("default video track")
            .items
            .push(TimelineItem {
                id: "composer-reference-item".to_string(),
                kind: TimelineItemKind::ImageClip,
                start_seconds: 0.0,
                duration_seconds: 1.0,
                source: TimelineSource::Media {
                    media_id: reference_media_id.clone(),
                },
                label: "Composer reference".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(0.0)),
                    ("sourceOut".to_string(), json!(1.0)),
                ]),
            });
        project.timeline.duration_seconds = 1.0;
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-hero".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Hero still".to_string()),
            target_folder_id: None,
            placement_intent: Some("timeline".to_string()),
            prompt: "soft studio product still".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_FLUX_SCHNELL_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("4:3".to_string()),
                resolution: None,
                generate_audio: None,
                timeline_start_seconds: Some(0.0),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-25T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-hero",
            JobStatus::Queued,
            "2026-06-25T12:01:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        ));
        project.jobs.push(job);
        save_split_project(project_dir.path(), &project).expect("save split project");

        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.path().to_str().expect("project dir utf8"),
            "generated-hero",
            "generated-hero",
            false,
            None,
        );
        let mut submission =
            temporal_generate_media_fal_queue_submission_from_project_dir(&start_request)
                .expect("project-dir fal submission");
        let Some(server) = FakeFalServer::start() else {
            return;
        };
        let base_url = server.base_url.clone();
        submission.url = format!("{}/{}", server.base_url, FAL_FLUX_SCHNELL_MODEL_ID);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");
        let output = run_generate_media_in_process_with_submission_builder(
            &client,
            &start_request,
            "2026-06-25T12:05:00Z",
            Some("in-process-run-1"),
            TemporalGenerateMediaProviderRunOptions {
                max_status_polls: 2,
                poll_interval_millis: 0,
            },
            "test-fal-key",
            || {
                Ok(super::TemporalGenerateMediaProviderSubmission::Fal(
                    submission,
                ))
            },
        )
        .expect("run in-process activity chain");
        let requests = server.join();
        let output_json = serde_json::to_string(&output).expect("serialize activity output");

        assert_eq!(output.request_id, "req-1");
        assert_eq!(output.project_id, "project-split");
        assert_eq!(output.asset_id, "generated-hero");
        assert_eq!(output.job_status, JobStatus::Completed);
        assert_eq!(output.provider_status, FalQueueStatusKind::Completed);
        assert_eq!(output.queue_position, None);
        assert_eq!(output.logs.len(), 1);
        assert_eq!(output.logs[0].message, "done");
        assert_eq!(
            output
                .metrics
                .as_ref()
                .and_then(|metrics| metrics.inference_time),
            Some(1.25)
        );
        assert_eq!(output.run_id.as_deref(), Some("in-process-run-1"));
        assert!(output
            .output_path
            .ends_with("generated/generated-hero/fal-output.png"));
        assert!(output
            .written_files
            .iter()
            .any(|path| path.ends_with("generated/generated-hero/asset.json")));
        assert!(output_json.contains("\"providerStatus\":\"COMPLETED\""));
        assert!(output_json.contains("\"logs\":[{\"message\":\"done\""));
        assert!(output_json.contains("\"inference_time\":1.25"));
        assert!(!output_json.contains("test-fal-key"));

        let reloaded = load_split_project(project_dir.path()).expect("reload split project");
        assert!(reloaded
            .media
            .iter()
            .any(|media| media.id == "generated-hero-fal-output"));
        let reloaded_job = reloaded
            .jobs
            .iter()
            .find(|job| job.id == "generated-hero")
            .expect("reloaded provider job");
        let provider_request = reloaded_job
            .provider_request
            .as_ref()
            .expect("persist provider request metadata");
        assert_eq!(provider_request.provider, FAL_PROVIDER);
        assert_eq!(provider_request.request_id, "req-1");
        assert_eq!(
            provider_request.status_url,
            format!("{base_url}/status/req-1")
        );
        assert_eq!(
            provider_request.response_url,
            format!("{base_url}/result/req-1")
        );
        assert_eq!(
            provider_request.cancel_url,
            format!("{base_url}/cancel/req-1")
        );
        assert_eq!(provider_request.submitted_at, "2026-06-25T12:05:00Z");
        let submit = requests
            .iter()
            .find(|request| request.path == "/fal-ai/flux/schnell")
            .expect("submit request");
        assert!(submit.authorization.as_deref() == Some("Key test-fal-key"));
        assert!(!submit.body.contains("test-fal-key"));

        let generated_item = reloaded
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "generated-hero-timeline-visual")
            .expect("generated output inserted on timeline");
        assert!(matches!(
            &generated_item.source,
            TimelineSource::Media { media_id } if media_id == "generated-hero-fal-output"
        ));
        let replaced = apply_project_action_to_split_project(
            project_dir.path(),
            ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
                replacement: ProjectActionReplaceGeneratedOutput {
                    item_id: "composer-reference-item".to_string(),
                    media_id: "generated-hero-fal-output".to_string(),
                },
            },
        )
        .expect("replace composer reference with generated output");
        let replaced_item = replaced
            .project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "composer-reference-item")
            .expect("replaced reference item");
        assert!(matches!(
            &replaced_item.source,
            TimelineSource::Media { media_id } if media_id == "generated-hero-fal-output"
        ));

        let mut multi_source_project = replaced.project.clone();
        for media in &mut multi_source_project.media {
            if media.id == reference_media_id || media.id == "generated-hero-fal-output" {
                media.duration_seconds = 30.0;
            }
        }
        multi_source_project.transcripts.push(Transcript {
            id: "provider-app-e2e-transcript".to_string(),
            media_id: reference_media_id.clone(),
            engine: Some("deterministic-fixture".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: vec![TranscriptSegment {
                text: "The local spoken hook explains the product clearly".to_string(),
                start_seconds: 0.0,
                end_seconds: 12.0,
            }],
            words: vec![TranscriptWord {
                text: "local spoken hook".to_string(),
                start_seconds: 0.0,
                end_seconds: 2.0,
                confidence: Some(1.0),
                speaker: None,
            }],
        });
        let semantic_hits = vec![SemanticVisualHit {
            media_id: "generated-hero-fal-output".to_string(),
            time_seconds: 10.0,
            shot_start_seconds: 5.0,
            shot_end_seconds: 20.0,
            score: 1.0,
            thumbnail_relative_path: "search/visual/generated-hero-fal-output/frame.png"
                .to_string(),
        }];
        let multi_source_draft = generate_spoken_semantic_multi_source_edit_timeline(
            &mut multi_source_project,
            EditJobRequest {
                media_id: reference_media_id.clone(),
                preset: EditPreset::TrailerCut,
                prompt: "Combine spoken context with local semantic visual proof".to_string(),
                target_duration_seconds: Some(30.0),
                language_mode: LanguageMode::English,
                caption_style: CaptionStyle::Bold,
                created_at: "2026-06-25T12:05:30Z".to_string(),
            },
            &semantic_hits,
        )
        .expect("build spoken plus semantic multi-source edit");
        assert_eq!(
            multi_source_draft
                .edl
                .clips
                .iter()
                .map(|clip| clip.media_id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([reference_media_id.as_str(), "generated-hero-fal-output"])
        );
        assert!(multi_source_draft
            .edl
            .clips
            .iter()
            .all(|clip| { !clip.reason.is_empty() && !clip.selection_reasons.is_empty() }));
        save_split_project(project_dir.path(), &multi_source_project)
            .expect("persist multi-source edit fixture");

        let render_job = temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            &multi_source_project.id,
            "provider-app-e2e-render",
            JobStatus::Queued,
            "2026-06-25T12:06:00Z",
        );
        let render = render_media_to_split_project_folder(
            project_dir.path(),
            &multi_source_project.id,
            ExportRenderOptions::new(
                ExportProfile::Mp4H264,
                RenderQuality::Final,
                multi_source_project.render_settings.width,
                multi_source_project.render_settings.height,
            )
            .expect("explicit export options"),
            render_job,
            "2026-06-25T12:06:00Z",
            Some("provider-app-e2e-render-run".to_string()),
            None,
        )
        .expect("native render generated timeline output");
        let rendered_output_path = project_dir.path().join(&render.output_path);
        assert!(rendered_output_path.is_file());
        assert!(render.project_render_report.duration_seconds > 0.0);
        assert!(render.project_render_report.streams.video);
        for check in [
            "duration",
            "streams",
            "captionAlignment",
            "overlayTiming",
            "artifactPaths",
            "logPath",
        ] {
            assert!(render.project_render_report.checks.contains_key(check));
        }
        assert!(render
            .project_render_report
            .artifacts
            .iter()
            .any(|artifact| artifact.ends_with("output.mp4")));
        assert!(project_dir
            .path()
            .join(&render.project_render_report.log_path)
            .is_file());

        let retained_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("output/provider-app-e2e");
        let _ = fs::remove_dir_all(&retained_dir);
        fs::create_dir_all(&retained_dir).expect("retained evidence dir");
        fs::copy(&rendered_output_path, retained_dir.join("output.mp4"))
            .expect("retain rendered output");
        fs::copy(
            project_dir
                .path()
                .join(&render.project_render_report.log_path),
            retained_dir.join("render.log"),
        )
        .expect("retain render log");
        let report = json!({
            "schemaVersion": 1,
            "status": "passed",
            "providerMode": "mock-http",
            "credential": "synthetic-redacted",
            "composerReferenceMediaId": reference_media_id,
            "generatedMediaId": "generated-hero-fal-output",
            "timelineItemId": generated_item.id,
            "replacedTimelineItemId": replaced_item.id,
            "multiSourceClips": multi_source_draft.edl.clips.iter().map(|clip| json!({
                "mediaId": clip.media_id,
                "sourceIn": clip.source_in,
                "sourceOut": clip.source_out,
                "reason": clip.reason,
                "selectionReasons": clip.selection_reasons
            })).collect::<Vec<_>>(),
            "render": {
                "durationSeconds": render.project_render_report.duration_seconds,
                "streams": render.project_render_report.streams,
                "artifacts": ["output.mp4", "render.log", "report.json"],
                "outputPath": "output.mp4",
                "logPath": "render.log"
                ,"checks": render.project_render_report.checks
            },
            "lifecycle": {
                "success": "completed",
                "failure": "failed-retryable",
                "cancel": "cancellation-requested",
                "retry": "typed-start-request-retained"
            }
        });
        fs::write(
            retained_dir.join("report.json"),
            serde_json::to_vec_pretty(&report).expect("serialize retained report"),
        )
        .expect("retain sanitized report");
        assert!(!serde_json::to_string(&report)
            .expect("serialize report")
            .contains("test-fal-key"));
    }

    #[test]
    fn generate_media_cancel_provider_activity_calls_fal_cancel_url_without_secret_payload() {
        let Some(server) = FakeFalServer::start_with_request_limit(1) else {
            return;
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");
        let cancel_url = format!("{}/cancel/req-1", server.base_url);

        let output = temporal_generate_media_cancel_provider_activity_with_client(
            &client,
            &cancel_url,
            "test-fal-key",
        )
        .expect("cancel provider activity");
        let requests = server.join();
        let output_json = serde_json::to_string(&output).expect("serialize cancel output");

        assert_eq!(output.request_id, "req-1");
        assert_eq!(output.status, FalQueueCancelStatus::CancellationRequested);
        assert_eq!(output.cancel_url, cancel_url);
        assert!(!output_json.contains("test-fal-key"));
        let cancel = requests.first().expect("cancel request");
        assert_eq!(cancel.method, "PUT");
        assert_eq!(cancel.path, "/cancel/req-1");
        assert_eq!(cancel.authorization.as_deref(), Some("Key test-fal-key"));
        assert!(!cancel.body.contains("test-fal-key"));
    }

    #[test]
    fn generate_media_cancel_provider_activity_calls_replicate_cancel_url_with_bearer_auth() {
        let Some(server) = FakeReplicateServer::start_with_request_limit(1) else {
            return;
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test client");
        let cancel_url = format!("{}/v1/predictions/pred-123/cancel", server.base_url);

        let output = temporal_generate_media_cancel_provider_activity_with_client(
            &client,
            &cancel_url,
            "test-replicate-token",
        )
        .expect("cancel replicate provider activity");
        let output_json = serde_json::to_string(&output).expect("serialize cancel output");
        let requests = server.join();

        assert_eq!(output.request_id, "pred-123");
        assert_eq!(output.cancel_url, cancel_url);
        assert_eq!(output.status, FalQueueCancelStatus::CancellationRequested);
        assert!(!output_json.contains("test-replicate-token"));
        let cancel = requests.first().expect("cancel request");
        assert_eq!(cancel.method, "POST");
        assert_eq!(cancel.path, "/v1/predictions/pred-123/cancel");
        assert_eq!(
            cancel.authorization.as_deref(),
            Some("Bearer test-replicate-token")
        );
        assert!(!cancel.body.contains("test-replicate-token"));
    }

    #[test]
    fn fal_restart_recovery_polls_existing_request_without_resubmitting() {
        let Some(server) = FakeFalServer::start_with_request_limit(3) else {
            return;
        };
        let project_dir = tempfile::tempdir().expect("project dir");
        let project = restart_recovery_project(
            project_dir.path(),
            FAL_PROVIDER,
            FAL_FLUX_SCHNELL_MODEL_ID,
            JobProviderRequest {
                provider: FAL_PROVIDER.to_string(),
                request_id: "req-1".to_string(),
                status_url: format!("{}/status/req-1", server.base_url),
                response_url: format!("{}/result/req-1", server.base_url),
                cancel_url: format!("{}/cancel/req-1", server.base_url),
                submitted_at: "2026-07-12T12:00:00Z".to_string(),
            },
        );
        save_split_project(project_dir.path(), &project).expect("save recovery project");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("client");

        let write = resume_interrupted_generate_media_job_with_client_and_credential_cancellable(
            &client,
            project_dir.path(),
            "restart-asset",
            "2026-07-12T12:05:00Z",
            "test-fal-key",
            TemporalGenerateMediaProviderRunOptions {
                max_status_polls: 1,
                poll_interval_millis: 0,
            },
            None,
        )
        .expect("resume fal request");
        let requests = server.join();

        assert_eq!(
            requests
                .iter()
                .map(|request| (request.method.as_str(), request.path.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("GET", "/status/req-1"),
                ("GET", "/result/req-1"),
                ("GET", "/media/fal-output.png")
            ]
        );
        assert_eq!(
            write
                .project
                .jobs
                .iter()
                .find(|job| job.id == "restart-asset")
                .expect("job")
                .status,
            JobStatus::Completed
        );
        assert_eq!(
            write
                .project
                .generated_assets
                .iter()
                .find(|asset| asset.id == "restart-asset")
                .expect("asset")
                .status,
            GeneratedAssetStatus::Completed
        );
    }

    #[test]
    fn replicate_restart_recovery_polls_existing_request_without_resubmitting() {
        let Some(server) = FakeReplicateServer::start_with_request_limit(2) else {
            return;
        };
        let project_dir = tempfile::tempdir().expect("project dir");
        let project = restart_recovery_project(
            project_dir.path(),
            REPLICATE_PROVIDER,
            REPLICATE_FLUX_SCHNELL_MODEL_ID,
            JobProviderRequest {
                provider: REPLICATE_PROVIDER.to_string(),
                request_id: "pred-123".to_string(),
                status_url: format!("{}/v1/predictions/pred-123", server.base_url),
                response_url: format!("{}/v1/predictions/pred-123", server.base_url),
                cancel_url: format!("{}/v1/predictions/pred-123/cancel", server.base_url),
                submitted_at: "2026-07-12T12:00:00Z".to_string(),
            },
        );
        save_split_project(project_dir.path(), &project).expect("save recovery project");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("client");

        let write = resume_interrupted_generate_media_job_with_client_and_credential_cancellable(
            &client,
            project_dir.path(),
            "restart-asset",
            "2026-07-12T12:05:00Z",
            "test-replicate-token",
            TemporalGenerateMediaProviderRunOptions {
                max_status_polls: 1,
                poll_interval_millis: 0,
            },
            None,
        )
        .expect("resume replicate request");
        let requests = server.join();

        assert_eq!(
            requests
                .iter()
                .map(|request| (request.method.as_str(), request.path.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("GET", "/v1/predictions/pred-123"),
                ("GET", "/media/replicate-output.png")
            ]
        );
        assert_eq!(
            write
                .project
                .jobs
                .iter()
                .find(|job| job.id == "restart-asset")
                .expect("job")
                .status,
            JobStatus::Completed
        );
    }

    #[test]
    fn project_open_recovery_fails_nonresumable_provider_without_provider_io() {
        let project_dir = tempfile::tempdir().expect("project dir");
        let project = restart_recovery_project(
            project_dir.path(),
            OPENAI_PROVIDER,
            OPENAI_GPT_IMAGE_2_MODEL_ID,
            JobProviderRequest {
                provider: OPENAI_PROVIDER.to_string(),
                request_id: "img-123".to_string(),
                status_url: "https://api.openai.com/v1/images/generations".to_string(),
                response_url: "https://api.openai.com/v1/images/generations".to_string(),
                cancel_url: "https://api.openai.com/v1/images/generations".to_string(),
                submitted_at: "2026-07-12T12:00:00Z".to_string(),
            },
        );
        save_split_project(project_dir.path(), &project).expect("save recovery project");

        let recovery = reconcile_interrupted_generation_jobs_on_project_open(
            project_dir.path(),
            "2026-07-12T12:05:00Z",
        )
        .expect("reconcile unsupported provider");

        assert!(recovery.resume_candidates.is_empty());
        assert_eq!(recovery.failed_job_ids, vec!["restart-asset"]);
        assert_eq!(
            recovery
                .project
                .jobs
                .iter()
                .find(|job| job.id == "restart-asset")
                .expect("job")
                .status,
            JobStatus::Failed
        );
        assert_eq!(
            recovery
                .project
                .generated_assets
                .iter()
                .find(|asset| asset.id == "restart-asset")
                .expect("asset")
                .status,
            GeneratedAssetStatus::Failed
        );
    }

    fn openai_restart_request() -> JobProviderRequest {
        JobProviderRequest {
            provider: OPENAI_PROVIDER.to_string(),
            request_id: "img-123".to_string(),
            status_url: "https://api.openai.com/v1/images/generations".to_string(),
            response_url: "https://api.openai.com/v1/images/generations".to_string(),
            cancel_url: "https://api.openai.com/v1/images/generations".to_string(),
            submitted_at: "2026-07-12T12:00:00Z".to_string(),
        }
    }

    fn set_restart_job(
        project: &mut VideoProject,
        status: JobStatus,
        run_id: Option<&str>,
        updated_at: &str,
    ) {
        let job = project
            .jobs
            .iter_mut()
            .find(|job| job.id == "restart-asset")
            .expect("restart job");
        job.status = status;
        job.updated_at = updated_at.to_string();
        job.workflow.as_mut().expect("workflow").run_id = run_id.map(str::to_string);
    }

    fn restart_job_status(project: &VideoProject) -> JobStatus {
        project
            .jobs
            .iter()
            .find(|job| job.id == "restart-asset")
            .expect("restart job")
            .status
            .clone()
    }

    #[test]
    fn project_open_recovery_fails_in_process_generations_an_earlier_session_left_behind() {
        for (status, run_id) in [
            (
                JobStatus::Running,
                Some("in-process/video-creater/restart-asset"),
            ),
            (JobStatus::Queued, None),
        ] {
            let project_dir = tempfile::tempdir().expect("project dir");
            let mut project = restart_recovery_project(
                project_dir.path(),
                OPENAI_PROVIDER,
                OPENAI_GPT_IMAGE_2_MODEL_ID,
                openai_restart_request(),
            );
            project.id = format!("stale-restart-project-{status:?}");
            set_restart_job(&mut project, status.clone(), run_id, "2026-07-12T12:00:00Z");
            save_split_project(project_dir.path(), &project).expect("save recovery project");

            let recovery = reconcile_interrupted_generation_jobs_on_project_open(
                project_dir.path(),
                "2026-07-12T12:05:00Z",
            )
            .expect("reconcile stale generation");

            assert_eq!(recovery.failed_job_ids, vec!["restart-asset"], "{status:?}");
            assert_eq!(restart_job_status(&recovery.project), JobStatus::Failed);
        }
    }

    #[test]
    fn project_open_recovery_leaves_a_live_in_process_generation_running() {
        let project_dir = tempfile::tempdir().expect("project dir");
        let mut project = restart_recovery_project(
            project_dir.path(),
            OPENAI_PROVIDER,
            OPENAI_GPT_IMAGE_2_MODEL_ID,
            openai_restart_request(),
        );
        project.id = "live-restart-project".to_string();
        set_restart_job(
            &mut project,
            JobStatus::Running,
            Some("in-process/video-creater/restart-asset"),
            "2026-07-12T12:00:00Z",
        );
        save_split_project(project_dir.path(), &project).expect("save live project");
        let _live = crate::generation::cancel::register_generation_cancellation(
            &project.id,
            "restart-asset",
        )
        .expect("register live generation");

        let recovery = reconcile_interrupted_generation_jobs_on_project_open(
            project_dir.path(),
            "2026-07-12T12:05:00Z",
        )
        .expect("reconcile live generation");

        assert!(recovery.failed_job_ids.is_empty());
        assert!(recovery.resume_candidates.is_empty());
        assert_eq!(restart_job_status(&recovery.project), JobStatus::Running);
    }

    #[test]
    fn project_open_recovery_leaves_temporal_generations_to_their_worker() {
        let project_dir = tempfile::tempdir().expect("project dir");
        let mut project = restart_recovery_project(
            project_dir.path(),
            FAL_PROVIDER,
            FAL_FLUX_SCHNELL_MODEL_ID,
            JobProviderRequest {
                provider: FAL_PROVIDER.to_string(),
                request_id: "req-1".to_string(),
                status_url: "https://queue.fal.run/status/req-1".to_string(),
                response_url: "https://queue.fal.run/result/req-1".to_string(),
                cancel_url: "https://queue.fal.run/cancel/req-1".to_string(),
                submitted_at: "2026-07-12T12:00:00Z".to_string(),
            },
        );
        project.id = "temporal-restart-project".to_string();
        set_restart_job(
            &mut project,
            JobStatus::Running,
            Some("01a09ad3-75e2-7bca-94a3-1cd878a60c43"),
            "2026-07-12T12:00:00Z",
        );
        save_split_project(project_dir.path(), &project).expect("save Temporal project");

        let recovery = reconcile_interrupted_generation_jobs_on_project_open(
            project_dir.path(),
            "2026-07-12T12:05:00Z",
        )
        .expect("reconcile Temporal generation");

        assert!(recovery.failed_job_ids.is_empty());
        assert!(recovery.resume_candidates.is_empty());
        assert_eq!(recovery.project.jobs, project.jobs);
        assert_eq!(recovery.project.generated_assets, project.generated_assets);
    }

    #[test]
    fn project_open_recovery_leaves_a_generation_queued_in_this_session_for_its_start() {
        super::app_session_started_at();
        let project_dir = tempfile::tempdir().expect("project dir");
        let mut project = restart_recovery_project(
            project_dir.path(),
            OPENAI_PROVIDER,
            OPENAI_GPT_IMAGE_2_MODEL_ID,
            openai_restart_request(),
        );
        project.id = "queued-restart-project".to_string();
        let recorded_at = chrono::Utc::now().to_rfc3339();
        set_restart_job(&mut project, JobStatus::Queued, None, &recorded_at);
        save_split_project(project_dir.path(), &project).expect("save queued project");

        let recovery =
            reconcile_interrupted_generation_jobs_on_project_open(project_dir.path(), &recorded_at)
                .expect("reconcile queued generation");

        assert!(recovery.failed_job_ids.is_empty());
        assert_eq!(restart_job_status(&recovery.project), JobStatus::Queued);
    }

    #[test]
    fn restart_recovery_rejects_untrusted_provider_urls_before_credentials_are_used() {
        assert!(super::validate_restart_recovery_url(
            FAL_PROVIDER,
            "https://attacker.invalid/status/req-1"
        )
        .is_err());
        assert!(super::validate_restart_recovery_url(
            REPLICATE_PROVIDER,
            "http://api.replicate.com/v1/predictions/pred-1"
        )
        .is_err());
    }

    fn restart_recovery_project(
        project_dir: &Path,
        provider: &str,
        model_id: &str,
        provider_request: JobProviderRequest,
    ) -> VideoProject {
        let mut project = VideoProject::new_empty(
            "restart-project".to_string(),
            "Restart project".to_string(),
            "2026-07-12T11:59:00Z".to_string(),
        );
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "restart-asset".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Running,
            name: Some("Restart asset".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "resume without resubmitting".to_string(),
            model: GenerationModel {
                provider: provider.to_string(),
                id: model_id.to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(768),
                aspect_ratio: Some("4:3".to_string()),
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-07-12T11:59:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let start_request = temporal_generate_media_start_request(
            &project.id,
            project_dir.to_str().expect("utf-8 project dir"),
            "restart-asset",
            "restart-asset",
            false,
            Some(TemporalGenerateMediaBrief::default()),
        );
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "restart-asset",
            JobStatus::Running,
            "2026-07-12T12:00:00Z",
        );
        job.start_request = Some(start_request);
        job.provider_request = Some(provider_request);
        project.jobs.push(job);
        project
    }

    fn write_executable_stub(path: &std::path::Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("write tool stub");
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(path).expect("stub metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("make stub executable");
        }
    }

    struct FakeFalServer {
        base_url: String,
        handle: thread::JoinHandle<Vec<FakeFalRequest>>,
    }

    fn tiny_png_fixture() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .expect("tiny PNG fixture")
    }

    #[derive(Debug)]
    struct FakeFalRequest {
        method: String,
        path: String,
        authorization: Option<String>,
        body: String,
    }

    impl FakeFalServer {
        fn start() -> Option<Self> {
            Self::start_with_request_limit(4)
        }

        fn start_with_request_limit(request_limit: usize) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind fake fal server: {error}"),
            };
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let server_base_url = base_url.clone();
            let handle = thread::spawn(move || {
                let mut requests = Vec::new();
                for stream in listener.incoming().take(request_limit) {
                    let mut stream = stream.expect("fake fal connection");
                    let request = read_fake_fal_request(&mut stream);
                    if request.path == "/media/fal-output.png" {
                        let body = tiny_png_fixture();
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(head.as_bytes()).expect("write PNG head");
                        stream.write_all(&body).expect("write PNG body");
                    } else {
                        let response = fake_fal_response(&server_base_url, &request);
                        stream
                            .write_all(response.as_bytes())
                            .expect("write fake fal response");
                    }
                    requests.push(request);
                }
                requests
            });

            Some(Self { base_url, handle })
        }

        fn join(self) -> Vec<FakeFalRequest> {
            self.handle.join().expect("fake fal server join")
        }
    }

    struct FakeReplicateServer {
        base_url: String,
        handle: thread::JoinHandle<Vec<FakeFalRequest>>,
    }

    impl FakeReplicateServer {
        fn start() -> Option<Self> {
            Self::start_with_request_limit(3)
        }

        fn start_with_request_limit(request_limit: usize) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind fake replicate server: {error}"),
            };
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let server_base_url = base_url.clone();
            let handle = thread::spawn(move || {
                let mut requests = Vec::new();
                for stream in listener.incoming().take(request_limit) {
                    let mut stream = stream.expect("fake replicate connection");
                    let request = read_fake_fal_request(&mut stream);
                    let response = fake_replicate_response(&server_base_url, &request);
                    stream
                        .write_all(response.as_bytes())
                        .expect("write fake replicate response");
                    requests.push(request);
                }
                requests
            });

            Some(Self { base_url, handle })
        }

        fn join(self) -> Vec<FakeFalRequest> {
            self.handle.join().expect("fake replicate server join")
        }
    }

    struct FakeOpenAiServer {
        base_url: String,
        handle: thread::JoinHandle<Vec<FakeFalRequest>>,
    }

    impl FakeOpenAiServer {
        fn start() -> Option<Self> {
            Self::start_with_request_limit(1)
        }

        fn start_with_request_limit(request_limit: usize) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind fake openai server: {error}"),
            };
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let handle = thread::spawn(move || {
                let mut requests = Vec::new();
                for stream in listener.incoming().take(request_limit) {
                    let mut stream = stream.expect("fake openai connection");
                    let request = read_fake_fal_request(&mut stream);
                    let response = fake_openai_response(&request);
                    stream
                        .write_all(response.as_bytes())
                        .expect("write fake openai response");
                    requests.push(request);
                }
                requests
            });

            Some(Self { base_url, handle })
        }

        fn join(self) -> Vec<FakeFalRequest> {
            self.handle.join().expect("fake openai server join")
        }
    }

    fn read_fake_fal_request(stream: &mut TcpStream) -> FakeFalRequest {
        let mut buffer = [0_u8; 4096];
        let mut bytes = Vec::new();
        loop {
            let read = stream.read(&mut buffer).expect("read fake fal request");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if request_body_complete(&bytes) {
                break;
            }
        }
        let request = String::from_utf8(bytes).expect("utf8 fake fal request");
        let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
        let mut lines = head.lines();
        let request_line = lines.next().expect("request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let target = request_parts.next().unwrap_or_default();
        let path = target.split('?').next().unwrap_or(target).to_string();
        let authorization = lines.find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim().to_string())
            })
        });

        FakeFalRequest {
            method,
            path,
            authorization,
            body: body.to_string(),
        }
    }

    fn request_body_complete(bytes: &[u8]) -> bool {
        let request = String::from_utf8_lossy(bytes);
        let Some((head, body)) = request.split_once("\r\n\r\n") else {
            return false;
        };
        let content_length = head.lines().find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
        });

        match content_length {
            Some(length) => body.len() >= length,
            None => true,
        }
    }

    fn fake_fal_response(base_url: &str, request: &FakeFalRequest) -> String {
        let (content_type, body) = match request.path.as_str() {
            "/fal-ai/flux/schnell" => (
                "application/json",
                serde_json::json!({
                    "request_id": "req-1",
                    "response_url": format!("{base_url}/result/req-1"),
                    "status_url": format!("{base_url}/status/req-1"),
                    "cancel_url": format!("{base_url}/cancel/req-1"),
                    "queue_position": 0
                })
                .to_string(),
            ),
            "/fal-ai/aura-sr" => (
                "application/json",
                serde_json::json!({
                    "request_id": "req-1",
                    "response_url": format!("{base_url}/result/req-1"),
                    "status_url": format!("{base_url}/status/req-1"),
                    "cancel_url": format!("{base_url}/cancel/req-1"),
                    "queue_position": 0
                })
                .to_string(),
            ),
            "/sonilo/v1.1/video-to-music" => (
                "application/json",
                serde_json::json!({
                    "request_id": "audio-req-1",
                    "response_url": format!("{base_url}/result/audio-req-1"),
                    "status_url": format!("{base_url}/status/audio-req-1"),
                    "cancel_url": format!("{base_url}/cancel/audio-req-1"),
                    "queue_position": 0
                })
                .to_string(),
            ),
            "/status/req-1" => (
                "application/json",
                serde_json::json!({
                    "status": "COMPLETED",
                    "request_id": "req-1",
                    "response_url": format!("{base_url}/result/req-1"),
                    "queue_position": null,
                    "logs": [{"message": "done", "timestamp": "2026-06-25T12:05:00Z"}],
                    "metrics": {"inference_time": 1.25},
                    "error": null,
                    "error_type": null
                })
                .to_string(),
            ),
            "/status/audio-req-1" => (
                "application/json",
                serde_json::json!({
                    "status": "COMPLETED",
                    "request_id": "audio-req-1",
                    "response_url": format!("{base_url}/result/audio-req-1"),
                    "queue_position": null,
                    "logs": [{"message": "done", "timestamp": "2026-06-25T12:05:00Z"}],
                    "metrics": {"inference_time": 1.25},
                    "error": null,
                    "error_type": null
                })
                .to_string(),
            ),
            "/result/req-1" => (
                "application/json",
                serde_json::json!({
                    "image": {
                        "url": format!("{base_url}/media/fal-output.png"),
                        "width": 2048,
                        "height": 1536
                    },
                    "images": [{
                        "url": format!("{base_url}/media/fal-output.png"),
                        "width": 1024,
                        "height": 768
                    }]
                })
                .to_string(),
            ),
            "/result/audio-req-1" => (
                "application/json",
                serde_json::json!({
                    "audio": {
                        "url": format!("{base_url}/media/fal-output.m4a"),
                        "duration": 3.5
                    }
                })
                .to_string(),
            ),
            "/media/fal-output.png" => ("image/png", "fake-png".to_string()),
            "/media/fal-output.m4a" => ("audio/mp4", "fake-m4a".to_string()),
            "/cancel/req-1" => (
                "application/json",
                serde_json::json!({ "status": "CANCELLATION_REQUESTED" }).to_string(),
            ),
            _ => ("text/plain", "not found".to_string()),
        };
        let status = if body == "not found" {
            "404 Not Found"
        } else if request.path == "/cancel/req-1" {
            "202 Accepted"
        } else {
            "200 OK"
        };

        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn fake_replicate_response(base_url: &str, request: &FakeFalRequest) -> String {
        let (content_type, body) = match request.path.as_str() {
            "/v1/predictions" => (
                "application/json",
                serde_json::json!({
                    "id": "pred-123",
                    "status": "starting",
                    "output": null,
                    "error": null,
                    "urls": {
                        "get": format!("{base_url}/v1/predictions/pred-123"),
                        "cancel": format!("{base_url}/v1/predictions/pred-123/cancel")
                    }
                })
                .to_string(),
            ),
            "/v1/predictions/pred-123" => (
                "application/json",
                serde_json::json!({
                    "id": "pred-123",
                    "status": "succeeded",
                    "output": [format!("{base_url}/media/replicate-output.png")],
                    "error": null,
                    "urls": {
                        "get": format!("{base_url}/v1/predictions/pred-123"),
                        "cancel": format!("{base_url}/v1/predictions/pred-123/cancel")
                    }
                })
                .to_string(),
            ),
            "/media/replicate-output.png" => ("image/png", "fake-png".to_string()),
            "/v1/predictions/pred-123/cancel" => (
                "application/json",
                serde_json::json!({
                    "id": "pred-123",
                    "status": "canceled",
                    "output": null,
                    "error": null,
                    "urls": {
                        "get": format!("{base_url}/v1/predictions/pred-123"),
                        "cancel": format!("{base_url}/v1/predictions/pred-123/cancel")
                    }
                })
                .to_string(),
            ),
            _ => ("text/plain", "not found".to_string()),
        };
        let status = if body == "not found" {
            "404 Not Found"
        } else {
            "200 OK"
        };

        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[cfg(feature = "ges-render")]
    #[test]
    fn export_media_plan_propagates_requested_quality_to_active_render_inputs() {
        // Planning checks export profile availability, which needs the started render runtime.
        crate::render_runtime::start_render_process_runtime()
            .expect("initialize curated render runtime");
        let options = crate::project::export_options::ExportRenderOptions::new(
            ExportProfile::Mp4H264,
            crate::edit::render_plan::RenderQuality::Draft,
            1920,
            1080,
        )
        .expect("explicit export options");
        let draft_request = super::temporal_export_media_start_request_with_options(
            "project-1",
            "/tmp/video-creater-project",
            "export-draft-h264",
            options,
            "exports/draft.mp4",
        );
        let draft_plan = super::temporal_export_media_workflow_activity_plan_value(
            draft_request.input,
            "2026-07-14T12:00:00Z",
            Some("run-draft"),
        )
        .expect("Draft export plan");

        for activity_input in [
            &draft_plan["buildRenderPlanInput"],
            &draft_plan["validateExportProfileInput"],
            &draft_plan["renderMediaInput"],
            &draft_plan["validateRenderedMediaInput"],
        ] {
            assert_eq!(activity_input["profile"], json!("mp4H264"));
            assert_eq!(activity_input["quality"], json!("draft"));
            assert_eq!(activity_input["width"], json!(1920));
            assert_eq!(activity_input["height"], json!(1080));
        }
        let serialized = serde_json::to_string(&draft_plan).expect("serialize plan");
        assert!(!serialized.contains("draftWebm"));
        assert!(!serialized.contains("finalWebm"));
    }

    #[test]
    fn explicit_render_options_reject_dimensionless_legacy_webm_input() {
        let error = super::temporal_render_options_from_input(&json!({
            "profile": "finalWebm"
        }))
        .expect_err("legacy WebM must use the project-aware compatibility entry point");

        assert!(error.to_string().contains("quality"));
    }

    #[test]
    fn export_guard_uses_the_requested_quality_reason() {
        let availability = crate::project::export_profiles::ExportProfileAvailability {
            profile: ExportProfile::ProResMov,
            label: "ProRes MOV".to_string(),
            available: true,
            container: "mov".to_string(),
            extension: "mov".to_string(),
            mime_type: "video/quicktime".to_string(),
            video_codec: "prores".to_string(),
            audio_codec: Some("pcm".to_string()),
            required_runtime: vec!["system:avfoundation".to_string()],
            policy_status: crate::project::export_profiles::ExportPolicyStatus::Approved,
            unavailable_reason: None,
            quality_availability: crate::project::export_profiles::ExportQualityAvailability {
                draft: false,
                final_quality: true,
            },
            quality_unavailable_reasons:
                crate::project::export_profiles::ExportQualityUnavailableReasons {
                    draft: Some(
                        "Draft ProRes requires native ProRes Proxy capability.".to_string(),
                    ),
                    final_quality: None,
                },
        };

        let error = super::ensure_export_profile_quality_available(
            &availability,
            "proResMov",
            crate::edit::render_plan::RenderQuality::Draft,
        )
        .expect_err("Draft workflow guard must reject unavailable ProRes Proxy");
        assert!(error
            .to_string()
            .contains("Draft ProRes requires native ProRes Proxy"));
        super::ensure_export_profile_quality_available(
            &availability,
            "proResMov",
            crate::edit::render_plan::RenderQuality::Final,
        )
        .expect("Final workflow guard should accept available ProRes 422");
    }

    fn fake_openai_response(request: &FakeFalRequest) -> String {
        let (content_type, body) = match request.path.as_str() {
            "/v1/images/generations" | "/v1/images/edits" => (
                "application/json",
                serde_json::json!({
                    "id": "img-123",
                    "data": [{ "b64_json": "ZmFrZS1wbmc=" }]
                })
                .to_string(),
            ),
            _ => ("text/plain", "not found".to_string()),
        };
        let status = if body == "not found" {
            "404 Not Found"
        } else {
            "200 OK"
        };

        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }
}
