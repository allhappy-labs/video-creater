use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use video_creater_lib::agent::codex_transport::CodexTurnTransport;
use video_creater_lib::agent::turn::{
    agent_session_for_next_turn, agent_turn_preferences, build_agent_conversation_request,
    finish_agent_conversation_turn, plan_agent_turn, AgentTurnBackendPlan, AgentTurnPreferences,
};
use video_creater_lib::agent::{AgentBackendKind, AgentTurnTransport};
use video_creater_lib::app_service::context::{ClientKind, RequestContext};
use video_creater_lib::app_service::events::NoopEventSink;
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::app_service::projects::ProjectService;
use video_creater_lib::codex::app_server::{
    bundled_codex_app_server_command, codex_app_server_deadline,
    start_codex_video_edit_turn_unpersisted_until, StdioCodexAppServerTransport,
};
use video_creater_lib::codex::context::load_project_skill_bundle;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal as apply_prepared_codex_conversation_proposal,
    undo_codex_conversation_edit, CodexConversationApplyRequest, CodexConversationApplyResult,
    CodexConversationEditProposal, CodexConversationEditRequest, CodexPreparedProposal,
};
use video_creater_lib::codex::proposal::CodexEditProposal;
use video_creater_lib::codex::tools::{
    call_codex_local_tool as call_codex_local_tool_impl,
    list_codex_local_tools as list_codex_local_tools_impl, list_models_payload,
    CodexLocalToolCallResult, CodexLocalToolDescriptor,
};
use video_creater_lib::codex::turn_cancel::{
    close_codex_turn_cancellation, register_codex_project_turn,
    request_codex_project_turn_cancellation,
};
use video_creater_lib::edit::preset::EditJobRequest;
use video_creater_lib::edit::render_plan::{
    generate_one_click_edit_timeline, generate_spoken_semantic_multi_source_edit_timeline,
    ExportEncodeTier, GeneratedEditDraft, RenderQuality, RenderQualityProfile,
};
use video_creater_lib::effects::effect_catalog_payload;
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
    GenerationCancellationRequestOutcome,
};
use video_creater_lib::generation::download::{
    download_url_to_atomic_file, output_limit_for_path, DownloadNetworkPolicy,
};
use video_creater_lib::generation::mock::mock_generation_completion_actions;
use video_creater_lib::generation::orphan_watch::watch_generation_job;
use video_creater_lib::generation::replicate::{
    REPLICATE_PROVIDER, VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR,
};
use video_creater_lib::gpu_graphics::templates::{
    builtin_shader_background_templates, project_shader_background_templates,
    ShaderBackgroundTemplate, ShaderTemplatePlacement, ShaderTemplatePreview,
    ShaderTemplateRenderContract, ShaderTemplateSourceKind,
};
use video_creater_lib::media_inspection::{
    derive_media_analysis_moments_with_sampled_signals, derive_media_silence_ranges, inspect_video,
    sample_video_audio_window_metrics, sample_video_frame_metrics, AudioWindowMetrics,
    ImageMetrics, VideoInspectionReport,
};
use video_creater_lib::precompose::{
    prepare_project_for_render, prepared_media_timeline_span, PrecomposeReport,
};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::command_queue::project_command_queue;
use video_creater_lib::project::export_destination::ExportOutputRequest;
use video_creater_lib::project::export_options::{ExportRenderOptions, JobExportSettings};
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile, ExportProfileAvailability,
};
use video_creater_lib::project::export_reveal::reveal_recorded_export_with;
use video_creater_lib::project::import::{import_media_files_with_names, ImportMediaResult};
use video_creater_lib::project::job_progress::{remove_settled_job_progress, JobProgressSnapshot};
use video_creater_lib::project::matte::{create_matte, MatteRequest, MatteResult};
use video_creater_lib::project::model::{
    GeneratedAssetStatus, JobStatus, JobSummary, MediaAnalysisMoment, MediaAsset, MediaKind,
    MediaSilenceRange, ProjectRenderPreviewComparison, ProjectRenderPreviewComparisonFrame,
    ProjectRenderPreviewComparisonRequest, ProjectRenderReport, RenderSettings,
    TemporalWorkflowStartRequest, TimelineSource, VideoProject,
};
use video_creater_lib::project::mutation::{
    acquire_split_project_artifact_lease, acquire_split_project_mutation_lease,
};
use video_creater_lib::project::nle_export::{
    export_project_timeline_to_nle_xml, nle_xml_export_artifact, write_nle_xml_export, NleXmlFormat,
};
use video_creater_lib::project::patch::{apply_timeline_patch, TimelinePatch};
use video_creater_lib::project::split::{
    apply_agent_session_action, apply_project_action_to_split_project,
    apply_project_actions_to_split_project, load_split_project, migrate_single_file_project,
    record_app_server_conversation_turn, replace_split_project_if_revision,
    resolve_project_relative_path, save_split_project, validate_split_project_write_path,
    AgentSessionAction, AppServerConversationTurn, ProjectActionWriteResult,
    ProjectAgentUndoOutcome, ProjectValidationIssue, ProjectValidationReport, ProjectWriteReport,
    SplitAgentSessionManifest, SplitAppServerConversationFile,
};
use video_creater_lib::project::storage;
use video_creater_lib::provider_credentials::{
    credential_store_display_name, delete_provider_credential as delete_stored_provider_credential,
    is_supported_provider,
    list_provider_credential_statuses as stored_provider_credential_statuses,
    provider_credential_status as stored_provider_credential_status, resolve_provider_credential,
    set_provider_credential as set_stored_provider_credential, supported_provider_count,
    supported_provider_ids, ProviderCredentialError, ProviderCredentialStatus,
};
#[cfg(test)]
use video_creater_lib::render_pipeline::cancel::RenderAttemptKey;
use video_creater_lib::render_pipeline::cancel::{
    request_render_cancellation_by_locator, RenderCancellationOutcome,
};
use video_creater_lib::render_pipeline::error::PipelineError;
use video_creater_lib::render_pipeline::project_export::{
    expand_project_nested_timelines_for_render, load_project_render_pipeline_report,
    recover_interrupted_local_render_jobs_with_lease, render_media_export_to_split_project_folder,
    render_prepared_preview_frame_to_split_project_folder,
    render_prepared_preview_frame_to_split_project_folder_with_lease,
    render_webm_to_split_project_folder_for_timeline as render_project_webm_to_split_project_folder_for_timeline,
    MediaExportRequest, PreparedPreviewFrameResult, ProjectWebmRenderResult,
};
use video_creater_lib::render_pipeline::report::{
    write_json_report, write_markdown_report, RenderPreviewComparison,
    RenderPreviewComparisonFrame, RenderReport,
};
use video_creater_lib::render_runtime::{
    begin_desktop_render_runtime_initialization, prepare_desktop_render_runtime,
};
use video_creater_lib::search::semantic_visual::SemanticVisualHit;
use video_creater_lib::search::{
    project_search_index_path, query_project_search_for_project_dir,
    rebuild_project_search_index as rebuild_project_search_index_for_project_dir,
    visual_cache::{cache_visual_frames, VisualFrameCacheReport},
    visual_caption::{caption_cached_visual_frames_with_fal, VisualCaptionReport},
    ProjectSearchQuery, SearchScope,
};
use video_creater_lib::settings::acceptance::{
    SettingsAcceptanceCheckpoint, SettingsAcceptanceContext, SettingsAcceptanceFailure,
    SettingsAcceptanceProgress, SettingsAcceptanceProgressStep, SettingsAcceptanceState,
};
use video_creater_lib::settings::agent::{
    bundled_mcp_server_executable, claude_turn_readiness,
    mcp_client_configuration as build_mcp_client_configuration,
    run_agent_component_self_test as run_agent_component_self_test_domain,
    McpClientConfigurationState,
};
use video_creater_lib::settings::health::{
    build_initial_settings_health_snapshot, build_system_health_section_by_id,
    build_system_health_snapshot, collect_system_health_inventory_if_needed,
    get_agent_settings_health as build_agent_settings_health,
    get_skills_settings_health as build_skills_settings_health,
    storage_category_health as build_storage_category_health, SettingsCategoryHealth,
    SettingsHealthCommandError, SettingsHealthSnapshot, SettingsHealthState, SystemHealthSection,
    SystemHealthSectionId, SystemHealthSnapshot,
};
use video_creater_lib::settings::operations::{
    SettingsOperation, SettingsOperationError, SettingsOperationKind, SettingsOperationRegistry,
    SettingsOperationState, SettingsOperationTransition, SettingsOperationsCommandError,
    SETTINGS_OPERATION_EVENT,
};
use video_creater_lib::settings::preferences::{
    AppPreferencesCommandError, AppPreferencesPatch, AppPreferencesState, AppPreferencesStore,
    AppPreferencesV2, GenerationExecutionBackend, LegacyAppPreferencesV1,
};
use video_creater_lib::settings::providers::{
    aggregate_provider_health, ProviderAccountValidation, ProviderHealth, ProviderModelDependency,
    ProviderValidationState,
};
use video_creater_lib::settings::render_system::{
    get_render_system_health as build_render_system_health, render_health_probe_mode_requested,
    render_health_target_id, run_render_health_probe_mode, RenderSystemHealth,
};
use video_creater_lib::settings::skills::{
    preview_bundled_skill_repair, repair_bundled_skills as repair_bundled_skill_files,
    validate_skill_root, SkillRepairConfirmation, SkillRepairError, SkillRepairOutcome,
    SkillRepairPreview, SkillRepairReport, SkillRootError,
};
use video_creater_lib::settings::storage::{
    acquire_storage_mutation_lease, collect_storage_inventory,
    is_storage_cleanup_confirmation_token,
    preview_storage_cleanup_for_generation as preview_storage_cleanup_domain,
    preview_storage_cleanup_for_generation_with_nonce as preview_storage_cleanup_domain_with_nonce,
    run_storage_cleanup_for_generation as run_storage_cleanup_domain, StorageCleanupError,
    StorageCleanupPreview, StorageCleanupRunFailure, StorageCleanupTarget, StorageInventoryItem,
    StorageMutationLease,
};
use video_creater_lib::speech_analysis::{
    analyze_wav_cached_production, assign_media_speaker, default_speech_helper_path,
    load_speaker_registry, project_persisted_speech_analysis, project_sidecar, recolor_speaker,
    rename_speaker, MediaSpeechSidecar, SpeakerRegistry,
};
use video_creater_lib::speech_models::{
    ProductionSpeechModelStatus, ProductionSpeechModelStore, SpeechModelError,
    SPEECH_ANALYSIS_MODEL_SET_ID,
};
use video_creater_lib::timeline_filmstrip::{
    cache_timeline_filmstrip, TimelineFilmstripReport, TimelineFilmstripRequest,
};
#[cfg(not(target_os = "linux"))]
use video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime as PlatformTranscriptionRuntime;
use video_creater_lib::transcription::job::validate_transcription_ready_for_generate_edit;
#[cfg(not(target_os = "linux"))]
use video_creater_lib::transcription::model::FLUID_AUDIO_COREML_RUNTIME_ID as PLATFORM_TRANSCRIPTION_RUNTIME_ID;
#[cfg(target_os = "linux")]
use video_creater_lib::transcription::model::SHERPA_ONNX_RUNTIME_ID as PLATFORM_TRANSCRIPTION_RUNTIME_ID;
use video_creater_lib::transcription::model::{
    transcription_model_catalog_entry, ModelInstallStatus,
};
use video_creater_lib::transcription::runtime::{
    InstalledModel, ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability,
    RuntimeSelection,
};
#[cfg(target_os = "linux")]
use video_creater_lib::transcription::sherpa_onnx::SherpaOnnxRuntime as PlatformTranscriptionRuntime;
use video_creater_lib::transcription::store::{
    DownloadToken, ModelStoreError, StartDownload, TranscriptionModelStatus,
    TranscriptionModelStore,
};
#[cfg(feature = "temporal-worker")]
use video_creater_lib::workflows::temporal_reconcile::{
    apply_temporal_job_reconciliation, connect_temporal_client_from_environment,
    reconcile_temporal_jobs_with_describer, temporal_reconciliation_workflow_ids,
    TemporalClientDescriber, TemporalReconcileOptions,
};
use video_creater_lib::workflows::temporal_reconcile::{
    unreachable_result as temporal_unreachable_result, TemporalJobReconciliationResult,
};
#[cfg(feature = "temporal-worker")]
use video_creater_lib::workflows::temporal_start_workflow_with_client;
#[cfg(not(feature = "temporal-worker"))]
use video_creater_lib::workflows::temporal_workflow_unavailable_start_result;
use video_creater_lib::workflows::{
    reconcile_interrupted_generation_jobs_on_project_open,
    resume_interrupted_generate_media_job_with_client_and_credential_cancellable,
    run_generate_media_in_process_with_client_and_credential_cancellable,
    temporal_codex_edit_start_request, temporal_export_media_start_request_with_output,
    temporal_export_nle_xml_start_request, temporal_export_project_bundle_start_request,
    temporal_export_project_bundle_write_activity_value,
    temporal_generate_media_attach_generated_asset_failure_to_project_dir,
    temporal_generate_media_cancel_provider_activity_with_client,
    temporal_generate_media_cancellation_actions, temporal_generate_media_failure_actions,
    temporal_generate_media_mock_completion_actions, temporal_generate_media_start_request,
    temporal_generate_media_workflow_input, temporal_job_summary, temporal_start_result_action,
    temporal_transcribe_media_start_request, temporal_worker_environment_report,
    TemporalGenerateMediaBrief, TemporalGenerateMediaCancelProviderActivityOutput,
    TemporalGenerateMediaProviderRunOptions, TemporalWorkerEnvironmentReport,
    TemporalWorkflowInputError, TemporalWorkflowKind, TemporalWorkflowStartResult,
};

#[cfg(test)]
mod command_thread_audit;
mod native_menu;

struct TranscriptionModelStoreState(Mutex<TranscriptionModelStore>);
struct ProductionSpeechModelStoreState(ProductionSpeechModelStore);
struct SettingsOperationRegistryState(SettingsOperationRegistry);
struct SettingsAcceptanceStateHolder(Option<SettingsAcceptanceState>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsAcceptanceProjectLoad {
    id: String,
    render_artifact_ids: Vec<String>,
}

#[tauri::command]
fn get_app_preferences(
    state: tauri::State<'_, AppPreferencesState>,
    legacy: Option<LegacyAppPreferencesV1>,
) -> Result<AppPreferencesV2, AppPreferencesCommandError> {
    state
        .0
        .lock()
        .map_err(|_| AppPreferencesCommandError::lock())?
        .load_or_migrate(legacy)
        .map_err(Into::into)
}

#[tauri::command]
fn update_app_preferences(
    state: tauri::State<'_, AppPreferencesState>,
    patch: AppPreferencesPatch,
) -> Result<AppPreferencesV2, AppPreferencesCommandError> {
    state
        .0
        .lock()
        .map_err(|_| AppPreferencesCommandError::lock())?
        .update(patch)
        .map_err(Into::into)
}

#[derive(Debug, Default)]
struct ProviderHealthCache {
    health: Vec<ProviderHealth>,
    next_request_sequence: u64,
    latest_sequence_by_provider: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
struct ProviderHealthRefreshRequest {
    sequence: u64,
    providers: HashSet<String>,
}

#[derive(Debug, Default)]
struct ProviderHealthState(Mutex<ProviderHealthCache>);

impl ProviderHealthState {
    fn snapshot(&self) -> Result<Vec<ProviderHealth>, String> {
        self.0
            .lock()
            .map(|cache| cache.health.clone())
            .map_err(|_| "provider health cache lock failed".to_string())
    }

    fn begin_refresh(
        &self,
        provider: Option<&str>,
    ) -> Result<ProviderHealthRefreshRequest, String> {
        let providers = provider
            .map(|provider| vec![provider.to_string()])
            .unwrap_or_else(|| supported_provider_ids().map(str::to_string).collect())
            .into_iter()
            .collect::<HashSet<_>>();
        let mut cache = self
            .0
            .lock()
            .map_err(|_| "provider health cache lock failed".to_string())?;
        cache.next_request_sequence = cache
            .next_request_sequence
            .checked_add(1)
            .ok_or_else(|| "provider health request sequence exhausted".to_string())?;
        let sequence = cache.next_request_sequence;
        for provider in &providers {
            cache
                .latest_sequence_by_provider
                .insert(provider.clone(), sequence);
        }
        Ok(ProviderHealthRefreshRequest {
            sequence,
            providers,
        })
    }

    fn merge_refresh(
        &self,
        request: &ProviderHealthRefreshRequest,
        refreshed: Vec<ProviderHealth>,
    ) -> Result<(), String> {
        let mut cache = self
            .0
            .lock()
            .map_err(|_| "provider health cache lock failed".to_string())?;
        for refreshed_health in refreshed {
            let provider = refreshed_health.provider.as_str();
            if !request.providers.contains(provider)
                || cache.latest_sequence_by_provider.get(provider) != Some(&request.sequence)
            {
                continue;
            }
            cache
                .health
                .retain(|existing| existing.provider != refreshed_health.provider);
            cache.health.push(refreshed_health);
        }
        cache
            .health
            .sort_by_key(|provider| provider_health_order(&provider.provider));
        Ok(())
    }

    #[cfg(test)]
    fn seed(&self, health: Vec<ProviderHealth>) -> Result<(), String> {
        let mut cache = self
            .0
            .lock()
            .map_err(|_| "provider health cache lock failed".to_string())?;
        cache.health = health;
        cache
            .health
            .sort_by_key(|provider| provider_health_order(&provider.provider));
        Ok(())
    }
}

fn provider_health_order(provider: &str) -> usize {
    match provider {
        "fal.ai" => 0,
        "replicate" => 1,
        "openai" => 2,
        "xai" => 3,
        "elevenlabs" => 4,
        "google" => 5,
        "minimax" => 6,
        _ => usize::MAX,
    }
}
/// Cooperative boundary for app-owned cleanup-scope and active-session mutations.
/// Project activation/open/create-save transitions and cleanup workers hold this lease.
/// Nonactivating existing-only snapshot restores use the project mutation metadata transaction.
/// External filesystem writers are outside the boundary, so cleanup also revalidates identity
/// and the full fingerprint immediately before deletion.
#[derive(Debug, Default)]
struct StorageProjectMutationCoordinatorState;

type StorageProjectMutationLease = StorageMutationLease;

impl StorageProjectMutationCoordinatorState {
    fn acquire(&self) -> Result<StorageProjectMutationLease, String> {
        acquire_storage_mutation_lease()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveProjectSession {
    canonical_root: PathBuf,
    generation: u64,
}

#[derive(Debug, Default)]
struct ActiveProjectSessionStore {
    active: Option<ActiveProjectSession>,
    generation: u64,
}

#[derive(Debug, Default)]
struct ActiveProjectSessionState(Mutex<ActiveProjectSessionStore>);

impl ActiveProjectSessionState {
    fn record_successful_project_root(
        &self,
        project_root: &Path,
        _lease: &StorageProjectMutationLease,
    ) -> Result<ActiveProjectSession, String> {
        let canonical_root = project_root
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize loaded project directory: {error}"))?;
        if !canonical_root.is_dir() {
            return Err(format!(
                "loaded project directory is not a directory: {}",
                canonical_root.display()
            ));
        }
        // The webview previews project media, so a project the backend has opened or saved
        // becomes readable through the asset protocol (and the Linux media stream server).
        if let Some(app) = DESKTOP_APP_HANDLE.get() {
            app.asset_protocol_scope()
                .allow_directory(&canonical_root, true)
                .map_err(|error| format!("failed to allow project media previews: {error}"))?;
        }
        let mut store = self
            .0
            .lock()
            .map_err(|_| "active project session lock is poisoned".to_string())?;
        store.generation = store
            .generation
            .checked_add(1)
            .ok_or_else(|| "active project session generation is exhausted".to_string())?;
        let session = ActiveProjectSession {
            canonical_root,
            generation: store.generation,
        };
        store.active = Some(session.clone());
        Ok(session)
    }

    fn current(&self) -> Result<Option<ActiveProjectSession>, String> {
        self.0
            .lock()
            .map(|store| store.active.clone())
            .map_err(|_| "active project session lock is poisoned".to_string())
    }

    fn matches(&self, expected: &ActiveProjectSession) -> Result<bool, String> {
        self.current()
            .map(|current| current.as_ref() == Some(expected))
    }
}

const STORAGE_CLEANUP_PREVIEW_TTL: Duration = Duration::from_secs(5 * 60);
const MODEL_DOWNLOAD_PROGRESS_MINIMUM_BYTES: u64 = 1024 * 1024;
const MODEL_DOWNLOAD_PROGRESS_MAXIMUM_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct ModelDownloadProgressPublication {
    last_downloaded_bytes: u64,
    last_published_at: Instant,
    has_published: bool,
}

impl ModelDownloadProgressPublication {
    fn new(started_at: Instant) -> Self {
        Self {
            last_downloaded_bytes: 0,
            last_published_at: started_at,
            has_published: false,
        }
    }

    fn should_publish(&mut self, downloaded_bytes: u64, now: Instant) -> bool {
        let progressed_by_minimum = downloaded_bytes.saturating_sub(self.last_downloaded_bytes)
            >= MODEL_DOWNLOAD_PROGRESS_MINIMUM_BYTES;
        let elapsed = now
            .checked_duration_since(self.last_published_at)
            .unwrap_or_default();
        if self.has_published
            && !progressed_by_minimum
            && elapsed < MODEL_DOWNLOAD_PROGRESS_MAXIMUM_INTERVAL
        {
            return false;
        }
        self.last_downloaded_bytes = downloaded_bytes;
        self.last_published_at = now;
        self.has_published = true;
        true
    }
}

#[derive(Debug, Clone)]
struct StorageCleanupPreviewRecord {
    preview: StorageCleanupPreview,
    preview_nonce: String,
    project_session: Option<ActiveProjectSession>,
    expires_at: Instant,
    consumed: bool,
}

#[derive(Debug, Default)]
struct StorageCleanupPreviewRegistryState(Mutex<HashMap<String, StorageCleanupPreviewRecord>>);

impl StorageCleanupPreviewRegistryState {
    fn register(
        &self,
        preview: &StorageCleanupPreview,
        project_session: Option<&ActiveProjectSession>,
        _lease: &StorageProjectMutationLease,
    ) -> Result<(), StorageCleanupError> {
        let mut records = self.0.lock().map_err(|_| {
            StorageCleanupError::TargetUnavailable(
                "cleanup preview registry".to_string(),
                "preview registry lock is poisoned".to_string(),
            )
        })?;
        let now = Instant::now();
        records.retain(|_, record| record.expires_at > now);
        if records
            .get(&preview.confirmation_token)
            .is_some_and(|record| record.consumed)
        {
            return Ok(());
        }
        records.insert(
            preview.confirmation_token.clone(),
            StorageCleanupPreviewRecord {
                preview: preview.clone(),
                preview_nonce: preview.preview_nonce.clone(),
                project_session: project_session.cloned(),
                expires_at: now + STORAGE_CLEANUP_PREVIEW_TTL,
                consumed: false,
            },
        );
        Ok(())
    }

    fn consume_valid(
        &self,
        confirmation_token: &str,
        target: &StorageCleanupTarget,
        project_session: Option<&ActiveProjectSession>,
        _lease: &StorageProjectMutationLease,
    ) -> Result<StorageCleanupPreviewRecord, StorageCleanupError> {
        let mut records = self.0.lock().map_err(|_| {
            StorageCleanupError::TargetUnavailable(
                "cleanup preview registry".to_string(),
                "preview registry lock is poisoned".to_string(),
            )
        })?;
        let record = records
            .get_mut(confirmation_token)
            .ok_or(StorageCleanupError::ConfirmationMismatch)?;
        if record.expires_at <= Instant::now() || record.consumed {
            return Err(StorageCleanupError::ConfirmationMismatch);
        }
        if &record.preview.target != target {
            return Err(StorageCleanupError::ConfirmationMismatch);
        }
        if record.project_session.as_ref() != project_session {
            return Err(StorageCleanupError::ProjectSessionMismatch);
        }
        record.consumed = true;
        Ok(record.clone())
    }
}
#[derive(Clone)]
struct StorageInventoryRootsState {
    global_model_root: PathBuf,
    app_cache_root: PathBuf,
}
#[derive(Default)]
struct ModelOperationCoordinator;
struct ModelOperationCoordinatorState(Mutex<ModelOperationCoordinator>);

const SETTINGS_STORAGE_HEALTH_EVENT: &str = "settings-storage-health";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CancelInProcessGenerationResult {
    outcome: String,
    project: VideoProject,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexVideoEditCommandResult {
    project: VideoProject,
    thread_id: String,
    thread_response: serde_json::Value,
    turn_response: serde_json::Value,
    proposal: Option<CodexEditProposal>,
    proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexConversationEditCommandResult {
    project: VideoProject,
    thread_id: String,
    thread_response: serde_json::Value,
    turn_response: serde_json::Value,
    proposal: Option<CodexConversationEditProposal>,
    /// The only bundle the client may apply: Rust-materialized actions, IDs,
    /// risk, and impact. `None` whenever validation fails.
    prepared_proposal: Option<CodexPreparedProposal>,
    proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillRepairCommandResponse {
    preview: SkillRepairPreview,
    operation: Option<SettingsOperation>,
    report: Option<SkillRepairReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NleXmlExportCommandResult {
    project: VideoProject,
    export_path: String,
    job: JobSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPreviewRenderComparisonRunResult {
    project: VideoProject,
    render_report: RenderReport,
    project_render_report: ProjectRenderReport,
    evidence_report: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaAnalysisCommandResult {
    project: VideoProject,
    write_report: ProjectWriteReport,
    moments: Vec<MediaAnalysisMoment>,
    silence_ranges: Vec<MediaSilenceRange>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedPreviewCommandResult {
    project: VideoProject,
    reports: Vec<PrecomposeReport>,
    frame_sequences: Vec<PreparedPreviewFrameSequence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedPreviewFrameSequence {
    item_id: String,
    prepared_media_id: String,
    start_seconds: f64,
    duration_seconds: f64,
    fps: f64,
    frame_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShaderBackgroundTemplateCatalogEntry {
    id: String,
    version: u32,
    name: String,
    source_kind: ShaderTemplateSourceKind,
    category: String,
    duration_seconds: f64,
    shader_profile_id: String,
    config_path: String,
    shader_path: String,
    utility_refs: Vec<String>,
    source_url: Option<String>,
    license: String,
    placement: ShaderTemplatePlacement,
    render_contract: ShaderTemplateRenderContract,
    preview: ShaderTemplatePreview,
    visual_treatment: String,
    motion: String,
    safe_zone: String,
    avoid: String,
    agent_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateHealth {
    state: SettingsHealthState,
    installed_version: String,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct NotificationCapability {
    state: SettingsHealthState,
    delivery_available: bool,
    permission_status: String,
    can_request: bool,
    summary: String,
    diagnostic_code: Option<String>,
    diagnostic_detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeNotificationAuthorizationStatus {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    NotDetermined,
    Denied,
    Authorized,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Provisional,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Ephemeral,
}

impl From<ShaderBackgroundTemplate> for ShaderBackgroundTemplateCatalogEntry {
    fn from(template: ShaderBackgroundTemplate) -> Self {
        Self {
            id: template.config.id,
            version: template.config.version,
            name: template.config.title,
            source_kind: template.config.source_kind,
            category: template.config.category,
            duration_seconds: template.config.default_duration_seconds,
            shader_profile_id: template.config.shader_profile_id,
            config_path: template.config_path,
            shader_path: template.shader_path,
            utility_refs: template.config.utility_refs,
            source_url: template.config.source_url,
            license: template.config.license,
            placement: template.config.placement,
            render_contract: template.config.render_contract,
            preview: template.config.preview,
            visual_treatment: template.config.visual_treatment,
            motion: template.config.motion,
            safe_zone: template.config.safe_zone,
            avoid: template.config.avoid,
            agent_summary: template.config.agent_summary,
        }
    }
}

fn desktop_project_service() -> ProjectService {
    ProjectService::new(Arc::new(NoopEventSink))
}

fn desktop_project_context(project: &VideoProject, expected_revision: u64) -> RequestContext {
    RequestContext::new(
        ClientKind::Desktop,
        uuid::Uuid::new_v4().to_string(),
        Some(project.id.clone()),
        Some(expected_revision),
        BTreeSet::from([
            AuthorizationScope::ProjectRead,
            AuthorizationScope::ProjectWrite,
        ]),
        None,
    )
    .expect("desktop-generated request context is valid")
}

#[tauri::command]
fn create_empty_project(name: String) -> VideoProject {
    let now = chrono::Utc::now().to_rfc3339();
    desktop_project_service().create_empty(uuid::Uuid::new_v4().to_string(), name, now)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAccountBalance {
    current_balance: f64,
    currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAccountStatus {
    provider: String,
    credential_status: String,
    account_name: Option<String>,
    balance: Option<ProviderAccountBalance>,
    detail: String,
    fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FalAccountBillingCredits {
    current_balance: f64,
    currency: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct FalAccountBillingResponse {
    username: Option<String>,
    credits: Option<FalAccountBillingCredits>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ReplicateAccountResponse {
    username: Option<String>,
    name: Option<String>,
}

#[tauri::command]
async fn list_provider_credential_statuses() -> Result<Vec<ProviderCredentialStatus>, String> {
    tauri::async_runtime::spawn_blocking(stored_provider_credential_statuses)
        .await
        .map_err(|error| format!("provider credential status task failed: {error}"))
}

#[tauri::command]
async fn set_provider_credential(
    provider: String,
    credential: String,
) -> Result<ProviderCredentialStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        set_stored_provider_credential(provider.trim(), &credential)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider credential write task failed: {error}"))?
}

#[tauri::command]
async fn delete_provider_credential(provider: String) -> Result<ProviderCredentialStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        delete_stored_provider_credential(provider.trim()).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("provider credential delete task failed: {error}"))?
}

#[tauri::command]
fn get_provider_account_status(provider: String) -> ProviderAccountStatus {
    let provider = provider.trim().to_string();
    match stored_provider_credential_status(&provider) {
        Ok(_) => {}
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &error.to_string(),
            );
        }
    };
    let credential = match resolve_provider_credential(&provider) {
        Ok(credential) => credential.into_secret(),
        Err(ProviderCredentialError::MissingCredential(_)) => {
            return provider_account_status(
                provider,
                "missing",
                None,
                None,
                &format!(
                    "No credential is stored in {}",
                    credential_store_display_name()
                ),
            );
        }
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &error.to_string(),
            );
        }
    };
    if credential.trim().is_empty() {
        return provider_account_status(
            provider,
            "missing",
            None,
            None,
            "Provider credential is empty",
        );
    }

    provider_account_status_with_credential(provider, &credential)
}

fn provider_account_status_with_credential(
    provider: String,
    credential: &str,
) -> ProviderAccountStatus {
    let normalized_provider = provider.to_ascii_lowercase();
    match normalized_provider.as_str() {
        "fal.ai" => fetch_fal_account_status(provider, credential),
        "replicate" => fetch_replicate_account_status(provider, credential),
        "openai" | "xai" | "elevenlabs" | "google" | "minimax" => provider_account_status(
            provider,
            "balanceUnavailable",
            None,
            None,
            &format!(
                "{} account balance API is not configured; credential presence checked",
                provider_account_display_name(normalized_provider.as_str())
            ),
        ),
        _ => provider_account_status(provider, "unavailable", None, None, "Unsupported provider"),
    }
}

fn provider_account_display_name(provider: &str) -> &'static str {
    match provider {
        "fal.ai" => "fal.ai",
        "replicate" => "Replicate",
        "openai" => "OpenAI",
        "xai" => "xAI",
        "elevenlabs" => "ElevenLabs",
        "google" => "Google",
        "minimax" => "MiniMax",
        _ => "Provider",
    }
}

fn provider_account_status(
    provider: String,
    credential_status: &str,
    account_name: Option<String>,
    balance: Option<ProviderAccountBalance>,
    detail: &str,
) -> ProviderAccountStatus {
    ProviderAccountStatus {
        provider,
        credential_status: credential_status.to_string(),
        account_name,
        balance,
        detail: detail.to_string(),
        fetched_at: Some(chrono::Utc::now().to_rfc3339()),
    }
}

fn provider_account_validation(status: ProviderAccountStatus) -> ProviderAccountValidation {
    let (validation_state, diagnostic_code) = match status.credential_status.as_str() {
        "available" => (ProviderValidationState::Available, None),
        "balanceUnavailable" => (ProviderValidationState::BalanceUnavailable, None),
        "missing" => (ProviderValidationState::Missing, None),
        "rejected" => (
            ProviderValidationState::Rejected,
            Some("providers.credentialRejected".to_string()),
        ),
        "unavailable" => (
            ProviderValidationState::Unavailable,
            Some("providers.validationUnavailable".to_string()),
        ),
        _ => (
            ProviderValidationState::Unavailable,
            Some("providers.validationStateUnknown".to_string()),
        ),
    };
    ProviderAccountValidation {
        provider: status.provider,
        validation_state,
        account_label: status.account_name,
        balance_label: status
            .balance
            .map(|balance| format!("{:.2} {}", balance.current_balance, balance.currency)),
        last_checked_at: status.fetched_at,
        diagnostic_code,
    }
}

fn provider_model_dependencies_from_payload(
    payload: &serde_json::Value,
    disabled_model_ids: &[String],
) -> Result<Vec<ProviderModelDependency>, String> {
    let models = payload
        .get("generationModels")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "generation model catalog did not contain generationModels".to_string())?;
    let disabled_model_ids = disabled_model_ids
        .iter()
        .map(|model_id| model_id.trim())
        .filter(|model_id| !model_id.is_empty())
        .collect::<HashSet<_>>();
    let mut dependencies = models
        .iter()
        .filter_map(|model| {
            let provider = model.get("provider")?.as_str()?.trim();
            let model_id = model.get("id")?.as_str()?.trim();
            if provider.is_empty() || model_id.is_empty() || !is_supported_provider(provider) {
                return None;
            }
            let preference_id = format!("{provider}:{model_id}");
            if disabled_model_ids.contains(model_id)
                || disabled_model_ids.contains(preference_id.as_str())
            {
                return None;
            }
            Some(ProviderModelDependency::new(provider, preference_id))
        })
        .collect::<Vec<_>>();
    dependencies.sort_by(|left, right| {
        (&left.provider, &left.model_id).cmp(&(&right.provider, &right.model_id))
    });
    dependencies.dedup();
    Ok(dependencies)
}

fn provider_account_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())
}

fn fetch_fal_account_status(provider: String, credential: &str) -> ProviderAccountStatus {
    let client = match provider_account_client() {
        Ok(client) => client,
        Err(error) => {
            return provider_account_status(provider, "unavailable", None, None, &error);
        }
    };

    let response = match client
        .get("https://api.fal.ai/v1/account/billing")
        .query(&[("expand", "credits")])
        .header(AUTHORIZATION, format!("Key {credential}"))
        .header(ACCEPT, "application/json")
        .send()
    {
        Ok(response) => response,
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &error.to_string(),
            );
        }
    };

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return provider_account_status(
            provider,
            "rejected",
            None,
            None,
            &format!(
                "fal account billing returned HTTP status {}",
                status.as_u16()
            ),
        );
    }
    if !status.is_success() {
        return provider_account_status(
            provider,
            "unavailable",
            None,
            None,
            &format!(
                "fal account billing returned HTTP status {}",
                status.as_u16()
            ),
        );
    }

    let billing = match response.json::<FalAccountBillingResponse>() {
        Ok(billing) => billing,
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &format!("fal account billing response could not be decoded: {error}"),
            );
        }
    };
    let balance = billing.credits.map(|credits| ProviderAccountBalance {
        current_balance: credits.current_balance,
        currency: credits.currency,
    });
    let credential_status = if balance.is_some() {
        "available"
    } else {
        "balanceUnavailable"
    };
    provider_account_status(
        provider,
        credential_status,
        billing.username,
        balance,
        "fal account billing checked",
    )
}

fn fetch_replicate_account_status(provider: String, credential: &str) -> ProviderAccountStatus {
    let client = match provider_account_client() {
        Ok(client) => client,
        Err(error) => {
            return provider_account_status(provider, "unavailable", None, None, &error);
        }
    };

    let response = match client
        .get("https://api.replicate.com/v1/account")
        .header(AUTHORIZATION, format!("Bearer {credential}"))
        .header(ACCEPT, "application/json")
        .send()
    {
        Ok(response) => response,
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &error.to_string(),
            );
        }
    };

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return provider_account_status(
            provider,
            "rejected",
            None,
            None,
            &format!("replicate account returned HTTP status {}", status.as_u16()),
        );
    }
    if !status.is_success() {
        return provider_account_status(
            provider,
            "unavailable",
            None,
            None,
            &format!("replicate account returned HTTP status {}", status.as_u16()),
        );
    }

    let account = match response.json::<ReplicateAccountResponse>() {
        Ok(account) => account,
        Err(error) => {
            return provider_account_status(
                provider,
                "unavailable",
                None,
                None,
                &format!("replicate account response could not be decoded: {error}"),
            );
        }
    };

    provider_account_status(
        provider,
        "balanceUnavailable",
        account.name.or(account.username),
        None,
        "Replicate account API does not expose credit balance",
    )
}

fn reserve_provider_health_refresh(
    registry: &SettingsOperationRegistry,
    provider: Option<&str>,
    provider_count: usize,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::ProviderRefresh,
            provider.unwrap_or("providers"),
            Some(provider_count as u64),
            Some("providers".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued provider health refresh.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

fn start_provider_health_refresh_with<R, F>(
    app: &tauri::AppHandle<R>,
    registry: &SettingsOperationRegistry,
    provider: Option<&str>,
    provider_count: usize,
    refresh: F,
) -> Result<SettingsOperation, SettingsOperationsCommandError>
where
    R: tauri::Runtime,
    F: FnOnce() -> Result<Vec<ProviderHealth>, String> + Send + 'static,
{
    let (operation, created) = reserve_provider_health_refresh(registry, provider, provider_count)?;
    if !created {
        return Ok(operation);
    }
    let refresh_request = app
        .state::<ProviderHealthState>()
        .begin_refresh(provider)
        .map_err(|detail| SettingsOperationsCommandError {
            code: "settings.providers.cacheUnavailable".to_string(),
            message: "Provider health is temporarily unavailable.".to_string(),
            detail,
        })?;
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let worker_app = app.clone();
    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_provider_health_refresh_operation(
            worker_app,
            operation_id,
            provider_count,
            refresh_request,
            refresh,
        );
    });
    Ok(operation)
}

fn run_provider_health_refresh_operation<R, F>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    provider_count: usize,
    refresh_request: ProviderHealthRefreshRequest,
    refresh: F,
) where
    R: tauri::Runtime,
    F: FnOnce() -> Result<Vec<ProviderHealth>, String>,
{
    {
        let registry = app.state::<SettingsOperationRegistryState>();
        let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
        running.phase = Some("validating".to_string());
        running.completed_units = Some(0);
        running.total_units = Some(provider_count as u64);
        running.unit = Some("providers".to_string());
        running.cancellable = Some(false);
        running.message =
            Some("Checking provider credentials and account availability.".to_string());
        match registry.0.update(&operation_id, running) {
            Ok(operation) => {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
            Err(error) => {
                if let Ok(operation) = registry.0.project_persistence_failure(
                    &operation_id,
                    "settings.providers.refreshPersistenceFailed",
                    "Provider health refresh could not save its running state.",
                    &error.to_string(),
                ) {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                return;
            }
        }
    }

    let refreshed = refresh().and_then(|health| {
        app.state::<ProviderHealthState>()
            .merge_refresh(&refresh_request, health.clone())?;
        Ok(health)
    });
    match refreshed {
        Ok(health) => {
            let registry = app.state::<SettingsOperationRegistryState>();
            let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
            succeeded.phase = Some("refreshed".to_string());
            succeeded.completed_units = Some(provider_count as u64);
            succeeded.total_units = Some(provider_count as u64);
            succeeded.unit = Some("providers".to_string());
            succeeded.cancellable = Some(false);
            succeeded.message = Some(format!(
                "Provider health refreshed for {} provider(s).",
                health.len()
            ));
            match registry.0.update(&operation_id, succeeded) {
                Ok(operation) => {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                Err(error) => {
                    if let Ok(operation) = registry.0.project_persistence_failure(
                        &operation_id,
                        "settings.providers.refreshPersistenceFailed",
                        "Provider health refresh finished, but its result could not be saved.",
                        &error.to_string(),
                    ) {
                        let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                    }
                }
            }
        }
        Err(_detail) => {
            let registry = app.state::<SettingsOperationRegistryState>();
            let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
            failed.phase = Some("failed".to_string());
            failed.total_units = Some(provider_count as u64);
            failed.unit = Some("providers".to_string());
            failed.cancellable = Some(false);
            failed.message = Some("Provider health refresh failed.".to_string());
            failed.error = Some(SettingsOperationError {
                code: "settings.providers.refreshFailed".to_string(),
                message: "Provider health could not be refreshed.".to_string(),
                recovery_action: Some("Retry provider validation.".to_string()),
                detail: None,
            });
            match registry.0.update(&operation_id, failed) {
                Ok(operation) => {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                Err(error) => {
                    if let Ok(operation) = registry.0.project_persistence_failure(
                        &operation_id,
                        "settings.providers.refreshPersistenceFailed",
                        "Provider health refresh failed and its terminal state could not be saved.",
                        &error.to_string(),
                    ) {
                        let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                    }
                }
            }
        }
    }
}

fn refresh_provider_health_snapshot(
    provider: Option<&str>,
    disabled_generation_model_ids: &[String],
) -> Result<Vec<ProviderHealth>, String> {
    let credentials = stored_provider_credential_statuses();
    let payload = list_models_payload(None).map_err(|error| error.to_string())?;
    let dependencies =
        provider_model_dependencies_from_payload(&payload, disabled_generation_model_ids)?;
    let providers = credentials
        .iter()
        .filter(|status| provider.is_none_or(|provider| status.provider == provider))
        .map(|status| status.provider.clone())
        .collect::<Vec<_>>();
    let validations = providers
        .into_iter()
        .map(get_provider_account_status)
        .map(provider_account_validation)
        .collect::<Vec<_>>();
    aggregate_provider_health(&credentials, &validations, &dependencies, provider)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_provider_health(
    state: tauri::State<'_, ProviderHealthState>,
) -> Result<Vec<ProviderHealth>, SettingsOperationsCommandError> {
    state
        .snapshot()
        .map_err(|detail| SettingsOperationsCommandError {
            code: "settings.providers.cacheUnavailable".to_string(),
            message: "Provider health is temporarily unavailable.".to_string(),
            detail,
        })
}

#[tauri::command]
fn refresh_provider_health(
    app: tauri::AppHandle,
    provider: Option<String>,
    disabled_generation_model_ids: Option<Vec<String>>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let provider = provider
        .as_deref()
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .map(str::to_string);
    if provider
        .as_deref()
        .is_some_and(|provider| !is_supported_provider(provider))
    {
        return Err(SettingsOperationsCommandError {
            code: "settings.providers.unsupportedProvider".to_string(),
            message: "This provider is not supported.".to_string(),
            detail: format!(
                "unsupported provider: {}",
                provider.as_deref().unwrap_or_default()
            ),
        });
    }
    let provider_count = if provider.is_some() {
        1
    } else {
        supported_provider_count()
    };
    let worker_provider = provider.clone();
    let disabled_generation_model_ids = disabled_generation_model_ids.unwrap_or_default();
    start_provider_health_refresh_with(
        &app,
        &operation_state.0,
        provider.as_deref(),
        provider_count,
        move || {
            refresh_provider_health_snapshot(
                worker_provider.as_deref(),
                &disabled_generation_model_ids,
            )
        },
    )
}

#[tauri::command]
async fn save_project_to_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
    project: VideoProject,
) -> Result<String, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project save", &queue_dir, move |_| {
        save_project_to_folder_blocking(
            project_dir,
            project,
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
        )
    })
    .await
}

fn save_project_to_folder_blocking(
    project_dir: String,
    project: VideoProject,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
) -> Result<String, String> {
    let lease = mutation_coordinator.acquire()?;
    let result = save_project_to_folder_impl(project_dir.clone(), project)?;
    let project_dir = resolve_project_dir(&project_dir)?;
    session_state.record_successful_project_root(&project_dir, &lease)?;
    Ok(result)
}

fn save_project_to_folder_impl(
    project_dir: String,
    project: VideoProject,
) -> Result<String, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let manifest_path = storage::project_file_path(&project_dir);
    if manifest_path.exists() {
        let manifest = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).map_err(|error| error.to_string())?;
        if manifest.get("layout").and_then(serde_json::Value::as_str) == Some("split") {
            return Err(
                "legacy monolithic save cannot overwrite an existing split project".to_string(),
            );
        }
    }
    storage::save_project(&project_dir, &project)
        .map(|path| path.display().to_string())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn load_project_from_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
) -> Result<VideoProject, String> {
    run_blocking_command("project load", move || {
        load_project_from_folder_blocking(
            project_dir,
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
        )
    })
    .await
}

fn load_project_from_folder_blocking(
    project_dir: String,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
) -> Result<VideoProject, String> {
    let lease = mutation_coordinator.acquire()?;
    let project = load_project_from_folder_impl(project_dir.clone())?;
    let project_dir = resolve_project_dir(&project_dir)?;
    session_state.record_successful_project_root(&project_dir, &lease)?;
    Ok(project)
}

fn load_project_from_folder_impl(project_dir: String) -> Result<VideoProject, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    storage::load_project(&project_dir).map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_split_project_to_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
    project: VideoProject,
    expected_revision: u64,
    activate_project: Option<bool>,
) -> Result<ProjectActionWriteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project save", &queue_dir, move |_| {
        save_split_project_to_folder_blocking(
            project_dir,
            project,
            expected_revision,
            activate_project.unwrap_or(true),
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
        )
    })
    .await
}

fn save_split_project_to_folder_blocking(
    project_dir: String,
    project: VideoProject,
    expected_revision: u64,
    activate_project: bool,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
) -> Result<ProjectActionWriteResult, String> {
    if !activate_project {
        let project_dir = resolve_project_dir(&project_dir)?;
        let identity =
            video_creater_lib::app_service::projects::read_project_identity(&project_dir)
                .map_err(|error| error.to_string())?;
        if identity.id != project.id {
            return Err(
                "snapshot restore project identity does not match the canonical project".into(),
            );
        }
        let context = desktop_project_context(&project, expected_revision);
        return desktop_project_service()
            .save_existing(&context, &project_dir, project)
            .map_err(|error| error.to_string());
    }
    let lease = mutation_coordinator.acquire()?;
    let result =
        save_split_project_to_folder_impl(project_dir.clone(), project, expected_revision)?;
    let project_dir = resolve_project_dir(&project_dir)?;
    session_state.record_successful_project_root(&project_dir, &lease)?;
    Ok(result)
}

fn save_split_project_to_folder_impl(
    project_dir: String,
    project: VideoProject,
    expected_revision: u64,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let context = desktop_project_context(&project, expected_revision);
    desktop_project_service()
        .save(&context, &project_dir, project)
        .map_err(|error| error.to_string())
}

/// Runs a command's blocking work on the async runtime's blocking pool.
///
/// Synchronous Tauri commands run on the main thread, which on Linux iterates the GLib main context
/// GTK and WebKitGTK share. Renders, preview preparation and media decoding there freeze the webview,
/// and while they hold the storage or project lease every other synchronous command waits behind
/// them. Commands that render, decode or wait on a lease a render holds run here instead.
async fn run_blocking_command<T: Send + 'static>(
    label: &'static str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("{label} task failed: {error}"))?
}

/// [`run_blocking_command`] for settings commands, which report structured errors.
async fn run_blocking_settings_command<T: Send + 'static>(
    label: &'static str,
    work: impl FnOnce() -> Result<T, SettingsOperationsCommandError> + Send + 'static,
) -> Result<T, SettingsOperationsCommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| SettingsOperationsCommandError {
            code: "settings.operation.taskFailed".to_string(),
            message: "The settings operation stopped unexpectedly.".to_string(),
            detail: format!("{label} task failed: {error}"),
        })?
}

/// Runs a command that writes a canonical project through the per-project FIFO queue.
///
/// The command returns to the async runtime at once; its work waits for the storage and project
/// leases on the project's queue worker and runs in the order the commands were submitted (see
/// `video_creater_lib::project::command_queue`). A render holds the storage lease and the
/// project's artifact lease for its whole run, so a command that needs either waits for the
/// render; a canonical project action needs only the project mutation lease, which a render
/// releases while it encodes.
async fn run_project_write_command<T: Send + 'static>(
    label: &'static str,
    project_dir: &str,
    work: impl FnOnce(PathBuf) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let project_dir = resolve_project_dir(project_dir)?;
    let queue_dir = project_dir.clone();
    project_command_queue()
        .submit(&queue_dir, label, move || work(project_dir))?
        .await
}

/// Loads the project off the main thread: the editor polls this while a render holds the storage
/// lease.
#[tauri::command]
async fn load_split_project_from_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
) -> Result<VideoProject, String> {
    run_blocking_command("project load", move || {
        load_split_project_from_folder_blocking(
            project_dir,
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
            &app.state::<SettingsAcceptanceStateHolder>(),
        )
    })
    .await
}

fn load_split_project_from_folder_blocking(
    project_dir: String,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
    acceptance_state: &SettingsAcceptanceStateHolder,
) -> Result<VideoProject, String> {
    write_settings_acceptance_storage_load_progress(
        acceptance_state,
        SettingsAcceptanceProgressStep::StorageLoadCommandReceived,
    )?;
    let lease = mutation_coordinator.acquire()?;
    write_settings_acceptance_storage_load_progress(
        acceptance_state,
        SettingsAcceptanceProgressStep::StorageMutationLeaseAcquired,
    )?;
    let project = load_split_project_from_folder_impl_with_lease(project_dir.clone(), &lease)?;
    let context = desktop_project_context(&project, project.content_revision);
    let project = desktop_project_service()
        .authorize_loaded(&context, project)
        .map_err(|error| error.to_string())?;
    let project_dir = resolve_project_dir(&project_dir)?;
    session_state.record_successful_project_root(&project_dir, &lease)?;
    write_settings_acceptance_storage_load_progress(
        acceptance_state,
        SettingsAcceptanceProgressStep::StorageProjectSessionRecorded,
    )?;
    Ok(project)
}

#[tauri::command]
async fn load_settings_acceptance_project<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
) -> Result<SettingsAcceptanceProjectLoad, String> {
    run_blocking_command("acceptance project load", move || {
        load_settings_acceptance_project_blocking(
            project_dir,
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
            &app.state::<SettingsAcceptanceStateHolder>(),
        )
    })
    .await
}

fn load_settings_acceptance_project_blocking(
    project_dir: String,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
    acceptance_state: &SettingsAcceptanceStateHolder,
) -> Result<SettingsAcceptanceProjectLoad, String> {
    let acceptance = acceptance_state
        .0
        .as_ref()
        .ok_or_else(|| "settings acceptance is not enabled".to_string())?;
    let requested_project_root = resolve_project_dir(&project_dir)?;
    if requested_project_root != acceptance.context().project_root {
        return Err("settings acceptance project root mismatch".to_string());
    }
    let project = load_split_project_from_folder_blocking(
        project_dir,
        session_state,
        mutation_coordinator,
        acceptance_state,
    )?;
    Ok(SettingsAcceptanceProjectLoad {
        id: project.id,
        render_artifact_ids: project
            .render_reports
            .into_iter()
            .map(|report| report.id)
            .collect(),
    })
}

#[cfg(test)]
fn load_split_project_from_folder_impl(project_dir: String) -> Result<VideoProject, String> {
    let lease = acquire_storage_mutation_lease()?;
    load_split_project_from_folder_impl_with_lease(project_dir, &lease)
}

fn load_split_project_from_folder_impl_with_lease(
    project_dir: String,
    lease: &StorageProjectMutationLease,
) -> Result<VideoProject, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let now = chrono::Utc::now().to_rfc3339();
    recover_interrupted_local_render_jobs_with_lease(&project_dir, &now, lease)
        .map_err(pipeline_errors_to_string)?;
    let recovery = reconcile_interrupted_generation_jobs_on_project_open(&project_dir, &now)
        .map_err(|error| error.to_string())?;
    let mut project = recovery.project;
    spawn_interrupted_generation_recovery(
        project_dir.clone(),
        project.id.clone(),
        recovery.resume_candidates,
        now,
    );
    project_persisted_speech_analysis(&project_dir, &mut project)
        .map_err(|error| error.to_string())?;
    // Progress snapshots are bookkeeping: a render that ended without clearing its snapshot (a
    // crash, a closed app) must not keep showing progress on a finished job.
    let _ = remove_settled_job_progress(&project_dir, &project);
    Ok(project)
}

/// Reads running jobs' progress snapshots. It takes no lease, so the editor can poll it while a
/// render holds the project lease and the project reload is blocked.
#[tauri::command]
async fn load_job_progress_from_split_project_folder(
    project_dir: String,
) -> Result<Vec<JobProgressSnapshot>, String> {
    run_blocking_command("job progress load", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        let identity =
            video_creater_lib::app_service::projects::read_project_identity(&project_dir)
                .map_err(|error| error.to_string())?;
        let context = RequestContext::new(
            ClientKind::Desktop,
            uuid::Uuid::new_v4().to_string(),
            Some(identity.id),
            None,
            BTreeSet::from([AuthorizationScope::ProjectRead]),
            None,
        )
        .map_err(|error| error.to_string())?;
        desktop_project_service()
            .job_progress(&context, &project_dir)
            .map_err(|error| error.to_string())
    })
    .await
}

/// Fails unfinished Temporal jobs whose workflows are closed, missing or never started. An
/// unreachable workflow service leaves every job untouched.
#[tauri::command]
async fn reconcile_temporal_jobs_in_split_project_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
    updated_at: String,
) -> Result<TemporalJobReconciliationResult, String> {
    let generation_backend_in_process = app
        .try_state::<AppPreferencesState>()
        .and_then(|state| {
            state
                .0
                .lock()
                .ok()
                .and_then(|store| store.load_saved_or_default().ok())
        })
        .is_none_or(|preferences| {
            preferences.generation_execution_backend == GenerationExecutionBackend::InProcess
        });
    reconcile_temporal_jobs(project_dir, updated_at, generation_backend_in_process).await
}

#[cfg(feature = "temporal-worker")]
async fn reconcile_temporal_jobs(
    project_dir: String,
    updated_at: String,
    generation_backend_in_process: bool,
) -> Result<TemporalJobReconciliationResult, String> {
    let resolved_dir = resolve_project_dir(&project_dir)?;
    let load_dir = resolved_dir.clone();
    let project = run_blocking_command("reconciliation project load", move || {
        load_split_project(&load_dir).map_err(|error| error.to_string())
    })
    .await?;
    if temporal_reconciliation_workflow_ids(&project, generation_backend_in_process).is_empty() {
        return Ok(TemporalJobReconciliationResult {
            project: Some(project),
            failed_job_ids: Vec::new(),
            service_reachable: true,
            detail: None,
        });
    }
    let Ok(client) = connect_temporal_client_from_environment().await else {
        return Ok(temporal_unreachable_result());
    };
    let describer = TemporalClientDescriber::new(client);
    let options = TemporalReconcileOptions::at(&updated_at, generation_backend_in_process);
    reconcile_temporal_jobs_with_describer(
        &describer,
        generation_backend_in_process,
        || async move { Ok(project) },
        |observations| async move {
            run_project_write_command(
                "Temporal job reconciliation",
                &project_dir,
                move |project_dir| {
                    apply_temporal_job_reconciliation(&project_dir, &observations, &options)
                },
            )
            .await
        },
    )
    .await
}

#[cfg(not(feature = "temporal-worker"))]
async fn reconcile_temporal_jobs(
    project_dir: String,
    _updated_at: String,
    _generation_backend_in_process: bool,
) -> Result<TemporalJobReconciliationResult, String> {
    resolve_project_dir(&project_dir)?;
    Ok(temporal_unreachable_result())
}

#[tauri::command]
async fn create_matte_in_split_project_folder(
    project_dir: String,
    request: MatteRequest,
) -> Result<MatteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("matte creation", &queue_dir, move |_| {
        create_matte_in_split_project_folder_blocking(project_dir, request)
    })
    .await
}

fn create_matte_in_split_project_folder_blocking(
    project_dir: String,
    request: MatteRequest,
) -> Result<MatteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    create_matte(&project_dir, request).map_err(|error| error.to_string())
}

fn spawn_interrupted_generation_recovery(
    project_dir: PathBuf,
    project_id: String,
    candidates: Vec<video_creater_lib::workflows::InterruptedGenerationResumeCandidate>,
    updated_at: String,
) {
    for candidate in candidates {
        let Ok(cancellation_guard) =
            register_generation_cancellation(&project_id, &candidate.job_id)
        else {
            continue;
        };
        let cancellation = cancellation_guard.token();
        let project_dir = project_dir.clone();
        let updated_at = updated_at.clone();
        std::thread::spawn(move || {
            let _cancellation_guard = cancellation_guard;
            let start_request = load_split_project(&project_dir)
                .ok()
                .and_then(|project| {
                    project
                        .jobs
                        .into_iter()
                        .find(|job| job.id == candidate.job_id)
                })
                .and_then(|job| job.start_request);
            let recovery = || -> Result<(), String> {
                let credential = resolve_provider_credential(&candidate.provider)
                    .map_err(|error| error.to_string())?
                    .into_secret();
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(60))
                    .build()
                    .map_err(|error| error.to_string())?;
                resume_interrupted_generate_media_job_with_client_and_credential_cancellable(
                    &client,
                    &project_dir,
                    &candidate.job_id,
                    &updated_at,
                    &credential,
                    TemporalGenerateMediaProviderRunOptions::default(),
                    Some(&cancellation),
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            };

            if recovery().is_err() && !cancellation.is_cancelled() {
                if let Some(start_request) = start_request {
                    let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                        &start_request,
                        Some(&format!("restart-recovery/{}", candidate.job_id)),
                        &updated_at,
                    );
                }
            }
            cancellation.mark_terminal();
        });
    }
}

#[tauri::command]
fn list_visual_effect_catalog() -> serde_json::Value {
    effect_catalog_payload()
}

#[tauri::command]
async fn prepare_project_preview(
    project_dir: String,
    project: VideoProject,
) -> Result<PreparedPreviewCommandResult, String> {
    run_blocking_command("preview preparation", move || {
        prepare_project_preview_blocking(project_dir, project)
    })
    .await
}

fn prepare_project_preview_blocking(
    project_dir: String,
    project: VideoProject,
) -> Result<PreparedPreviewCommandResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    // Preparation owns the project's derived files for its whole run, so it cannot race a
    // render's precompose publication; the project lease covers its read alone, so a
    // render-length preparation never keeps an editor write waiting.
    let _artifacts = acquire_split_project_artifact_lease(&project_dir)?;
    let project = {
        let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
        if storage::project_file_path(&project_dir).exists() {
            load_split_project(&project_dir).map_err(|error| error.to_string())?
        } else {
            project
        }
    };
    let expanded =
        expand_project_nested_timelines_for_render(&project).map_err(pipeline_errors_to_string)?;
    let prepared =
        prepare_project_for_render(&project_dir, &expanded).map_err(pipeline_errors_to_string)?;
    let mut frame_sequences = Vec::new();
    for track in &prepared.project.timeline.tracks {
        for item in &track.items {
            let TimelineSource::Media { media_id } = &item.source else {
                continue;
            };
            let Some(report) = prepared
                .reports
                .iter()
                .find(|report| report.prepared_media_id == *media_id)
            else {
                continue;
            };
            if !report.has_frame_sequence() {
                continue;
            }
            let frames_dir = Path::new(&report.intermediate)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join("frames");
            let absolute_frames_dir = project_dir.join(&frames_dir);
            let mut frame_paths = fs::read_dir(&absolute_frames_dir)
                .map_err(|error| format!("prepared preview frames could not be listed: {error}"))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
                .collect::<Vec<_>>();
            frame_paths.sort();
            let frame_paths = frame_paths
                .into_iter()
                .map(|path| {
                    path.strip_prefix(&project_dir)
                        .map(|relative| relative.to_string_lossy().to_string())
                        .map_err(|_| {
                            "prepared preview frame escaped the project folder".to_string()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if frame_paths.is_empty() {
                return Err(format!(
                    "prepared preview sequence for {} contains no PNG frames",
                    item.id
                ));
            }
            let fps = prepared
                .project
                .media
                .iter()
                .find(|media| media.id == *media_id)
                .and_then(|media| media.fps)
                .unwrap_or(prepared.project.render_settings.fps);
            // Prepared frames can begin with transition handles before the clip.
            let (start_seconds, duration_seconds) =
                prepared_media_timeline_span(&prepared.project, item);
            frame_sequences.push(PreparedPreviewFrameSequence {
                item_id: item.id.clone(),
                prepared_media_id: media_id.clone(),
                start_seconds,
                duration_seconds,
                fps,
                frame_paths,
            });
        }
    }
    Ok(PreparedPreviewCommandResult {
        project: prepared.project,
        reports: prepared.reports,
        frame_sequences,
    })
}

#[tauri::command]
async fn capture_canonical_preview_frame_in_split_project_folder(
    project_dir: String,
    playhead_seconds: f64,
    job_id: String,
    updated_at: String,
) -> Result<PreparedPreviewFrameResult, String> {
    run_blocking_command("canonical preview frame", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        render_prepared_preview_frame_to_split_project_folder(
            &project_dir,
            playhead_seconds,
            &job_id,
            &updated_at,
        )
        .map_err(pipeline_errors_to_string)
    })
    .await
}

#[tauri::command]
async fn analyze_project_speech(
    project_dir: String,
    media_id: String,
    prepared_pcm_path: String,
    model_state: tauri::State<'_, ProductionSpeechModelStoreState>,
) -> Result<MediaSpeechSidecar, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let pcm_path = resolve_project_relative_path(&project_dir, &prepared_pcm_path)
        .map_err(|error| error.to_string())?;
    let model_store = model_state.0.clone();
    if !model_store.status().ready {
        return Err("production speech models are not installed; download them from Model settings, then retry analysis".to_string());
    }
    let helper_path = default_speech_helper_path();
    let model_root = model_store.root().to_path_buf();
    let analysis_project_dir = project_dir.clone();
    let analysis_media_id = media_id.clone();
    let sidecar = tauri::async_runtime::spawn_blocking(move || {
        let _project_lease = acquire_split_project_mutation_lease(&analysis_project_dir)?;
        let sidecar = analyze_wav_cached_production(
            &analysis_project_dir,
            &analysis_media_id,
            &pcm_path,
            &helper_path,
            &model_root,
        )
        .map_err(|error| error.to_string())?;
        let registry =
            load_speaker_registry(&analysis_project_dir).map_err(|error| error.to_string())?;
        let mut project =
            load_split_project(&analysis_project_dir).map_err(|error| error.to_string())?;
        project_sidecar(&mut project, &sidecar, &registry);
        save_split_project(&analysis_project_dir, &project).map_err(|error| error.to_string())?;
        Ok::<_, String>(sidecar)
    })
    .await
    .map_err(|error| format!("production speech analysis task failed: {error}"))??;
    Ok(sidecar)
}

#[tauri::command]
async fn rename_project_speaker(
    project_dir: String,
    speaker_id: String,
    name: String,
) -> Result<SpeakerRegistry, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("speaker rename", &queue_dir, move |_| {
        rename_project_speaker_blocking(project_dir, speaker_id, name)
    })
    .await
}

fn rename_project_speaker_blocking(
    project_dir: String,
    speaker_id: String,
    name: String,
) -> Result<SpeakerRegistry, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let registry =
        rename_speaker(&project_dir, &speaker_id, &name).map_err(|error| error.to_string())?;
    reproject_and_save_speech(&project_dir)?;
    Ok(registry)
}

#[tauri::command]
fn get_project_speaker_registry(project_dir: String) -> Result<SpeakerRegistry, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    load_speaker_registry(&project_dir).map_err(|error| error.to_string())
}

#[tauri::command]
async fn recolor_project_speaker(
    project_dir: String,
    speaker_id: String,
    color: String,
) -> Result<SpeakerRegistry, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("speaker recolor", &queue_dir, move |_| {
        recolor_project_speaker_blocking(project_dir, speaker_id, color)
    })
    .await
}

fn recolor_project_speaker_blocking(
    project_dir: String,
    speaker_id: String,
    color: String,
) -> Result<SpeakerRegistry, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let registry =
        recolor_speaker(&project_dir, &speaker_id, &color).map_err(|error| error.to_string())?;
    reproject_and_save_speech(&project_dir)?;
    Ok(registry)
}

#[tauri::command]
async fn assign_project_media_speaker(
    project_dir: String,
    fingerprint: String,
    start_seconds: f64,
    end_seconds: f64,
    speaker_id: String,
) -> Result<MediaSpeechSidecar, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("speaker assignment", &queue_dir, move |_| {
        assign_project_media_speaker_blocking(
            project_dir,
            fingerprint,
            start_seconds,
            end_seconds,
            speaker_id,
        )
    })
    .await
}

fn assign_project_media_speaker_blocking(
    project_dir: String,
    fingerprint: String,
    start_seconds: f64,
    end_seconds: f64,
    speaker_id: String,
) -> Result<MediaSpeechSidecar, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let sidecar = assign_media_speaker(
        &project_dir,
        &fingerprint,
        start_seconds,
        end_seconds,
        &speaker_id,
    )
    .map_err(|error| error.to_string())?;
    let registry = load_speaker_registry(&project_dir).map_err(|error| error.to_string())?;
    let mut project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    project_sidecar(&mut project, &sidecar, &registry);
    save_split_project(&project_dir, &project).map_err(|error| error.to_string())?;
    Ok(sidecar)
}

fn reproject_and_save_speech(project_dir: &Path) -> Result<(), String> {
    let _project_lease = acquire_split_project_mutation_lease(project_dir)?;
    let mut project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    project_persisted_speech_analysis(project_dir, &mut project)
        .map_err(|error| error.to_string())?;
    save_split_project(project_dir, &project).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn validate_split_project_folder(
    project_dir: String,
) -> Result<ProjectValidationReport, String> {
    run_blocking_command("project validation", move || {
        validate_split_project_folder_blocking(project_dir)
    })
    .await
}

fn validate_split_project_folder_blocking(
    project_dir: String,
) -> Result<ProjectValidationReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let context = desktop_project_context(&project, project.content_revision);
    desktop_project_service()
        .validate(&context, &project_dir)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn load_app_server_conversation_history_from_split_project_folder(
    project_dir: String,
) -> Result<SplitAppServerConversationFile, String> {
    run_blocking_command("conversation history load", move || {
        load_app_server_conversation_history_from_split_project_folder_blocking(project_dir)
    })
    .await
}

fn load_app_server_conversation_history_from_split_project_folder_blocking(
    project_dir: String,
) -> Result<SplitAppServerConversationFile, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let context = desktop_project_context(&project, project.content_revision);
    desktop_project_service()
        .conversation_history(&context, &project_dir)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn load_agent_sessions_from_split_project_folder(
    project_dir: String,
) -> Result<SplitAgentSessionManifest, String> {
    run_blocking_command("agent sessions load", move || {
        load_agent_sessions_from_split_project_folder_blocking(project_dir)
    })
    .await
}

fn load_agent_sessions_from_split_project_folder_blocking(
    project_dir: String,
) -> Result<SplitAgentSessionManifest, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let context = desktop_project_context(&project, project.content_revision);
    desktop_project_service()
        .agent_sessions(&context, &project_dir)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn apply_agent_session_action_to_split_project_folder(
    project_dir: String,
    project_id: String,
    action: AgentSessionAction,
) -> Result<SplitAgentSessionManifest, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("agent session action", &queue_dir, move |_| {
        apply_agent_session_action_to_split_project_folder_blocking(project_dir, project_id, action)
    })
    .await
}

fn apply_agent_session_action_to_split_project_folder_blocking(
    project_dir: String,
    project_id: String,
    action: AgentSessionAction,
) -> Result<SplitAgentSessionManifest, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    apply_agent_session_action(&project_dir, &project_id, action).map_err(|error| error.to_string())
}

#[tauri::command]
async fn search_project_media(
    project_dir: String,
    query: String,
    limit: Option<usize>,
    scope: Option<String>,
    media_id: Option<String>,
) -> Result<serde_json::Value, String> {
    run_blocking_command("media search", move || {
        search_project_media_blocking(project_dir, query, limit, scope, media_id)
    })
    .await
}

fn search_project_media_blocking(
    project_dir: String,
    query: String,
    limit: Option<usize>,
    scope: Option<String>,
    media_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let scope = SearchScope::parse(scope.as_deref().unwrap_or("both"))
        .map_err(|error| error.to_string())?;
    query_project_search_for_project_dir(
        &project_dir,
        &project,
        ProjectSearchQuery {
            query,
            limit: limit.unwrap_or(20),
            scope,
            media_id,
        },
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSearchIndexRebuildReport {
    project_id: String,
    index_path: String,
    media_count: usize,
    transcript_count: usize,
    generated_asset_count: usize,
}

static VISUAL_FRAME_CACHE_CANCELLED_JOB_IDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn visual_frame_cache_cancelled_job_ids() -> &'static Mutex<HashSet<String>> {
    VISUAL_FRAME_CACHE_CANCELLED_JOB_IDS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn visual_frame_cache_job_is_cancelled(job_id: &str) -> bool {
    visual_frame_cache_cancelled_job_ids()
        .lock()
        .map(|job_ids| job_ids.contains(job_id))
        .unwrap_or(true)
}

fn finish_visual_frame_cache_job(job_id: Option<&str>) {
    if let Some(job_id) = job_id {
        if let Ok(mut job_ids) = visual_frame_cache_cancelled_job_ids().lock() {
            job_ids.remove(job_id);
        }
    }
}

#[tauri::command]
async fn rebuild_project_search_index(
    project_dir: String,
) -> Result<ProjectSearchIndexRebuildReport, String> {
    run_blocking_command("search index rebuild", move || {
        rebuild_project_search_index_blocking(project_dir)
    })
    .await
}

fn rebuild_project_search_index_blocking(
    project_dir: String,
) -> Result<ProjectSearchIndexRebuildReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let index = rebuild_project_search_index_for_project_dir(&project_dir, &project)
        .map_err(|error| error.to_string())?;
    Ok(ProjectSearchIndexRebuildReport {
        project_id: index.project_id,
        index_path: project_search_index_path(&project_dir)
            .display()
            .to_string(),
        media_count: index.media_count,
        transcript_count: index.transcript_count,
        generated_asset_count: index.generated_asset_count,
    })
}

#[tauri::command]
async fn extract_visual_frame_cache_in_split_project_folder(
    project_dir: String,
    media_id: String,
    job_id: Option<String>,
) -> Result<VisualFrameCacheReport, String> {
    run_blocking_command("visual frame extraction", move || {
        extract_visual_frame_cache_in_split_project_folder_blocking(project_dir, media_id, job_id)
    })
    .await
}

fn extract_visual_frame_cache_in_split_project_folder_blocking(
    project_dir: String,
    media_id: String,
    job_id: Option<String>,
) -> Result<VisualFrameCacheReport, String> {
    let job_id = job_id.filter(|job_id| !job_id.trim().is_empty());
    if job_id
        .as_deref()
        .is_some_and(visual_frame_cache_job_is_cancelled)
    {
        finish_visual_frame_cache_job(job_id.as_deref());
        return Err("visual frame extraction was cancelled".to_string());
    }
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .cloned()
        .ok_or_else(|| format!("media was not found: {media_id}"))?;
    let source_path = resolve_project_relative_path(&project_dir, &media.relative_path)
        .map_err(|error| error.to_string())?;
    // Populate the stable media-id sidecar first so the immutable frame cache
    // can retain its timing and media-analysis labels without invoking a model.
    let result = (|| {
        rebuild_project_search_index_for_project_dir(&project_dir, &project)
            .map_err(|error| error.to_string())?;
        cache_visual_frames(&project_dir, &media, &source_path, || {
            job_id
                .as_deref()
                .is_some_and(visual_frame_cache_job_is_cancelled)
        })
        .map_err(|error| error.to_string())
    })();
    finish_visual_frame_cache_job(job_id.as_deref());
    result
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri exposes these stable filmstrip fields as named IPC arguments"
)]
async fn cache_timeline_filmstrip_in_split_project_folder(
    project_dir: String,
    media_id: String,
    source_in: f64,
    source_out: f64,
    speed: f64,
    zoom_bucket: u32,
    height_bucket: u32,
    clip_pixel_width: f64,
) -> Result<TimelineFilmstripReport, String> {
    run_blocking_command("timeline filmstrip", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        // Filmstrip caching owns the project's derived files for its whole run; the project
        // lease covers its read alone, so decoding never keeps an editor write waiting.
        let _artifacts = acquire_split_project_artifact_lease(&project_dir)?;
        let (media, source) = {
            let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
            let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
            let media = project
                .media
                .iter()
                .find(|media| media.id == media_id)
                .ok_or_else(|| format!("media was not found: {media_id}"))?
                .clone();
            let source = resolve_project_relative_path(&project_dir, &media.relative_path)
                .map_err(|error| error.to_string())?;
            (media, source)
        };
        cache_timeline_filmstrip(
            &project_dir,
            &media,
            &source,
            TimelineFilmstripRequest {
                source_in,
                source_out,
                speed,
                zoom_bucket,
                height_bucket,
                clip_pixel_width,
            },
        )
    })
    .await
}

#[tauri::command]
fn cancel_visual_frame_cache_job(job_id: String) -> bool {
    let job_id = job_id.trim();
    if job_id.is_empty() {
        return false;
    }
    visual_frame_cache_cancelled_job_ids()
        .lock()
        .map(|mut job_ids| job_ids.insert(job_id.to_string()))
        .unwrap_or(false)
}

#[tauri::command]
async fn caption_visual_frame_cache_in_split_project_folder(
    project_dir: String,
    media_id: String,
    job_id: Option<String>,
) -> Result<VisualCaptionReport, String> {
    run_blocking_command("visual frame captioning", move || {
        caption_visual_frame_cache_in_split_project_folder_blocking(project_dir, media_id, job_id)
    })
    .await
}

fn caption_visual_frame_cache_in_split_project_folder_blocking(
    project_dir: String,
    media_id: String,
    job_id: Option<String>,
) -> Result<VisualCaptionReport, String> {
    let job_id = job_id.filter(|job_id| !job_id.trim().is_empty());
    if job_id
        .as_deref()
        .is_some_and(visual_frame_cache_job_is_cancelled)
    {
        finish_visual_frame_cache_job(job_id.as_deref());
        return Err("visual captioning was cancelled".to_string());
    }
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let result = caption_cached_visual_frames_with_fal(&project_dir, &media_id, || {
        job_id
            .as_deref()
            .is_some_and(visual_frame_cache_job_is_cancelled)
    })
    .map_err(|error| error.to_string());
    finish_visual_frame_cache_job(job_id.as_deref());
    result
}

#[tauri::command]
async fn migrate_single_file_project_to_split(
    project_dir: String,
) -> Result<ProjectWriteReport, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project migration", &queue_dir, move |_| {
        migrate_single_file_project_to_split_blocking(project_dir)
    })
    .await
}

fn migrate_single_file_project_to_split_blocking(
    project_dir: String,
) -> Result<ProjectWriteReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    migrate_single_file_project(&project_dir).map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_media_to_project(
    project_dir: String,
    project: VideoProject,
    source_paths: Vec<String>,
    names: Option<BTreeMap<String, String>>,
) -> Result<ImportMediaResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("media import", &queue_dir, move |_| {
        import_media_to_project_blocking(project_dir, project, source_paths, names)
    })
    .await
}

/// `names` maps a source path to the display name of its media, such as a
/// saved timeline range; sources without an entry keep their file stem.
fn import_media_to_project_blocking(
    project_dir: String,
    project: VideoProject,
    source_paths: Vec<String>,
    names: Option<BTreeMap<String, String>>,
) -> Result<ImportMediaResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let source_paths = source_paths
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let names = names
        .unwrap_or_default()
        .into_iter()
        .map(|(source, name)| (PathBuf::from(source), name))
        .collect::<BTreeMap<_, _>>();
    import_media_files_with_names(&project_dir, project, &source_paths, &names)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_timeline_patch_to_project(
    mut project: VideoProject,
    patch: TimelinePatch,
) -> Result<VideoProject, String> {
    apply_timeline_patch(&mut project, patch)
        .map(|_| project)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_project_action_to_project(
    mut project: VideoProject,
    action: ProjectAction,
) -> Result<VideoProject, String> {
    apply_project_action(&mut project, action)
        .map(|_| project)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn apply_project_action_to_split_project_folder(
    project_dir: String,
    action: ProjectAction,
    expected_revision: Option<u64>,
) -> Result<ProjectActionWriteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project action", &queue_dir, move |_| {
        apply_project_action_to_split_project_folder_blocking(
            project_dir,
            action,
            expected_revision,
        )
    })
    .await
}

fn apply_project_action_to_split_project_folder_blocking(
    project_dir: String,
    action: ProjectAction,
    expected_revision: Option<u64>,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let identity = video_creater_lib::app_service::projects::read_project_identity(&project_dir)
        .map_err(|error| error.to_string())?;
    let context = RequestContext::new(
        ClientKind::Desktop,
        uuid::Uuid::new_v4().to_string(),
        Some(identity.id),
        Some(expected_revision.unwrap_or(identity.content_revision)),
        BTreeSet::from([AuthorizationScope::ProjectWrite]),
        None,
    )
    .map_err(|error| error.to_string())?;
    desktop_project_service()
        .apply_action(&context, &project_dir, action)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn apply_project_actions_to_split_project_folder(
    project_dir: String,
    actions: Vec<ProjectAction>,
    expected_revision: Option<u64>,
) -> Result<ProjectActionWriteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project actions", &queue_dir, move |_| {
        apply_project_actions_to_split_project_folder_blocking(
            project_dir,
            actions,
            expected_revision,
        )
    })
    .await
}

fn apply_project_actions_to_split_project_folder_blocking(
    project_dir: String,
    actions: Vec<ProjectAction>,
    expected_revision: Option<u64>,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let identity = video_creater_lib::app_service::projects::read_project_identity(&project_dir)
        .map_err(|error| error.to_string())?;
    let context = RequestContext::new(
        ClientKind::Desktop,
        uuid::Uuid::new_v4().to_string(),
        Some(identity.id),
        Some(expected_revision.unwrap_or(identity.content_revision)),
        BTreeSet::from([AuthorizationScope::ProjectWrite]),
        None,
    )
    .map_err(|error| error.to_string())?;
    desktop_project_service()
        .apply_actions(&context, &project_dir, actions)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn update_project_settings_in_split_project_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
    name: String,
    render_settings: RenderSettings,
) -> Result<ProjectActionWriteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("project settings update", &queue_dir, move |_| {
        update_project_settings_in_split_project_folder_blocking(
            project_dir,
            name,
            render_settings,
            &app.state::<StorageProjectMutationCoordinatorState>(),
        )
    })
    .await
}

fn update_project_settings_in_split_project_folder_blocking(
    project_dir: String,
    name: String,
    render_settings: RenderSettings,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
) -> Result<ProjectActionWriteResult, String> {
    let _lease = mutation_coordinator.acquire()?;
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let context = desktop_project_context(&project, project.content_revision);
    desktop_project_service()
        .update_settings(&context, &project_dir, name, render_settings)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn complete_mock_generated_asset_in_split_project_folder(
    project_dir: String,
    asset_id: String,
    updated_at: String,
    replacement_item_id: Option<String>,
) -> Result<ProjectActionWriteResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("mock generation completion", &queue_dir, move |_| {
        complete_mock_generated_asset_in_split_project_folder_blocking(
            project_dir,
            asset_id,
            updated_at,
            replacement_item_id,
        )
    })
    .await
}

fn complete_mock_generated_asset_in_split_project_folder_blocking(
    project_dir: String,
    asset_id: String,
    updated_at: String,
    replacement_item_id: Option<String>,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .cloned()
        .ok_or_else(|| format!("generated asset was not found: {asset_id}"))?;
    let actions =
        mock_generation_completion_actions(&asset, &updated_at, replacement_item_id.as_deref())
            .map_err(|error| error.to_string())?;

    apply_project_actions_to_split_project(&project_dir, actions).map_err(|error| error.to_string())
}

#[tauri::command]
async fn retry_generated_asset_output_download_in_split_project_folder(
    project_dir: String,
    asset_id: String,
    output_media_id: String,
) -> Result<ProjectActionWriteResult, String> {
    run_blocking_command("generated output retry", move || {
        retry_generated_asset_output_download_in_split_project_folder_blocking(
            project_dir,
            asset_id,
            output_media_id,
        )
    })
    .await
}

fn retry_generated_asset_output_download_in_split_project_folder_blocking(
    project_dir: String,
    asset_id: String,
    output_media_id: String,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    retry_generated_asset_output_download_with_policy_and_hook(
        &project_dir,
        &asset_id,
        &output_media_id,
        None,
        || Ok(()),
        || Ok(()),
    )
}

fn generated_output_retry_network_policy_for_provider(
    provider: &str,
    asset_id: &str,
) -> Result<DownloadNetworkPolicy, String> {
    if !is_supported_provider(provider) {
        return Err(format!(
            "generated asset {asset_id} uses an unsupported provider"
        ));
    }
    let mut policy = DownloadNetworkPolicy::public_only();
    if provider == REPLICATE_PROVIDER {
        if let Ok(origin) = std::env::var(VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR) {
            if !origin.trim().is_empty() {
                policy = policy
                    .with_trusted_origin(origin.trim())
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(policy)
}

#[cfg(test)]
fn retry_generated_asset_output_download_with_policy(
    project_dir: &Path,
    asset_id: &str,
    output_media_id: &str,
    policy: &DownloadNetworkPolicy,
) -> Result<ProjectActionWriteResult, String> {
    retry_generated_asset_output_download_with_policy_and_hook(
        project_dir,
        asset_id,
        output_media_id,
        Some(policy),
        || Ok(()),
        || Ok(()),
    )
}

trait RetryOutputFileOps {
    fn exists(&self, path: &Path) -> Result<bool, String>;
    fn copy_backup(&self, source: &Path, backup: &Path) -> Result<(), String>;
    fn sync_backup(&self, backup: &Path) -> Result<(), String>;
    fn replace(&self, source: &Path, destination: &Path) -> Result<(), String>;
    fn remove(&self, path: &Path) -> Result<(), String>;
}

struct SystemRetryOutputFileOps;

impl RetryOutputFileOps for SystemRetryOutputFileOps {
    fn exists(&self, path: &Path) -> Result<bool, String> {
        path.try_exists().map_err(|error| error.to_string())
    }

    fn copy_backup(&self, source: &Path, backup: &Path) -> Result<(), String> {
        fs::copy(source, backup)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn sync_backup(&self, backup: &Path) -> Result<(), String> {
        fs::File::open(backup)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())
    }

    fn replace(&self, source: &Path, destination: &Path) -> Result<(), String> {
        fs::rename(source, destination).map_err(|error| error.to_string())
    }

    fn remove(&self, path: &Path) -> Result<(), String> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn create_retry_backup(
    ops: &impl RetryOutputFileOps,
    source: &Path,
    backup: &Path,
) -> Result<(), String> {
    let creation_result = ops
        .copy_backup(source, backup)
        .and_then(|()| ops.sync_backup(backup));
    if let Err(creation_error) = creation_result {
        return match ops.remove(backup) {
            Ok(()) => Err(format!(
                "generated output retry backup failed; partial backup removed: {creation_error}"
            )),
            Err(cleanup_error) => Err(format!(
                "generated output retry backup failed: {creation_error}; partial backup cleanup failed and remains recoverable at {}: {cleanup_error}",
                backup.display()
            )),
        };
    }
    Ok(())
}

fn publish_retry_output_with_rollback<T>(
    ops: &impl RetryOutputFileOps,
    staging_path: &Path,
    output_path: &Path,
    backup_path: &Path,
    commit_metadata: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let had_existing = ops
        .exists(output_path)
        .map_err(|error| format!("generated output retry destination check failed: {error}"))?;
    if had_existing {
        create_retry_backup(ops, output_path, backup_path)?;
    }
    if let Err(publish_error) = ops.replace(staging_path, output_path) {
        let mut cleanup_failures = Vec::new();
        if had_existing {
            if let Err(cleanup_error) = ops.remove(backup_path) {
                cleanup_failures.push(format!(
                    "backup cleanup failed and remains recoverable at {}: {cleanup_error}",
                    backup_path.display()
                ));
            }
        }
        if let Err(cleanup_error) = ops.remove(staging_path) {
            cleanup_failures.push(format!(
                "staging cleanup failed and remains recoverable at {}: {cleanup_error}",
                staging_path.display()
            ));
        }
        if !cleanup_failures.is_empty() {
            return Err(format!(
                "generated output retry publication failed: {publish_error}; {}",
                cleanup_failures.join("; ")
            ));
        }
        return Err(format!(
            "generated output retry publication failed: {publish_error}"
        ));
    }

    match commit_metadata() {
        Ok(value) => {
            if had_existing {
                ops.remove(backup_path).map_err(|error| {
                    format!(
                        "generated output retry committed, but backup cleanup failed at {}: {error}",
                        backup_path.display()
                    )
                })?;
            }
            Ok(value)
        }
        Err(commit_error) if had_existing => {
            if let Err(restore_error) = ops.replace(backup_path, output_path) {
                return Err(format!(
                    "generated output retry metadata commit failed: {commit_error}; prior output restoration failed, recoverable backup remains at {}: {restore_error}",
                    backup_path.display()
                ));
            }
            Err(format!(
                "generated output retry metadata commit failed; prior output restored: {commit_error}"
            ))
        }
        Err(commit_error) => {
            if let Err(cleanup_error) = ops.remove(output_path) {
                return Err(format!(
                    "generated output retry metadata commit failed: {commit_error}; new output cleanup failed at {}: {cleanup_error}",
                    output_path.display()
                ));
            }
            Err(format!(
                "generated output retry metadata commit failed: {commit_error}"
            ))
        }
    }
}

fn publish_retry_output_and_load_response<T, U>(
    ops: &impl RetryOutputFileOps,
    staging_path: &Path,
    output_path: &Path,
    backup_path: &Path,
    commit_metadata: impl FnOnce() -> Result<T, String>,
    load_response: impl FnOnce() -> Result<U, String>,
) -> Result<(T, U), String> {
    let committed = publish_retry_output_with_rollback(
        ops,
        staging_path,
        output_path,
        backup_path,
        commit_metadata,
    )?;
    let response = load_response().map_err(|error| {
        format!("generated output retry committed, but response reload failed: {error}")
    })?;
    Ok((committed, response))
}

fn cleanup_retry_staging(path: &Path, primary_error: String) -> String {
    match SystemRetryOutputFileOps.remove(path) {
        Ok(()) => primary_error,
        Err(cleanup_error) => format!(
            "{primary_error}; retry staging cleanup failed at {}: {cleanup_error}",
            path.display()
        ),
    }
}

fn retry_generated_asset_output_download_with_policy_and_hook(
    project_dir: &Path,
    asset_id: &str,
    output_media_id: &str,
    policy_override: Option<&DownloadNetworkPolicy>,
    before_snapshot: impl FnOnce() -> Result<(), String>,
    after_download: impl FnOnce() -> Result<(), String>,
) -> Result<ProjectActionWriteResult, String> {
    retry_generated_asset_output_download_with_policy_resolver_and_hooks(
        project_dir,
        asset_id,
        output_media_id,
        |provider| match policy_override {
            Some(policy) => Ok(policy.clone()),
            None => generated_output_retry_network_policy_for_provider(provider, asset_id),
        },
        before_snapshot,
        after_download,
    )
}

#[cfg_attr(not(test), allow(dead_code))]
fn retry_generated_asset_output_download_with_policy_resolver_and_hooks(
    project_dir: &Path,
    asset_id: &str,
    output_media_id: &str,
    policy_for_provider: impl FnOnce(&str) -> Result<DownloadNetworkPolicy, String>,
    before_snapshot: impl FnOnce() -> Result<(), String>,
    after_download: impl FnOnce() -> Result<(), String>,
) -> Result<ProjectActionWriteResult, String> {
    before_snapshot()?;
    let (
        expected_revision,
        expected_provider,
        expected_model,
        expected_output,
        source_url,
        output_path,
        policy,
    ) = {
        let _lease = acquire_split_project_mutation_lease(project_dir)?;
        let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
        let asset = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| format!("generated asset was not found: {asset_id}"))?;
        if asset.status != GeneratedAssetStatus::Completed {
            return Err(format!("generated asset {asset_id} is not completed; rerun generation before retrying output download"));
        }
        let provider = asset.model.provider.trim().to_string();
        let model = asset.model.id.trim().to_string();
        let policy = policy_for_provider(&provider)?;
        let output = asset
            .outputs
            .iter()
            .find(|output| output.media_id == output_media_id)
            .cloned()
            .ok_or_else(|| {
                format!("generated asset {asset_id} has no output media: {output_media_id}")
            })?;
        let source_url = output.source_url.as_ref().map(|url| url.trim()).filter(|url| !url.is_empty())
            .map(str::to_string).ok_or_else(|| format!("generated asset {asset_id} output {output_media_id} has no provider source URL"))?;
        let output_path = resolve_project_relative_path(project_dir, &output.relative_path)
            .map_err(|error| error.to_string())?;
        validate_split_project_write_path(project_dir, &output_path)
            .map_err(|error| error.to_string())?;
        (
            project.content_revision,
            provider,
            model,
            output,
            source_url,
            output_path,
            policy,
        )
    };
    let file_name = output_path
        .file_name()
        .ok_or_else(|| "generated output retry path must include a file name".to_string())?;
    let staging_path = output_path.with_file_name(format!(
        ".{}.retry-{}",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let limit = output_limit_for_path(&expected_output.relative_path);
    let download_result =
        download_generated_output_source_url(&source_url, &staging_path, limit, &policy);
    if let Err(error) = download_result {
        return Err(cleanup_retry_staging(&staging_path, error));
    }
    if let Err(error) = after_download() {
        return Err(cleanup_retry_staging(&staging_path, error));
    }

    let publish_result = (|| {
        let _lease = acquire_split_project_mutation_lease(project_dir)?;
        let mut project = load_split_project(project_dir).map_err(|error| error.to_string())?;
        if project.content_revision != expected_revision {
            return Err(format!("project revision conflict: expected {expected_revision}, but canonical revision is {}", project.content_revision));
        }
        let asset = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| format!("generated asset was not found: {asset_id}"))?;
        if asset.status != GeneratedAssetStatus::Completed {
            return Err(format!(
                "generated asset {asset_id} changed during output retry"
            ));
        }
        if asset.model.provider.trim() != expected_provider
            || asset.model.id.trim() != expected_model
        {
            return Err(format!(
                "generated asset {asset_id} provider or model changed during output retry"
            ));
        }
        let output = asset
            .outputs
            .iter()
            .find(|output| output.media_id == output_media_id)
            .ok_or_else(|| {
                format!("generated asset {asset_id} has no output media: {output_media_id}")
            })?;
        if output != &expected_output {
            return Err(format!(
                "generated asset {asset_id} output changed during output retry"
            ));
        }
        let revalidated_path = resolve_project_relative_path(project_dir, &output.relative_path)
            .map_err(|error| error.to_string())?;
        validate_split_project_write_path(project_dir, &revalidated_path)
            .map_err(|error| error.to_string())?;
        if revalidated_path != output_path {
            return Err("generated output retry destination changed".to_string());
        }
        let target_folder_id = asset.target_folder_id.clone();
        let output = output.clone();
        let backup_path = output_path.with_file_name(format!(
            ".{}.backup-{}",
            file_name.to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        if let Some(media) = project
            .media
            .iter_mut()
            .find(|media| media.id == output.media_id)
        {
            media.name = None;
            media.relative_path = output.relative_path.clone();
            media.kind = MediaKind::Generated;
            media.duration_seconds = output.duration_seconds;
            media.width = Some(output.width);
            media.height = Some(output.height);
            media.fps = (output.fps > 0.0).then_some(output.fps);
            media.folder_id = target_folder_id;
        } else {
            project.media.push(MediaAsset {
                id: output.media_id.clone(),
                name: None,
                relative_path: output.relative_path.clone(),
                kind: MediaKind::Generated,
                duration_seconds: output.duration_seconds,
                width: Some(output.width),
                height: Some(output.height),
                fps: (output.fps > 0.0).then_some(output.fps),
                folder_id: target_folder_id,
            });
        }
        publish_retry_output_and_load_response(
            &SystemRetryOutputFileOps,
            &staging_path,
            &output_path,
            &backup_path,
            || {
                save_split_project(project_dir, &project)
                    .map(|result| result.report)
                    .map_err(|error| error.to_string())
            },
            || load_split_project(project_dir).map_err(|error| error.to_string()),
        )
        .map(|(report, project)| ProjectActionWriteResult { project, report })
    })();
    publish_result.map_err(|error| cleanup_retry_staging(&staging_path, error))
}

fn download_generated_output_source_url(
    source_url: &str,
    output_path: &Path,
    limit: u64,
    policy: &DownloadNetworkPolicy,
) -> Result<(), String> {
    download_url_to_atomic_file(source_url, output_path, limit, || false, policy, None)
        .map(|_| ())
        .map_err(|error| format!("generated output retry download failed: {error}"))
}

#[tauri::command]
fn build_temporal_job_summary(
    kind: TemporalWorkflowKind,
    project_id: String,
    job_id: String,
    status: JobStatus,
    updated_at: String,
) -> JobSummary {
    temporal_job_summary(kind, &project_id, &job_id, status, &updated_at)
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri exposes the established generation brief as named IPC arguments"
)]
fn build_temporal_generate_media_start_request(
    project_id: String,
    project_dir: String,
    asset_id: String,
    job_id: String,
    mock_mode: bool,
    name: Option<String>,
    target_folder_id: Option<String>,
    placement_intent: Option<String>,
    prompt: Option<String>,
    model: Option<serde_json::Value>,
    references: Option<serde_json::Value>,
    settings: Option<serde_json::Value>,
) -> TemporalWorkflowStartRequest {
    let brief = if name.is_some()
        || target_folder_id.is_some()
        || placement_intent.is_some()
        || prompt.is_some()
        || model.is_some()
        || references.is_some()
        || settings.is_some()
    {
        Some(TemporalGenerateMediaBrief {
            name,
            target_folder_id,
            placement_intent,
            prompt,
            model,
            references,
            settings,
        })
    } else {
        None
    };

    temporal_generate_media_start_request(
        &project_id,
        &project_dir,
        &asset_id,
        &job_id,
        mock_mode,
        brief,
    )
}

#[tauri::command]
fn build_temporal_codex_edit_start_request(
    project_id: String,
    project_root: Option<String>,
    project_dir: String,
    job_id: String,
    request: EditJobRequest,
) -> Result<TemporalWorkflowStartRequest, String> {
    let project_root = resolve_codex_project_root(project_root.as_deref())?;
    let project_root = project_root
        .to_str()
        .ok_or_else(|| "project root must be valid UTF-8".to_string())?;

    Ok(temporal_codex_edit_start_request(
        &project_id,
        project_root,
        &project_dir,
        &job_id,
        request,
    ))
}

#[tauri::command]
fn build_temporal_transcribe_media_start_request(
    project_id: String,
    project_dir: String,
    media_id: String,
    job_id: String,
    language_mode: String,
) -> TemporalWorkflowStartRequest {
    temporal_transcribe_media_start_request(
        &project_id,
        &project_dir,
        &media_id,
        &job_id,
        &language_mode,
    )
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri exposes export settings as named IPC arguments for typed callers"
)]
fn build_temporal_export_media_start_request(
    project_id: String,
    project_dir: String,
    job_id: String,
    profile: ExportProfile,
    quality: RenderQuality,
    width: u32,
    height: u32,
    output_path: String,
    fps: Option<f64>,
    encode_tier: Option<ExportEncodeTier>,
    output: Option<ExportOutputRequest>,
) -> Result<TemporalWorkflowStartRequest, String> {
    let options = export_render_options(profile, quality, width, height, fps, encode_tier)?;
    Ok(temporal_export_media_start_request_with_output(
        &project_id,
        &project_dir,
        &job_id,
        options,
        &output_path,
        output.as_ref(),
    ))
}

fn export_render_options(
    profile: ExportProfile,
    quality: RenderQuality,
    width: u32,
    height: u32,
    fps: Option<f64>,
    encode_tier: Option<ExportEncodeTier>,
) -> Result<ExportRenderOptions, String> {
    ExportRenderOptions::new(profile, quality, width, height)
        .and_then(|options| options.with_fps(fps))
        .and_then(|options| options.with_encode_tier(encode_tier.unwrap_or_default()))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn build_temporal_export_project_bundle_start_request(
    project_id: String,
    project_dir: String,
    job_id: String,
    output_path: String,
) -> TemporalWorkflowStartRequest {
    temporal_export_project_bundle_start_request(
        &project_id,
        &project_dir,
        &job_id,
        &output_path,
        true,
    )
}

#[tauri::command]
fn build_temporal_export_nle_xml_start_request(
    project_id: String,
    project_dir: String,
    job_id: String,
    format: NleXmlFormat,
    output_path: String,
) -> TemporalWorkflowStartRequest {
    temporal_export_nle_xml_start_request(&project_id, &project_dir, &job_id, format, &output_path)
}

#[tauri::command]
fn build_temporal_start_result_action(
    job: JobSummary,
    run_id: String,
    updated_at: String,
) -> Result<ProjectAction, String> {
    temporal_start_result_action(&job, &run_id, &updated_at).map_err(|error| error.to_string())
}

#[tauri::command]
#[cfg(not(feature = "temporal-worker"))]
async fn start_temporal_workflow(job: JobSummary) -> Result<TemporalWorkflowStartResult, String> {
    temporal_workflow_unavailable_start_result(&job).map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_generate_media_in_process(
    start_request: TemporalWorkflowStartRequest,
    updated_at: String,
) -> Result<VideoProject, String> {
    let workflow_input = temporal_generate_media_workflow_input(&start_request)
        .map_err(|error| error.to_string())?;
    let project_dir = resolve_project_dir(&workflow_input.project_dir)?;
    let run_id = format!("in-process/{}", start_request.workflow_id);
    let project = run_blocking_command("generation project load", {
        let project_dir = project_dir.clone();
        move || load_split_project(&project_dir).map_err(|error| error.to_string())
    })
    .await?;
    if project.id != workflow_input.project_id {
        return Err("generation start request projectId does not match project folder".to_string());
    }
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == workflow_input.job_id)
        .ok_or_else(|| format!("generation job was not found: {}", workflow_input.job_id))?;
    let generation_provider = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == workflow_input.asset_id)
        .map(|asset| asset.model.provider.trim().to_string())
        .filter(|provider| !provider.is_empty())
        .ok_or_else(|| {
            format!(
                "generation asset was not found or has no provider: {}",
                workflow_input.asset_id
            )
        })?;
    let mock_mode = workflow_input.mock_mode;
    let cancellation_guard =
        register_generation_cancellation(&workflow_input.project_id, &workflow_input.job_id)
            .map_err(|error| error.to_string())?;
    let cancellation = cancellation_guard.token();
    // An agent Undo in the MCP sidecar can remove this job; only this process can stop the run.
    let job_watch = watch_generation_job(
        &workflow_input.project_id,
        &workflow_input.job_id,
        Duration::from_secs(5),
        {
            let watched_dir = project_dir.clone();
            let watched_job = workflow_input.job_id.clone();
            move || {
                load_split_project(&watched_dir)
                    .map(|project| project.jobs.iter().any(|job| job.id == watched_job))
                    .map_err(|error| error.to_string())
            }
        },
    );
    let refreshed_project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let was_cancelled_before_registration = refreshed_project
        .jobs
        .iter()
        .find(|candidate| candidate.id == workflow_input.job_id)
        .is_some_and(|candidate| candidate.status == JobStatus::Cancelled)
        || refreshed_project
            .generated_assets
            .iter()
            .find(|candidate| candidate.id == workflow_input.asset_id)
            .is_some_and(|candidate| candidate.status == GeneratedAssetStatus::Cancelled);
    if was_cancelled_before_registration {
        let _ = request_generation_cancellation(&workflow_input.project_id, &workflow_input.job_id);
        cancellation.mark_terminal();
        return Ok(refreshed_project);
    }
    let start_action = temporal_start_result_action(job, &run_id, &updated_at)
        .map_err(|error| error.to_string())?;
    if let Err(error) = apply_project_actions_to_split_project(
        &project_dir,
        vec![
            start_action,
            ProjectAction::UpdateGeneratedAssetStatus {
                asset_id: workflow_input.asset_id.clone(),
                status: GeneratedAssetStatus::Running,
            },
        ],
    ) {
        if cancellation.is_cancelled() {
            cancellation.mark_terminal();
            return load_split_project(&project_dir).map_err(|load_error| load_error.to_string());
        }
        return Err(error.to_string());
    }

    if mock_mode {
        let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
        let actions = temporal_generate_media_mock_completion_actions(
            &project,
            &start_request,
            &updated_at,
            None,
        )
        .map_err(|error| error.to_string())?;
        return apply_project_actions_to_split_project(&project_dir, actions)
            .map(|write| write.project)
            .map_err(|error| error.to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let _cancellation_guard = cancellation_guard;
        let _job_watch = job_watch;
        if cancellation.is_cancelled() {
            cancellation.mark_terminal();
            return load_split_project(&project_dir).map_err(|error| error.to_string());
        }
        let credential_result = resolve_provider_credential(&generation_provider)
            .map(|credential| credential.into_secret());
        let credential = match credential_result {
            Ok(credential) => credential,
            Err(_error) if cancellation.is_cancelled() => {
                cancellation.mark_terminal();
                return load_split_project(&project_dir)
                    .map_err(|load_error| load_error.to_string());
            }
            Err(error) => {
                let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                    &start_request,
                    Some(&run_id),
                    &updated_at,
                );
                return Err(error.to_string());
            }
        };
        let client_result = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build();
        let client = match client_result {
            Ok(client) => client,
            Err(_error) if cancellation.is_cancelled() => {
                cancellation.mark_terminal();
                return load_split_project(&project_dir)
                    .map_err(|load_error| load_error.to_string());
            }
            Err(error) => {
                let _ = temporal_generate_media_attach_generated_asset_failure_to_project_dir(
                    &start_request,
                    Some(&run_id),
                    &updated_at,
                );
                return Err(error.to_string());
            }
        };
        match run_generate_media_in_process_with_client_and_credential_cancellable(
            &client,
            &start_request,
            &updated_at,
            Some(&run_id),
            TemporalGenerateMediaProviderRunOptions::default(),
            &credential,
            Some(&cancellation),
        ) {
            Ok(_) => {}
            Err(TemporalWorkflowInputError::GenerationCancelled) => {
                cancellation.mark_terminal();
                return load_split_project(&project_dir).map_err(|error| error.to_string());
            }
            Err(error) => return Err(error.to_string()),
        }
        load_split_project(&project_dir).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("in-process generation task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "temporal-worker")]
async fn start_temporal_workflow(job: JobSummary) -> Result<TemporalWorkflowStartResult, String> {
    let (connection_options, client_options) = temporalio_client::ClientOptions::load_from_config(
        temporalio_client::envconfig::LoadClientConfigProfileOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    let connection = temporalio_client::Connection::connect(connection_options)
        .await
        .map_err(|error| error.to_string())?;
    let client = temporalio_client::Client::new(connection, client_options)
        .map_err(|error| error.to_string())?;

    temporal_start_workflow_with_client(&client, &job)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_temporal_worker_environment_report() -> TemporalWorkerEnvironmentReport {
    temporal_worker_environment_report()
}

#[tauri::command]
fn get_export_profile_availability_report() -> Vec<ExportProfileAvailability> {
    mp4_export_profile_availability_report()
}

#[tauri::command]
fn list_generation_model_catalog() -> Result<serde_json::Value, String> {
    list_models_payload(None).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    platform: &'static str,
}

#[tauri::command]
fn get_platform_info() -> PlatformInfo {
    PlatformInfo {
        platform: video_creater_lib::desktop_integration::host_platform(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteAccessStatus {
    service_state: &'static str,
    tailscale: video_creater_lib::remote_access::TailscaleReport,
    management_available: bool,
    detail: String,
}

fn remote_access_status_blocking() -> RemoteAccessStatus {
    #[cfg(target_os = "linux")]
    let (service_state, management_available, detail) = {
        let load_state = Command::new("systemctl")
            .args([
                "--user",
                "show",
                "--property=LoadState",
                "--value",
                "video-creater-host.service",
            ])
            .output();
        match load_state {
            Ok(output)
                if output.status.success()
                    && String::from_utf8_lossy(&output.stdout).trim() == "loaded" =>
            {
                match Command::new("systemctl")
                    .args(["--user", "is-active", "video-creater-host.service"])
                    .output()
                {
                    Ok(output) if output.status.success() => (
                        "running",
                        true,
                        "The user service is running; URL readiness is verified separately."
                            .to_string(),
                    ),
                    Ok(output) => {
                        let state = String::from_utf8_lossy(&output.stdout);
                        (
                            if state.trim() == "failed" {
                                "failed"
                            } else {
                                "stopped"
                            },
                            true,
                            "The user service is installed but is not running.".to_string(),
                        )
                    }
                    Err(_) => (
                        "unavailable",
                        false,
                        "The user service manager is unavailable.".to_string(),
                    ),
                }
            }
            _ => (
                "unavailable",
                false,
                "Install the packaged systemd user service to manage remote access here."
                    .to_string(),
            ),
        }
    };
    #[cfg(not(target_os = "linux"))]
    let (service_state, management_available, detail) = (
        "unavailable",
        false,
        "Desktop service management is currently packaged for Linux only.".to_string(),
    );
    RemoteAccessStatus {
        service_state,
        tailscale: video_creater_lib::remote_access::detect_and_verify(
            video_creater_lib::DEFAULT_REMOTE_HOST_PORT,
        ),
        management_available,
        detail,
    }
}

#[tauri::command]
async fn get_remote_access_status() -> Result<RemoteAccessStatus, String> {
    tauri::async_runtime::spawn_blocking(remote_access_status_blocking)
        .await
        .map_err(|_| "Remote access status check failed.".to_string())
}

#[tauri::command]
async fn set_remote_access_running(running: bool) -> Result<RemoteAccessStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let action = if running { "start" } else { "stop" };
            let output = Command::new("systemctl")
                .args(["--user", action, "video-creater-host.service"])
                .output()
                .map_err(|_| "The user service manager is unavailable.".to_string())?;
            if !output.status.success() {
                return Err(
                    "The Video Creater user service could not be changed. Check its installation and journal."
                        .to_string(),
                );
            }
            Ok(remote_access_status_blocking())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = running;
            Err("Desktop service management is currently packaged for Linux only.".to_string())
        }
    })
    .await
    .map_err(|_| "Remote access service control failed.".to_string())?
}

#[tauri::command]
fn get_update_health() -> UpdateHealth {
    UpdateHealth {
        state: SettingsHealthState::Unavailable,
        installed_version: env!("CARGO_PKG_VERSION").to_string(),
        summary: "Updates are not configured for this build".to_string(),
    }
}

fn notification_capability_for_authorization_status(
    status: NativeNotificationAuthorizationStatus,
) -> NotificationCapability {
    notification_capability_for_native_settings(status, true)
}

fn notification_capability_for_native_settings(
    status: NativeNotificationAuthorizationStatus,
    delivery_available: bool,
) -> NotificationCapability {
    if matches!(
        status,
        NativeNotificationAuthorizationStatus::Authorized
            | NativeNotificationAuthorizationStatus::Provisional
            | NativeNotificationAuthorizationStatus::Ephemeral
    ) && !delivery_available
    {
        return NotificationCapability {
            state: SettingsHealthState::ActionRequired,
            delivery_available: false,
            permission_status: match status {
                NativeNotificationAuthorizationStatus::Authorized => "authorized",
                NativeNotificationAuthorizationStatus::Provisional => "provisional",
                NativeNotificationAuthorizationStatus::Ephemeral => "ephemeral",
                NativeNotificationAuthorizationStatus::NotDetermined
                | NativeNotificationAuthorizationStatus::Denied => unreachable!(),
            }
            .to_string(),
            can_request: false,
            summary: if cfg!(target_os = "macos") {
                "Native notification delivery is disabled in System Settings."
            } else {
                "Native notification delivery is disabled in desktop settings."
            }
            .to_string(),
            diagnostic_code: Some("notifications.deliveryDisabled".to_string()),
            diagnostic_detail: Some(
                if cfg!(target_os = "macos") {
                    "Enable alerts for Video Creater in macOS System Settings."
                } else {
                    "Enable notifications for Video Creater in your desktop settings."
                }
                .to_string(),
            ),
        };
    }
    match status {
        NativeNotificationAuthorizationStatus::NotDetermined => NotificationCapability {
            state: SettingsHealthState::ActionRequired,
            delivery_available: true,
            permission_status: "notDetermined".to_string(),
            can_request: true,
            summary: "Notification permission has not been requested.".to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
        },
        NativeNotificationAuthorizationStatus::Denied => NotificationCapability {
            state: SettingsHealthState::ActionRequired,
            delivery_available: true,
            permission_status: "denied".to_string(),
            can_request: false,
            summary: if cfg!(target_os = "macos") {
                "Native notifications are disabled in System Settings."
            } else {
                "Native notifications are disabled in desktop settings."
            }
            .to_string(),
            diagnostic_code: Some("notifications.permissionDenied".to_string()),
            diagnostic_detail: Some(
                if cfg!(target_os = "macos") {
                    "Enable Video Creater notifications in macOS System Settings."
                } else {
                    "Enable notifications for Video Creater in your desktop settings."
                }
                .to_string(),
            ),
        },
        NativeNotificationAuthorizationStatus::Authorized => NotificationCapability {
            state: SettingsHealthState::Ready,
            delivery_available: true,
            permission_status: "authorized".to_string(),
            can_request: false,
            summary: "Native notifications are authorized.".to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
        },
        NativeNotificationAuthorizationStatus::Provisional => NotificationCapability {
            state: SettingsHealthState::Ready,
            delivery_available: true,
            permission_status: "provisional".to_string(),
            can_request: false,
            summary: "Native notifications have provisional authorization.".to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
        },
        NativeNotificationAuthorizationStatus::Ephemeral => NotificationCapability {
            state: SettingsHealthState::Ready,
            delivery_available: true,
            permission_status: "ephemeral".to_string(),
            can_request: false,
            summary: "Native notifications have temporary authorization.".to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
        },
    }
}

fn unavailable_notification_capability() -> NotificationCapability {
    NotificationCapability {
        state: SettingsHealthState::Unavailable,
        delivery_available: false,
        permission_status: "unavailable".to_string(),
        can_request: false,
        summary: "Native notifications are unavailable on this platform.".to_string(),
        diagnostic_code: Some("notifications.nativeUnavailable".to_string()),
        diagnostic_detail: Some(
            if cfg!(target_os = "linux") {
                "No desktop notification service (org.freedesktop.Notifications) is available."
            } else {
                "The native UserNotifications delivery runtime is not available."
            }
            .to_string(),
        ),
    }
}

fn notification_settings_query_failed(error: String) -> NotificationCapability {
    NotificationCapability {
        state: SettingsHealthState::Failed,
        delivery_available: false,
        permission_status: "authorized".to_string(),
        can_request: false,
        summary:
            "Notification permission was granted, but delivery availability could not be verified."
                .to_string(),
        diagnostic_code: Some("notifications.settingsQueryFailed".to_string()),
        diagnostic_detail: Some(error),
    }
}

fn notification_capability_after_permission_request<F>(
    permission_result: Result<bool, String>,
    query_native_settings: F,
) -> NotificationCapability
where
    F: FnOnce() -> Result<(NativeNotificationAuthorizationStatus, bool), String>,
{
    match permission_result {
        Ok(true) => query_native_settings()
            .map(|(status, delivery_available)| {
                notification_capability_for_native_settings(status, delivery_available)
            })
            .unwrap_or_else(notification_settings_query_failed),
        Ok(false) => notification_capability_for_authorization_status(
            NativeNotificationAuthorizationStatus::Denied,
        ),
        Err(error) => {
            let mut capability = unavailable_notification_capability();
            capability.diagnostic_detail = Some(error);
            capability
        }
    }
}

fn executable_has_native_notification_bundle_context(executable: &Path) -> bool {
    let Some(macos_dir) = executable.parent() else {
        return false;
    };
    let Some(contents_dir) = macos_dir.parent() else {
        return false;
    };
    let Some(app_bundle) = contents_dir.parent() else {
        return false;
    };

    macos_dir.file_name().is_some_and(|name| name == "MacOS")
        && contents_dir
            .file_name()
            .is_some_and(|name| name == "Contents")
        && app_bundle
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
}

fn with_native_notification_bundle_context<T, F>(
    executable: &Path,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    // Only macOS UserNotifications needs a packaged application bundle; the
    // freedesktop notification service works for any executable.
    if cfg!(target_os = "macos") && !executable_has_native_notification_bundle_context(executable) {
        return Err(
            "native UserNotifications requires a packaged macOS application bundle".to_string(),
        );
    }
    operation()
}

fn notification_capability_for_executable<F>(
    executable: &Path,
    query_native_settings: F,
) -> NotificationCapability
where
    F: FnOnce() -> Result<(NativeNotificationAuthorizationStatus, bool), String>,
{
    with_native_notification_bundle_context(executable, query_native_settings)
        .map(|(status, delivery_available)| {
            notification_capability_for_native_settings(status, delivery_available)
        })
        .unwrap_or_else(|error| {
            let mut capability = unavailable_notification_capability();
            capability.diagnostic_detail = Some(error);
            capability
        })
}

#[cfg(target_os = "macos")]
fn query_native_notification_authorization_status(
) -> Result<(NativeNotificationAuthorizationStatus, bool), String> {
    use block2::RcBlock;
    use objc2_user_notifications::{
        UNAuthorizationStatus, UNNotificationSetting, UNNotificationSettings,
        UNUserNotificationCenter,
    };
    use std::ptr::NonNull;
    use std::sync::mpsc;

    let (sender, receiver) = mpsc::sync_channel(1);
    let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| unsafe {
        let settings = settings.as_ref();
        let _ = sender.send((
            settings.authorizationStatus(),
            settings.alertSetting() == UNNotificationSetting::Enabled,
        ));
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .getNotificationSettingsWithCompletionHandler(&completion);
    let (status, delivery_available) = receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("native notification settings timed out: {error}"))?;
    let authorization_status = match status {
        UNAuthorizationStatus::NotDetermined => {
            NativeNotificationAuthorizationStatus::NotDetermined
        }
        UNAuthorizationStatus::Denied => NativeNotificationAuthorizationStatus::Denied,
        UNAuthorizationStatus::Authorized => NativeNotificationAuthorizationStatus::Authorized,
        UNAuthorizationStatus::Provisional => NativeNotificationAuthorizationStatus::Provisional,
        UNAuthorizationStatus::Ephemeral => NativeNotificationAuthorizationStatus::Ephemeral,
        _ => NativeNotificationAuthorizationStatus::Denied,
    };
    Ok((authorization_status, delivery_available))
}

#[cfg(target_os = "linux")]
fn query_native_notification_authorization_status(
) -> Result<(NativeNotificationAuthorizationStatus, bool), String> {
    linux_notification_authorization(
        video_creater_lib::desktop_integration::notification_service_available(),
    )
}

/// The freedesktop notification service has no per-application permission:
/// a running (or activatable) `org.freedesktop.Notifications` owner means
/// notifications are authorized and deliverable.
#[cfg(target_os = "linux")]
fn linux_notification_authorization(
    service_available: Result<bool, String>,
) -> Result<(NativeNotificationAuthorizationStatus, bool), String> {
    match service_available {
        Ok(true) => Ok((NativeNotificationAuthorizationStatus::Authorized, true)),
        Ok(false) => Err(
            "No desktop notification service (org.freedesktop.Notifications) is running on the session bus."
                .to_string(),
        ),
        Err(error) => Err(error),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn query_native_notification_authorization_status(
) -> Result<(NativeNotificationAuthorizationStatus, bool), String> {
    Err("native notifications are not supported on this platform".to_string())
}

fn native_notification_authorization_status(
) -> Result<(NativeNotificationAuthorizationStatus, bool), String> {
    let executable = std::env::current_exe().map_err(|error| {
        format!("native notification executable could not be resolved: {error}")
    })?;
    with_native_notification_bundle_context(
        &executable,
        query_native_notification_authorization_status,
    )
}

#[cfg(target_os = "macos")]
fn request_native_notification_permission_from_center() -> Result<bool, String> {
    use block2::RcBlock;
    use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
    use std::sync::mpsc;

    let (sender, receiver) = mpsc::sync_channel(1);
    let completion: RcBlock<dyn Fn(objc2::runtime::Bool, *mut objc2_foundation::NSError)> =
        RcBlock::new(
            move |granted: objc2::runtime::Bool, error: *mut objc2_foundation::NSError| {
                let result = if error.is_null() {
                    Ok(granted.as_bool())
                } else {
                    Err("macOS rejected the notification permission request".to_string())
                };
                let _ = sender.send(result);
            },
        );
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &completion,
        );
    receiver
        .recv_timeout(Duration::from_secs(15))
        .map_err(|error| format!("native notification permission timed out: {error}"))?
}

#[cfg(target_os = "linux")]
fn request_native_notification_permission_from_center() -> Result<bool, String> {
    query_native_notification_authorization_status().map(|_| true)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn request_native_notification_permission_from_center() -> Result<bool, String> {
    Err("native notifications are not supported on this platform".to_string())
}

fn request_native_notification_permission() -> Result<bool, String> {
    let executable = std::env::current_exe().map_err(|error| {
        format!("native notification executable could not be resolved: {error}")
    })?;
    with_native_notification_bundle_context(
        &executable,
        request_native_notification_permission_from_center,
    )
}

#[tauri::command]
async fn get_notification_capability() -> NotificationCapability {
    tauri::async_runtime::spawn_blocking(|| match std::env::current_exe() {
        Ok(executable) => notification_capability_for_executable(
            &executable,
            query_native_notification_authorization_status,
        ),
        Err(error) => {
            let mut capability = unavailable_notification_capability();
            capability.diagnostic_detail = Some(format!(
                "native notification executable could not be resolved: {error}"
            ));
            capability
        }
    })
    .await
    .unwrap_or_else(|error| {
        let mut capability = unavailable_notification_capability();
        capability.diagnostic_detail =
            Some(format!("notification capability task failed: {error}"));
        capability
    })
}

#[tauri::command]
async fn request_notification_permission() -> NotificationCapability {
    tauri::async_runtime::spawn_blocking(|| {
        notification_capability_after_permission_request(
            request_native_notification_permission(),
            native_notification_authorization_status,
        )
    })
    .await
    .unwrap_or_else(|error| {
        let mut capability = unavailable_notification_capability();
        capability.diagnostic_detail =
            Some(format!("notification permission task failed: {error}"));
        capability
    })
}

fn attach_media_analysis_report_to_split_project_folder(
    project_dir: &Path,
    media_id: &str,
    report: &VideoInspectionReport,
    preview_metrics: Option<&ImageMetrics>,
    sampled_frame_metrics: &[ImageMetrics],
    sampled_audio_metrics: &[AudioWindowMetrics],
) -> Result<MediaAnalysisCommandResult, String> {
    let _project_lease = acquire_split_project_mutation_lease(project_dir)?;
    let mut project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    if !project.media.iter().any(|media| media.id == media_id) {
        return Err(format!("media was not found: {media_id}"));
    }

    let moments = derive_media_analysis_moments_with_sampled_signals(
        media_id,
        report,
        preview_metrics,
        sampled_frame_metrics,
        sampled_audio_metrics,
    );
    let silence_ranges = derive_media_silence_ranges(media_id, sampled_audio_metrics);
    project
        .media_analysis
        .retain(|moment| moment.media_id != media_id);
    project.media_analysis.extend(moments.clone());
    project
        .media_silence_ranges
        .retain(|range| range.media_id != media_id);
    project.media_silence_ranges.extend(silence_ranges.clone());
    let save_result =
        save_split_project(project_dir, &project).map_err(|error| error.to_string())?;
    project = save_result.project;

    Ok(MediaAnalysisCommandResult {
        project,
        write_report: save_result.report,
        moments,
        silence_ranges,
    })
}

#[tauri::command]
async fn analyze_media_for_edit_in_split_project_folder(
    project_dir: String,
    media_id: String,
) -> Result<MediaAnalysisCommandResult, String> {
    run_blocking_command("media analysis", move || {
        analyze_media_for_edit_in_split_project_folder_blocking(project_dir, media_id)
    })
    .await
}

fn analyze_media_for_edit_in_split_project_folder_blocking(
    project_dir: String,
    media_id: String,
) -> Result<MediaAnalysisCommandResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .ok_or_else(|| format!("media was not found: {media_id}"))?;
    let media_path = resolve_project_relative_path(&project_dir, &media.relative_path)
        .map_err(|error| error.to_string())?;
    let report = inspect_video(&media_path).map_err(|error| error.to_string())?;
    let sampled_frame_metrics =
        sample_video_frame_metrics(&media_path, report.media.duration_seconds);
    let sampled_audio_metrics = if report.media.audio.is_some() {
        sample_video_audio_window_metrics(&media_path, report.media.duration_seconds)
    } else {
        Vec::new()
    };

    attach_media_analysis_report_to_split_project_folder(
        &project_dir,
        &media_id,
        &report,
        None,
        &sampled_frame_metrics,
        &sampled_audio_metrics,
    )
}

#[tauri::command]
fn build_temporal_generate_media_failure_actions(
    job: JobSummary,
    asset_id: String,
    run_id: Option<String>,
    updated_at: String,
) -> Result<Vec<ProjectAction>, String> {
    temporal_generate_media_failure_actions(&job, &asset_id, run_id.as_deref(), &updated_at)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn cancel_generate_media_in_process(
    project_dir: String,
    job_id: String,
    updated_at: String,
) -> Result<CancelInProcessGenerationResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("generation cancellation", &queue_dir, move |_| {
        cancel_generate_media_in_process_blocking(project_dir, job_id, updated_at)
    })
    .await
}

fn cancel_generate_media_in_process_blocking(
    project_dir: String,
    job_id: String,
    updated_at: String,
) -> Result<CancelInProcessGenerationResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| format!("job was not found: {job_id}"))?;
    if job.kind != TemporalWorkflowKind::GenerateMedia.job_kind() {
        return Err(format!("job is not a generate_media job: {job_id}"));
    }
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| format!("job is missing start request: {job_id}"))?;
    let asset_id = start_request
        .input
        .get("assetId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "job start request is missing assetId".to_string())?;
    let asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| format!("generation asset was not found: {asset_id}"))?;

    if job.status == JobStatus::Cancelled || asset.status == GeneratedAssetStatus::Cancelled {
        let _ = request_generation_cancellation(&project.id, &job_id);
        return Ok(CancelInProcessGenerationResult {
            outcome: "alreadyCancelled".to_string(),
            project,
        });
    }
    if matches!(job.status, JobStatus::Completed | JobStatus::Failed)
        || matches!(
            asset.status,
            GeneratedAssetStatus::Completed | GeneratedAssetStatus::Failed
        )
    {
        return Ok(CancelInProcessGenerationResult {
            outcome: "alreadyTerminal".to_string(),
            project,
        });
    }

    let request_outcome = request_generation_cancellation(&project.id, &job_id);
    if request_outcome == GenerationCancellationRequestOutcome::TooLate {
        return Ok(CancelInProcessGenerationResult {
            outcome: "alreadyTerminal".to_string(),
            project,
        });
    }
    let actions = temporal_generate_media_cancellation_actions(
        job,
        asset_id,
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        &updated_at,
    )
    .map_err(|error| error.to_string())?;
    let write = apply_project_actions_to_split_project(&project_dir, actions)
        .map_err(|error| error.to_string())?;
    Ok(CancelInProcessGenerationResult {
        outcome: if request_outcome == GenerationCancellationRequestOutcome::AlreadyRequested {
            "alreadyCancelled".to_string()
        } else {
            "cancelled".to_string()
        },
        project: write.project,
    })
}

#[tauri::command]
async fn cancel_generate_media_provider_request_in_split_project_folder(
    project_dir: String,
    job_id: String,
) -> Result<TemporalGenerateMediaCancelProviderActivityOutput, String> {
    run_blocking_command("provider cancellation", move || {
        cancel_generate_media_provider_request_in_split_project_folder_blocking(project_dir, job_id)
    })
    .await
}

fn cancel_generate_media_provider_request_in_split_project_folder_blocking(
    project_dir: String,
    job_id: String,
) -> Result<TemporalGenerateMediaCancelProviderActivityOutput, String> {
    cancel_generate_media_provider_request_in_split_project_folder_with_credential_resolver(
        project_dir,
        job_id,
        |provider| {
            resolve_provider_credential(provider)
                .map(|credential| credential.into_secret())
                .map_err(|error| error.to_string())
        },
    )
}

fn cancel_generate_media_provider_request_in_split_project_folder_with_credential_resolver(
    project_dir: String,
    job_id: String,
    resolve_credential: impl FnOnce(&str) -> Result<String, String>,
) -> Result<TemporalGenerateMediaCancelProviderActivityOutput, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| format!("job was not found: {job_id}"))?;
    let provider_request = job
        .provider_request
        .as_ref()
        .ok_or_else(|| format!("job is missing provider request metadata: {job_id}"))?;
    let start_request = job
        .start_request
        .as_ref()
        .ok_or_else(|| format!("job is missing start request: {job_id}"))?;
    let asset_id = start_request
        .input
        .get("assetId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "job start request is missing assetId".to_string())?;
    let provider = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .map(|asset| asset.model.provider.trim())
        .filter(|provider| !provider.is_empty())
        .ok_or_else(|| format!("generation asset was not found: {asset_id}"))?;
    if provider_request.provider != provider {
        return Err("provider request metadata does not match the generated asset".to_string());
    }
    if !matches!(provider, "fal.ai" | "replicate") {
        return Err(format!(
            "provider-side cancellation is unavailable for {provider}; use local cancellation"
        ));
    }
    let credential = resolve_credential(provider)?;
    let client = reqwest::blocking::Client::new();

    temporal_generate_media_cancel_provider_activity_with_client(
        &client,
        &provider_request.cancel_url,
        &credential,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn export_nle_xml_to_split_project_folder(
    project_dir: String,
    format: NleXmlFormat,
    job_id: String,
    updated_at: String,
    timeline_id: Option<String>,
) -> Result<NleXmlExportCommandResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("NLE XML export", &queue_dir, move |_| {
        export_nle_xml_to_split_project_folder_blocking(
            project_dir,
            format,
            job_id,
            updated_at,
            timeline_id,
        )
    })
    .await
}

fn export_nle_xml_to_split_project_folder_blocking(
    project_dir: String,
    format: NleXmlFormat,
    job_id: String,
    updated_at: String,
    timeline_id: Option<String>,
) -> Result<NleXmlExportCommandResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let project = match timeline_id.as_deref() {
        Some(timeline_id) => project
            .projected_for_timeline(timeline_id)
            .ok_or_else(|| format!("Requested timeline `{timeline_id}` was not found."))?,
        None => project,
    };
    let export =
        export_project_timeline_to_nle_xml(&project, format).map_err(|error| error.to_string())?;
    let export_path =
        write_nle_xml_export(&project_dir, &export).map_err(|error| error.to_string())?;
    let artifact = nle_xml_export_artifact(&export, format, &job_id, &updated_at);
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportNleXml,
        &project.id,
        &job_id,
        JobStatus::Queued,
        &updated_at,
    );
    let mut start_request = temporal_export_nle_xml_start_request(
        &project.id,
        &project_dir.display().to_string(),
        &job_id,
        format,
        &format!("exports/{}", export.filename),
    );
    if let Some(timeline_id) = timeline_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        start_request.input["timelineId"] = serde_json::json!(timeline_id);
    }
    job.start_request = Some(start_request);
    let result = apply_project_actions_to_split_project(
        &project_dir,
        vec![
            ProjectAction::RecordJob {
                job: Box::new(job.clone()),
            },
            ProjectAction::UpdateJobStatus {
                job_id,
                status: JobStatus::Completed,
                updated_at,
                run_id: None,
            },
            ProjectAction::RecordExportArtifact { artifact },
        ],
    )
    .map_err(|error| error.to_string())?;
    let job = result
        .project
        .jobs
        .iter()
        .find(|recorded_job| recorded_job.id == job.id)
        .cloned()
        .unwrap_or(job);

    Ok(NleXmlExportCommandResult {
        project: result.project,
        export_path: export_path.display().to_string(),
        job,
    })
}

#[tauri::command]
async fn export_palmier_project_package_to_split_project_folder(
    project_dir: String,
    job_id: String,
    output_path: String,
    updated_at: String,
) -> Result<NleXmlExportCommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_palmier_project_package(&project_dir, job_id, output_path, updated_at)
    })
    .await
    .map_err(|error| format!("Palmier Project export task failed: {error}"))?
}

/// Runs the Palmier project-package export activities in the desktop process, recording the same
/// job and export artifact that the Temporal workflow records.
fn write_palmier_project_package(
    project_dir: &str,
    job_id: String,
    output_path: String,
    updated_at: String,
) -> Result<NleXmlExportCommandResult, String> {
    let project_dir = resolve_project_dir(project_dir)?;
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)?;
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let project_dir_text = project_dir.display().to_string();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        &job_id,
        JobStatus::Running,
        &updated_at,
    );
    let start_request = temporal_export_project_bundle_start_request(
        &project.id,
        &project_dir_text,
        &job_id,
        &output_path,
        true,
    );
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.activity_types = start_request.activity_types.clone();
    }
    job.start_request = Some(start_request);
    apply_project_actions_to_split_project(
        &project_dir,
        vec![ProjectAction::RecordJob {
            job: Box::new(job.clone()),
        }],
    )
    .map_err(|error| error.to_string())?;

    let written = temporal_export_project_bundle_write_activity_value(serde_json::json!({
        "projectId": project.id,
        "projectDir": project_dir_text,
        "jobId": job_id,
        "profile": "palmierProject",
        "outputPath": output_path,
        "createdAt": updated_at,
    }));
    let written = match written {
        Ok(written) => written,
        Err(error) => {
            let _ = apply_project_actions_to_split_project(
                &project_dir,
                vec![ProjectAction::UpdateJobStatus {
                    job_id,
                    status: JobStatus::Failed,
                    updated_at,
                    run_id: None,
                }],
            );
            return Err(error.to_string());
        }
    };
    let export_path = written
        .get("writtenPath")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Palmier Project export did not report its written path".to_string())?
        .to_string();
    let project = load_split_project(&project_dir).map_err(|error| error.to_string())?;
    let job = project
        .jobs
        .iter()
        .find(|recorded_job| recorded_job.id == job.id)
        .cloned()
        .unwrap_or(job);

    Ok(NleXmlExportCommandResult {
        project,
        export_path,
        job,
    })
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri exposes render settings and optional range fields as named IPC arguments"
)]
async fn render_webm_to_split_project_folder(
    project_dir: String,
    project_id: String,
    profile: RenderQualityProfile,
    job_id: String,
    attempt_id: String,
    updated_at: String,
    range_start_seconds: Option<f64>,
    range_end_seconds: Option<f64>,
    timeline_id: Option<String>,
) -> Result<ProjectWebmRenderResult, String> {
    run_blocking_command("WebM render", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        let job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project_id,
            &job_id,
            JobStatus::Queued,
            &updated_at,
        );
        let range_seconds = match (range_start_seconds, range_end_seconds) {
            (Some(start_seconds), Some(end_seconds)) => Some((start_seconds, end_seconds)),
            (None, None) => None,
            _ => {
                return Err(
                    "rangeStartSeconds and rangeEndSeconds must be provided together.".to_string(),
                );
            }
        };

        render_project_webm_to_split_project_folder_for_timeline(
            &project_dir,
            &project_id,
            profile,
            job,
            &updated_at,
            Some(attempt_id),
            range_seconds,
            timeline_id.as_deref(),
        )
        .map_err(pipeline_errors_to_string)
    })
    .await
}

/// Renders off the main thread. The in-process export and Save Range as Media both call this, and
/// the editor keeps polling the project while it runs.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri exposes render settings and optional range fields as named IPC arguments"
)]
async fn render_media_to_split_project_folder(
    project_dir: String,
    project_id: String,
    profile: ExportProfile,
    quality: RenderQuality,
    width: u32,
    height: u32,
    job_id: String,
    attempt_id: String,
    updated_at: String,
    range_start_seconds: Option<f64>,
    range_end_seconds: Option<f64>,
    timeline_id: Option<String>,
    fps: Option<f64>,
    encode_tier: Option<ExportEncodeTier>,
    output: Option<ExportOutputRequest>,
    export_settings: Option<JobExportSettings>,
    admission_protocol: Option<u32>,
    expected_revision: Option<u64>,
) -> Result<serde_json::Value, String> {
    run_blocking_command("media render", move || {
        let options = export_render_options(profile, quality, width, height, fps, encode_tier)?;
        let project_dir = resolve_project_dir(&project_dir)?;
        if let Some(protocol) = admission_protocol {
            if protocol != 1 {
                return Err("Unsupported render admission protocol".into());
            }
            let expected_revision =
                expected_revision.ok_or("Expected revision is required for render admission")?;
            let input = video_creater_lib::render_pipeline::project_export::MediaRenderInput {
                project_id,
                profile,
                quality,
                width,
                height,
                job_id,
                attempt_id,
                updated_at,
                range_start_seconds,
                range_end_seconds,
                timeline_id,
                fps,
                encode_tier,
                output,
                export_settings,
            };
            return serde_json::to_value(
                video_creater_lib::app_service::render_jobs::RenderJobService::admit(
                    &RequestContext::new(
                        ClientKind::Desktop,
                        uuid::Uuid::new_v4().to_string(),
                        Some(input.project_id.clone()),
                        Some(expected_revision),
                        BTreeSet::from([AuthorizationScope::ProjectWrite]),
                        None,
                    )
                    .map_err(|error| error.to_string())?,
                    &project_dir,
                    input,
                )
                .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string());
        }
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project_id,
            &job_id,
            JobStatus::Queued,
            &updated_at,
        );
        job.export_settings = output
            .clone()
            .map(|output| JobExportSettings::for_export(options, export_settings, output));
        let range_seconds = match (range_start_seconds, range_end_seconds) {
            (Some(start_seconds), Some(end_seconds)) => Some((start_seconds, end_seconds)),
            (None, None) => None,
            _ => {
                return Err(
                    "rangeStartSeconds and rangeEndSeconds must be provided together.".to_string(),
                );
            }
        };

        let result = render_media_export_to_split_project_folder(MediaExportRequest {
            project_dir: &project_dir,
            project_id: &project_id,
            options,
            job,
            updated_at: &updated_at,
            run_id: Some(attempt_id),
            range_seconds,
            timeline_id: timeline_id.as_deref(),
            output: output.as_ref(),
        })
        .map_err(pipeline_errors_to_string)?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
async fn load_render_attempt_in_split_project_folder(
    project_dir: String,
    job_id: String,
    attempt_id: String,
) -> Result<video_creater_lib::render_pipeline::project_export::MediaRenderAttempt, String> {
    run_blocking_command("render attempt", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        let identity =
            video_creater_lib::app_service::projects::read_project_identity(&project_dir)
                .map_err(|error| error.to_string())?;
        let context = RequestContext::new(
            ClientKind::Desktop,
            uuid::Uuid::new_v4().to_string(),
            Some(identity.id),
            None,
            BTreeSet::from([AuthorizationScope::ProjectRead]),
            None,
        )
        .map_err(|error| error.to_string())?;
        video_creater_lib::app_service::render_jobs::RenderJobService::attempt(
            &context,
            &project_dir,
            video_creater_lib::app_service::render_jobs::RenderAttemptQuery { job_id, attempt_id },
        )
        .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
async fn recover_render_attempt_in_split_project_folder(
    project_dir: String,
    job_id: String,
    attempt_id: String,
) -> Result<video_creater_lib::render_pipeline::project_export::MediaRenderAttempt, String> {
    run_blocking_command("render attempt recovery", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        let identity =
            video_creater_lib::app_service::projects::read_project_identity(&project_dir)
                .map_err(|error| error.to_string())?;
        let context = RequestContext::new(
            ClientKind::Desktop,
            uuid::Uuid::new_v4().to_string(),
            Some(identity.id),
            None,
            BTreeSet::from([AuthorizationScope::ProjectWrite]),
            None,
        )
        .map_err(|error| error.to_string())?;
        video_creater_lib::app_service::render_jobs::RenderJobService::recover(
            &context,
            &project_dir,
            video_creater_lib::app_service::render_jobs::RenderAttemptQuery { job_id, attempt_id },
        )
        .map_err(|error| error.to_string())
    })
    .await
}

/// A consistent read for polling/reconciliation. Opening a project remains a separate
/// activation path with recovery and session ownership; ordinary reads do not take that path.
#[tauri::command]
async fn read_project_snapshot_from_split_project_folder(
    project_dir: String,
) -> Result<VideoProject, String> {
    run_blocking_command("project snapshot", move || {
        let project_dir = resolve_project_dir(&project_dir)?;
        let identity =
            video_creater_lib::app_service::projects::read_project_identity(&project_dir)
                .map_err(|error| error.to_string())?;
        let context = RequestContext::new(
            ClientKind::Desktop,
            uuid::Uuid::new_v4().to_string(),
            Some(identity.id),
            None,
            BTreeSet::from([AuthorizationScope::ProjectRead]),
            None,
        )
        .map_err(|error| error.to_string())?;
        desktop_project_service()
            .load(&context, &project_dir)
            .map_err(|error| error.to_string())
    })
    .await
}

fn resolve_project_relative_file(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty() {
        return Err("preview/render QA path cannot be empty".to_string());
    }
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "preview/render QA path must be project-relative without traversal: {relative_path}"
        ));
    }

    Ok(project_dir.join(path))
}

fn validate_preview_render_comparison_request(
    project_dir: &Path,
    request: &ProjectRenderPreviewComparisonRequest,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    if !request.duration_seconds.is_finite() || request.duration_seconds <= 0.0 {
        return Err("preview/render QA durationSeconds must be positive".to_string());
    }
    if !request.frame_time_seconds.is_finite() || request.frame_time_seconds < 0.0 {
        return Err("preview/render QA frameTimeSeconds must be non-negative".to_string());
    }

    let rendered_frame = request
        .rendered_frames
        .first()
        .ok_or_else(|| "preview/render QA has no retained rendered frame".to_string())?;
    Ok((
        resolve_project_relative_file(project_dir, &request.rendered_video)?,
        resolve_project_relative_file(project_dir, &request.render_report_path)?,
        resolve_project_relative_file(project_dir, rendered_frame)?,
    ))
}

fn compare_preview_render_frames(
    preview_frame: &Path,
    rendered_frame: &Path,
    diff_frame: &Path,
) -> Result<(f64, bool), String> {
    const CHANNEL_THRESHOLD: u8 = 8;
    const MISMATCH_RATIO_THRESHOLD: f64 = 0.01;

    let preview = image::open(preview_frame)
        .map_err(|error| format!("could not decode canonical preview frame: {error}"))?
        .to_rgba8();
    let rendered = image::open(rendered_frame)
        .map_err(|error| format!("could not decode rendered frame: {error}"))?
        .to_rgba8();
    if preview.dimensions() != rendered.dimensions() {
        return Err(format!(
            "preview/render dimensions differ: preview={}x{}, rendered={}x{}",
            preview.width(),
            preview.height(),
            rendered.width(),
            rendered.height()
        ));
    }

    let mut mismatched_pixels = 0_u64;
    let mut diff = image::RgbaImage::new(preview.width(), preview.height());
    for (x, y, preview_pixel) in preview.enumerate_pixels() {
        let rendered_pixel = rendered.get_pixel(x, y);
        let mismatched = preview_pixel
            .0
            .iter()
            .zip(rendered_pixel.0.iter())
            .any(|(left, right)| left.abs_diff(*right) > CHANNEL_THRESHOLD);
        if mismatched {
            mismatched_pixels += 1;
            diff.put_pixel(x, y, image::Rgba([255, 0, 64, 255]));
        } else {
            diff.put_pixel(
                x,
                y,
                image::Rgba([preview_pixel[0], preview_pixel[1], preview_pixel[2], 64]),
            );
        }
    }
    let pixel_count = u64::from(preview.width()) * u64::from(preview.height());
    let mismatch_ratio = mismatched_pixels as f64 / pixel_count.max(1) as f64;
    let passed = mismatch_ratio <= MISMATCH_RATIO_THRESHOLD;
    if !passed {
        if let Some(parent) = diff_frame.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        diff.save(diff_frame).map_err(|error| error.to_string())?;
    }
    Ok((mismatch_ratio, passed))
}

#[tauri::command]
async fn run_preview_render_comparison_request_in_split_project_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    project_dir: String,
    request: ProjectRenderPreviewComparisonRequest,
    updated_at: String,
) -> Result<ProjectPreviewRenderComparisonRunResult, String> {
    run_blocking_command("preview/render comparison", move || {
        run_preview_render_comparison_request_blocking(
            project_dir,
            request,
            updated_at,
            &app.state::<StorageProjectMutationCoordinatorState>(),
        )
    })
    .await
}

fn run_preview_render_comparison_request_blocking(
    project_dir: String,
    request: ProjectRenderPreviewComparisonRequest,
    updated_at: String,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
) -> Result<ProjectPreviewRenderComparisonRunResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    if request.status != "pending" {
        return Err("preview/render QA request is not pending".to_string());
    }

    let _lease = mutation_coordinator.acquire()?;
    // The comparison runs a canonical capture, which owns the project's derived files; its own
    // canonical writes take the project lease for themselves.
    let _artifacts = acquire_split_project_artifact_lease(&project_dir)?;

    let (_rendered_video, render_report_path, rendered_frame_path) =
        validate_preview_render_comparison_request(&project_dir, &request)?;
    let capture_job_id = format!("{}-preview-review", request.project_report_id);
    let capture = render_prepared_preview_frame_to_split_project_folder_with_lease(
        &project_dir,
        request.frame_time_seconds,
        &capture_job_id,
        &updated_at,
        &_lease,
    )
    .map_err(pipeline_errors_to_string)?;
    let preview_frame_path = resolve_project_relative_file(&project_dir, &capture.preview_frame)?;
    let evidence_dir = render_report_path
        .parent()
        .ok_or_else(|| "preview/render report has no parent directory".to_string())?
        .join("preview-qa");
    validate_split_project_write_path(&project_dir, &evidence_dir)
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&evidence_dir).map_err(|error| error.to_string())?;
    let diff_frame_path = evidence_dir.join("diff-0001.png");
    validate_split_project_write_path(&project_dir, &diff_frame_path)
        .map_err(|error| error.to_string())?;
    let (mismatch_ratio, passed) =
        compare_preview_render_frames(&preview_frame_path, &rendered_frame_path, &diff_frame_path)?;
    let relative_diff_frame = (!passed).then(|| {
        diff_frame_path
            .strip_prefix(&project_dir)
            .unwrap_or(&diff_frame_path)
            .to_string_lossy()
            .into_owned()
    });
    let comparison = RenderPreviewComparison {
        status: if passed { "passed" } else { "failed" }.to_string(),
        compared_frames: vec![RenderPreviewComparisonFrame {
            timeline_seconds: request.frame_time_seconds,
            preview_frame: capture.preview_frame.clone(),
            rendered_frame: request.rendered_frames[0].clone(),
            diff_frame: relative_diff_frame.clone(),
            mismatch_ratio,
            passed,
        }],
    };
    let evidence_report_path = evidence_dir.join("preview-comparison.json");
    validate_split_project_write_path(&project_dir, &evidence_report_path)
        .map_err(|error| error.to_string())?;
    fs::write(
        &evidence_report_path,
        serde_json::to_vec_pretty(&comparison).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let evidence_report = evidence_report_path
        .strip_prefix(&project_dir)
        .unwrap_or(&evidence_report_path)
        .to_string_lossy()
        .into_owned();

    let mut render_report: RenderReport =
        serde_json::from_slice(&fs::read(&render_report_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if let Some(comparison_request) = render_report.preview_comparison_request.as_mut() {
        comparison_request.status = "completed".to_string();
    }
    for artifact in [
        evidence_report.as_str(),
        capture.preview_frame.as_str(),
        request.rendered_frames[0].as_str(),
    ] {
        if !render_report
            .artifacts
            .iter()
            .any(|current| current == artifact)
        {
            render_report.artifacts.push(artifact.to_string());
        }
    }
    if let Some(diff_frame) = &relative_diff_frame {
        if !render_report.artifacts.contains(diff_frame) {
            render_report.artifacts.push(diff_frame.clone());
        }
    }
    render_report.preview_comparison = Some(comparison);
    validate_split_project_write_path(&project_dir, &render_report_path)
        .map_err(|error| error.to_string())?;
    let markdown_render_report_path = render_report_path.with_extension("md");
    validate_split_project_write_path(&project_dir, &markdown_render_report_path)
        .map_err(|error| error.to_string())?;
    write_json_report(&render_report_path, &render_report).map_err(pipeline_errors_to_string)?;
    write_markdown_report(&markdown_render_report_path, &render_report)
        .map_err(pipeline_errors_to_string)?;

    let mut project = capture.project;
    let project_render_report = project
        .render_reports
        .iter_mut()
        .find(|report| report.id == request.project_report_id)
        .ok_or_else(|| "project render report was not found".to_string())?;
    if let Some(comparison_request) = project_render_report.preview_comparison_request.as_mut() {
        comparison_request.status = "completed".to_string();
    }
    project_render_report.preview_comparison = Some(ProjectRenderPreviewComparison {
        status: if passed { "passed" } else { "failed" }.to_string(),
        compared_frames: vec![ProjectRenderPreviewComparisonFrame {
            timeline_seconds: request.frame_time_seconds,
            preview_frame: capture.preview_frame,
            rendered_frame: request.rendered_frames[0].clone(),
            diff_frame: relative_diff_frame,
            mismatch_ratio,
            passed,
        }],
    });
    for artifact in &render_report.artifacts {
        if !project_render_report.artifacts.contains(artifact) {
            project_render_report.artifacts.push(artifact.clone());
        }
    }
    let project_render_report = project_render_report.clone();
    project = save_preview_render_comparison_project(&project_dir, &project)?;

    Ok(ProjectPreviewRenderComparisonRunResult {
        project,
        render_report,
        project_render_report,
        evidence_report,
    })
}

fn save_preview_render_comparison_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<VideoProject, String> {
    save_split_project(project_dir, project)
        .map(|result| result.project)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn cancel_render_job_in_split_project_folder(
    project_dir: String,
    job_id: String,
    attempt_id: String,
    updated_at: String,
) -> Result<ProjectActionWriteResult, String> {
    // Request cancellation before queueing the status write: the request has to reach the render
    // even when the queued write waits for a lease the render is holding.
    let resolved_dir = resolve_project_dir(&project_dir)?;
    let outcome = request_render_cancellation_by_locator(&resolved_dir, &job_id, &attempt_id);
    run_project_write_command("render cancellation", &project_dir, move |project_dir| {
        record_render_cancellation_outcome(&project_dir, outcome, job_id, attempt_id, updated_at)
    })
    .await
}

#[cfg(test)]
fn cancel_render_job_in_split_project_folder_blocking(
    project_dir: String,
    job_id: String,
    attempt_id: String,
    updated_at: String,
) -> Result<ProjectActionWriteResult, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    let outcome = request_render_cancellation_by_locator(&project_dir, &job_id, &attempt_id);
    record_render_cancellation_outcome(&project_dir, outcome, job_id, attempt_id, updated_at)
}

/// The project, unchanged, when the job already records `attempt_id` as cancelled.
fn already_cancelled_render_write(
    project_dir: &Path,
    job_id: &str,
    attempt_id: &str,
) -> Option<ProjectActionWriteResult> {
    let project = load_split_project(project_dir).ok()?;
    let cancelled = project.jobs.iter().any(|job| {
        job.id == job_id
            && job.status == JobStatus::Cancelled
            && job
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref())
                == Some(attempt_id)
    });
    cancelled.then(|| ProjectActionWriteResult {
        project,
        report: ProjectWriteReport {
            manifest_path: video_creater_lib::project::split::split_project_manifest_path(
                project_dir,
            )
            .display()
            .to_string(),
            written_files: Vec::new(),
            removed_files: Vec::new(),
            recovery_pending: false,
        },
    })
}

fn record_render_cancellation_outcome(
    project_dir: &Path,
    outcome: RenderCancellationOutcome,
    job_id: String,
    attempt_id: String,
    updated_at: String,
) -> Result<ProjectActionWriteResult, String> {
    match outcome {
        RenderCancellationOutcome::Requested | RenderCancellationOutcome::AlreadyRequested => {
            let error = match apply_project_action_to_split_project(
                project_dir,
                ProjectAction::UpdateJobStatus {
                    job_id: job_id.clone(),
                    status: JobStatus::Cancelled,
                    updated_at,
                    run_id: Some(attempt_id.clone()),
                },
            ) {
                Ok(write) => return Ok(write),
                Err(error) => error.to_string(),
            };
            // This write waits for the render's project lease, and the stopped render records its
            // own cancellation before releasing it. That cancellation is this request's result.
            return already_cancelled_render_write(project_dir, &job_id, &attempt_id).ok_or(error);
        }
        RenderCancellationOutcome::TooLate => {
            return Err(format!(
                "Render attempt `{attempt_id}` is already completing."
            ));
        }
        RenderCancellationOutcome::NotFound => {}
    }
    let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| format!("Render job `{job_id}` was not found."))?;
    if !matches!(
        job.kind.as_str(),
        "render_draft" | "export_media" | "exportMedia"
    ) {
        return Err(format!("Job `{job_id}` is not a render/export job."));
    }
    if !matches!(job.status, JobStatus::Running | JobStatus::Progress) {
        return Err(format!(
            "Render job `{job_id}` is already terminal or not active."
        ));
    }
    let persisted_attempt = job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref())
        .ok_or_else(|| format!("Render job `{job_id}` has no active attempt identity."))?;
    if persisted_attempt != attempt_id {
        return Err(format!("Render attempt `{attempt_id}` is stale."));
    }
    Err(format!("Render attempt `{attempt_id}` is not active."))
}

#[tauri::command]
fn load_render_pipeline_report_from_split_project_folder(
    project_dir: String,
    job_id: String,
) -> Result<RenderReport, String> {
    let project_dir = resolve_project_dir(&project_dir)?;
    load_project_render_pipeline_report(&project_dir, &job_id).map_err(pipeline_errors_to_string)
}

#[tauri::command]
fn list_shader_background_templates(
    project_dir: Option<String>,
) -> Result<Vec<ShaderBackgroundTemplateCatalogEntry>, String> {
    let templates = match project_dir
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        Some(project_dir) => {
            let project_dir = resolve_project_dir(project_dir)?;
            project_shader_background_templates(&project_dir).map_err(|error| error.to_string())?
        }
        None => builtin_shader_background_templates().map_err(|error| error.to_string())?,
    };

    Ok(templates
        .into_iter()
        .map(ShaderBackgroundTemplateCatalogEntry::from)
        .collect())
}

#[tauri::command]
fn generate_one_click_edit_for_project(
    mut project: VideoProject,
    request: EditJobRequest,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<VideoProject, String> {
    validate_generate_edit_transcription_gate(&project, &request, model_state)?;
    generate_one_click_edit_timeline(&mut project, request)
        .map(|_| project)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn generate_spoken_semantic_multi_source_edit_for_project(
    mut project: VideoProject,
    request: EditJobRequest,
    semantic_hits: Vec<SemanticVisualHit>,
) -> Result<(VideoProject, GeneratedEditDraft), String> {
    let draft =
        generate_spoken_semantic_multi_source_edit_timeline(&mut project, request, &semantic_hits)
            .map_err(|error| error.to_string())?;
    Ok((project, draft))
}

#[tauri::command]
fn list_transcription_models(
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<Vec<TranscriptionModelStatus>, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;

    store.list().map_err(|error| error.to_string())
}

fn settings_health_snapshot_for_model_state(
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&Path>,
    provider_health: &[ProviderHealth],
    claude_executable_override: Option<&str>,
) -> Result<SettingsHealthSnapshot, SettingsHealthCommandError> {
    build_initial_settings_health_snapshot(
        model_state,
        configured_skill_root,
        provider_health,
        claude_executable_override,
    )
    .map_err(SettingsHealthCommandError::from)
}

/// The user's explicit `claude` path, so readiness probes the binary a turn would
/// actually run. An unreadable store just means "resolve it the usual way".
fn claude_executable_override(preferences: &AppPreferencesState) -> Option<String> {
    preferences
        .0
        .lock()
        .ok()
        .and_then(|store| store.load_saved_or_default().ok())
        .and_then(|preferences| preferences.claude_executable_override().map(str::to_string))
}

fn claude_executable_override_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<String> {
    app.try_state::<AppPreferencesState>()
        .and_then(|state| claude_executable_override(&state))
}

/// The agent settings a conversation turn runs under.
///
/// Read here rather than taken as a command argument, so the command's TypeScript-visible
/// signature keeps saying nothing about agents. An unmanaged state, a poisoned lock or an
/// unreadable preferences file all land on the shipped defaults — Automatic on the default
/// model — because none of them is a reason to refuse a turn.
fn agent_turn_preferences_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> AgentTurnPreferences {
    app.try_state::<AppPreferencesState>()
        .and_then(|state| {
            state
                .0
                .lock()
                .ok()
                .map(|store| agent_turn_preferences(&store))
        })
        .unwrap_or_default()
}

#[tauri::command]
fn get_settings_acceptance_context(
    state: tauri::State<'_, SettingsAcceptanceStateHolder>,
) -> Option<SettingsAcceptanceContext> {
    state.0.as_ref().map(SettingsAcceptanceState::context)
}

fn write_settings_acceptance_storage_load_progress(
    state: &SettingsAcceptanceStateHolder,
    step: SettingsAcceptanceProgressStep,
) -> Result<(), String> {
    let Some(acceptance) = state.0.as_ref() else {
        return Ok(());
    };
    acceptance
        .write_progress(SettingsAcceptanceProgress {
            stage: acceptance.stage(),
            step,
        })
        .map(|_| ())
}

#[tauri::command]
fn write_settings_acceptance_checkpoint(
    state: tauri::State<'_, SettingsAcceptanceStateHolder>,
    checkpoint: SettingsAcceptanceCheckpoint,
) -> Result<(), String> {
    let acceptance = state
        .0
        .as_ref()
        .ok_or_else(|| "settings acceptance is not enabled".to_string())?;
    acceptance.write_checkpoint(checkpoint).map(|_| ())
}

#[tauri::command]
fn write_settings_acceptance_progress(
    state: tauri::State<'_, SettingsAcceptanceStateHolder>,
    progress: SettingsAcceptanceProgress,
) -> Result<(), String> {
    let acceptance = state
        .0
        .as_ref()
        .ok_or_else(|| "settings acceptance is not enabled".to_string())?;
    acceptance.write_progress(progress).map(|_| ())
}

#[tauri::command]
fn write_settings_acceptance_failure(
    state: tauri::State<'_, SettingsAcceptanceStateHolder>,
    failure: SettingsAcceptanceFailure,
) -> Result<(), String> {
    let acceptance = state
        .0
        .as_ref()
        .ok_or_else(|| "settings acceptance is not enabled".to_string())?;
    acceptance.write_failure(failure).map(|_| ())
}

#[tauri::command]
fn abort_settings_acceptance_run(
    state: tauri::State<'_, SettingsAcceptanceStateHolder>,
    failure: SettingsAcceptanceFailure,
) -> Result<(), String> {
    let acceptance = state
        .0
        .as_ref()
        .ok_or_else(|| "settings acceptance is not enabled".to_string())?;
    acceptance.validate_failure(&failure)?;
    std::process::exit(86)
}

#[tauri::command]
fn get_settings_health_snapshot(
    project_root: Option<String>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
    provider_state: tauri::State<'_, ProviderHealthState>,
    preferences_state: tauri::State<'_, AppPreferencesState>,
) -> Result<SettingsHealthSnapshot, SettingsHealthCommandError> {
    let configured_root = project_root
        .as_deref()
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    let cached_provider_health =
        provider_state
            .snapshot()
            .map_err(|_| SettingsHealthCommandError {
                code: "settings.providers.cacheUnavailable".to_string(),
                message: "Provider health is temporarily unavailable.".to_string(),
                detail: "provider health cache lock failed".to_string(),
            })?;
    let baseline_provider_health =
        aggregate_provider_health(&stored_provider_credential_statuses(), &[], &[], None).map_err(
            |_| SettingsHealthCommandError {
                code: "settings.providers.catalogUnavailable".to_string(),
                message: "Provider health could not be initialized.".to_string(),
                detail: "provider catalog could not be aggregated".to_string(),
            },
        )?;
    let mut provider_health_by_id = baseline_provider_health
        .into_iter()
        .map(|health| (health.provider.clone(), health))
        .collect::<HashMap<_, _>>();
    for health in cached_provider_health {
        if provider_health_by_id.contains_key(&health.provider) {
            provider_health_by_id.insert(health.provider.clone(), health);
        }
    }
    let provider_health = supported_provider_ids()
        .filter_map(|provider| provider_health_by_id.remove(provider))
        .collect::<Vec<_>>();
    settings_health_snapshot_for_model_state(
        &model_state.0,
        configured_root,
        &provider_health,
        claude_executable_override(&preferences_state).as_deref(),
    )
}

fn system_health_inventory(
    roots: &StorageInventoryRootsState,
    project_root: Option<&str>,
) -> Vec<StorageInventoryItem> {
    let project_root = project_root
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    collect_storage_inventory(
        &roots.global_model_root,
        &roots.app_cache_root,
        project_root,
    )
}

#[tauri::command]
fn get_system_health_snapshot(
    project_root: Option<String>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
    roots: tauri::State<'_, StorageInventoryRootsState>,
    preferences_state: tauri::State<'_, AppPreferencesState>,
) -> SystemHealthSnapshot {
    let configured_root = project_root
        .as_deref()
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    let inventory = system_health_inventory(&roots, project_root.as_deref());
    build_system_health_snapshot(
        &model_state.0,
        configured_root,
        &inventory,
        claude_executable_override(&preferences_state).as_deref(),
    )
}

#[tauri::command]
fn refresh_system_health_section(
    section_id: String,
    project_root: Option<String>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
    roots: tauri::State<'_, StorageInventoryRootsState>,
    preferences_state: tauri::State<'_, AppPreferencesState>,
) -> Result<SystemHealthSection, SettingsHealthCommandError> {
    let section_id = SystemHealthSectionId::try_from(section_id.as_str())
        .map_err(SettingsHealthCommandError::from)?;
    let configured_root = project_root
        .as_deref()
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    let inventory = collect_system_health_inventory_if_needed(section_id, || {
        system_health_inventory(&roots, project_root.as_deref())
    });
    build_system_health_section_by_id(
        section_id,
        &model_state.0,
        configured_root,
        inventory.as_deref().unwrap_or_default(),
        claude_executable_override(&preferences_state).as_deref(),
    )
    .map_err(SettingsHealthCommandError::from)
}

#[tauri::command]
fn get_agent_settings_health(
    preferences_state: tauri::State<'_, AppPreferencesState>,
) -> Result<SettingsCategoryHealth, SettingsHealthCommandError> {
    Ok(build_agent_settings_health(
        claude_executable_override(&preferences_state).as_deref(),
    ))
}

#[tauri::command]
fn get_skills_settings_health(
    project_root: Option<String>,
) -> Result<SettingsCategoryHealth, SettingsHealthCommandError> {
    let configured_root = project_root
        .as_deref()
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    Ok(build_skills_settings_health(configured_root))
}

fn storage_health_for_roots(
    roots: &StorageInventoryRootsState,
    active_project_dir: Option<&str>,
) -> SettingsCategoryHealth {
    let active_project_dir = active_project_dir
        .filter(|path| !path.trim().is_empty())
        .map(Path::new);
    let inventory = collect_storage_inventory(
        &roots.global_model_root,
        &roots.app_cache_root,
        active_project_dir,
    );
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    build_storage_category_health(&inventory, &generated_at)
}

#[tauri::command]
fn get_storage_health(
    active_project_dir: Option<String>,
    roots: tauri::State<'_, StorageInventoryRootsState>,
) -> SettingsCategoryHealth {
    storage_health_for_roots(&roots, active_project_dir.as_deref())
}

fn reveal_reported_storage_path_with<F>(
    requested_path: &Path,
    inventory: &[StorageInventoryItem],
    reveal: F,
) -> Result<(), SettingsOperationsCommandError>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    if !requested_path.is_absolute()
        || !inventory.iter().any(|item| {
            item.unavailable_reason.is_none()
                && item.path.as_deref().map(Path::new) == Some(requested_path)
        })
    {
        return Err(SettingsOperationsCommandError {
            code: "settings.storage.revealPathNotReported".to_string(),
            message: "The storage location could not be revealed safely.".to_string(),
            detail: "The requested path is not an available item in the current storage inventory. Refresh Storage and try again.".to_string(),
        });
    }

    let canonical_path =
        fs::canonicalize(requested_path).map_err(|error| SettingsOperationsCommandError {
            code: "settings.storage.revealPathUnavailable".to_string(),
            message: "The storage location is no longer available.".to_string(),
            detail: error.to_string(),
        })?;
    reveal(&canonical_path).map_err(|detail| SettingsOperationsCommandError {
        code: "settings.storage.revealFailed".to_string(),
        message: if cfg!(target_os = "macos") {
            "Finder could not reveal the storage location."
        } else {
            "The file manager could not show the storage location."
        }
        .to_string(),
        detail,
    })
}

#[tauri::command]
fn reveal_storage_inventory_item(
    path: String,
    active_project_dir: Option<String>,
    roots: tauri::State<'_, StorageInventoryRootsState>,
) -> Result<(), SettingsOperationsCommandError> {
    let active_project_dir = active_project_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(Path::new);
    let inventory = collect_storage_inventory(
        &roots.global_model_root,
        &roots.app_cache_root,
        active_project_dir,
    );
    reveal_reported_storage_path_with(
        Path::new(&path),
        &inventory,
        reveal_path_in_platform_file_manager,
    )
}

/// Selects `path` in Finder on macOS or the freedesktop file manager on Linux.
fn reveal_path_in_platform_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("/usr/bin/open")
            .arg("-R")
            .arg(path)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Finder exited with status {status}."))
        }
    }
    #[cfg(target_os = "linux")]
    {
        video_creater_lib::desktop_integration::reveal_path_in_file_manager(path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
        Err("Showing items in the file manager is not supported on this platform.".to_string())
    }
}

/// "Show in folder" for a file the project records as export output (see
/// `project::export_reveal::recorded_export_paths`); there is no generic path reveal.
#[tauri::command]
async fn reveal_export_artifact_in_split_project_folder(
    project_dir: String,
    artifact_path: String,
) -> Result<(), String> {
    run_blocking_command("export reveal", move || {
        reveal_export_artifact_in_split_project_folder_blocking(project_dir, artifact_path)
    })
    .await
}

fn reveal_export_artifact_in_split_project_folder_blocking(
    project_dir: String,
    artifact_path: String,
) -> Result<(), String> {
    let resolved_dir = resolve_project_dir(&project_dir)?;
    let project = load_split_project(&resolved_dir).map_err(|error| error.to_string())?;
    reveal_recorded_export_with(Path::new(&project_dir), &project, &artifact_path, |path| {
        reveal_path_in_platform_file_manager(path).inspect_err(|detail| {
            eprintln!("reveal_export_artifact_in_split_project_folder: {detail}");
        })
    })
}

fn reserve_storage_inventory_refresh(
    registry: &SettingsOperationRegistry,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::StorageRefresh,
            "storage",
            Some(6),
            Some("scopes".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued storage inventory refresh.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

fn start_storage_inventory_refresh_with<R, F>(
    app: &tauri::AppHandle<R>,
    registry: &SettingsOperationRegistry,
    inspect: F,
) -> Result<SettingsOperation, SettingsOperationsCommandError>
where
    R: tauri::Runtime,
    F: FnOnce() -> usize + Send + 'static,
{
    let (operation, created) = reserve_storage_inventory_refresh(registry)?;
    if !created {
        return Ok(operation);
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let worker_app = app.clone();
    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_storage_inventory_refresh_operation(worker_app, operation_id, inspect);
    });
    Ok(operation)
}

fn run_storage_inventory_refresh_operation<R, F>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    inspect: F,
) where
    R: tauri::Runtime,
    F: FnOnce() -> usize,
{
    {
        let registry = app.state::<SettingsOperationRegistryState>();
        let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
        running.phase = Some("inspecting".to_string());
        running.completed_units = Some(0);
        running.total_units = Some(6);
        running.unit = Some("scopes".to_string());
        running.cancellable = Some(false);
        running.message = Some("Inspecting storage usage and volume free space.".to_string());
        match registry.0.update(&operation_id, running) {
            Ok(operation) => {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
            Err(error) => {
                if let Ok(operation) = registry.0.project_persistence_failure(
                    &operation_id,
                    "settings.storage.refreshPersistenceFailed",
                    "Storage inventory refresh could not save its running state.",
                    &error.to_string(),
                ) {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                return;
            }
        }
    }

    let unavailable_count = inspect();
    let registry = app.state::<SettingsOperationRegistryState>();
    let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
    succeeded.phase = Some(if unavailable_count == 0 {
        "refreshed".to_string()
    } else {
        "refreshed_with_unavailable_scopes".to_string()
    });
    succeeded.completed_units = Some(6);
    succeeded.total_units = Some(6);
    succeeded.unit = Some("scopes".to_string());
    succeeded.cancellable = Some(false);
    succeeded.message = Some(if unavailable_count == 0 {
        "Storage inventory refreshed.".to_string()
    } else {
        format!("Storage inventory refreshed; {unavailable_count} scopes are unavailable.")
    });
    match registry.0.update(&operation_id, succeeded) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            if let Ok(operation) = registry.0.project_persistence_failure(
                &operation_id,
                "settings.storage.refreshPersistenceFailed",
                "Storage inventory refresh finished, but its result could not be saved.",
                &error.to_string(),
            ) {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
        }
    }
}

#[tauri::command]
fn refresh_storage_inventory(
    app: tauri::AppHandle,
    active_project_dir: Option<String>,
    roots: tauri::State<'_, StorageInventoryRootsState>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let roots = roots.inner().clone();
    start_storage_inventory_refresh_with(&app, &operation_state.0, move || {
        storage_health_for_roots(&roots, active_project_dir.as_deref())
            .items
            .iter()
            .filter(|item| item.state != SettingsHealthState::Ready)
            .count()
    })
}

fn storage_cleanup_command_error(error: StorageCleanupError) -> SettingsOperationsCommandError {
    SettingsOperationsCommandError {
        code: error.code().to_string(),
        message: "Storage cleanup was refused or could not be completed safely.".to_string(),
        detail: error.to_string(),
    }
}

fn trusted_cleanup_project_session(
    target: &StorageCleanupTarget,
    requested_project_dir: Option<&str>,
    session_state: &ActiveProjectSessionState,
) -> Result<Option<ActiveProjectSession>, SettingsOperationsCommandError> {
    if matches!(target, StorageCleanupTarget::DisposableAppCache) {
        return Ok(None);
    }
    let active = session_state
        .current()
        .map_err(|detail| {
            storage_cleanup_command_error(StorageCleanupError::TargetUnavailable(
                "active project session".to_string(),
                detail,
            ))
        })?
        .ok_or_else(|| storage_cleanup_command_error(StorageCleanupError::ActiveProjectRequired))?;
    if let Some(requested_project_dir) = requested_project_dir {
        let requested = resolve_project_dir(requested_project_dir).map_err(|_| {
            storage_cleanup_command_error(StorageCleanupError::ProjectSessionMismatch)
        })?;
        if requested != active.canonical_root {
            return Err(storage_cleanup_command_error(
                StorageCleanupError::ProjectSessionMismatch,
            ));
        }
    }
    Ok(Some(active))
}

fn active_project_session_still_matches<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    expected: Option<&ActiveProjectSession>,
) -> Result<(), StorageCleanupError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    app.state::<ActiveProjectSessionState>()
        .matches(expected)
        .map_err(|_| StorageCleanupError::ProjectSessionMismatch)
        .and_then(|matches| {
            if matches {
                Ok(())
            } else {
                Err(StorageCleanupError::ProjectSessionMismatch)
            }
        })
}

fn storage_cleanup_run_failure(error: StorageCleanupError) -> StorageCleanupRunFailure {
    StorageCleanupRunFailure {
        error: Box::new(error),
        partial_report: video_creater_lib::settings::storage::StorageCleanupReport {
            removed_paths: Vec::new(),
            removed_bytes: 0,
            removed_count: 0,
        },
        quarantined_paths: Vec::new(),
        persistence_boundary_unknown: false,
    }
}

fn storage_cleanup_failed_transition(
    failure: &StorageCleanupRunFailure,
    total_items: u64,
) -> SettingsOperationTransition {
    let report = &failure.partial_report;
    let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
    failed.phase = Some("failed".to_string());
    failed.completed_units = Some(report.removed_count);
    failed.total_units = Some(total_items);
    failed.unit = Some("items".to_string());
    failed.cancellable = Some(false);
    failed.message = Some(if failure.persistence_boundary_unknown {
        format!(
            "Storage cleanup removed {} allowlisted item(s) ({} bytes) before stopping; the operation journal persistence boundary is unknown. Storage health was refreshed.",
            report.removed_count, report.removed_bytes
        )
    } else if report.removed_count > 0 {
        format!(
            "Storage cleanup removed {} allowlisted item(s) ({} bytes) before stopping safely. Storage health was refreshed.",
            report.removed_count, report.removed_bytes
        )
    } else {
        "Storage cleanup stopped safely before removing any item; storage health was refreshed."
            .to_string()
    });
    let removed_paths = if report.removed_paths.is_empty() {
        "none".to_string()
    } else {
        report.removed_paths.join(", ")
    };
    failed.error = Some(SettingsOperationError {
        code: failure.code().to_string(),
        message: format!(
            "Storage cleanup stopped after removing {} confirmed item(s).",
            report.removed_count
        ),
        recovery_action: Some(
            "Preview the cleanup again, review the exact paths, and retry with the new confirmation token."
                .to_string(),
        ),
        detail: Some(format!("{failure} Removed paths: {removed_paths}.")),
    });
    failed
}

#[tauri::command]
async fn preview_storage_cleanup<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: StorageCleanupTarget,
    active_project_dir: Option<String>,
) -> Result<StorageCleanupPreview, SettingsOperationsCommandError> {
    run_blocking_settings_command("storage cleanup preview", move || {
        preview_storage_cleanup_blocking(
            target,
            active_project_dir,
            &app.state::<StorageInventoryRootsState>(),
            &app.state::<ActiveProjectSessionState>(),
            &app.state::<StorageProjectMutationCoordinatorState>(),
            &app.state::<StorageCleanupPreviewRegistryState>(),
        )
    })
    .await
}

fn preview_storage_cleanup_blocking(
    target: StorageCleanupTarget,
    active_project_dir: Option<String>,
    roots: &StorageInventoryRootsState,
    session_state: &ActiveProjectSessionState,
    mutation_coordinator: &StorageProjectMutationCoordinatorState,
    preview_registry: &StorageCleanupPreviewRegistryState,
) -> Result<StorageCleanupPreview, SettingsOperationsCommandError> {
    let lease = mutation_coordinator.acquire().map_err(|detail| {
        storage_cleanup_command_error(StorageCleanupError::TargetUnavailable(
            "storage/project mutation coordinator".to_string(),
            detail,
        ))
    })?;
    let project_session =
        trusted_cleanup_project_session(&target, active_project_dir.as_deref(), session_state)?;
    let preview = preview_storage_cleanup_domain(
        &target,
        &roots.app_cache_root,
        project_session
            .as_ref()
            .map(|session| session.canonical_root.as_path()),
        project_session.as_ref().map(|session| session.generation),
    )
    .map_err(storage_cleanup_command_error)?;
    preview_registry
        .register(&preview, project_session.as_ref(), &lease)
        .map_err(storage_cleanup_command_error)?;
    Ok(preview)
}

fn storage_cleanup_target_id(target: &StorageCleanupTarget, confirmation_token: &str) -> String {
    let scope = match target {
        StorageCleanupTarget::DisposableAppCache => "disposable-app-cache",
        StorageCleanupTarget::ProjectRenderArtifacts { .. } => "project-render-artifacts",
    };
    format!("{scope}:{confirmation_token}")
}

fn reserve_storage_cleanup(
    registry: &SettingsOperationRegistry,
    target: &StorageCleanupTarget,
    confirmation_token: &str,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::StorageCleanup,
            storage_cleanup_target_id(target, confirmation_token),
            None,
            Some("items".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued allowlisted storage cleanup.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

fn start_storage_cleanup<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    registry: &SettingsOperationRegistry,
    target: StorageCleanupTarget,
    confirmation_token: String,
    roots: StorageInventoryRootsState,
    active_project_dir: Option<String>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    if !is_storage_cleanup_confirmation_token(&confirmation_token) {
        return Err(storage_cleanup_command_error(
            StorageCleanupError::ConfirmationMismatch,
        ));
    }
    let mutation_coordinator = app.state::<StorageProjectMutationCoordinatorState>();
    let lease = mutation_coordinator.acquire().map_err(|detail| {
        storage_cleanup_command_error(StorageCleanupError::TargetUnavailable(
            "storage/project mutation coordinator".to_string(),
            detail,
        ))
    })?;
    let session_state = app.state::<ActiveProjectSessionState>();
    let project_session =
        trusted_cleanup_project_session(&target, active_project_dir.as_deref(), &session_state)?;
    let preview_record = app
        .state::<StorageCleanupPreviewRegistryState>()
        .consume_valid(
            &confirmation_token,
            &target,
            project_session.as_ref(),
            &lease,
        )
        .map_err(storage_cleanup_command_error)?;
    let (operation, created) = reserve_storage_cleanup(registry, &target, &confirmation_token)?;
    drop(lease);
    if !created {
        return Err(storage_cleanup_command_error(
            StorageCleanupError::ConfirmationMismatch,
        ));
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let worker_app = app.clone();
    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_storage_cleanup_operation(
            worker_app,
            operation_id,
            target,
            confirmation_token,
            roots,
            preview_record,
        );
    });
    Ok(operation)
}

fn run_storage_cleanup_operation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
    target: StorageCleanupTarget,
    confirmation_token: String,
    roots: StorageInventoryRootsState,
    preview_record: StorageCleanupPreviewRecord,
) {
    let mut total_items = 0_u64;
    {
        let registry = app.state::<SettingsOperationRegistryState>();
        if registry.0.get(&operation_id).is_err() {
            return;
        }
        let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
        running.phase = Some("deleting".to_string());
        running.completed_units = Some(0);
        running.unit = Some("items".to_string());
        running.cancellable = Some(false);
        running.message =
            Some("Revalidating allowlisted cleanup paths before deletion.".to_string());
        match registry.0.update(&operation_id, running) {
            Ok(operation) => {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
            Err(error) => {
                if let Ok(operation) = registry.0.project_persistence_failure(
                    &operation_id,
                    "settings.storage.cleanupProgressFailed",
                    "Storage cleanup could not save its running state, so no files were removed.",
                    &error.to_string(),
                ) {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                return;
            }
        }
    }

    let project_session = preview_record.project_session.clone();
    let mutation_coordinator = app.state::<StorageProjectMutationCoordinatorState>();
    let cleanup_result = match mutation_coordinator.acquire() {
        Err(detail) => Err(storage_cleanup_run_failure(
            StorageCleanupError::TargetUnavailable(
                "storage/project mutation coordinator".to_string(),
                detail,
            ),
        )),
        Ok(_lease) => match project_session
            .as_ref()
            .map(|session| {
                video_creater_lib::settings::storage::acquire_storage_cleanup_project_ownership(
                    &session.canonical_root,
                )
            })
            .transpose()
        {
            Err(detail) => Err(storage_cleanup_run_failure(
                StorageCleanupError::TargetUnavailable(
                    "project mutation coordinator".to_string(),
                    detail,
                ),
            )),
            Ok(_project_ownership) => match active_project_session_still_matches(
                &app,
                project_session.as_ref(),
            )
            .and_then(|()| {
                preview_storage_cleanup_domain_with_nonce(
                    &target,
                    &roots.app_cache_root,
                    project_session
                        .as_ref()
                        .map(|session| session.canonical_root.as_path()),
                    project_session.as_ref().map(|session| session.generation),
                    &preview_record.preview_nonce,
                )
            }) {
                Ok(preview) => {
                    total_items = preview.items.len() as u64;
                    if confirmation_token != preview.confirmation_token
                        || preview != preview_record.preview
                    {
                        Err(storage_cleanup_run_failure(
                            StorageCleanupError::ConfirmationMismatch,
                        ))
                    } else {
                        let validated = {
                            let registry = app.state::<SettingsOperationRegistryState>();
                            let mut transition =
                                SettingsOperationTransition::to(SettingsOperationState::Running);
                            transition.phase = Some("validated".to_string());
                            transition.completed_units = Some(0);
                            transition.total_units = Some(total_items);
                            transition.unit = Some("items".to_string());
                            transition.cancellable = Some(false);
                            transition.message = Some(format!(
                                "Confirmed {total_items} exact allowlisted cleanup item(s)."
                            ));
                            registry.0.update(&operation_id, transition)
                        };
                        match validated {
                            Ok(operation) => {
                                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                                run_storage_cleanup_domain(
                                    &target,
                                    &confirmation_token,
                                    &roots.app_cache_root,
                                    project_session
                                        .as_ref()
                                        .map(|session| session.canonical_root.as_path()),
                                    project_session.as_ref().map(|session| session.generation),
                                    |progress| {
                                        let registry =
                                            app.state::<SettingsOperationRegistryState>();
                                        let mut transition = SettingsOperationTransition::to(
                                            SettingsOperationState::Running,
                                        );
                                        transition.phase = Some("deleting".to_string());
                                        transition.completed_units = Some(progress.completed_items);
                                        transition.total_units = Some(progress.total_items);
                                        transition.unit = Some("items".to_string());
                                        transition.cancellable = Some(false);
                                        transition.message = Some(format!(
                                            "Removed allowlisted item {}.",
                                            progress.path
                                        ));
                                        let operation = registry
                                            .0
                                            .update(&operation_id, transition)
                                            .map_err(|error| {
                                                StorageCleanupError::Progress(error.to_string())
                                            })?;
                                        let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                                        Ok(())
                                    },
                                )
                            }
                            Err(error) => Err(storage_cleanup_run_failure(
                                StorageCleanupError::Progress(error.to_string()),
                            )),
                        }
                    }
                }
                Err(error) => Err(storage_cleanup_run_failure(error)),
            },
        },
    };

    let health_project = app
        .state::<ActiveProjectSessionState>()
        .current()
        .ok()
        .flatten();
    let health = storage_health_for_roots(
        &roots,
        health_project
            .as_ref()
            .and_then(|session| session.canonical_root.to_str()),
    );
    let unavailable_count = health
        .items
        .iter()
        .filter(|item| item.state != SettingsHealthState::Ready)
        .count();
    let _ = app.emit(SETTINGS_STORAGE_HEALTH_EVENT, health);
    let registry = app.state::<SettingsOperationRegistryState>();
    let terminal_persistence_context = match &cleanup_result {
        Ok(report) => format!(
            "Storage cleanup removed {} item(s) ({} bytes), but its terminal state could not be saved.",
            report.removed_count, report.removed_bytes
        ),
        Err(failure) => format!(
            "Storage cleanup terminal state could not be saved. {failure} Removed paths: {}.",
            if failure.partial_report.removed_paths.is_empty() {
                "none".to_string()
            } else {
                failure.partial_report.removed_paths.join(", ")
            }
        ),
    };
    let transition = match cleanup_result {
        Ok(report) => {
            let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
            succeeded.phase = Some(if unavailable_count == 0 {
                "cleaned".to_string()
            } else {
                "cleaned_with_unavailable_scopes".to_string()
            });
            succeeded.completed_units = Some(report.removed_count);
            succeeded.total_units = Some(total_items);
            succeeded.unit = Some("items".to_string());
            succeeded.cancellable = Some(false);
            succeeded.message = Some(if unavailable_count == 0 {
                format!(
                    "Storage cleanup removed {} allowlisted item(s) and refreshed storage health.",
                    report.removed_count
                )
            } else {
                format!(
                    "Storage cleanup removed {} allowlisted item(s); storage health refreshed with {unavailable_count} unavailable scope(s).",
                    report.removed_count
                )
            });
            succeeded
        }
        Err(failure) => storage_cleanup_failed_transition(&failure, total_items),
    };
    match registry.0.update(&operation_id, transition) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            if let Ok(operation) = registry.0.project_persistence_failure(
                &operation_id,
                "settings.storage.cleanupTerminalPersistenceFailed",
                &terminal_persistence_context,
                &format!("{terminal_persistence_context} Journal error: {error}"),
            ) {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
        }
    }
}

#[tauri::command]
async fn run_storage_cleanup<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: StorageCleanupTarget,
    confirmation_token: String,
    active_project_dir: Option<String>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    run_blocking_settings_command("storage cleanup", move || {
        run_storage_cleanup_blocking(
            &app,
            target,
            confirmation_token,
            active_project_dir,
            &app.state::<StorageInventoryRootsState>(),
            &app.state::<SettingsOperationRegistryState>(),
        )
    })
    .await
}

fn run_storage_cleanup_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    target: StorageCleanupTarget,
    confirmation_token: String,
    active_project_dir: Option<String>,
    roots: &StorageInventoryRootsState,
    operation_state: &SettingsOperationRegistryState,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    start_storage_cleanup(
        app,
        &operation_state.0,
        target,
        confirmation_token,
        roots.clone(),
        active_project_dir,
    )
}

fn reserve_agent_component_self_test(
    component_id: &str,
    registry: &SettingsOperationRegistry,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::HealthCheck,
            component_id,
            Some(1),
            Some("components".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued component self-test.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

fn run_agent_component_self_test_operation<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    operation_id: &str,
    component_id: &str,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let registry = app.state::<SettingsOperationRegistryState>();
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("checking".to_string());
    running.completed_units = Some(0);
    running.total_units = Some(1);
    running.unit = Some("components".to_string());
    running.cancellable = Some(false);
    running.message = Some("Running component self-test.".to_string());
    let running = registry
        .0
        .update(operation_id, running)
        .map_err(SettingsOperationsCommandError::from)?;
    let _ = app.emit(SETTINGS_OPERATION_EVENT, running);

    let result = run_agent_component_self_test_domain(
        component_id,
        claude_executable_override_for_app(app).as_deref(),
    );
    let mut terminal = SettingsOperationTransition::to(result.state.clone());
    terminal.phase = Some(result.phase);
    terminal.completed_units = Some(result.completed_units);
    terminal.total_units = result.total_units;
    terminal.unit = result.unit;
    terminal.cancellable = Some(false);
    terminal.message = Some(result.message);
    terminal.error = result.error;
    let terminal = registry
        .0
        .update(operation_id, terminal)
        .map_err(SettingsOperationsCommandError::from)?;
    let _ = app.emit(SETTINGS_OPERATION_EVENT, terminal.clone());
    Ok(terminal)
}

fn start_agent_component_self_test<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    component_id: &str,
    registry: &SettingsOperationRegistry,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let (operation, created) = reserve_agent_component_self_test(component_id, registry)?;
    if !created {
        return Ok(operation);
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let operation_id = operation.id.clone();
    let component_id = component_id.to_string();
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = run_agent_component_self_test_operation(&worker_app, &operation_id, &component_id);
    });
    Ok(operation)
}

#[tauri::command]
fn run_agent_component_self_test(
    app: tauri::AppHandle,
    component_id: String,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    start_agent_component_self_test(&app, &component_id, &operation_state.0)
}

#[tauri::command]
fn mcp_client_configuration(active_project_dir: Option<String>) -> McpClientConfigurationState {
    build_mcp_client_configuration(
        active_project_dir
            .as_deref()
            .filter(|project_dir| !project_dir.trim().is_empty())
            .map(Path::new),
    )
}

fn skill_command_error(
    code: &str,
    message: &str,
    detail: impl Into<String>,
) -> SettingsOperationsCommandError {
    SettingsOperationsCommandError {
        code: code.to_string(),
        message: message.to_string(),
        detail: detail.into(),
    }
}

fn configured_skill_root(
    project_root: Option<&str>,
) -> Result<PathBuf, SettingsOperationsCommandError> {
    let configured = project_root
        .filter(|root| !root.trim().is_empty())
        .map(Path::new);
    validate_skill_root(configured).map_err(|error| {
        let code = match error {
            SkillRootError::Missing => "skills.rootMissing",
            _ => "skills.rootInvalid",
        };
        skill_command_error(
            code,
            "A valid project repository root is required for skill repair.",
            error.to_string(),
        )
    })
}

fn skill_repair_error(error: SkillRepairError) -> SettingsOperationsCommandError {
    let code = match error {
        SkillRepairError::UnknownSkillId(_) => "skills.unknownSkill",
        SkillRepairError::Io { .. } => "skills.repairFailed",
    };
    skill_command_error(
        code,
        "Bundled skills could not be repaired.",
        error.to_string(),
    )
}

fn repair_bundled_skills_for_root(
    project_root: Option<&str>,
    skill_ids: &[String],
    confirmed_paths: Option<&[String]>,
    registry: &SettingsOperationRegistry,
) -> Result<SkillRepairCommandResponse, SettingsOperationsCommandError> {
    let root = configured_skill_root(project_root)?;
    let ids = skill_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let preview = preview_bundled_skill_repair(&root, &ids).map_err(skill_repair_error)?;
    let Some(confirmed_paths) = confirmed_paths else {
        return Ok(SkillRepairCommandResponse {
            preview,
            operation: None,
            report: None,
        });
    };
    let confirmed_paths = confirmed_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if confirmed_paths != preview.affected_paths {
        return Err(skill_command_error(
            "skills.confirmationMismatch",
            "Skill repair confirmation no longer matches the affected files.",
            format!(
                "expected [{}], received [{}]",
                preview
                    .affected_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                confirmed_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }

    let target_id = preview.skill_ids.join(",");
    let operation = registry
        .start(
            SettingsOperationKind::SkillRepair,
            target_id,
            Some(preview.affected_paths.len() as u64),
            Some("files".to_string()),
        )
        .map_err(SettingsOperationsCommandError::from)?;
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("repairing".to_string());
    running.cancellable = Some(false);
    running.message = Some("Repairing confirmed bundled skill files.".to_string());
    registry
        .update(&operation.id, running)
        .map_err(SettingsOperationsCommandError::from)?;

    let confirmation = SkillRepairConfirmation {
        affected_paths: confirmed_paths,
    };
    let report = match repair_bundled_skill_files(&root, &ids, Some(&confirmation)) {
        Ok(SkillRepairOutcome::Repaired(report)) => report,
        Ok(SkillRepairOutcome::ConfirmationRequired(next_preview)) => {
            let detail = format!(
                "affected paths changed to [{}]",
                next_preview
                    .affected_paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
            failed.phase = Some("confirmation_changed".to_string());
            failed.cancellable = Some(false);
            failed.message = Some("Skill repair confirmation changed before writing.".to_string());
            failed.error = Some(SettingsOperationError {
                code: "skills.confirmationMismatch".to_string(),
                message: "Skill repair confirmation no longer matches the affected files."
                    .to_string(),
                recovery_action: Some("Review the affected files and confirm again.".to_string()),
                detail: Some(detail.clone()),
            });
            registry
                .update(&operation.id, failed)
                .map_err(SettingsOperationsCommandError::from)?;
            return Err(skill_command_error(
                "skills.confirmationMismatch",
                "Skill repair confirmation no longer matches the affected files.",
                detail,
            ));
        }
        Err(error) => {
            let command_error = skill_repair_error(error);
            let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
            failed.phase = Some("failed".to_string());
            failed.cancellable = Some(false);
            failed.message = Some(command_error.message.clone());
            failed.error = Some(SettingsOperationError {
                code: command_error.code.clone(),
                message: command_error.message.clone(),
                recovery_action: Some("Review diagnostics and retry the repair.".to_string()),
                detail: Some(command_error.detail.clone()),
            });
            registry
                .update(&operation.id, failed)
                .map_err(SettingsOperationsCommandError::from)?;
            return Err(command_error);
        }
    };

    let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
    succeeded.phase = Some("verified".to_string());
    succeeded.completed_units = Some(report.repaired_paths.len() as u64);
    succeeded.total_units = Some(preview.affected_paths.len() as u64);
    succeeded.unit = Some("files".to_string());
    succeeded.cancellable = Some(false);
    succeeded.message = Some("Bundled skills were repaired and verified.".to_string());
    let operation = registry
        .update(&operation.id, succeeded)
        .map_err(SettingsOperationsCommandError::from)?;

    Ok(SkillRepairCommandResponse {
        preview,
        operation: Some(operation),
        report: Some(report),
    })
}

#[tauri::command]
fn repair_bundled_skills(
    project_root: Option<String>,
    skill_ids: Vec<String>,
    confirmed_paths: Option<Vec<String>>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<SkillRepairCommandResponse, SettingsOperationsCommandError> {
    repair_bundled_skills_for_root(
        project_root.as_deref(),
        &skill_ids,
        confirmed_paths.as_deref(),
        &operation_state.0,
    )
}

#[tauri::command]
fn get_render_system_health() -> RenderSystemHealth {
    build_render_system_health()
}

fn reserve_render_system_health_operation(
    registry: &SettingsOperationRegistry,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::HealthCheck,
            render_health_target_id(),
            Some(4),
            Some("components".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued render-system health check.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

#[tauri::command]
fn check_render_system(
    app: tauri::AppHandle,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let (operation, created) = reserve_render_system_health_operation(&operation_state.0)?;
    if !created {
        return Ok(operation);
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_render_system_health_operation(app, operation_id);
    });
    Ok(operation)
}

fn run_render_system_health_operation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    operation_id: String,
) {
    let registry = app.state::<SettingsOperationRegistryState>();
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("checking".to_string());
    running.completed_units = Some(0);
    running.total_units = Some(4);
    running.unit = Some("components".to_string());
    running.cancellable = Some(false);
    running.message = Some("Checking the required render system.".to_string());
    match registry.0.update(&operation_id, running) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            match registry.0.project_persistence_failure(
                &operation_id,
                "settings.renderHealth.runningPersistenceFailed",
                "The render-system check could not save its running state.",
                &error.to_string(),
            ) {
                Ok(operation) => {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                Err(projection_error) => {
                    eprintln!(
                        "failed to project running render health failure {operation_id}: {projection_error}"
                    );
                }
            }
            return;
        }
    }

    let health = build_render_system_health();
    let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
    succeeded.phase = Some(if health.composition_ready {
        "ready".to_string()
    } else {
        "action_required".to_string()
    });
    succeeded.completed_units = Some(4);
    succeeded.total_units = Some(4);
    succeeded.unit = Some("components".to_string());
    succeeded.cancellable = Some(false);
    succeeded.message = Some(if health.composition_ready {
        if health.native_delivery_degraded || health.compatibility_degraded {
            "Composition is ready; one or more optional delivery paths are degraded.".to_string()
        } else {
            "Render system is ready.".to_string()
        }
    } else {
        "Render system check completed; required composition action is needed.".to_string()
    });
    match registry.0.update(&operation_id, succeeded) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            match registry.0.project_persistence_failure(
                &operation_id,
                "settings.renderHealth.terminalPersistenceFailed",
                "The render-system check finished, but its result could not be saved.",
                &error.to_string(),
            ) {
                Ok(operation) => {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
                }
                Err(projection_error) => {
                    eprintln!(
                        "failed to project completed render health failure {operation_id}: {projection_error}"
                    );
                }
            }
        }
    }
}

#[tauri::command]
fn list_settings_operations(
    state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<Vec<SettingsOperation>, SettingsOperationsCommandError> {
    state
        .0
        .list_recent()
        .map_err(SettingsOperationsCommandError::from)
}

fn model_operation_command_error(
    code: &str,
    message: &str,
    detail: impl Into<String>,
) -> SettingsOperationsCommandError {
    SettingsOperationsCommandError {
        code: code.to_string(),
        message: message.to_string(),
        detail: detail.into(),
    }
}

fn model_store_command_error(error: ModelStoreError) -> SettingsOperationsCommandError {
    model_operation_command_error(
        error.stable_code(),
        "The model operation could not be started.",
        error.to_string(),
    )
}

fn persist_model_operation_failure(
    registry: &SettingsOperationRegistry,
    operation_id: &str,
    error: &ModelStoreError,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
    failed.phase = Some("failed".to_string());
    failed.message = Some("Model download could not start.".to_string());
    failed.error = Some(SettingsOperationError {
        code: error.stable_code().to_string(),
        message: "Model download could not start.".to_string(),
        recovery_action: Some("Retry the download.".to_string()),
        detail: Some(error.to_string()),
    });
    registry
        .update(operation_id, failed)
        .map_err(SettingsOperationsCommandError::from)
}

fn persist_cancelled_operation(
    registry: &SettingsOperationRegistry,
    operation_id: &str,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let mut cancelled = SettingsOperationTransition::to(SettingsOperationState::Cancelled);
    cancelled.phase = Some("cancelled".to_string());
    cancelled.message = Some("Download cancelled.".to_string());
    registry
        .update(operation_id, cancelled)
        .map_err(SettingsOperationsCommandError::from)
}

fn terminal_persistence_warning_projection(
    operation: &SettingsOperation,
    detail: String,
) -> SettingsOperation {
    let mut projection = operation.clone();
    projection.phase = "ready_with_persistence_warning".to_string();
    projection.state = SettingsOperationState::Succeeded;
    projection.cancellable = false;
    projection.message =
        "Model is ready, but operation history could not be persisted.".to_string();
    projection.error = Some(SettingsOperationError {
        code: "settings.operation.terminalPersistenceFailed".to_string(),
        message: "The model is ready, but operation history could not be saved.".to_string(),
        recovery_action: Some("Restart Video Creater to reconcile history.".to_string()),
        detail: Some(detail),
    });
    projection
}

fn project_speech_operation_persistence_failure<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &ProductionSpeechModelStore,
    operation_id: &str,
    code: &str,
    message: &str,
    detail: String,
    store_error: Option<&SpeechModelError>,
) {
    if let Some(store_error) = store_error {
        if let Err(error) = store.record_error(store_error) {
            eprintln!("failed to persist speech model error metadata for {operation_id}: {error}");
        }
    }
    let registry = app.state::<SettingsOperationRegistryState>();
    match registry
        .0
        .project_persistence_failure(operation_id, code, message, &detail)
    {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            eprintln!("failed to project speech model persistence failure {operation_id}: {error}");
        }
    }
}

#[tauri::command]
fn get_production_speech_model_status(
    state: tauri::State<'_, ProductionSpeechModelStoreState>,
) -> ProductionSpeechModelStatus {
    state.0.status()
}

#[tauri::command]
fn download_production_speech_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProductionSpeechModelStoreState>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
    coordinator_state: tauri::State<'_, ModelOperationCoordinatorState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let store = state.0.clone();
    let status = store.status();
    let _coordinator = coordinator_state.0.lock().map_err(|_| {
        model_operation_command_error(
            "settings.operation.coordinatorUnavailable",
            "Speech model setup is temporarily unavailable.",
            "model operation coordinator lock failed",
        )
    })?;
    let (operation, created) = reserve_speech_model_operation(&operation_state.0, &status)?;
    if !created {
        return Ok(operation);
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());
    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_production_speech_model_download_operation(app, store, operation_id);
    });
    Ok(operation)
}

fn reserve_speech_model_operation(
    registry: &SettingsOperationRegistry,
    status: &ProductionSpeechModelStatus,
) -> Result<(SettingsOperation, bool), SettingsOperationsCommandError> {
    if !status.ready || status.last_error_code.is_some() {
        registry
            .invalidate_target_terminal_persistence_warning(
                SettingsOperationKind::SpeechModelsDownload,
                &status.model_set_id,
            )
            .map_err(SettingsOperationsCommandError::from)?;
    }
    registry
        .resolve_or_start_active_configured(
            SettingsOperationKind::SpeechModelsDownload,
            status.model_set_id.clone(),
            Some(status.total_bytes),
            Some("bytes".to_string()),
            |operation| {
                operation.cancellable = false;
                operation.message = "Queued verified speech model setup.".to_string();
            },
        )
        .map_err(SettingsOperationsCommandError::from)
}

fn speech_model_command_error(
    error: SpeechModelError,
    message: &str,
) -> SettingsOperationsCommandError {
    model_operation_command_error(error.stable_code(), message, error.to_string())
}

fn run_production_speech_model_download_operation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    store: ProductionSpeechModelStore,
    operation_id: String,
) {
    let registry = app.state::<SettingsOperationRegistryState>();
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("downloading".to_string());
    running.cancellable = Some(false);
    running.message = Some("Downloading pinned speech analysis model files.".to_string());
    match registry.0.update(&operation_id, running) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            let detail = error.to_string();
            let store_error = SpeechModelError::ProgressPersistence(detail.clone());
            project_speech_operation_persistence_failure(
                &app,
                &store,
                &operation_id,
                "settings.operation.runningPersistenceFailed",
                "Speech model setup could not save its running state.",
                detail,
                Some(&store_error),
            );
            return;
        }
    }

    let progress_app = app.clone();
    let progress_operation_id = operation_id.clone();
    let result = store.download_and_verify_with_observer(move |snapshot| {
        let registry = progress_app.state::<SettingsOperationRegistryState>();
        let mut progress = SettingsOperationTransition::to(SettingsOperationState::Running);
        progress.phase = Some("downloading".to_string());
        progress.completed_units = Some(snapshot.downloaded_bytes);
        progress.total_units = Some(snapshot.total_bytes);
        progress.unit = Some("bytes".to_string());
        progress.cancellable = Some(false);
        progress.message = Some(format!(
            "Downloaded {} of {} files.",
            snapshot.downloaded_files, snapshot.total_files
        ));
        let operation = registry
            .0
            .update(&progress_operation_id, progress)
            .map_err(|error| SpeechModelError::ProgressPersistence(error.to_string()))?;
        let _ = progress_app.emit(SETTINGS_OPERATION_EVENT, operation);
        Ok(())
    });

    let (transition, operation_failed) = match result {
        Ok(status) => {
            let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
            succeeded.phase = Some("ready".to_string());
            succeeded.completed_units = Some(status.total_bytes);
            succeeded.total_units = Some(status.total_bytes);
            succeeded.unit = Some("bytes".to_string());
            succeeded.cancellable = Some(false);
            succeeded.message = Some("Speech analysis models downloaded and verified.".to_string());
            (succeeded, false)
        }
        Err(error) => {
            let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
            failed.phase = Some("failed".to_string());
            failed.cancellable = Some(false);
            failed.message = Some("Speech analysis model setup failed.".to_string());
            failed.error = Some(SettingsOperationError {
                code: error.stable_code().to_string(),
                message: "Speech analysis model setup failed.".to_string(),
                recovery_action: Some("Retry the verified download.".to_string()),
                detail: Some(error.to_string()),
            });
            (failed, true)
        }
    };
    match registry.0.update(&operation_id, transition) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            let status = store.status();
            if !operation_failed && status.ready && status.last_error_code.is_none() {
                if let Ok(durable) = registry.0.get(&operation_id) {
                    let projection =
                        terminal_persistence_warning_projection(&durable, error.to_string());
                    if let Ok(projection) =
                        registry.0.replace_in_memory_without_persisting(projection)
                    {
                        let _ = app.emit(SETTINGS_OPERATION_EVENT, projection);
                    }
                }
            } else {
                project_speech_operation_persistence_failure(
                    &app,
                    &store,
                    &operation_id,
                    "settings.operation.failurePersistenceFailed",
                    "Speech model setup failed and its terminal state could not be saved.",
                    error.to_string(),
                    None,
                );
            }
            eprintln!("failed to persist terminal speech model operation {operation_id}: {error}");
        }
    }
}

#[tauri::command]
async fn verify_production_speech_models(
    state: tauri::State<'_, ProductionSpeechModelStoreState>,
) -> Result<ProductionSpeechModelStatus, SettingsOperationsCommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || store.verify())
        .await
        .map_err(|error| {
            model_operation_command_error(
                "speechModels.verify.taskFailed",
                "Speech model verification could not finish.",
                error.to_string(),
            )
        })?
        .map_err(|error| speech_model_command_error(error, "Speech model verification failed."))
}

#[tauri::command]
async fn remove_production_speech_models(
    state: tauri::State<'_, ProductionSpeechModelStoreState>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<ProductionSpeechModelStatus, SettingsOperationsCommandError> {
    let store = state.0.clone();
    let status = tauri::async_runtime::spawn_blocking(move || store.remove())
        .await
        .map_err(|error| {
            model_operation_command_error(
                "speechModels.remove.taskFailed",
                "Speech model removal could not finish.",
                error.to_string(),
            )
        })?
        .map_err(|error| speech_model_command_error(error, "Speech model removal failed."))?;
    operation_state
        .0
        .invalidate_target(
            SettingsOperationKind::SpeechModelsDownload,
            &status.model_set_id,
        )
        .map_err(SettingsOperationsCommandError::from)?;
    Ok(status)
}

#[tauri::command]
fn download_transcription_model(
    app: tauri::AppHandle,
    model_id: String,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
    coordinator_state: tauri::State<'_, ModelOperationCoordinatorState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let store = {
        let store = model_state.0.lock().map_err(|_| {
            model_operation_command_error(
                "settings.modelStoreUnavailable",
                "Model downloads are temporarily unavailable.",
                "model store lock failed",
            )
        })?;
        store.clone()
    };
    let status = store.status(&model_id).map_err(model_store_command_error)?;
    let _coordinator = coordinator_state.0.lock().map_err(|_| {
        model_operation_command_error(
            "settings.operation.coordinatorUnavailable",
            "Model downloads are temporarily unavailable.",
            "model operation coordinator lock failed",
        )
    })?;
    let approximate_size_bytes = status.approximate_size_bytes;
    let (operation, created) = operation_state
        .0
        .resolve_or_start_active(
            SettingsOperationKind::ModelDownload,
            model_id.clone(),
            Some(approximate_size_bytes),
            Some("bytes".to_string()),
        )
        .map_err(SettingsOperationsCommandError::from)?;
    if !created {
        return Ok(operation);
    }
    let token = match store.mark_download_started(&model_id, status.total_files) {
        Ok(StartDownload::Started { token, .. }) => token,
        Ok(StartDownload::AlreadyActive(_)) => {
            let error = ModelStoreError::DownloadAlreadyActive(model_id);
            persist_model_operation_failure(&operation_state.0, &operation.id, &error)?;
            return Err(model_store_command_error(error));
        }
        Err(error) => {
            persist_model_operation_failure(&operation_state.0, &operation.id, &error)?;
            return Err(model_store_command_error(error));
        }
    };
    let _ = app.emit(SETTINGS_OPERATION_EVENT, operation.clone());

    let operation_id = operation.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_transcription_model_download_operation(
            app,
            store,
            model_id,
            operation_id,
            approximate_size_bytes,
            token,
        );
    });
    Ok(operation)
}

fn run_transcription_model_download_operation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    store: TranscriptionModelStore,
    model_id: String,
    operation_id: String,
    approximate_size_bytes: u64,
    token: DownloadToken,
) {
    let registry = app.state::<SettingsOperationRegistryState>();
    let coordinator = app.state::<ModelOperationCoordinatorState>();
    {
        let _coordinator = match coordinator.0.lock() {
            Ok(coordinator) => coordinator,
            Err(_) => {
                let _ = store.cancel_download_if_active(&model_id);
                return;
            }
        };
        let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
        running.phase = Some("downloading".to_string());
        running.message = Some("Downloading model files.".to_string());
        match registry.0.update(&operation_id, running) {
            Ok(operation) => {
                let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
            }
            Err(error) => {
                let _ = store.cancel_download_if_active(&model_id);
                if registry
                    .0
                    .get(&operation_id)
                    .is_ok_and(|operation| operation.state == SettingsOperationState::Cancelling)
                {
                    let _ = persist_cancelled_operation(&registry.0, &operation_id);
                } else {
                    let persistence_error = ModelStoreError::ProgressPersistence(error.to_string());
                    let _ = persist_model_operation_failure(
                        &registry.0,
                        &operation_id,
                        &persistence_error,
                    );
                    eprintln!("failed to persist running model operation {operation_id}: {error}");
                }
                return;
            }
        }
    }

    let progress_app = app.clone();
    let progress_operation_id = operation_id.clone();
    let progress_publication = Mutex::new(ModelDownloadProgressPublication::new(Instant::now()));
    let result = store.download_reserved_with_observer_and_publisher(
        &model_id,
        token,
        move |snapshot| {
            let should_publish = progress_publication
                .lock()
                .map_err(|_| {
                    ModelStoreError::ProgressPersistence(
                        "model download progress publisher lock failed".to_string(),
                    )
                })?
                .should_publish(snapshot.downloaded_bytes, Instant::now());
            if !should_publish {
                return Ok(());
            }
            let registry = progress_app.state::<SettingsOperationRegistryState>();
            let mut transition = SettingsOperationTransition::to(SettingsOperationState::Running);
            transition.phase = Some("downloading".to_string());
            transition.completed_units = Some(snapshot.downloaded_bytes);
            transition.total_units = Some(
                snapshot
                    .total_bytes
                    .max(snapshot.downloaded_bytes)
                    .max(approximate_size_bytes),
            );
            transition.unit = Some("bytes".to_string());
            transition.message = Some(format!(
                "Downloaded {} of {} files.",
                snapshot.downloaded_files, snapshot.total_files
            ));
            let operation = registry
                .0
                .update(&progress_operation_id, transition)
                .map_err(|error| ModelStoreError::ProgressPersistence(error.to_string()))?;
            let _ = progress_app.emit(SETTINGS_OPERATION_EVENT, operation);
            Ok(())
        },
        |pending| {
            let coordinator = app.state::<ModelOperationCoordinatorState>();
            let _coordinator = coordinator.0.lock().map_err(|_| {
                ModelStoreError::ProgressPersistence(
                    "model operation coordinator lock failed".to_string(),
                )
            })?;
            let operation = registry
                .0
                .get(&operation_id)
                .map_err(|error| ModelStoreError::ProgressPersistence(error.to_string()))?;
            if operation.state != SettingsOperationState::Running {
                return Ok(false);
            }
            if !pending.publish()? {
                return Ok(false);
            }
            let mut succeeded = SettingsOperationTransition::to(SettingsOperationState::Succeeded);
            succeeded.phase = Some("ready".to_string());
            succeeded.completed_units = Some(operation.completed_units);
            succeeded.total_units = Some(operation.completed_units);
            succeeded.unit = Some("bytes".to_string());
            succeeded.message = Some("Model downloaded and verified.".to_string());
            let succeeded = registry
                .0
                .update(&operation_id, succeeded)
                .or_else(|first_error| {
                    registry
                        .0
                        .update(&operation_id, {
                            let mut retry =
                                SettingsOperationTransition::to(SettingsOperationState::Succeeded);
                            retry.phase = Some("ready".to_string());
                            retry.completed_units = Some(operation.completed_units);
                            retry.total_units = Some(operation.completed_units);
                            retry.unit = Some("bytes".to_string());
                            retry.message = Some("Model downloaded and verified.".to_string());
                            retry
                        })
                        .map_err(|retry_error| {
                            ModelStoreError::TerminalPersistence(format!(
                                "first attempt: {first_error}; retry: {retry_error}"
                            ))
                        })
                })?;
            let _ = app.emit(SETTINGS_OPERATION_EVENT, succeeded);
            Ok(true)
        },
    );

    let mut transition = match result {
        Ok(_) => return,
        Err(ModelStoreError::TerminalPersistence(error)) => {
            let durable = registry.0.get(&operation_id).ok();
            if let Some(durable) = durable {
                let projection = terminal_persistence_warning_projection(&durable, error.clone());
                if let Ok(projection) = registry.0.replace_in_memory_without_persisting(projection)
                {
                    let _ = app.emit(SETTINGS_OPERATION_EVENT, projection);
                }
            }
            eprintln!(
                "model {model_id} published but terminal operation {operation_id} was not persisted: {error}"
            );
            return;
        }
        Err(error) if error.stable_code() == "model.download.cancelled" => {
            let mut cancelled = SettingsOperationTransition::to(SettingsOperationState::Cancelled);
            cancelled.phase = Some("cancelled".to_string());
            cancelled.message = Some("Download cancelled.".to_string());
            cancelled
        }
        Err(error) => {
            let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
            failed.phase = Some("failed".to_string());
            failed.message = Some("Model download failed.".to_string());
            failed.error = Some(SettingsOperationError {
                code: error.stable_code().to_string(),
                message: "Model download failed.".to_string(),
                recovery_action: Some("Retry the download.".to_string()),
                detail: Some(error.to_string()),
            });
            failed
        }
    };
    let _coordinator = match coordinator.0.lock() {
        Ok(coordinator) => coordinator,
        Err(_) => return,
    };
    if registry
        .0
        .get(&operation_id)
        .is_ok_and(|operation| operation.state == SettingsOperationState::Cancelling)
    {
        transition = SettingsOperationTransition::to(SettingsOperationState::Cancelled);
        transition.phase = Some("cancelled".to_string());
        transition.message = Some("Download cancelled.".to_string());
    }
    match registry.0.update(&operation_id, transition) {
        Ok(operation) => {
            let _ = app.emit(SETTINGS_OPERATION_EVENT, operation);
        }
        Err(error) => {
            eprintln!("failed to persist terminal model operation {operation_id}: {error}");
        }
    }
}

#[tauri::command]
fn cancel_settings_operation(
    app: tauri::AppHandle,
    operation_id: String,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
    operation_state: tauri::State<'_, SettingsOperationRegistryState>,
    coordinator_state: tauri::State<'_, ModelOperationCoordinatorState>,
) -> Result<SettingsOperation, SettingsOperationsCommandError> {
    let _coordinator = coordinator_state.0.lock().map_err(|_| {
        model_operation_command_error(
            "settings.operation.coordinatorUnavailable",
            "Cancellation is temporarily unavailable.",
            "model operation coordinator lock failed",
        )
    })?;
    let operation = operation_state
        .0
        .get(&operation_id)
        .map_err(SettingsOperationsCommandError::from)?;
    if matches!(
        operation.state,
        SettingsOperationState::Succeeded
            | SettingsOperationState::Failed
            | SettingsOperationState::Cancelled
    ) {
        return Err(model_operation_command_error(
            "settings.operation.notCancellable",
            "The model download has already completed.",
            "the visible operation state is terminal",
        ));
    }
    if operation.kind == SettingsOperationKind::ModelDownload {
        let store = model_state
            .0
            .lock()
            .map_err(|_| {
                model_operation_command_error(
                    "settings.modelStoreUnavailable",
                    "Cancellation is temporarily unavailable.",
                    "model store lock failed",
                )
            })?
            .clone();
        if !store
            .download_is_active(&operation.target_id)
            .map_err(model_store_command_error)?
            && store
                .status(&operation.target_id)
                .map_err(model_store_command_error)?
                .install_status
                == ModelInstallStatus::Ready
        {
            return Err(model_operation_command_error(
                "settings.operation.notCancellable",
                "The model download has already completed.",
                "the model is ready and has no active download owner",
            ));
        }
    }
    let cancelling = operation_state
        .0
        .request_cancel(&operation_id)
        .map_err(SettingsOperationsCommandError::from)?;
    if operation.kind == SettingsOperationKind::ModelDownload {
        let store = model_state
            .0
            .lock()
            .map_err(|_| {
                model_operation_command_error(
                    "settings.modelStoreUnavailable",
                    "Cancellation is temporarily unavailable.",
                    "model store lock failed",
                )
            })?
            .clone();
        store
            .cancel_download_if_active(&operation.target_id)
            .map_err(model_store_command_error)?;
    }
    let _ = app.emit(SETTINGS_OPERATION_EVENT, cancelling.clone());
    Ok(cancelling)
}

#[tauri::command]
async fn import_transcription_model(
    model_id: String,
    source_path: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = {
        let store = state
            .0
            .lock()
            .map_err(|_| "model store lock failed".to_string())?;
        store.clone()
    };

    tauri::async_runtime::spawn_blocking(move || {
        store
            .import_model(&model_id, Path::new(&source_path))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("model import task failed: {error}"))?
}

#[tauri::command]
fn cancel_model_download(
    model_id: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = {
        let store = state
            .0
            .lock()
            .map_err(|_| "model store lock failed".to_string())?;
        store.clone()
    };

    store
        .cancel_download(&model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn verify_transcription_model(
    model_id: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;

    store.verify(&model_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_transcription_model(
    model_id: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;

    store.remove(&model_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_active_transcription_model(
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;

    store.active_status().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_active_transcription_model(
    model_id: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;

    store
        .set_active_model(&model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_transcription_runtime_status(
    model_id: Option<String>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<RuntimeSelection, String> {
    current_transcription_runtime_selection(model_id.as_deref(), model_state)
}

fn codex_project_for_turn(
    project: VideoProject,
    project_dir: Option<&Path>,
) -> Result<VideoProject, String> {
    match project_dir {
        Some(project_dir) => load_split_project(project_dir).map_err(|error| error.to_string()),
        None => Ok(project),
    }
}

/// Stops the project's in-flight Codex turn; `false` means none was running.
fn cancel_codex_turn_for_project(
    project_root: Option<String>,
    project_dir: Option<String>,
) -> Result<bool, String> {
    let root = resolve_codex_project_root(project_root.as_deref())?;
    let project_dir = project_dir
        .as_deref()
        .map(resolve_project_dir)
        .transpose()?;
    request_codex_project_turn_cancellation(&root, project_dir.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_codex_video_edit_for_project(
    project_root: Option<String>,
    project_dir: Option<String>,
) -> Result<bool, String> {
    cancel_codex_turn_for_project(project_root, project_dir)
}

/// Stops an in-flight conversation turn before it returns. The stopped start
/// command fails with "app-server turn was interrupted" and persists nothing.
#[tauri::command]
fn cancel_codex_conversation_edit_for_project(
    project_root: Option<String>,
    project_dir: Option<String>,
) -> Result<bool, String> {
    cancel_codex_turn_for_project(project_root, project_dir)
}

#[tauri::command]
async fn start_codex_video_edit_for_project(
    project_root: Option<String>,
    project_dir: Option<String>,
    project: VideoProject,
    request: EditJobRequest,
) -> Result<CodexVideoEditCommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_codex_project_root(project_root.as_deref())?;
        let project_dir = project_dir
            .as_deref()
            .map(resolve_project_dir)
            .transpose()?;
        let skills = load_project_skill_bundle(&root).map_err(|error| error.to_string())?;
        let (_cancellation_guard, cancellation) =
            register_codex_project_turn(&root, project_dir.as_deref())
                .map_err(|error| error.to_string())?;
        let deadline = codex_app_server_deadline();
        let command = bundled_codex_app_server_command().map_err(|error| error.to_string())?;
        let mut transport = StdioCodexAppServerTransport::spawn_until(
            &command,
            None,
            deadline,
            Some(&cancellation),
        )
        .map_err(|error| error.to_string())?;
        let (mut project, expected_revision) = {
            let _project_lease = project_dir
                .as_deref()
                .map(acquire_split_project_mutation_lease)
                .transpose()?;
            let project = codex_project_for_turn(project, project_dir.as_deref())?;
            let revision = project.content_revision;
            (project, revision)
        };
        let cwd = root
            .to_str()
            .ok_or_else(|| "project root must be valid UTF-8".to_string())?;
        let saved_request = request.clone();
        let result = start_codex_video_edit_turn_unpersisted_until(
            &mut transport,
            1,
            cwd,
            &mut project,
            request,
            &skills,
            project_dir.as_deref(),
            deadline,
            Some(&cancellation),
        )
        .map_err(|error| error.to_string())?;

        if let Some(project_dir) = project_dir.as_deref() {
            project = persist_codex_turn_for_project(
                project_dir,
                expected_revision,
                &result.thread_id,
                AppServerConversationTurn {
                    turn_id: result
                        .turn_response
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    turn_status: result
                        .turn_response
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    prompt: saved_request.prompt.clone(),
                    created_at: saved_request.created_at.clone(),
                    request: serde_json::to_value(&saved_request)
                        .map_err(|error| error.to_string())?,
                    thread_response: result.thread_response.clone(),
                    turn_response: result.turn_response.clone(),
                    has_proposal: result.proposal.is_some(),
                    provider: None,
                    provider_session_id: None,
                },
            )?;
        }

        Ok(CodexVideoEditCommandResult {
            project,
            thread_id: result.thread_id,
            thread_response: result.thread_response,
            turn_response: result.turn_response,
            proposal: result.proposal,
            proposal_validation_issues: result.proposal_validation_issues,
        })
    })
    .await
    .map_err(|error| format!("Codex app-server task failed: {error}"))?
}

/// Stores the Codex thread id on the canonical project when its revision still
/// matches the turn's starting revision, then records the turn in the
/// app-server conversation history.
fn persist_codex_turn_for_project(
    project_dir: &Path,
    expected_revision: u64,
    thread_id: &str,
    turn: AppServerConversationTurn,
) -> Result<VideoProject, String> {
    let _project_lease = acquire_split_project_mutation_lease(project_dir)?;
    let mut canonical = load_split_project(project_dir).map_err(|error| error.to_string())?;
    if canonical.content_revision != expected_revision {
        return Err(format!(
            "project revision conflict: expected {expected_revision}, but canonical revision is {}",
            canonical.content_revision
        ));
    }
    canonical.codex_thread_id = Some(thread_id.to_string());
    let project = replace_split_project_if_revision(project_dir, canonical, expected_revision)
        .map_err(|error| error.to_string())?
        .project;
    record_app_server_conversation_turn(project_dir, &project.id, thread_id, turn)
        .map_err(|error| error.to_string())?;
    Ok(project)
}

#[tauri::command]
async fn start_codex_conversation_edit_for_project(
    app: tauri::AppHandle,
    project_root: Option<String>,
    project_dir: Option<String>,
    project: VideoProject,
    request: CodexConversationEditRequest,
) -> Result<CodexConversationEditCommandResult, String> {
    let agent_preferences = agent_turn_preferences_for_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_codex_project_root(project_root.as_deref())?;
        let project_dir = project_dir
            .as_deref()
            .map(resolve_project_dir)
            .transpose()?;
        let (mut project, expected_revision) = {
            let _project_lease = project_dir
                .as_deref()
                .map(acquire_split_project_mutation_lease)
                .transpose()?;
            let project = codex_project_for_turn(project, project_dir.as_deref())?;
            let revision = project.content_revision;
            (project, revision)
        };
        // Reject an invalid prompt or stale focus before starting Codex.
        let request = request
            .validate(&project)
            .map_err(|error| error.to_string())?;
        let skills = load_project_skill_bundle(&root).map_err(|error| error.to_string())?;
        let (cancellation_guard, cancellation) =
            register_codex_project_turn(&root, project_dir.as_deref())
                .map_err(|error| error.to_string())?;
        let deadline = codex_app_server_deadline();
        let saved_request = request.clone();

        // The backend is chosen inside the command: its name, signature and result shape are
        // the frontend's contract and none of them mention an agent.
        // Claude readiness is the zero-token probe, not just "the binary exists": Automatic
        // prefers Claude, so an installed-but-signed-out `claude` has to yield to Codex here
        // rather than fail in the child.
        let (selection, backend_plan) = plan_agent_turn(
            &agent_preferences,
            bundled_codex_app_server_command().is_ok(),
            claude_turn_readiness(agent_preferences.claude_executable_arg()),
            bundled_mcp_server_executable(),
        )
        .map_err(|error| error.to_string())?;

        let turn_request = build_agent_conversation_request(
            &project,
            &request,
            &root,
            project_dir.as_deref(),
            &skills,
            agent_session_for_next_turn(selection.kind, &project, project_dir.as_deref())
                .map_err(|error| error.to_string())?,
        );

        let outcome = match backend_plan {
            AgentTurnBackendPlan::Codex => {
                let command =
                    bundled_codex_app_server_command().map_err(|error| error.to_string())?;
                let mut app_server = StdioCodexAppServerTransport::spawn_until(
                    &command,
                    None,
                    deadline,
                    Some(&cancellation),
                )
                .map_err(|error| error.to_string())?;
                CodexTurnTransport::new(&mut app_server, 1).run_conversation_turn(
                    &turn_request,
                    deadline,
                    Some(&cancellation),
                )
            }
            AgentTurnBackendPlan::Claude(mut transport) => {
                transport.run_conversation_turn(&turn_request, deadline, Some(&cancellation))
            }
        }
        .map_err(|error| error.to_string())?;

        // A Stop that landed before this point withholds the proposal and history.
        close_codex_turn_cancellation(cancellation_guard, &cancellation)
            .map_err(|error| error.to_string())?;

        let turn =
            finish_agent_conversation_turn(selection.kind, &project, &saved_request, &outcome);

        // `codex_thread_id` keeps its Codex-only meaning, so a Claude turn neither sets it nor
        // overwrites the thread a chat already has. Claude's handle rides
        // `provider_session_id` instead, which is what the next turn resumes from.
        let history_thread_id = match selection.kind {
            AgentBackendKind::Codex => {
                project.codex_thread_id = Some(turn.thread_id.clone());
                turn.thread_id.clone()
            }
            AgentBackendKind::Claude => project
                .codex_thread_id
                .clone()
                .unwrap_or_else(|| turn.thread_id.clone()),
        };

        if let Some(project_dir) = project_dir.as_deref() {
            project = persist_codex_turn_for_project(
                project_dir,
                expected_revision,
                &history_thread_id,
                turn.record,
            )?;
        }

        Ok(CodexConversationEditCommandResult {
            project,
            thread_id: turn.thread_id,
            thread_response: turn.thread_response,
            turn_response: turn.turn_response,
            proposal: turn.proposal,
            prepared_proposal: turn.prepared_proposal,
            proposal_validation_issues: turn.proposal_validation_issues,
        })
    })
    .await
    .map_err(|error| format!("Codex app-server task failed: {error}"))?
}

/// Re-prepares the proposal under the project lease and applies the exact
/// bundle atomically; review-level bundles need `review_approved`.
#[tauri::command]
async fn apply_codex_conversation_proposal(
    project_dir: String,
    proposal: CodexConversationEditProposal,
    action_ids: Vec<String>,
    review_approved: bool,
    session_id: Option<String>,
) -> Result<CodexConversationApplyResult, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("Codex apply", &queue_dir, move |project_dir| {
        apply_prepared_codex_conversation_proposal(
            &project_dir,
            CodexConversationApplyRequest {
                proposal,
                action_ids,
                review_approved,
                session_id,
            },
        )
        .map_err(|error| error.to_string())
    })
    .await
}

/// Undoes the latest agent batch, or reports why Undo is unavailable. Generations
/// the batch started that are still running are cancelled.
#[tauri::command]
async fn undo_latest_codex_conversation_edit(
    project_dir: String,
    history_entry_id: Option<String>,
) -> Result<ProjectAgentUndoOutcome, String> {
    let queue_dir = project_dir.clone();
    run_project_write_command("Codex undo", &queue_dir, move |project_dir| {
        undo_codex_conversation_edit(&project_dir, history_entry_id.as_deref())
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
fn list_codex_local_tools() -> Vec<CodexLocalToolDescriptor> {
    list_codex_local_tools_impl()
}

#[tauri::command]
async fn call_codex_local_tool(
    project_dir: Option<String>,
    project: VideoProject,
    tool_name: String,
    args: serde_json::Value,
) -> Result<CodexLocalToolCallResult, String> {
    run_blocking_command("Codex local tool", move || {
        call_codex_local_tool_blocking(project_dir, project, tool_name, args)
    })
    .await
}

fn call_codex_local_tool_blocking(
    project_dir: Option<String>,
    project: VideoProject,
    tool_name: String,
    args: serde_json::Value,
) -> Result<CodexLocalToolCallResult, String> {
    let project_dir = project_dir
        .as_deref()
        .map(resolve_project_dir)
        .transpose()?;
    let _project_lease = project_dir
        .as_deref()
        .map(acquire_split_project_mutation_lease)
        .transpose()?;
    let project = codex_project_for_turn(project, project_dir.as_deref())?;
    call_codex_local_tool_impl(&project, &tool_name, args).map_err(|error| error.to_string())
}

fn resolve_project_dir(project_dir: &str) -> Result<PathBuf, String> {
    if project_dir.trim().is_empty() {
        return Err("project directory cannot be empty".to_string());
    }

    let path = PathBuf::from(project_dir);
    if !path.is_absolute() {
        return Err("project directory must be an absolute path".to_string());
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("project directory cannot contain parent traversal".to_string());
    }

    canonicalize_project_dir(&path)
}

fn pipeline_errors_to_string(errors: Vec<PipelineError>) -> String {
    errors
        .into_iter()
        .map(|error| {
            let details = if error.details.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    error
                        .details
                        .into_iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                "{}: {} Fix: {}{}",
                error.path, error.message, error.fix, details
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn resolve_codex_project_root(project_root: Option<&str>) -> Result<PathBuf, String> {
    let root = match project_root {
        Some(project_root) if !project_root.trim().is_empty() => PathBuf::from(project_root),
        _ => std::env::current_dir()
            .map_err(|error| format!("failed to resolve current project root: {error}"))?,
    };

    if !root.is_absolute() {
        return Err("project root must be an absolute path".to_string());
    }

    if root
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("project root cannot contain parent traversal".to_string());
    }

    root.canonicalize()
        .map_err(|error| format!("failed to canonicalize project root: {error}"))
}

fn canonicalize_project_dir(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize project directory: {error}"));
    }

    let parent = path
        .parent()
        .ok_or_else(|| "project directory must have a parent directory".to_string())?;
    let folder_name = path
        .file_name()
        .ok_or_else(|| "project directory must include a folder name".to_string())?;

    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize project parent directory: {error}"))?;

    Ok(canonical_parent.join(folder_name))
}

fn validate_generate_edit_transcription_gate(
    project: &VideoProject,
    request: &EditJobRequest,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<(), String> {
    if project
        .transcripts
        .iter()
        .any(|transcript| transcript.media_id == request.media_id)
    {
        return validate_transcription_ready_for_generate_edit(
            project,
            request,
            ModelInstallStatus::Missing,
            RuntimeSelection::Unavailable,
        )
        .map_err(|error| error.to_string());
    }

    let (install_status, runtime_selection) = {
        let store = model_state
            .0
            .lock()
            .map_err(|_| "model store lock failed".to_string())?;
        let status = store.active_status().map_err(|error| error.to_string())?;
        let runtime_selection = runtime_selection_for_model_store(&store, &status.model_id)?;
        (status.install_status, runtime_selection)
    };

    validate_transcription_ready_for_generate_edit(
        project,
        request,
        install_status,
        runtime_selection,
    )
    .map_err(|error| error.to_string())
}

fn current_transcription_runtime_selection(
    model_id: Option<&str>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<RuntimeSelection, String> {
    let (resolved_model_id, runtime_selection) = {
        let store = model_state
            .0
            .lock()
            .map_err(|_| "model store lock failed".to_string())?;
        let resolved_model_id = match model_id {
            Some(model_id) => model_id.to_string(),
            None => store.active_model_id().map_err(|error| error.to_string())?,
        };
        let runtime_selection = runtime_selection_for_model_store(&store, &resolved_model_id)?;
        (resolved_model_id, runtime_selection)
    };

    let _ = resolved_model_id;
    Ok(runtime_selection)
}

fn runtime_selection_for_model_store(
    store: &TranscriptionModelStore,
    model_id: &str,
) -> Result<RuntimeSelection, String> {
    let entry = transcription_model_catalog_entry(model_id)
        .ok_or_else(|| format!("unsupported transcription model {model_id}"))?;
    let model_dir = store
        .runtime_model_dir(entry.id, PLATFORM_TRANSCRIPTION_RUNTIME_ID)
        .map_err(|error| error.to_string())?;
    let installed_model = InstalledModel {
        model_id: entry.id.to_string(),
        model_dir,
        artifact_format: ModelArtifactFormat::from(entry.artifact_format),
        modality: ModelModality::Transcription,
        runtime_family: PLATFORM_TRANSCRIPTION_RUNTIME_ID.to_string(),
    };
    let runtime =
        PlatformTranscriptionRuntime::new(PlatformTranscriptionRuntime::default_helper_path());

    if !runtime.supports_installed_model(&installed_model) {
        return Ok(RuntimeSelection::Unavailable);
    }

    Ok(match runtime.probe_installed_model(&installed_model) {
        RuntimeCapability::Ready => RuntimeSelection::Native,
        RuntimeCapability::UnsupportedPlatform => RuntimeSelection::UnsupportedPlatform,
        RuntimeCapability::Unavailable => RuntimeSelection::Unavailable,
    })
}

const BUNDLED_SAMPLE_MEDIA_PATHS: [&str; 3] = [
    "media/input.mp4",
    "media/voiceover.m4a",
    "sample/generated/product-reveal.mp4",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledSampleManifest {
    schema_version: u32,
    files: Vec<BundledSampleManifestFile>,
}

#[derive(Deserialize)]
struct BundledSampleManifestFile {
    path: String,
    bytes: usize,
    sha256: String,
}

fn materialize_sample_project_media_from_resources(
    resource_root: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    let sample_root = resource_root.join("sample-project");
    let manifest_path = sample_root.join("manifest.json");
    let manifest: BundledSampleManifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?,
    )
    .map_err(|error| format!("invalid bundled sample manifest: {error}"))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported bundled sample manifest schema {}",
            manifest.schema_version
        ));
    }
    let expected_paths = BUNDLED_SAMPLE_MEDIA_PATHS
        .into_iter()
        .collect::<HashSet<_>>();
    let manifest_paths = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    if manifest.files.len() != expected_paths.len() || manifest_paths != expected_paths {
        return Err("bundled sample manifest file inventory does not match the application".into());
    }
    for file in manifest.files {
        let relative_path = Path::new(&file.path);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("unsafe bundled sample media path: {}", file.path));
        }
        let source = sample_root.join(relative_path);
        if !source.is_file() {
            return Err(format!(
                "bundled sample media is missing: {}",
                source.display()
            ));
        }
        let source_bytes = fs::read(&source)
            .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        if source_bytes.len() != file.bytes {
            return Err(format!("bundled sample media size mismatch: {}", file.path));
        }
        let actual_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
        if actual_sha256 != file.sha256.to_ascii_lowercase() {
            return Err(format!("bundled sample media hash mismatch: {}", file.path));
        }
        let destination = project_dir.join(relative_path);
        let parent = destination
            .parent()
            .ok_or_else(|| format!("sample media path has no parent: {}", file.path))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create sample media directory {}: {error}",
                parent.display()
            )
        })?;
        fs::write(&destination, source_bytes).map_err(|error| {
            format!(
                "failed to copy bundled sample media {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

#[tauri::command]
fn materialize_sample_project_media(
    app: tauri::AppHandle,
    project_dir: String,
) -> Result<(), String> {
    let resource_root = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?;
    let project_dir = PathBuf::from(project_dir);
    materialize_sample_project_media_from_resources(&resource_root, &project_dir)?;
    app.asset_protocol_scope()
        .allow_directory(&project_dir, true)
        .map_err(|error| format!("failed to allow sample preview directory: {error}"))?;
    Ok(())
}

/// The running desktop app, for components that outlive a single command invocation.
static DESKTOP_APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// WebKitGTK does not play media from custom URI schemes, so Linux serves webview media over
/// an authenticated loopback HTTP server that honours the asset-protocol scope.
#[cfg(target_os = "linux")]
fn with_linux_media_stream(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let authorize: video_creater_lib::media_stream_server::MediaPathAuthorizer =
        std::sync::Arc::new(|path: &Path| {
            DESKTOP_APP_HANDLE
                .get()
                .is_some_and(|app| app.asset_protocol_scope().is_allowed(path))
        });
    match video_creater_lib::media_stream_server::start_media_stream_server(authorize) {
        Ok(server) => builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("media-stream")
                .js_init_script(server.initialization_script())
                .build(),
        ),
        Err(error) => {
            eprintln!("Webview media streaming could not start: {error}");
            builder
        }
    }
}

pub fn run() {
    video_creater_lib::workflows::app_session_started_at();
    if render_health_probe_mode_requested() {
        std::process::exit(run_render_health_probe_mode());
    }
    if let Err(error) = prepare_desktop_render_runtime() {
        eprintln!("Required render runtime configuration failed: {error}");
    }
    let builder = tauri::Builder::default();
    #[cfg(target_os = "linux")]
    let builder = with_linux_media_stream(builder);
    builder
        .menu(native_menu::build_native_menu)
        .on_menu_event(|app, event| native_menu::handle_native_menu_event(app, event.id()))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let _ = DESKTOP_APP_HANDLE.set(app.handle().clone());
            app.manage("video-creater");
            let acceptance = SettingsAcceptanceState::from_environment().map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, error)
            })?;
            app.manage(SettingsAcceptanceStateHolder(acceptance));
            let main_window = app.get_webview_window("main").ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "configured main webview window was not created",
                )
            })?;
            main_window.show()?;
            main_window.set_focus()?;
            if let Err(error) = begin_desktop_render_runtime_initialization() {
                eprintln!("Required render runtime initialization could not start: {error}");
            }
            let app_data_dir = app.path().app_data_dir()?;
            let app_cache_root = app.path().app_cache_dir()?;
            app.manage(AppPreferencesState(Mutex::new(AppPreferencesStore::new(
                app_data_dir.join("settings/preferences.json"),
            ))));
            let model_root = app_data_dir.join("models");
            let transcription_model_store = TranscriptionModelStore::new(model_root.clone());
            let recovery_model_store = transcription_model_store.clone();
            let production_speech_model_store = ProductionSpeechModelStore::new(model_root.clone());
            let recovery_speech_model_store = production_speech_model_store.clone();
            let operation_registry = SettingsOperationRegistry::load_resilient(
                app_data_dir.join("settings/operations.json"),
                move |operation| {
                    settings_operation_target_is_complete(
                        operation,
                        &recovery_model_store,
                        &recovery_speech_model_store,
                    )
                },
            );
            app.manage(TranscriptionModelStoreState(Mutex::new(
                transcription_model_store,
            )));
            app.manage(ProductionSpeechModelStoreState(
                production_speech_model_store,
            ));
            app.manage(SettingsOperationRegistryState(operation_registry));
            app.manage(ProviderHealthState::default());
            app.manage(StorageInventoryRootsState {
                global_model_root: model_root,
                app_cache_root,
            });
            app.manage(ActiveProjectSessionState::default());
            app.manage(StorageProjectMutationCoordinatorState);
            app.manage(StorageCleanupPreviewRegistryState::default());
            app.manage(ModelOperationCoordinatorState(Mutex::new(
                ModelOperationCoordinator,
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            native_menu::sync_native_menu_state,
            get_app_preferences,
            update_app_preferences,
            materialize_sample_project_media,
            create_empty_project,
            save_project_to_folder,
            load_project_from_folder,
            save_split_project_to_folder,
            load_split_project_from_folder,
            load_job_progress_from_split_project_folder,
            reconcile_temporal_jobs_in_split_project_folder,
            load_settings_acceptance_project,
            create_matte_in_split_project_folder,
            list_visual_effect_catalog,
            prepare_project_preview,
            capture_canonical_preview_frame_in_split_project_folder,
            analyze_project_speech,
            get_project_speaker_registry,
            rename_project_speaker,
            recolor_project_speaker,
            assign_project_media_speaker,
            validate_split_project_folder,
            load_app_server_conversation_history_from_split_project_folder,
            load_agent_sessions_from_split_project_folder,
            apply_agent_session_action_to_split_project_folder,
            search_project_media,
            rebuild_project_search_index,
            extract_visual_frame_cache_in_split_project_folder,
            cache_timeline_filmstrip_in_split_project_folder,
            cancel_visual_frame_cache_job,
            caption_visual_frame_cache_in_split_project_folder,
            migrate_single_file_project_to_split,
            import_media_to_project,
            apply_timeline_patch_to_project,
            apply_project_action_to_project,
            apply_project_action_to_split_project_folder,
            apply_project_actions_to_split_project_folder,
            update_project_settings_in_split_project_folder,
            complete_mock_generated_asset_in_split_project_folder,
            retry_generated_asset_output_download_in_split_project_folder,
            build_temporal_job_summary,
            build_temporal_codex_edit_start_request,
            build_temporal_generate_media_start_request,
            build_temporal_transcribe_media_start_request,
            build_temporal_export_media_start_request,
            build_temporal_export_project_bundle_start_request,
            build_temporal_export_nle_xml_start_request,
            start_temporal_workflow,
            run_generate_media_in_process,
            get_temporal_worker_environment_report,
            get_export_profile_availability_report,
            list_generation_model_catalog,
            list_provider_credential_statuses,
            set_provider_credential,
            delete_provider_credential,
            get_provider_account_status,
            get_provider_health,
            refresh_provider_health,
            get_update_health,
            get_platform_info,
            get_remote_access_status,
            set_remote_access_running,
            get_notification_capability,
            request_notification_permission,
            analyze_media_for_edit_in_split_project_folder,
            build_temporal_start_result_action,
            build_temporal_generate_media_failure_actions,
            cancel_generate_media_in_process,
            cancel_generate_media_provider_request_in_split_project_folder,
            export_nle_xml_to_split_project_folder,
            export_palmier_project_package_to_split_project_folder,
            render_webm_to_split_project_folder,
            render_media_to_split_project_folder,
            load_render_attempt_in_split_project_folder,
            recover_render_attempt_in_split_project_folder,
            read_project_snapshot_from_split_project_folder,
            load_render_pipeline_report_from_split_project_folder,
            run_preview_render_comparison_request_in_split_project_folder,
            cancel_render_job_in_split_project_folder,
            list_shader_background_templates,
            generate_one_click_edit_for_project,
            generate_spoken_semantic_multi_source_edit_for_project,
            start_codex_video_edit_for_project,
            start_codex_conversation_edit_for_project,
            apply_codex_conversation_proposal,
            undo_latest_codex_conversation_edit,
            cancel_codex_video_edit_for_project,
            cancel_codex_conversation_edit_for_project,
            list_codex_local_tools,
            call_codex_local_tool,
            get_settings_acceptance_context,
            write_settings_acceptance_checkpoint,
            write_settings_acceptance_progress,
            write_settings_acceptance_failure,
            abort_settings_acceptance_run,
            get_settings_health_snapshot,
            get_system_health_snapshot,
            refresh_system_health_section,
            get_agent_settings_health,
            get_skills_settings_health,
            get_storage_health,
            reveal_storage_inventory_item,
            reveal_export_artifact_in_split_project_folder,
            preview_storage_cleanup,
            run_storage_cleanup,
            refresh_storage_inventory,
            run_agent_component_self_test,
            mcp_client_configuration,
            repair_bundled_skills,
            get_render_system_health,
            check_render_system,
            list_settings_operations,
            list_transcription_models,
            get_production_speech_model_status,
            download_production_speech_models,
            verify_production_speech_models,
            remove_production_speech_models,
            download_transcription_model,
            cancel_settings_operation,
            import_transcription_model,
            cancel_model_download,
            verify_transcription_model,
            remove_transcription_model,
            get_active_transcription_model,
            set_active_transcription_model,
            get_transcription_runtime_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn settings_operation_target_is_complete(
    operation: &SettingsOperation,
    model_store: &TranscriptionModelStore,
    speech_model_store: &ProductionSpeechModelStore,
) -> bool {
    match operation.kind {
        SettingsOperationKind::ModelDownload => model_store
            .status(&operation.target_id)
            .is_ok_and(|status| status.install_status == ModelInstallStatus::Ready),
        SettingsOperationKind::SpeechModelsDownload => {
            let status = speech_model_store.status();
            operation.target_id == SPEECH_ANALYSIS_MODEL_SET_ID
                && status.ready
                && status.last_error_code.is_none()
        }
        _ => false,
    }
}

fn main() {
    run();
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "temporal-worker"))]
    use super::start_temporal_workflow;
    use super::{
        attach_media_analysis_report_to_split_project_folder,
        build_temporal_codex_edit_start_request, build_temporal_export_nle_xml_start_request,
        build_temporal_generate_media_failure_actions, build_temporal_start_result_action,
        build_temporal_transcribe_media_start_request, call_codex_local_tool_blocking,
        cancel_codex_conversation_edit_for_project, cancel_generate_media_in_process_blocking,
        cancel_generate_media_provider_request_in_split_project_folder_blocking,
        cancel_generate_media_provider_request_in_split_project_folder_with_credential_resolver,
        cancel_render_job_in_split_project_folder_blocking, codex_project_for_turn,
        complete_mock_generated_asset_in_split_project_folder_blocking,
        export_nle_xml_to_split_project_folder_blocking, get_export_profile_availability_report,
        get_temporal_worker_environment_report, get_update_health, list_codex_local_tools,
        list_shader_background_templates,
        load_app_server_conversation_history_from_split_project_folder_blocking,
        notification_capability_after_permission_request,
        notification_capability_for_authorization_status, notification_capability_for_executable,
        notification_capability_for_native_settings, prepare_project_preview_blocking,
        publish_retry_output_and_load_response, publish_retry_output_with_rollback,
        resolve_codex_project_root, resolve_project_dir,
        retry_generated_asset_output_download_in_split_project_folder_blocking,
        retry_generated_asset_output_download_with_policy,
        retry_generated_asset_output_download_with_policy_and_hook,
        retry_generated_asset_output_download_with_policy_resolver_and_hooks, save_split_project,
        unavailable_notification_capability, validate_preview_render_comparison_request,
        validate_split_project_folder_blocking, write_palmier_project_package,
        CodexVideoEditCommandResult, DownloadNetworkPolicy, NativeNotificationAuthorizationStatus,
        RenderAttemptKey, RetryOutputFileOps, SystemRetryOutputFileOps,
    };
    use std::cell::Cell;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::path::Path;
    use std::thread;
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::Manager;
    use video_creater_lib::codex::turn_cancel::register_codex_project_turn;
    use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
    use video_creater_lib::generation::fal::{FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID};
    use video_creater_lib::media_inspection::{AudioWindowMetrics, VideoInspectionReport};
    use video_creater_lib::process_supervisor::CancellationSignal;
    use video_creater_lib::project::action::ProjectAction;
    use video_creater_lib::project::export_profiles::ExportProfile;
    use video_creater_lib::project::fixtures::sample_project;
    use video_creater_lib::project::model::{
        GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
        GeneratedAssetStatus, GenerationModel, JobProviderRequest, JobStatus, MediaKind,
        ProjectRenderPreviewComparisonRequest, TimelineSource,
    };
    use video_creater_lib::project::nle_export::NleXmlFormat;
    use video_creater_lib::project::split::{
        load_split_project, record_app_server_conversation_turn, AppServerConversationTurn,
        ProjectValidationIssue,
    };
    use video_creater_lib::render_pipeline::probe::{AudioProbe, MediaProbe, VideoProbe};
    use video_creater_lib::settings::health::SettingsHealthState;
    use video_creater_lib::settings::operations::{
        SettingsOperation, SettingsOperationError, SettingsOperationKind, SettingsOperationState,
        SettingsOperationTransition,
    };

    #[test]
    fn model_download_progress_publication_is_bounded_and_responsive() {
        let started_at = std::time::Instant::now();
        let mut publication = super::ModelDownloadProgressPublication::new(started_at);

        assert!(publication.should_publish(0, started_at));
        assert!(!publication.should_publish(
            64 * 1024,
            started_at + std::time::Duration::from_millis(100)
        ));
        assert!(publication.should_publish(
            1024 * 1024,
            started_at + std::time::Duration::from_millis(200)
        ));
        assert!(!publication.should_publish(
            1024 * 1024 + 64 * 1024,
            started_at + std::time::Duration::from_millis(300)
        ));
        assert!(publication.should_publish(
            1024 * 1024 + 64 * 1024,
            started_at + std::time::Duration::from_millis(1_200)
        ));
    }

    fn assert_provider_payload_secret_free(value: &serde_json::Value, fixture_secret: &str) {
        match value {
            serde_json::Value::Object(entries) => {
                for (key, child) in entries {
                    assert!(
                        !matches!(
                            key.to_ascii_lowercase().as_str(),
                            "secret" | "token" | "apikey" | "credentialvalue"
                        ),
                        "secret-shaped key crossed provider IPC: {key}"
                    );
                    assert_provider_payload_secret_free(child, fixture_secret);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    assert_provider_payload_secret_free(child, fixture_secret);
                }
            }
            serde_json::Value::String(value) => assert!(
                !value.contains(fixture_secret),
                "raw provider credential crossed a serialized boundary"
            ),
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            }
        }
    }

    fn save_split_project_to_folder(
        project_dir: String,
        project: video_creater_lib::project::model::VideoProject,
    ) -> Result<video_creater_lib::project::split::ProjectWriteReport, String> {
        let project_dir = super::resolve_project_dir(&project_dir)?;
        video_creater_lib::project::split::save_split_project(&project_dir, &project)
            .map(|result| result.report)
            .map_err(|error| error.to_string())
    }

    fn load_split_project_from_folder(
        project_dir: String,
    ) -> Result<video_creater_lib::project::model::VideoProject, String> {
        super::load_split_project_from_folder_impl(project_dir)
    }
    use video_creater_lib::transcription::model::parakeet_v3_catalog_entry;
    use video_creater_lib::workflows::{
        temporal_generate_media_start_request, temporal_job_summary, TemporalWorkflowKind,
    };

    fn sample_edit_request() -> EditJobRequest {
        EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a sharp trailer cut with bold captions".to_string(),
            target_duration_seconds: Some(45.0),
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-26T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn codex_video_edit_command_result_serializes_nullable_validation_issues() {
        let result = CodexVideoEditCommandResult {
            project: sample_project(),
            thread_id: "thread-1".to_string(),
            thread_response: serde_json::json!({}),
            turn_response: serde_json::json!({}),
            proposal: None,
            proposal_validation_issues: Some(vec![ProjectValidationIssue {
                path: "clips".to_string(),
                message: "invalid EDL".to_string(),
                fix: "Select source ranges".to_string(),
            }]),
        };

        let value = serde_json::to_value(result).expect("serialize command result");
        assert_eq!(value["proposalValidationIssues"][0]["path"], "clips");
        assert_eq!(
            value["proposalValidationIssues"][0]["fix"],
            "Select source ranges"
        );
    }

    #[test]
    fn codex_conversation_command_result_withholds_prepared_bundle_with_issues() {
        let result = super::CodexConversationEditCommandResult {
            project: sample_project(),
            thread_id: "thread-1".to_string(),
            thread_response: serde_json::json!({}),
            turn_response: serde_json::json!({}),
            proposal: None,
            prepared_proposal: None,
            proposal_validation_issues: Some(vec![ProjectValidationIssue {
                path: "projectActions".to_string(),
                message: "invalid action".to_string(),
                fix: "Use canonical targets".to_string(),
            }]),
        };

        let value = serde_json::to_value(result).expect("serialize command result");
        assert_eq!(value["preparedProposal"], serde_json::Value::Null);
        assert_eq!(
            value["proposalValidationIssues"][0]["path"],
            "projectActions"
        );
    }

    #[test]
    fn preview_render_comparison_request_resolves_only_project_relative_evidence() {
        let request = ProjectRenderPreviewComparisonRequest {
            status: "pending".to_string(),
            project_dir: "/tmp/ignored-project".to_string(),
            project_report_id: "render-draft-1".to_string(),
            render_report_path: "renders/render-draft-1/report.json".to_string(),
            rendered_video: "renders/render-draft-1/output.webm".to_string(),
            duration_seconds: 4.0,
            frame_time_seconds: 1.25,
            rendered_frames: vec!["renders/render-draft-1/frames/frame-000001.png".to_string()],
            fail_on_mismatch: true,
        };

        let (_, report, frame) =
            validate_preview_render_comparison_request(Path::new("/tmp/project"), &request)
                .expect("typed request should resolve native QA evidence");

        assert_eq!(
            report,
            Path::new("/tmp/project/renders/render-draft-1/report.json")
        );
        assert_eq!(
            frame,
            Path::new("/tmp/project/renders/render-draft-1/frames/frame-000001.png")
        );
    }

    #[test]
    fn preview_comparison_save_returns_the_committed_revision_for_the_next_cas_save() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.name = "Preview comparison committed".to_string();

        let committed = super::save_preview_render_comparison_project(&project_dir, &project)
            .expect("save preview comparison project");
        let persisted = load_split_project(&project_dir).expect("load committed project");

        assert_eq!(committed, persisted);
        video_creater_lib::project::split::replace_split_project_if_revision(
            &project_dir,
            committed.clone(),
            committed.content_revision,
        )
        .expect("subsequent CAS save");
    }

    #[test]
    fn rejects_relative_project_dir() {
        let error = resolve_project_dir("relative/project").expect_err("relative path must fail");

        assert_eq!(error, "project directory must be an absolute path");
    }

    #[test]
    fn rejects_parent_traversal_project_dir() {
        let error = resolve_project_dir("/tmp/../project").expect_err("parent traversal must fail");

        assert_eq!(error, "project directory cannot contain parent traversal");
    }

    #[test]
    fn resolves_new_project_dir_under_existing_parent() {
        let parent = tempfile::tempdir().expect("temp parent");
        let project_dir = parent.path().join("new-project");

        let resolved = resolve_project_dir(project_dir.to_str().expect("utf-8 path"))
            .expect("project dir should resolve");

        assert_eq!(
            resolved,
            parent
                .path()
                .canonicalize()
                .expect("canonical parent")
                .join("new-project")
        );
    }

    #[test]
    fn resolves_existing_codex_project_root() {
        let root = tempfile::tempdir().expect("temp root");

        let resolved = resolve_codex_project_root(Some(root.path().to_str().expect("utf-8 path")))
            .expect("project root should resolve");

        assert_eq!(
            resolved,
            root.path().canonicalize().expect("canonical root")
        );
    }

    #[test]
    fn runtime_selection_uses_native_model_store_and_helper_readiness() {
        let root = tempfile::tempdir().expect("temp model root");
        let store = super::TranscriptionModelStore::new(root.path().to_path_buf());
        let selection =
            super::runtime_selection_for_model_store(&store, "nvidia/parakeet-tdt-0.6b-v3")
                .expect("runtime selection");

        // macOS (FluidAudio Core ML) and Linux (sherpa-onnx) have native runtimes that are merely
        // unavailable until a model is installed; other platforms have none.
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert_eq!(selection, super::RuntimeSelection::Unavailable);
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        assert_eq!(selection, super::RuntimeSelection::UnsupportedPlatform);
    }

    #[test]
    fn speech_model_operation_reservation_is_atomic_deduplicated_and_noncancellable() {
        let directory = tempfile::tempdir().expect("temporary operation root");
        let registry = super::SettingsOperationRegistry::load(
            directory.path().join("operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let store = super::ProductionSpeechModelStore::new(directory.path().join("models"));
        let status = store.status();

        let (first, created) = super::reserve_speech_model_operation(&registry, &status)
            .expect("reserve speech operation");
        let (duplicate, duplicate_created) =
            super::reserve_speech_model_operation(&registry, &status)
                .expect("deduplicate speech operation");

        assert!(created);
        assert!(!duplicate_created);
        assert_eq!(first.id, duplicate.id);
        assert_eq!(first.kind, SettingsOperationKind::SpeechModelsDownload);
        assert_eq!(first.state, SettingsOperationState::Queued);
        assert!(!first.cancellable);
        assert_eq!(first.total_units, Some(status.total_bytes));
        assert_eq!(first.unit.as_deref(), Some("bytes"));
    }

    #[test]
    fn missing_speech_target_discards_stale_terminal_warning_before_reservation() {
        let directory = tempfile::tempdir().expect("temporary operation root");
        let registry = super::SettingsOperationRegistry::load(
            directory.path().join("operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let store = super::ProductionSpeechModelStore::new(directory.path().join("models"));
        let status = store.status();
        let (operation, _) = super::reserve_speech_model_operation(&registry, &status)
            .expect("reserve speech operation");
        let mut warning = operation.clone();
        warning.state = SettingsOperationState::Succeeded;
        warning.error = Some(SettingsOperationError {
            code: "settings.operation.terminalPersistenceFailed".to_string(),
            message: "History persistence failed.".to_string(),
            recovery_action: None,
            detail: Some("injected failure".to_string()),
        });
        registry
            .replace_in_memory_without_persisting(warning)
            .expect("install warning");

        let (retry, created) =
            super::reserve_speech_model_operation(&registry, &status).expect("reserve replacement");

        assert!(created);
        assert_ne!(retry.id, operation.id);
    }

    #[test]
    fn running_write_failure_projects_visible_terminal_speech_failure() {
        let directory = tempfile::tempdir().expect("temporary operation root");
        let journal = directory.path().join("operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let store = super::ProductionSpeechModelStore::new(directory.path().join("models"));
        let status = store.status();
        let (operation, _) = super::reserve_speech_model_operation(&registry, &status)
            .expect("reserve speech operation");
        std::fs::remove_file(&journal).expect("remove journal");
        std::fs::create_dir(&journal).expect("block journal replacement");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("running persistence test app");

        super::run_production_speech_model_download_operation(
            app.handle().clone(),
            store.clone(),
            operation.id.clone(),
        );

        let visible = app
            .state::<super::SettingsOperationRegistryState>()
            .0
            .get(&operation.id)
            .expect("visible operation");
        assert_eq!(visible.state, SettingsOperationState::Failed);
        assert!(!visible.cancellable);
        assert_eq!(
            visible.error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.runningPersistenceFailed")
        );
        assert_eq!(
            store.status().last_error_code.as_deref(),
            Some("speechModels.progressPersistenceFailed")
        );
    }

    #[test]
    fn progress_and_terminal_write_failures_still_project_visible_terminal_state() {
        let directory = tempfile::tempdir().expect("temporary operation root");
        let journal = directory.path().join("operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let store = super::ProductionSpeechModelStore::new(directory.path().join("models"));
        let status = store.status();
        let (operation, _) = super::reserve_speech_model_operation(&registry, &status)
            .expect("reserve speech operation");
        registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("persist running operation");
        std::fs::remove_file(&journal).expect("remove journal");
        std::fs::create_dir(&journal).expect("block journal replacement");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("terminal persistence test app");
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let progress_error = registry
            .0
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect_err("progress write must fail");
        let speech_error = super::SpeechModelError::ProgressPersistence(progress_error.to_string());
        store
            .record_error(&speech_error)
            .expect("persist durable speech failure");
        let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
        failed.error = Some(SettingsOperationError {
            code: speech_error.stable_code().to_string(),
            message: "Speech analysis model setup failed.".to_string(),
            recovery_action: Some("Retry.".to_string()),
            detail: Some(speech_error.to_string()),
        });
        let terminal_error = registry
            .0
            .update(&operation.id, failed)
            .expect_err("terminal write must fail");
        super::project_speech_operation_persistence_failure(
            app.handle(),
            &store,
            &operation.id,
            "settings.operation.failurePersistenceFailed",
            "Speech model setup failed and its terminal state could not be saved.",
            terminal_error.to_string(),
            None,
        );

        let visible = registry.0.get(&operation.id).expect("visible operation");
        assert_eq!(visible.state, SettingsOperationState::Failed);
        assert!(!visible.cancellable);
        assert_eq!(
            visible.error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.failurePersistenceFailed")
        );
        assert_eq!(
            store.status().last_error_code.as_deref(),
            Some("speechModels.progressPersistenceFailed")
        );
    }

    fn invoke_settings_health(
        app: &tauri::App<tauri::test::MockRuntime>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let webview = tauri::WebviewWindowBuilder::new(app, "settings-health", Default::default())
            .build()
            .expect("settings health test webview");
        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_settings_health_snapshot".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| {
            body.deserialize::<serde_json::Value>()
                .expect("settings health response JSON")
        })
    }

    /// Health commands read the user's optional Claude path from preferences. The
    /// store only reads, and only when its file exists, so an absent path gives the
    /// mock apps defaults without touching the real store or the disk.
    fn test_app_preferences_state() -> video_creater_lib::settings::preferences::AppPreferencesState
    {
        video_creater_lib::settings::preferences::AppPreferencesState(std::sync::Mutex::new(
            video_creater_lib::settings::preferences::AppPreferencesStore::new(
                std::path::PathBuf::from("/nonexistent/video-creater-test/preferences.json"),
            ),
        ))
    }

    fn settings_health_test_app(
        state: super::TranscriptionModelStoreState,
    ) -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .manage(state)
            .manage(super::ProviderHealthState::default())
            .manage(test_app_preferences_state())
            .invoke_handler(tauri::generate_handler![
                super::get_settings_health_snapshot
            ])
            .build(mock_context(noop_assets()))
            .expect("settings health test app")
    }

    fn settings_acceptance_test_app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .manage(super::SettingsAcceptanceStateHolder(None))
            .invoke_handler(tauri::generate_handler![
                super::get_settings_acceptance_context,
                super::write_settings_acceptance_checkpoint,
                super::write_settings_acceptance_progress,
                super::write_settings_acceptance_failure,
                super::abort_settings_acceptance_run
            ])
            .build(mock_context(noop_assets()))
            .expect("settings acceptance test app")
    }

    #[test]
    fn app_preferences_command_serializes_schema_version_2() {
        let root = tempfile::tempdir().expect("preferences tempdir");
        let app = mock_builder()
            .manage(
                video_creater_lib::settings::preferences::AppPreferencesState(
                    std::sync::Mutex::new(
                        video_creater_lib::settings::preferences::AppPreferencesStore::new(
                            root.path().join("preferences.json"),
                        ),
                    ),
                ),
            )
            .invoke_handler(tauri::generate_handler![super::get_app_preferences])
            .build(mock_context(noop_assets()))
            .expect("app preferences test app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "app-preferences-get", Default::default())
                .build()
                .expect("app preferences test webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_app_preferences".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({ "legacy": null })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("get app preferences response");
        let payload = response
            .deserialize::<serde_json::Value>()
            .expect("app preferences response JSON");

        assert_eq!(payload["schemaVersion"], serde_json::json!(2));
    }

    #[test]
    fn app_preferences_command_returns_structured_validation_errors() {
        let root = tempfile::tempdir().expect("preferences tempdir");
        let app = mock_builder()
            .manage(
                video_creater_lib::settings::preferences::AppPreferencesState(
                    std::sync::Mutex::new(
                        video_creater_lib::settings::preferences::AppPreferencesStore::new(
                            root.path().join("preferences.json"),
                        ),
                    ),
                ),
            )
            .invoke_handler(tauri::generate_handler![super::update_app_preferences])
            .build(mock_context(noop_assets()))
            .expect("app preferences test app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "app-preferences-update", Default::default())
                .build()
                .expect("app preferences test webview");

        let error = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "update_app_preferences".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "patch": {
                        "newProjectDefaults": {
                            "width": 0,
                            "height": 1080,
                            "fps": 30.0,
                            "loudnessLufs": -14.0,
                            "captions": "burn_in"
                        }
                    }
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect_err("invalid dimensions must be rejected");

        assert_eq!(
            error["code"],
            serde_json::json!("settings.preferences.invalidDimensions")
        );
        assert!(error["message"].is_string());
        assert!(error["detail"].is_string());
    }

    fn invoke_settings_acceptance(
        app: &tauri::App<tauri::test::MockRuntime>,
        command: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let webview = tauri::WebviewWindowBuilder::new(
            app,
            format!("settings-acceptance-{command}"),
            Default::default(),
        )
        .build()
        .expect("settings acceptance test webview");
        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| {
            body.deserialize::<serde_json::Value>()
                .expect("settings acceptance response JSON")
        })
    }

    #[test]
    fn settings_acceptance_ipc_is_inert_and_checkpoint_writes_fail_closed_by_default() {
        let app = settings_acceptance_test_app();

        let context = invoke_settings_acceptance(
            &app,
            "get_settings_acceptance_context",
            serde_json::json!({}),
        )
        .expect("normal launch acceptance context");
        assert!(context.is_null());

        let error = invoke_settings_acceptance(
            &app,
            "write_settings_acceptance_checkpoint",
            serde_json::json!({
                "checkpoint": {
                    "stage": "pre_restart",
                    "checks": [{
                        "id": "settingsDom",
                        "status": "passed",
                        "diagnosticCode": "settings.acceptance.settingsDom.passed"
                    }]
                }
            }),
        )
        .expect_err("normal launch checkpoint writer must be disabled");
        assert_eq!(error, "settings acceptance is not enabled");

        let error = invoke_settings_acceptance(
            &app,
            "write_settings_acceptance_failure",
            serde_json::json!({
                "failure": {
                    "stage": "pre_restart",
                    "phase": "settingsDom",
                    "diagnosticCode": "settings.acceptance.settingsDom.failed"
                }
            }),
        )
        .expect_err("normal launch failure writer must be disabled");
        assert_eq!(error, "settings acceptance is not enabled");

        let error = invoke_settings_acceptance(
            &app,
            "abort_settings_acceptance_run",
            serde_json::json!({
                "failure": {
                    "stage": "pre_restart",
                    "phase": "settingsDom",
                    "diagnosticCode": "settings.acceptance.settingsDom.failed"
                }
            }),
        )
        .expect_err("normal launch abort command must be disabled");
        assert_eq!(error, "settings acceptance is not enabled");
    }

    fn invoke_settings_operations(
        app: &tauri::App<tauri::test::MockRuntime>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let webview =
            tauri::WebviewWindowBuilder::new(app, "settings-operations", Default::default())
                .build()
                .expect("settings operations test webview");
        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "list_settings_operations".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| {
            body.deserialize::<serde_json::Value>()
                .expect("settings operations response JSON")
        })
    }

    #[test]
    fn settings_operations_command_lists_managed_journal_entries() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        registry
            .start(
                SettingsOperationKind::StorageCleanup,
                "cache",
                Some(10),
                Some("bytes".to_string()),
            )
            .and_then(|operation| {
                let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
                failed.error = Some(SettingsOperationError {
                    code: "settings.storage.cleanupFailed".to_string(),
                    message: "Storage cleanup failed.".to_string(),
                    recovery_action: Some("Retry".to_string()),
                    detail: None,
                });
                registry.update(&operation.id, failed)
            })
            .expect("failed recent operation");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .invoke_handler(tauri::generate_handler![super::list_settings_operations])
            .build(mock_context(noop_assets()))
            .expect("settings operations test app");

        let response =
            invoke_settings_operations(&app).expect("settings operations command response");

        assert_eq!(response[0]["kind"], "storageCleanup");
        assert_eq!(response[0]["targetId"], "cache");
        assert_eq!(response[0]["state"], "failed");
        assert_eq!(response[0]["totalUnits"], 10);
    }

    #[test]
    fn render_system_health_operation_is_deduplicated_and_noncancellable() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");

        let (first, created) = super::reserve_render_system_health_operation(&registry)
            .expect("reserve render health operation");
        let (duplicate, duplicate_created) =
            super::reserve_render_system_health_operation(&registry)
                .expect("deduplicate render health operation");

        assert!(created);
        assert!(!duplicate_created);
        assert_eq!(first.id, duplicate.id);
        assert_eq!(first.kind, SettingsOperationKind::HealthCheck);
        assert_eq!(first.target_id, "render-system");
        assert_eq!(first.total_units, Some(4));
        assert_eq!(first.unit.as_deref(), Some("components"));
        assert!(!first.cancellable);
    }

    #[test]
    fn settings_operations_command_surfaces_corrupt_journal_recovery() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        std::fs::create_dir_all(journal.parent().expect("journal parent"))
            .expect("create journal parent");
        std::fs::write(&journal, b"{ corrupt").expect("write corrupt journal");
        let registry = super::SettingsOperationRegistry::load_resilient(journal, |_| false);
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .invoke_handler(tauri::generate_handler![super::list_settings_operations])
            .build(mock_context(noop_assets()))
            .expect("settings operations recovery test app");

        let response =
            invoke_settings_operations(&app).expect("settings operations recovery response");

        assert_eq!(response[0]["state"], "failed");
        assert_eq!(
            response[0]["error"]["code"],
            "settings.operation.journalRecovered"
        );
        assert!(response[0]["error"]["detail"]
            .as_str()
            .expect("journal recovery detail")
            .contains("invalid"));
    }

    #[test]
    fn queued_model_cancellation_prevents_worker_ownership_and_download() {
        let root = tempfile::tempdir().expect("operation root");
        let model_store = super::TranscriptionModelStore::new(root.path().join("models"));
        let entry = parakeet_v3_catalog_entry();
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let operation = registry
            .start_or_get_active(
                SettingsOperationKind::ModelDownload,
                entry.id,
                Some(entry.approximate_size_bytes),
                Some("bytes".to_string()),
            )
            .expect("queue operation")
            .0;
        let super::StartDownload::Started { token, .. } = model_store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("reserve model")
        else {
            panic!("model should reserve");
        };
        registry
            .request_cancel(&operation.id)
            .expect("cancel queued operation");
        model_store
            .cancel_download_if_active(entry.id)
            .expect("cancel model reservation");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ModelOperationCoordinatorState(
                std::sync::Mutex::new(super::ModelOperationCoordinator),
            ))
            .build(mock_context(noop_assets()))
            .expect("queued cancellation app");

        super::run_transcription_model_download_operation(
            app.handle().clone(),
            model_store.clone(),
            entry.id.to_string(),
            operation.id.clone(),
            entry.approximate_size_bytes,
            token,
        );

        let registry = app.state::<super::SettingsOperationRegistryState>();
        assert_eq!(
            registry.0.get(&operation.id).expect("operation").state,
            SettingsOperationState::Cancelled
        );
        assert!(!model_store
            .download_is_active(entry.id)
            .expect("active state"));
        assert!(!model_store.model_dir(entry.id).expect("model dir").exists());
    }

    #[test]
    fn published_model_operation_rejects_same_process_cancellation() {
        let operation = SettingsOperation {
            id: "operation-1".to_string(),
            kind: SettingsOperationKind::ModelDownload,
            target_id: "model-1".to_string(),
            phase: "ready_with_persistence_warning".to_string(),
            state: SettingsOperationState::Succeeded,
            completed_units: 100,
            total_units: Some(100),
            unit: Some("bytes".to_string()),
            cancellable: false,
            message: "Model is ready.".to_string(),
            error: None,
            started_at: "2026-07-16T00:00:00Z".to_string(),
            updated_at: "2026-07-16T00:00:01Z".to_string(),
        };

        assert!(matches!(
            operation.state,
            SettingsOperationState::Succeeded
                | SettingsOperationState::Failed
                | SettingsOperationState::Cancelled
        ));
        assert!(!operation.cancellable);
    }

    #[test]
    fn terminal_persistence_warning_is_visible_and_noncancellable() {
        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let running = registry
            .start(
                SettingsOperationKind::ModelDownload,
                "model-1",
                Some(100),
                Some("bytes".to_string()),
            )
            .and_then(|operation| {
                registry.update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Running),
                )
            })
            .expect("running operation");
        let projection = super::terminal_persistence_warning_projection(
            &running,
            "injected terminal journal failure".to_string(),
        );

        registry
            .replace_in_memory_without_persisting(projection.clone())
            .expect("install warning");
        let visible = registry.list_recent().expect("visible operations");

        let mut expected = projection.clone();
        expected.updated_at.clone_from(&visible[0].updated_at);
        assert_eq!(visible[0], expected);
        assert!(visible[0].updated_at >= projection.updated_at);
        assert_eq!(visible[0].state, SettingsOperationState::Succeeded);
        assert!(!visible[0].cancellable);
        assert_eq!(
            visible[0].error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.terminalPersistenceFailed")
        );
        assert_eq!(registry.list_recent().expect("visible operations").len(), 1);
    }

    #[test]
    fn settings_health_domain_uses_catalog_when_model_is_missing() {
        let root = tempfile::tempdir().expect("temp model root");
        let state = super::TranscriptionModelStoreState(std::sync::Mutex::new(
            super::TranscriptionModelStore::new(root.path().to_path_buf()),
        ));
        let snapshot = super::settings_health_snapshot_for_model_state(&state.0, None, &[], None)
            .expect("settings health snapshot");
        let catalog_entry = parakeet_v3_catalog_entry();
        let models = snapshot.categories.get("models").expect("models category");
        let model = models
            .items
            .iter()
            .find(|item| item.id == catalog_entry.id)
            .expect("catalog-backed model health");

        assert_eq!(models.state, SettingsHealthState::ActionRequired);
        assert_eq!(model.state, SettingsHealthState::ActionRequired);
        assert_eq!(
            model.diagnostic_code.as_deref(),
            Some("models.modelMissing")
        );
        assert_eq!(
            snapshot.categories["storage"].state,
            SettingsHealthState::Unavailable
        );
    }

    #[test]
    fn settings_health_command_invokes_with_managed_catalog_state() {
        let root = tempfile::tempdir().expect("temp model root");
        let app =
            settings_health_test_app(super::TranscriptionModelStoreState(std::sync::Mutex::new(
                super::TranscriptionModelStore::new(root.path().to_path_buf()),
            )));

        let response = invoke_settings_health(&app).expect("settings health command response");

        assert_eq!(
            response["categories"]["models"]["items"][0]["diagnosticCode"],
            "models.modelMissing"
        );
        assert_eq!(
            response["categories"]["providers"]["items"]
                .as_array()
                .map(Vec::len),
            Some(7)
        );
        assert_eq!(response["categories"]["providers"]["state"], "checking");
        assert!(response["categories"]["providers"]["items"]
            .as_array()
            .expect("provider health items")
            .iter()
            .all(|item| !item["summary"]
                .as_str()
                .unwrap_or_default()
                .contains("not implemented")));
    }

    #[test]
    fn settings_health_snapshot_overlays_scoped_refresh_on_seven_provider_baselines() {
        let root = tempfile::tempdir().expect("temp model root");
        let provider_state = super::ProviderHealthState::default();
        let refreshed_openai = video_creater_lib::settings::providers::ProviderHealth {
            provider: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            credential_source:
                video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
            configured: true,
            validation_state:
                video_creater_lib::settings::providers::ProviderValidationState::Available,
            account_label: Some("Refreshed studio".to_string()),
            balance_label: Some("42.00 USD".to_string()),
            dependent_model_ids: vec!["openai:gpt-image-2".to_string()],
            last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
            diagnostic_code: None,
        };
        let mut duplicate_openai = refreshed_openai.clone();
        duplicate_openai.account_label = Some("Stale studio".to_string());
        duplicate_openai.balance_label = Some("1.00 USD".to_string());
        let mut unsupported = refreshed_openai.clone();
        unsupported.provider = "unsupported".to_string();
        unsupported.display_name = "Unsupported".to_string();
        provider_state
            .seed(vec![unsupported, duplicate_openai, refreshed_openai])
            .expect("seed refreshed provider health");
        let app = mock_builder()
            .manage(super::TranscriptionModelStoreState(std::sync::Mutex::new(
                super::TranscriptionModelStore::new(root.path().to_path_buf()),
            )))
            .manage(provider_state)
            .manage(test_app_preferences_state())
            .invoke_handler(tauri::generate_handler![
                super::get_settings_health_snapshot
            ])
            .build(mock_context(noop_assets()))
            .expect("settings health app");

        let response = invoke_settings_health(&app).expect("settings health command response");
        let providers = &response["categories"]["providers"];

        let items = providers["items"]
            .as_array()
            .expect("provider health items");
        let ids = items
            .iter()
            .map(|item| item["id"].as_str().expect("provider health id"))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "providers.fal.ai",
                "providers.replicate",
                "providers.openai",
                "providers.xai",
                "providers.elevenlabs",
                "providers.google",
                "providers.minimax",
            ]
        );
        assert_eq!(providers["state"], "checking");
        assert_eq!(
            items
                .iter()
                .filter(|item| item["state"] == "checking")
                .count(),
            6
        );
        let openai = items
            .iter()
            .find(|item| item["id"] == "providers.openai")
            .expect("refreshed OpenAI health");
        assert_eq!(openai["state"], "ready");
        assert_eq!(openai["provenance"]["group"], "configured");
        assert_eq!(openai["provenance"]["validationState"], "available");
        assert_eq!(
            openai["summary"],
            "Credential validated; balance 42.00 USD."
        );
        let serialized = serde_json::to_string(providers).expect("serialize provider category");
        assert!(!serialized.contains("envVar"));
        assert!(!serialized.contains("credentialEnvVar"));
        assert!(!serialized.contains("unsupported"));
        assert!(!serialized.contains("Stale studio"));
    }

    #[test]
    fn storage_health_command_is_registered_and_returns_six_rows() {
        let root = tempfile::tempdir().expect("storage command root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        std::fs::create_dir_all(&models).expect("models root");
        std::fs::create_dir_all(&cache).expect("cache root");
        std::fs::write(models.join("model.bin"), vec![0; 9]).expect("model fixture");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache,
            })
            .invoke_handler(tauri::generate_handler![super::get_storage_health])
            .build(mock_context(noop_assets()))
            .expect("storage command app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "storage-health", Default::default())
            .build()
            .expect("storage command webview");

        let health = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_storage_health".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "activeProjectDir": null
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("storage health response")
        .deserialize::<serde_json::Value>()
        .expect("storage health JSON");
        assert_eq!(health["id"], "storage");
        assert_eq!(health["items"].as_array().map(Vec::len), Some(6));
        assert_eq!(health["items"][0]["provenance"]["bytes"], "9");
        assert_eq!(health["items"][2]["summary"], "Open a project to inspect");
    }

    #[test]
    fn storage_reveal_accepts_only_an_exact_current_inventory_path() {
        let root = tempfile::tempdir().expect("storage reveal root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        std::fs::create_dir_all(&models).expect("models directory");
        std::fs::create_dir_all(&cache).expect("cache directory");
        let inventory =
            video_creater_lib::settings::storage::collect_storage_inventory(&models, &cache, None);
        let revealed = std::sync::Mutex::new(Vec::<std::path::PathBuf>::new());

        super::reveal_reported_storage_path_with(&models, &inventory, |path| {
            revealed
                .lock()
                .expect("revealed paths")
                .push(path.to_path_buf());
            Ok(())
        })
        .expect("reported path is revealable");

        assert_eq!(
            revealed.into_inner().expect("revealed paths"),
            vec![std::fs::canonicalize(&models).expect("canonical models")]
        );

        let unreported = models.join("private-child");
        std::fs::create_dir_all(&unreported).expect("unreported directory");
        let error = super::reveal_reported_storage_path_with(&unreported, &inventory, |_| Ok(()))
            .expect_err("unreported child must be refused");
        assert_eq!(error.code, "settings.storage.revealPathNotReported");
    }

    #[test]
    fn storage_refresh_reservation_is_atomic_and_deduplicated() {
        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");

        let (first, created) =
            super::reserve_storage_inventory_refresh(&registry).expect("reserve refresh");
        let (duplicate, duplicate_created) =
            super::reserve_storage_inventory_refresh(&registry).expect("deduplicate refresh");

        assert!(created);
        assert!(!duplicate_created);
        assert_eq!(first.id, duplicate.id);
        assert_eq!(first.kind, SettingsOperationKind::StorageRefresh);
        assert_eq!(first.total_units, Some(6));
        assert_eq!(first.unit.as_deref(), Some("scopes"));
        assert!(!first.cancellable);
    }

    #[test]
    fn provider_refresh_reservation_is_scoped_atomic_and_noncancellable() {
        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");

        let (first, created) = super::reserve_provider_health_refresh(&registry, Some("openai"), 1)
            .expect("reserve provider refresh");
        let (duplicate, duplicate_created) =
            super::reserve_provider_health_refresh(&registry, Some("openai"), 1)
                .expect("deduplicate provider refresh");
        let (other, other_created) =
            super::reserve_provider_health_refresh(&registry, Some("google"), 1)
                .expect("reserve other provider refresh");

        assert!(created);
        assert!(!duplicate_created);
        assert_eq!(first.id, duplicate.id);
        assert!(other_created);
        assert_ne!(first.id, other.id);
        assert_eq!(first.kind, SettingsOperationKind::ProviderRefresh);
        assert_eq!(first.target_id, "openai");
        assert_eq!(first.total_units, Some(1));
        assert_eq!(first.unit.as_deref(), Some("providers"));
        assert!(!first.cancellable);
    }

    #[test]
    fn provider_refresh_returns_queued_updates_cache_and_emits_terminal_state() {
        use std::sync::{mpsc, Arc, Mutex};
        use tauri::Listener;

        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ProviderHealthState::default())
            .build(mock_context(noop_assets()))
            .expect("provider refresh app");
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_listener = observed.clone();
        app.listen(super::SETTINGS_OPERATION_EVENT, move |event| {
            let operation: SettingsOperation =
                serde_json::from_str(event.payload()).expect("settings operation event");
            observed_for_listener
                .lock()
                .expect("observed events")
                .push(operation.state);
        });
        let (started_sender, started_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let registry = app.state::<super::SettingsOperationRegistryState>();

        let started_at = std::time::Instant::now();
        let queued = super::start_provider_health_refresh_with(
            app.handle(),
            &registry.0,
            Some("openai"),
            1,
            move || {
                started_sender.send(()).expect("signal provider refresh");
                release_receiver.recv().expect("release provider refresh");
                Ok(vec![video_creater_lib::settings::providers::ProviderHealth {
                    provider: "openai".to_string(),
                    display_name: "OpenAI".to_string(),
                    credential_source:
                        video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
                    configured: true,
                    validation_state:
                        video_creater_lib::settings::providers::ProviderValidationState::BalanceUnavailable,
                    account_label: None,
                    balance_label: None,
                    dependent_model_ids: vec!["openai:gpt-image-2".to_string()],
                    last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                    diagnostic_code: None,
                }])
            },
        )
        .expect("start provider refresh");

        assert!(started_at.elapsed() < std::time::Duration::from_millis(100));
        assert_eq!(queued.state, SettingsOperationState::Queued);
        started_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("background provider refresh started");
        assert_eq!(
            registry.0.get(&queued.id).expect("running refresh").state,
            SettingsOperationState::Running
        );
        release_sender.send(()).expect("release provider refresh");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if registry
                .0
                .get(&queued.id)
                .is_ok_and(|operation| operation.state == SettingsOperationState::Succeeded)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "provider refresh timed out"
            );
            std::thread::yield_now();
        }
        let cached = app
            .state::<super::ProviderHealthState>()
            .snapshot()
            .expect("provider health cache");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].provider, "openai");

        let event_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while observed.lock().expect("observed events").len() < 3 {
            assert!(
                std::time::Instant::now() < event_deadline,
                "provider refresh events did not arrive"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            *observed.lock().expect("observed events"),
            vec![
                SettingsOperationState::Queued,
                SettingsOperationState::Running,
                SettingsOperationState::Succeeded,
            ]
        );
    }

    #[test]
    fn provider_refresh_failure_projects_secret_free_journal_event_and_ipc_payloads() {
        use std::sync::{Arc, Mutex};
        use tauri::Listener;

        let fixture_secret = "provider-refresh-fixture-secret-814f";
        let root = tempfile::tempdir().expect("operation root");
        let journal = root.path().join("settings/operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ProviderHealthState::default())
            .invoke_handler(tauri::generate_handler![super::list_settings_operations])
            .build(mock_context(noop_assets()))
            .expect("provider refresh failure app");
        let terminal_event = Arc::new(Mutex::new(None::<serde_json::Value>));
        let terminal_event_for_listener = terminal_event.clone();
        app.listen(super::SETTINGS_OPERATION_EVENT, move |event| {
            let value: serde_json::Value =
                serde_json::from_str(event.payload()).expect("settings operation event");
            if value["state"] == "failed" {
                *terminal_event_for_listener
                    .lock()
                    .expect("terminal provider event") = Some(value);
            }
        });
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let secret_for_worker = fixture_secret.to_string();

        let queued = super::start_provider_health_refresh_with(
            app.handle(),
            &registry.0,
            Some("openai"),
            1,
            move || Err(secret_for_worker),
        )
        .expect("start failing provider refresh");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if registry
                .0
                .get(&queued.id)
                .is_ok_and(|operation| operation.state == SettingsOperationState::Failed)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "provider failure did not become terminal"
            );
            std::thread::yield_now();
        }
        let event_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while terminal_event
            .lock()
            .expect("terminal provider event")
            .is_none()
        {
            assert!(
                std::time::Instant::now() < event_deadline,
                "terminal provider event did not arrive"
            );
            std::thread::yield_now();
        }

        let journal_payload: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&journal).expect("read provider operation journal"),
        )
        .expect("decode provider operation journal");
        let event_payload = terminal_event
            .lock()
            .expect("terminal provider event")
            .clone()
            .expect("terminal provider event payload");
        let ipc_payload =
            invoke_settings_operations(&app).expect("provider operations IPC response");
        for payload in [&journal_payload, &event_payload, &ipc_payload] {
            assert_provider_payload_secret_free(payload, fixture_secret);
        }
    }

    #[test]
    fn newer_scoped_refresh_merges_before_older_all_without_stale_overwrite() {
        use std::sync::mpsc;

        fn health(
            provider: &str,
            account: &str,
        ) -> video_creater_lib::settings::providers::ProviderHealth {
            video_creater_lib::settings::providers::ProviderHealth {
                provider: provider.to_string(),
                display_name: provider.to_string(),
                credential_source:
                    video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
                configured: true,
                validation_state:
                    video_creater_lib::settings::providers::ProviderValidationState::Available,
                account_label: Some(account.to_string()),
                balance_label: None,
                dependent_model_ids: vec![format!("{provider}:model")],
                last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                diagnostic_code: None,
            }
        }

        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ProviderHealthState::default())
            .build(mock_context(noop_assets()))
            .expect("provider refresh app");
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let (first_started_sender, first_started_receiver) = mpsc::sync_channel(1);
        let (release_first_sender, release_first_receiver) = mpsc::sync_channel(1);
        let (second_started_sender, second_started_receiver) = mpsc::sync_channel(1);

        let older_all = super::start_provider_health_refresh_with(
            app.handle(),
            &registry.0,
            None,
            7,
            move || {
                first_started_sender.send(()).expect("first started");
                release_first_receiver.recv().expect("release first");
                Ok(vec![
                    health("openai", "stale account"),
                    health("google", "all refresh account"),
                ])
            },
        )
        .expect("start all-provider refresh");
        first_started_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("all-provider refresh started");
        let newer_scoped = super::start_provider_health_refresh_with(
            app.handle(),
            &registry.0,
            Some("openai"),
            1,
            move || {
                second_started_sender.send(()).expect("second started");
                Ok(vec![health("openai", "new account")])
            },
        )
        .expect("start scoped refresh");

        second_started_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("newer scoped refresh executes while older all refresh is blocked");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while registry
            .0
            .get(&newer_scoped.id)
            .is_ok_and(|operation| operation.state != SettingsOperationState::Succeeded)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "newer scoped refresh did not merge first"
            );
            std::thread::yield_now();
        }
        release_first_sender
            .send(())
            .expect("release first refresh");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while registry
            .0
            .get(&older_all.id)
            .is_ok_and(|operation| operation.state != SettingsOperationState::Succeeded)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "older all-provider refresh did not finish"
            );
            std::thread::yield_now();
        }

        let snapshot = app
            .state::<super::ProviderHealthState>()
            .snapshot()
            .expect("provider health snapshot");
        assert_eq!(
            snapshot
                .iter()
                .find(|health| health.provider == "openai")
                .and_then(|health| health.account_label.as_deref()),
            Some("new account")
        );
        assert_eq!(
            snapshot
                .iter()
                .find(|health| health.provider == "google")
                .and_then(|health| health.account_label.as_deref()),
            Some("all refresh account")
        );
    }

    #[test]
    fn provider_refresh_generations_reject_stale_all_result_for_newer_scoped_provider() {
        fn health(
            provider: &str,
            account: &str,
        ) -> video_creater_lib::settings::providers::ProviderHealth {
            video_creater_lib::settings::providers::ProviderHealth {
                provider: provider.to_string(),
                display_name: provider.to_string(),
                credential_source:
                    video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
                configured: true,
                validation_state:
                    video_creater_lib::settings::providers::ProviderValidationState::Available,
                account_label: Some(account.to_string()),
                balance_label: None,
                dependent_model_ids: vec![format!("{provider}:model")],
                last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                diagnostic_code: None,
            }
        }

        let state = super::ProviderHealthState::default();
        let older_all = state
            .begin_refresh(None)
            .expect("begin older all-provider refresh");
        let newer_openai = state
            .begin_refresh(Some("openai"))
            .expect("begin newer scoped refresh");

        state
            .merge_refresh(&newer_openai, vec![health("openai", "new account")])
            .expect("merge newer scoped refresh first");
        state
            .merge_refresh(
                &older_all,
                vec![
                    health("openai", "stale account"),
                    health("google", "all refresh account"),
                ],
            )
            .expect("merge older all-provider refresh last");

        let snapshot = state.snapshot().expect("provider health snapshot");
        let openai = snapshot
            .iter()
            .find(|health| health.provider == "openai")
            .expect("openai health");
        let google = snapshot
            .iter()
            .find(|health| health.provider == "google")
            .expect("google health");
        assert_eq!(openai.account_label.as_deref(), Some("new account"));
        assert_eq!(google.account_label.as_deref(), Some("all refresh account"));
    }

    #[test]
    fn provider_health_get_command_registers_with_secret_free_cache() {
        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let state = super::ProviderHealthState::default();
        state
            .seed(vec![video_creater_lib::settings::providers::ProviderHealth {
                provider: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                credential_source:
                    video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
                configured: true,
                validation_state:
                    video_creater_lib::settings::providers::ProviderValidationState::BalanceUnavailable,
                account_label: None,
                balance_label: None,
                dependent_model_ids: vec!["openai:gpt-image-2".to_string()],
                last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                diagnostic_code: None,
            }])
            .expect("seed provider health");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(state)
            .invoke_handler(tauri::generate_handler![super::get_provider_health])
            .build(mock_context(noop_assets()))
            .expect("provider health command app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "provider-health", Default::default())
            .build()
            .expect("provider health command webview");

        let health = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_provider_health".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("provider health response")
        .deserialize::<serde_json::Value>()
        .expect("provider health JSON");
        assert_eq!(health[0]["credentialSource"], "keychain");
        assert_eq!(health[0]["dependentModelIds"][0], "openai:gpt-image-2");
        assert!(health[0].get("envVar").is_none());
    }

    #[test]
    fn storage_refresh_returns_queued_while_inventory_runs_in_background_and_emits_transitions() {
        use std::sync::{mpsc, Arc, Mutex};
        use tauri::Listener;

        let root = tempfile::tempdir().expect("operation root");
        let journal = root.path().join("settings/operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("storage refresh app");
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_listener = observed.clone();
        app.listen(super::SETTINGS_OPERATION_EVENT, move |event| {
            let operation: SettingsOperation =
                serde_json::from_str(event.payload()).expect("settings operation event");
            observed_for_listener
                .lock()
                .expect("observed events")
                .push(operation.state);
        });
        let (started_sender, started_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let registry = app.state::<super::SettingsOperationRegistryState>();

        let started_at = std::time::Instant::now();
        let queued =
            super::start_storage_inventory_refresh_with(app.handle(), &registry.0, move || {
                started_sender.send(()).expect("signal inventory start");
                release_receiver.recv().expect("release inventory");
                0
            })
            .expect("start storage refresh");

        assert!(started_at.elapsed() < std::time::Duration::from_millis(100));
        assert_eq!(queued.state, SettingsOperationState::Queued);
        assert!(!queued.cancellable);
        started_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("background inventory started");
        let running = registry.0.get(&queued.id).expect("running refresh");
        assert_eq!(running.state, SettingsOperationState::Running);

        release_sender.send(()).expect("release inventory");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if registry
                .0
                .get(&queued.id)
                .is_ok_and(|operation| operation.state == SettingsOperationState::Succeeded)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "storage refresh did not finish"
            );
            std::thread::yield_now();
        }
        let event_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while observed.lock().expect("observed events").len() < 3 {
            assert!(
                std::time::Instant::now() < event_deadline,
                "storage refresh events did not arrive"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            *observed.lock().expect("observed events"),
            vec![
                SettingsOperationState::Queued,
                SettingsOperationState::Running,
                SettingsOperationState::Succeeded,
            ]
        );

        let restarted = super::SettingsOperationRegistry::load(journal, |_| false)
            .expect("reload operation registry");
        let persisted = restarted.get(&queued.id).expect("persisted refresh");
        assert_eq!(persisted.state, SettingsOperationState::Succeeded);
        assert_eq!(persisted.completed_units, 6);
    }

    #[test]
    fn interrupted_storage_refresh_is_reconciled_and_can_be_retried() {
        let root = tempfile::tempdir().expect("operation root");
        let journal = root.path().join("settings/operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let (queued, created) =
            super::reserve_storage_inventory_refresh(&registry).expect("reserve refresh");
        assert!(created);
        registry
            .update(
                &queued.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("persist running refresh");
        drop(registry);

        let restarted = super::SettingsOperationRegistry::load(journal, |_| false)
            .expect("reload operation registry");
        let interrupted = restarted.get(&queued.id).expect("interrupted refresh");
        assert_eq!(interrupted.state, SettingsOperationState::Failed);
        assert_eq!(interrupted.phase, "interrupted");
        let (retry, retry_created) =
            super::reserve_storage_inventory_refresh(&restarted).expect("retry refresh");
        assert!(retry_created);
        assert_ne!(retry.id, queued.id);
        assert_eq!(retry.state, SettingsOperationState::Queued);
    }

    #[test]
    fn storage_cleanup_preview_command_returns_exact_paths_bytes_and_confirmation() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        std::fs::create_dir_all(&models).expect("models root");
        std::fs::create_dir_all(&cache).expect("cache root");
        std::fs::write(cache.join("preview.tmp"), vec![0; 9]).expect("cache fixture");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::ActiveProjectSessionState::default())
            .manage(super::StorageProjectMutationCoordinatorState)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![super::preview_storage_cleanup])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup preview app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "storage-cleanup-preview", Default::default())
                .build()
                .expect("storage cleanup preview webview");

        let preview = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "preview_storage_cleanup".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "target": { "kind": "disposableAppCache" },
                    "activeProjectDir": null
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("storage cleanup preview response")
        .deserialize::<serde_json::Value>()
        .expect("storage cleanup preview JSON");

        assert_eq!(preview["items"].as_array().map(Vec::len), Some(1));
        assert_eq!(preview["items"][0]["bytes"], 9);
        assert_eq!(
            preview["items"][0]["path"],
            cache
                .join("preview.tmp")
                .canonicalize()
                .expect("canonical cache item")
                .display()
                .to_string()
        );
        assert_eq!(preview["totalBytes"], 9);
        assert!(preview["confirmationToken"]
            .as_str()
            .is_some_and(|token| token.starts_with("storage-cleanup-v1:")));
    }

    #[test]
    fn consumed_empty_cache_preview_can_be_reissued_and_queued_again() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        std::fs::create_dir_all(&models).expect("models root");
        std::fs::create_dir_all(&cache).expect("empty cache root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ActiveProjectSessionState::default())
            .manage(super::StorageProjectMutationCoordinatorState)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![super::preview_storage_cleanup])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "empty-cache-reissue", Default::default())
                .build()
                .expect("storage cleanup webview");
        let invoke_preview = || {
            get_ipc_response(
                &webview,
                tauri::webview::InvokeRequest {
                    cmd: "preview_storage_cleanup".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().expect("invoke URL"),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                        "target": { "kind": "disposableAppCache" },
                        "activeProjectDir": null
                    })),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .expect("empty cache preview")
            .deserialize::<serde_json::Value>()
            .expect("preview JSON")
        };
        let target = video_creater_lib::settings::storage::StorageCleanupTarget::DisposableAppCache;
        let roots = super::StorageInventoryRootsState {
            global_model_root: root.path().join("models"),
            app_cache_root: cache,
        };
        let first = invoke_preview();
        let first_token = first["confirmationToken"]
            .as_str()
            .expect("first token")
            .to_string();
        let operations = app.state::<super::SettingsOperationRegistryState>();
        let first_operation = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target.clone(),
            first_token.clone(),
            roots.clone(),
            None,
        )
        .expect("first preview queues");
        assert_eq!(first_operation.state, SettingsOperationState::Queued);
        let replay = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target.clone(),
            first_token.clone(),
            roots.clone(),
            None,
        )
        .expect_err("consumed preview must not replay");
        assert_eq!(replay.code, "settings.storage.cleanupConfirmationMismatch");

        let second = invoke_preview();
        let second_token = second["confirmationToken"]
            .as_str()
            .expect("second token")
            .to_string();
        assert_ne!(second_token, first_token);
        let second_operation = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target,
            second_token,
            roots,
            None,
        )
        .expect("fresh unchanged preview queues");
        assert_eq!(second_operation.state, SettingsOperationState::Queued);
        assert_ne!(second_operation.id, first_operation.id);
    }

    #[test]
    fn storage_cleanup_preview_refuses_an_arbitrary_absolute_project_directory() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let trusted = root.path().join("trusted-project");
        let attacker = root.path().join("attacker-project");
        for directory in [
            &models,
            &cache,
            &trusted.join("renders/trusted-render"),
            &attacker.join("renders/attacker-render"),
        ] {
            std::fs::create_dir_all(directory).expect("fixture directory");
        }
        std::fs::write(
            attacker.join("renders/attacker-render/output.mp4"),
            b"must stay",
        )
        .expect("attacker render");
        let session = super::ActiveProjectSessionState::default();
        let coordinator = super::StorageProjectMutationCoordinatorState;
        {
            let lease = coordinator.acquire().expect("mutation lease");
            session
                .record_successful_project_root(&trusted, &lease)
                .expect("record trusted project");
        }
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache,
            })
            .manage(session)
            .manage(coordinator)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![super::preview_storage_cleanup])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup preview app");
        let webview = tauri::WebviewWindowBuilder::new(
            &app,
            "storage-cleanup-untrusted-project",
            Default::default(),
        )
        .build()
        .expect("storage cleanup preview webview");

        let error = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "preview_storage_cleanup".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "target": {
                        "kind": "projectRenderArtifacts",
                        "artifact_ids": ["attacker-render"]
                    },
                    "activeProjectDir": attacker.display().to_string()
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect_err("untrusted absolute project directory must fail");

        assert_eq!(
            error["code"],
            "settings.storage.cleanupProjectSessionMismatch"
        );
        assert!(attacker
            .join("renders/attacker-render/output.mp4")
            .is_file());
    }

    #[test]
    fn snapshot_restore_does_not_wait_for_global_activation_or_change_the_session() {
        let root = tempfile::tempdir().expect("snapshot restore root");
        let project_dir = root.path().join("project");
        let saved = super::save_split_project_to_folder_impl(
            project_dir.display().to_string(),
            sample_project(),
            0,
        )
        .expect("initial save");
        let session = std::sync::Arc::new(super::ActiveProjectSessionState::default());
        let coordinator = super::StorageProjectMutationCoordinatorState;
        let lease = coordinator.acquire().expect("held global lease");
        session
            .record_successful_project_root(&project_dir, &lease)
            .expect("active session");
        let before = session.current().expect("session before restore");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_session = session.clone();
        let worker_dir = project_dir.clone();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).expect("worker started");
            let mut snapshot = saved.project;
            snapshot.name = "restored snapshot".into();
            let expected_revision = snapshot.content_revision;
            let result = super::save_split_project_to_folder_blocking(
                worker_dir.display().to_string(),
                snapshot,
                expected_revision,
                false,
                &worker_session,
                &super::StorageProjectMutationCoordinatorState,
            );
            done_tx.send(result).expect("restore result");
        });
        let started = started_rx.recv_timeout(std::time::Duration::from_secs(5));
        let early = done_rx.recv_timeout(std::time::Duration::from_secs(5));
        // Always release and join before asserting, including when the old global gate blocks.
        drop(lease);
        worker.join().expect("restore worker");
        assert!(started.is_ok(), "restore worker must start");
        let restored = early
            .expect("snapshot restore must finish while global activation is held")
            .expect("snapshot restore succeeds");
        assert_eq!(restored.project.name, "restored snapshot");
        assert_eq!(session.current().expect("session after restore"), before);
    }

    #[test]
    fn snapshot_restore_requires_an_existing_matching_safe_manifest() {
        let root = tempfile::tempdir().expect("snapshot identity root");
        let missing = root.path().join("missing");
        let session = super::ActiveProjectSessionState::default();
        let coordinator = super::StorageProjectMutationCoordinatorState;
        assert!(super::save_split_project_to_folder_blocking(
            missing.display().to_string(),
            sample_project(),
            0,
            false,
            &session,
            &coordinator,
        )
        .is_err());
        assert!(
            !missing.exists(),
            "snapshot restore must not create a project"
        );
        let project_dir = root.path().join("project");
        let saved = super::save_split_project_to_folder_impl(
            project_dir.display().to_string(),
            sample_project(),
            0,
        )
        .expect("initial save");
        let mut wrong = saved.project.clone();
        wrong.id = "different-project".into();
        assert!(super::save_split_project_to_folder_blocking(
            project_dir.display().to_string(),
            wrong,
            saved.project.content_revision,
            false,
            &session,
            &coordinator,
        )
        .is_err());
        assert_eq!(
            super::load_split_project_from_folder_impl(project_dir.display().to_string()).unwrap(),
            saved.project
        );
        #[cfg(unix)]
        {
            let manifest =
                video_creater_lib::project::split::split_project_manifest_path(&project_dir);
            let target = root.path().join("external-manifest.json");
            std::fs::rename(&manifest, &target).unwrap();
            std::os::unix::fs::symlink(target, manifest).unwrap();
            assert!(super::save_split_project_to_folder_blocking(
                project_dir.display().to_string(),
                saved.project.clone(),
                saved.project.content_revision,
                false,
                &session,
                &coordinator,
            )
            .is_err());
        }
        assert!(session.current().unwrap().is_none());
    }

    #[test]
    fn active_project_session_updates_only_after_successful_split_project_commands() {
        let root = tempfile::tempdir().expect("project session root");
        let project_dir = root.path().join("project");
        let invalid_parent = root.path().join("not-a-directory");
        std::fs::write(&invalid_parent, b"file").expect("invalid parent fixture");
        let app = mock_builder()
            .manage(super::ActiveProjectSessionState::default())
            .manage(super::StorageProjectMutationCoordinatorState)
            .manage(super::SettingsAcceptanceStateHolder(None))
            .invoke_handler(tauri::generate_handler![
                super::save_split_project_to_folder,
                super::load_split_project_from_folder
            ])
            .build(mock_context(noop_assets()))
            .expect("project session app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "active-project-session", Default::default())
                .build()
                .expect("project session webview");
        let project = sample_project();

        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "save_split_project_to_folder".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectDir": project_dir.display().to_string(),
                    "project": project,
                    "expectedRevision": 0
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("successful split project save");
        let state = app.state::<super::ActiveProjectSessionState>();
        let saved = state
            .current()
            .expect("active project state")
            .expect("saved project session");
        assert_eq!(
            saved.canonical_root,
            project_dir.canonicalize().expect("canonical project")
        );

        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "load_split_project_from_folder".into(),
                callback: tauri::ipc::CallbackFn(2),
                error: tauri::ipc::CallbackFn(3),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectDir": project_dir.display().to_string()
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("successful split project load");
        let loaded = state
            .current()
            .expect("active project state")
            .expect("loaded project session");
        assert!(loaded.generation > saved.generation);

        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "save_split_project_to_folder".into(),
                callback: tauri::ipc::CallbackFn(4),
                error: tauri::ipc::CallbackFn(5),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectDir": invalid_parent.join("child").display().to_string(),
                    "project": sample_project(),
                    "expectedRevision": 0
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect_err("failed split project save");
        assert_eq!(state.current().expect("active project state"), Some(loaded));
    }

    #[test]
    fn same_project_save_refuses_old_preview_before_reservation_and_allows_new_preview() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        for directory in [&models, &cache] {
            std::fs::create_dir_all(directory).expect("fixture directory");
        }
        super::save_split_project_to_folder_impl(
            project.display().to_string(),
            sample_project(),
            0,
        )
        .expect("initial project save");
        std::fs::create_dir_all(project.join("renders/render-1"))
            .expect("render fixture directory");
        std::fs::write(project.join("renders/render-1/output.mp4"), b"render")
            .expect("render fixture");
        let session = super::ActiveProjectSessionState::default();
        let coordinator = super::StorageProjectMutationCoordinatorState;
        {
            let lease = coordinator.acquire().expect("mutation lease");
            session
                .record_successful_project_root(&project, &lease)
                .expect("initial project session");
        }
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(session)
            .manage(coordinator)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![
                super::preview_storage_cleanup,
                super::save_split_project_to_folder
            ])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup app");
        let webview = tauri::WebviewWindowBuilder::new(
            &app,
            "same-project-preview-generation",
            Default::default(),
        )
        .build()
        .expect("storage cleanup webview");
        let invoke_preview = || {
            get_ipc_response(
                &webview,
                tauri::webview::InvokeRequest {
                    cmd: "preview_storage_cleanup".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().expect("invoke URL"),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                        "target": {
                            "kind": "projectRenderArtifacts",
                            "artifact_ids": ["render-1"]
                        },
                        "activeProjectDir": project.display().to_string()
                    })),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .expect("render cleanup preview")
            .deserialize::<serde_json::Value>()
            .expect("preview JSON")
        };
        let old_preview = invoke_preview();
        let old_token = old_preview["confirmationToken"]
            .as_str()
            .expect("old token")
            .to_string();
        get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "save_split_project_to_folder".into(),
                callback: tauri::ipc::CallbackFn(2),
                error: tauri::ipc::CallbackFn(3),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectDir": project.display().to_string(),
                    "project": sample_project(),
                    "expectedRevision": 1
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("same-project successful save increments generation");
        let operations = app.state::<super::SettingsOperationRegistryState>();
        let target =
            video_creater_lib::settings::storage::StorageCleanupTarget::ProjectRenderArtifacts {
                artifact_ids: vec!["render-1".to_string()],
            };

        let error = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target.clone(),
            old_token.clone(),
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: cache.clone(),
            },
            Some(project.display().to_string()),
        )
        .expect_err("same-project generation change must invalidate preview");
        assert_eq!(error.code, "settings.storage.cleanupProjectSessionMismatch");
        assert!(operations
            .0
            .list_recent()
            .expect("recent operations")
            .is_empty());

        let new_preview = invoke_preview();
        let new_token = new_preview["confirmationToken"]
            .as_str()
            .expect("new token")
            .to_string();
        assert_ne!(new_token, old_token);
        let writer_lease =
            video_creater_lib::project::mutation::acquire_split_project_mutation_lease(&project)
                .expect("hold canonical writer lease");
        let queued = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target.clone(),
            new_token.clone(),
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: cache,
            },
            Some(project.display().to_string()),
        )
        .expect("new generation preview queues");
        assert_eq!(queued.state, SettingsOperationState::Queued);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            project.join("renders/render-1/output.mp4").is_file(),
            "cleanup must wait for the canonical writer lease"
        );
        assert_ne!(
            operations.0.get(&queued.id).expect("queued cleanup").state,
            SettingsOperationState::Succeeded
        );
        drop(writer_lease);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while operations
            .0
            .get(&queued.id)
            .is_ok_and(|operation| operation.state != SettingsOperationState::Succeeded)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "project cleanup did not continue after writer released"
            );
            std::thread::yield_now();
        }
        assert!(!project.join("renders/render-1/output.mp4").exists());
        let replay = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            target,
            new_token,
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: root.path().join("cache"),
            },
            Some(project.display().to_string()),
        )
        .expect_err("consumed preview token must not replay");
        assert_eq!(replay.code, "settings.storage.cleanupConfirmationMismatch");
    }

    #[test]
    fn switching_away_and_back_refuses_the_old_cleanup_preview_before_reservation() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project_a = root.path().join("project-a");
        let project_b = root.path().join("project-b");
        for directory in [
            &models,
            &cache,
            &project_a.join("renders/render-1"),
            &project_b,
        ] {
            std::fs::create_dir_all(directory).expect("fixture directory");
        }
        std::fs::write(project_a.join("renders/render-1/output.mp4"), b"render")
            .expect("render fixture");
        let session = super::ActiveProjectSessionState::default();
        let coordinator = super::StorageProjectMutationCoordinatorState;
        {
            let lease = coordinator.acquire().expect("mutation lease");
            session
                .record_successful_project_root(&project_a, &lease)
                .expect("project A session");
        }
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(session)
            .manage(coordinator)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![super::preview_storage_cleanup])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup app");
        let webview = tauri::WebviewWindowBuilder::new(
            &app,
            "away-back-preview-generation",
            Default::default(),
        )
        .build()
        .expect("storage cleanup webview");
        let preview = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "preview_storage_cleanup".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "target": {
                        "kind": "projectRenderArtifacts",
                        "artifact_ids": ["render-1"]
                    },
                    "activeProjectDir": project_a.display().to_string()
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("project A preview")
        .deserialize::<serde_json::Value>()
        .expect("preview JSON");
        {
            let coordinator = app.state::<super::StorageProjectMutationCoordinatorState>();
            let lease = coordinator.acquire().expect("mutation lease");
            let state = app.state::<super::ActiveProjectSessionState>();
            state
                .record_successful_project_root(&project_b, &lease)
                .expect("switch to project B");
            state
                .record_successful_project_root(&project_a, &lease)
                .expect("switch back to project A");
        }
        let operations = app.state::<super::SettingsOperationRegistryState>();
        let error = super::start_storage_cleanup(
            app.handle(),
            &operations.0,
            video_creater_lib::settings::storage::StorageCleanupTarget::ProjectRenderArtifacts {
                artifact_ids: vec!["render-1".to_string()],
            },
            preview["confirmationToken"]
                .as_str()
                .expect("old token")
                .to_string(),
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: cache,
            },
            Some(project_a.display().to_string()),
        )
        .expect_err("A to B to A must invalidate the project A preview");

        assert_eq!(error.code, "settings.storage.cleanupProjectSessionMismatch");
        assert!(operations
            .0
            .list_recent()
            .expect("recent operations")
            .is_empty());
        assert!(project_a.join("renders/render-1/output.mp4").is_file());
    }

    #[test]
    fn project_session_mutation_waits_for_verified_cleanup_deletion() {
        use std::sync::{mpsc, Arc};

        let root = tempfile::tempdir().expect("storage cleanup root");
        let cache = root.path().join("cache");
        let first_project = root.path().join("first-project");
        let second_project = root.path().join("second-project");
        for directory in [&cache, &first_project, &second_project] {
            std::fs::create_dir_all(directory).expect("fixture directory");
        }
        let selected = cache.join("selected.tmp");
        std::fs::write(&selected, b"delete after lease release").expect("cache fixture");
        let target = video_creater_lib::settings::storage::StorageCleanupTarget::DisposableAppCache;
        let preview =
            video_creater_lib::settings::storage::preview_storage_cleanup(&target, &cache, None)
                .expect("cleanup preview");
        let coordinator = Arc::new(super::StorageProjectMutationCoordinatorState);
        let session = Arc::new(super::ActiveProjectSessionState::default());
        {
            let lease = coordinator.acquire().expect("initial mutation lease");
            session
                .record_successful_project_root(&first_project, &lease)
                .expect("first project session");
        }
        let (quarantine_ready_tx, quarantine_ready_rx) = mpsc::sync_channel(1);
        let (continue_tx, continue_rx) = mpsc::sync_channel(1);
        let cleanup_coordinator = Arc::clone(&coordinator);
        let cleanup_cache = cache.clone();
        let cleanup_selected = selected.clone();
        let cleanup_target = target.clone();
        let cleanup_token = preview.confirmation_token.clone();
        let cleanup = std::thread::spawn(move || {
            let _lease = cleanup_coordinator
                .acquire()
                .expect("cleanup mutation lease");
            video_creater_lib::settings::storage::run_storage_cleanup_with_hook(
                &cleanup_target,
                &cleanup_token,
                &cleanup_cache,
                None,
                |_| {
                    quarantine_ready_tx.send(()).expect("quarantine ready");
                    continue_rx.recv().expect("continue cleanup");
                    Ok(())
                },
                |_| Ok(()),
            )
            .expect("cleanup result")
        });
        quarantine_ready_rx
            .recv()
            .expect("cleanup reached boundary");
        let (mutation_finished_tx, mutation_finished_rx) = mpsc::sync_channel(1);
        let mutation_coordinator = Arc::clone(&coordinator);
        let mutation_session = Arc::clone(&session);
        let mutation_project = second_project.clone();
        let mutation = std::thread::spawn(move || {
            let lease = mutation_coordinator
                .acquire()
                .expect("project mutation lease");
            assert!(
                !cleanup_selected.exists(),
                "project mutation acquired before verified deletion"
            );
            mutation_session
                .record_successful_project_root(&mutation_project, &lease)
                .expect("second project session");
            mutation_finished_tx.send(()).expect("mutation finished");
        });

        assert!(matches!(
            mutation_finished_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(selected.is_file());
        continue_tx.send(()).expect("continue verified cleanup");
        let report = cleanup.join().expect("cleanup thread");
        mutation_finished_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("project mutation eventually completes");
        mutation.join().expect("mutation thread");

        assert_eq!(report.removed_count, 1);
        assert!(!selected.exists());
        assert_eq!(
            session
                .current()
                .expect("active project state")
                .expect("second project session")
                .canonical_root,
            second_project
                .canonicalize()
                .expect("canonical second project")
        );
    }

    #[test]
    fn storage_cleanup_project_switch_invalidates_an_old_preview_token() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let first = root.path().join("first-project");
        let second = root.path().join("second-project");
        for directory in [
            &models,
            &cache,
            &first.join("renders/render-1"),
            &second.join("renders/render-2"),
        ] {
            std::fs::create_dir_all(directory).expect("fixture directory");
        }
        std::fs::write(first.join("renders/render-1/output.mp4"), b"first").expect("first render");
        let session = super::ActiveProjectSessionState::default();
        let coordinator = super::StorageProjectMutationCoordinatorState;
        let first_session = {
            let lease = coordinator.acquire().expect("mutation lease");
            session
                .record_successful_project_root(&first, &lease)
                .expect("record first project")
        };
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(session)
            .manage(coordinator)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .build(mock_context(noop_assets()))
            .expect("storage cleanup app");
        let target =
            video_creater_lib::settings::storage::StorageCleanupTarget::ProjectRenderArtifacts {
                artifact_ids: vec!["render-1".to_string()],
            };
        let preview = video_creater_lib::settings::storage::preview_storage_cleanup_for_generation(
            &target,
            &cache,
            Some(&first),
            Some(first_session.generation),
        )
        .expect("first project preview");
        {
            let coordinator = app.state::<super::StorageProjectMutationCoordinatorState>();
            let lease = coordinator.acquire().expect("mutation lease");
            app.state::<super::StorageCleanupPreviewRegistryState>()
                .register(&preview, Some(&first_session), &lease)
                .expect("register preview");
            app.state::<super::ActiveProjectSessionState>()
                .record_successful_project_root(&second, &lease)
                .expect("switch active project");
        }
        let registry = app.state::<super::SettingsOperationRegistryState>();

        let error = super::start_storage_cleanup(
            app.handle(),
            &registry.0,
            target,
            preview.confirmation_token,
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: cache,
            },
            Some(first.display().to_string()),
        )
        .expect_err("old project preview must be invalid after project switch");

        assert_eq!(error.code, "settings.storage.cleanupProjectSessionMismatch");
        assert!(first.join("renders/render-1/output.mp4").is_file());
    }

    #[test]
    fn storage_cleanup_terminal_reports_partial_completion_after_progress_persistence_failure() {
        let root = tempfile::tempdir().expect("storage cleanup root");
        let cache = root.path().join("cache");
        std::fs::create_dir_all(&cache).expect("cache root");
        std::fs::write(cache.join("a-first.tmp"), b"first").expect("first cache item");
        std::fs::write(cache.join("b-second.tmp"), b"second").expect("second cache item");
        let target = video_creater_lib::settings::storage::StorageCleanupTarget::DisposableAppCache;
        let preview =
            video_creater_lib::settings::storage::preview_storage_cleanup(&target, &cache, None)
                .expect("cleanup preview");
        let failure = video_creater_lib::settings::storage::run_storage_cleanup(
            &target,
            &preview.confirmation_token,
            &cache,
            None,
            |_| {
                Err(
                    video_creater_lib::settings::storage::StorageCleanupError::Progress(
                        "injected journal persistence failure".to_string(),
                    ),
                )
            },
        )
        .expect_err("progress persistence failure");

        let transition = super::storage_cleanup_failed_transition(&failure, 2);

        assert_eq!(transition.completed_units, Some(1));
        assert_eq!(failure.partial_report.removed_bytes, 5);
        assert!(transition
            .message
            .as_deref()
            .is_some_and(|message| message.contains("persistence boundary is unknown")));
        assert!(transition
            .error
            .as_ref()
            .and_then(|error| error.detail.as_deref())
            .is_some_and(|detail| detail.contains("a-first.tmp")));
        assert!(!cache.join("a-first.tmp").exists());
        assert!(cache.join("b-second.tmp").is_file());
    }

    #[test]
    fn storage_cleanup_runs_in_background_with_item_progress_and_refreshed_health() {
        use std::sync::{Arc, Mutex};
        use tauri::Listener;

        let root = tempfile::tempdir().expect("storage cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let journal = root.path().join("settings/operations.json");
        std::fs::create_dir_all(&models).expect("models root");
        std::fs::create_dir_all(&cache).expect("cache root");
        std::fs::write(cache.join("a.tmp"), vec![0; 3]).expect("first cache fixture");
        std::fs::write(cache.join("b.tmp"), vec![0; 5]).expect("second cache fixture");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let app = mock_builder()
            .manage(super::StorageInventoryRootsState {
                global_model_root: models,
                app_cache_root: cache.clone(),
            })
            .manage(super::SettingsOperationRegistryState(registry))
            .manage(super::ActiveProjectSessionState::default())
            .manage(super::StorageProjectMutationCoordinatorState)
            .manage(super::StorageCleanupPreviewRegistryState::default())
            .invoke_handler(tauri::generate_handler![super::preview_storage_cleanup])
            .build(mock_context(noop_assets()))
            .expect("storage cleanup app");
        let observed_operations = Arc::new(Mutex::new(Vec::new()));
        let observed_operations_for_listener = observed_operations.clone();
        app.listen(super::SETTINGS_OPERATION_EVENT, move |event| {
            let operation: SettingsOperation =
                serde_json::from_str(event.payload()).expect("settings operation event");
            observed_operations_for_listener
                .lock()
                .expect("observed operations")
                .push(operation);
        });
        let observed_health = Arc::new(Mutex::new(Vec::new()));
        let observed_health_for_listener = observed_health.clone();
        app.listen("settings-storage-health", move |event| {
            let health: serde_json::Value =
                serde_json::from_str(event.payload()).expect("storage health event");
            observed_health_for_listener
                .lock()
                .expect("observed storage health")
                .push(health);
        });
        let webview = tauri::WebviewWindowBuilder::new(&app, "storage-cleanup", Default::default())
            .build()
            .expect("storage cleanup webview");
        let target_json = serde_json::json!({ "kind": "disposableAppCache" });
        let preview = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "preview_storage_cleanup".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "target": target_json,
                    "activeProjectDir": null
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("preview response")
        .deserialize::<serde_json::Value>()
        .expect("preview JSON");
        let confirmation_token = preview["confirmationToken"]
            .as_str()
            .expect("confirmation token")
            .to_string();

        let started_at = std::time::Instant::now();
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let queued = super::start_storage_cleanup(
            app.handle(),
            &registry.0,
            video_creater_lib::settings::storage::StorageCleanupTarget::DisposableAppCache,
            confirmation_token,
            super::StorageInventoryRootsState {
                global_model_root: root.path().join("models"),
                app_cache_root: cache.clone(),
            },
            None,
        )
        .expect("queued cleanup operation");
        assert!(started_at.elapsed() < std::time::Duration::from_millis(100));
        assert_eq!(queued.kind, SettingsOperationKind::StorageCleanup);
        assert_eq!(queued.state, SettingsOperationState::Queued);
        assert_eq!(queued.total_units, None);
        assert_eq!(queued.unit.as_deref(), Some("items"));
        assert!(!queued.cancellable);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if registry
                .0
                .get(&queued.id)
                .is_ok_and(|operation| operation.state == SettingsOperationState::Succeeded)
                && !observed_health
                    .lock()
                    .expect("observed storage health")
                    .is_empty()
                && observed_operations
                    .lock()
                    .expect("observed operations")
                    .last()
                    .is_some_and(|operation| operation.state == SettingsOperationState::Succeeded)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "storage cleanup did not finish"
            );
            std::thread::yield_now();
        }

        assert!(!cache.join("a.tmp").exists());
        assert!(!cache.join("b.tmp").exists());
        let operations = observed_operations.lock().expect("observed operations");
        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.state.clone())
                .collect::<Vec<_>>(),
            vec![
                SettingsOperationState::Queued,
                SettingsOperationState::Running,
                SettingsOperationState::Running,
                SettingsOperationState::Running,
                SettingsOperationState::Running,
                SettingsOperationState::Succeeded,
            ]
        );
        assert_eq!(operations[2].completed_units, 0);
        assert_eq!(operations[2].total_units, Some(2));
        assert_eq!(operations[3].completed_units, 1);
        assert_eq!(operations[4].completed_units, 2);
        let health = observed_health.lock().expect("observed storage health");
        assert_eq!(health[0]["id"], "storage");
        assert_eq!(health[0]["items"][1]["provenance"]["bytes"], "0");
        drop(operations);
        drop(health);

        let restarted = super::SettingsOperationRegistry::load(journal, |_| false)
            .expect("restarted operation registry");
        assert_eq!(
            restarted.get(&queued.id).expect("persisted cleanup").state,
            SettingsOperationState::Succeeded
        );
    }

    #[test]
    fn interrupted_storage_cleanup_is_reconciled_and_can_be_retried() {
        let root = tempfile::tempdir().expect("operation root");
        let journal = root.path().join("settings/operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let target = video_creater_lib::settings::storage::StorageCleanupTarget::DisposableAppCache;
        let confirmation_token = format!("storage-cleanup-v1:{}", "a".repeat(64));
        let (queued, created) =
            super::reserve_storage_cleanup(&registry, &target, &confirmation_token)
                .expect("reserve cleanup");
        assert!(created);
        registry
            .update(
                &queued.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("persist running cleanup");
        drop(registry);

        let restarted = super::SettingsOperationRegistry::load(journal, |_| false)
            .expect("reload operation registry");
        let interrupted = restarted.get(&queued.id).expect("interrupted cleanup");
        assert_eq!(interrupted.state, SettingsOperationState::Failed);
        assert_eq!(interrupted.phase, "interrupted");
        assert_eq!(
            interrupted.error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.interrupted")
        );
        let (retry, retry_created) =
            super::reserve_storage_cleanup(&restarted, &target, &confirmation_token)
                .expect("retry cleanup");
        assert!(retry_created);
        assert_ne!(retry.id, queued.id);
        assert_eq!(retry.state, SettingsOperationState::Queued);
    }

    #[test]
    fn settings_health_command_returns_stable_structured_error() {
        let root = tempfile::tempdir().expect("temp model root");
        let state = super::TranscriptionModelStoreState(std::sync::Mutex::new(
            super::TranscriptionModelStore::new(root.path().to_path_buf()),
        ));
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            let _store = state.0.lock().expect("model store lock");
            panic!("poison model store for command error test");
        }));
        assert!(poisoned.is_err());
        let app = settings_health_test_app(state);

        let error = invoke_settings_health(&app).expect_err("structured settings health error");

        assert_eq!(error["code"], "settings.modelStoreUnavailable");
        assert_eq!(
            error["message"],
            "Settings health is temporarily unavailable."
        );
        assert_eq!(error["detail"], "model store lock failed");
    }

    #[test]
    fn agent_category_health_is_reachable_without_model_store_state() {
        let app = mock_builder()
            .manage(test_app_preferences_state())
            .invoke_handler(tauri::generate_handler![super::get_agent_settings_health])
            .build(mock_context(noop_assets()))
            .expect("agent category health app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "agent-health", Default::default())
            .build()
            .expect("agent category health webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_agent_settings_health".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("agent category response")
        .deserialize::<serde_json::Value>()
        .expect("agent category JSON");

        assert_eq!(response["id"], "agent");
        assert_eq!(
            response["items"].as_array().map(|items| items
                .iter()
                .filter_map(|item| item["id"].as_str())
                .collect::<Vec<_>>()),
            Some(vec![
                "agent.codex",
                "agent.claude",
                "agent.mcpServer",
                "agent.proposalValidator"
            ])
        );
    }

    #[test]
    fn skills_category_health_is_reachable_with_a_poisoned_model_store() {
        let project = tempfile::tempdir().expect("project root");
        std::fs::create_dir_all(project.path().join(".git")).expect("project repository marker");
        std::fs::write(project.path().join("AGENTS.md"), "# Test project\n")
            .expect("write project marker");
        for definition in video_creater_lib::settings::skills::MANDATORY_SKILLS {
            let path = project.path().join(definition.relative_path);
            std::fs::create_dir_all(path.parent().expect("skill parent"))
                .expect("create skill parent");
            std::fs::write(path, definition.bundled_content).expect("write bundled skill");
        }
        let model_root = tempfile::tempdir().expect("model root");
        let model_state = super::TranscriptionModelStoreState(std::sync::Mutex::new(
            super::TranscriptionModelStore::new(model_root.path().to_path_buf()),
        ));
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            let _store = model_state.0.lock().expect("model store lock");
            panic!("poison model store");
        }));
        assert!(poisoned.is_err());
        let app = mock_builder()
            .manage(model_state)
            .invoke_handler(tauri::generate_handler![super::get_skills_settings_health])
            .build(mock_context(noop_assets()))
            .expect("skills category health app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "skills-health", Default::default())
            .build()
            .expect("skills category health webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "get_skills_settings_health".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectRoot": project.path().display().to_string()
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("skills category response")
        .deserialize::<serde_json::Value>()
        .expect("skills category JSON");

        assert_eq!(response["id"], "skills");
        assert_eq!(response["state"], "ready");
        assert_eq!(response["items"].as_array().map(Vec::len), Some(3));
    }

    #[test]
    fn agent_component_self_test_uses_the_managed_operation_registry() {
        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("agent self-test app");
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let queued = super::start_agent_component_self_test(
            app.handle(),
            "agent.proposalValidator",
            &registry.0,
        )
        .expect("agent self-test response");
        assert_eq!(queued.kind, SettingsOperationKind::HealthCheck);
        assert_eq!(queued.target_id, "agent.proposalValidator");
        assert_eq!(queued.state, SettingsOperationState::Queued);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let current = registry.0.get(&queued.id).expect("self-test operation");
            if current.state == SettingsOperationState::Succeeded {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "self-test did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let persisted = app
            .state::<super::SettingsOperationRegistryState>()
            .0
            .list_recent()
            .expect("persisted self-test operation");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].target_id, "agent.proposalValidator");
        assert_eq!(persisted[0].state, SettingsOperationState::Succeeded);
    }

    #[test]
    fn interrupted_agent_self_test_is_reconciled_from_the_shared_journal() {
        let root = tempfile::tempdir().expect("operation root");
        let journal = root.path().join("settings/operations.json");
        let registry = super::SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let (queued, created) =
            super::reserve_agent_component_self_test("agent.proposalValidator", &registry)
                .expect("reserve self-test");
        assert!(created);
        registry
            .update(
                &queued.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("persist running self-test");
        drop(registry);

        let restarted = super::SettingsOperationRegistry::load(journal, |_| false)
            .expect("reload operation registry");
        let recovered = restarted.list_recent().expect("recovered operations");

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].target_id, "agent.proposalValidator");
        assert_eq!(recovered[0].state, SettingsOperationState::Failed);
        assert_eq!(recovered[0].phase, "interrupted");
        assert_eq!(
            recovered[0].error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.interrupted")
        );
    }

    #[test]
    fn agent_self_test_emits_every_shared_operation_transition() {
        use std::sync::{Arc, Mutex};
        use tauri::Listener;

        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("agent self-test event app");
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_listener = observed.clone();
        app.listen(super::SETTINGS_OPERATION_EVENT, move |event| {
            let operation: SettingsOperation =
                serde_json::from_str(event.payload()).expect("settings operation event");
            observed_for_listener
                .lock()
                .expect("observed events")
                .push(operation.state);
        });
        let registry = app.state::<super::SettingsOperationRegistryState>();

        let queued = super::start_agent_component_self_test(
            app.handle(),
            "agent.proposalValidator",
            &registry.0,
        )
        .expect("run self-test");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if observed.lock().expect("observed events").len() == 3 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "events did not arrive"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(queued.state, SettingsOperationState::Queued);

        assert_eq!(
            *observed.lock().expect("observed events"),
            vec![
                SettingsOperationState::Queued,
                SettingsOperationState::Running,
                SettingsOperationState::Succeeded,
            ]
        );
    }

    #[test]
    fn agent_self_test_start_returns_queued_before_background_terminal() {
        use std::time::{Duration, Instant};

        let root = tempfile::tempdir().expect("operation root");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .build(mock_context(noop_assets()))
            .expect("agent self-test start app");
        let registry = app.state::<super::SettingsOperationRegistryState>();
        let started_at = Instant::now();

        let queued = super::start_agent_component_self_test(
            app.handle(),
            "agent.proposalValidator",
            &registry.0,
        )
        .expect("queue self-test");

        assert_eq!(queued.state, SettingsOperationState::Queued);
        assert!(started_at.elapsed() < Duration::from_millis(100));
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let terminal = registry.0.get(&queued.id).expect("self-test operation");
            if terminal.state == SettingsOperationState::Succeeded {
                break;
            }
            assert!(Instant::now() < deadline, "self-test did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn mcp_client_configuration_is_reachable_and_project_aware_through_tauri_ipc() {
        let project = tempfile::tempdir().expect("project directory");
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![super::mcp_client_configuration])
            .build(mock_context(noop_assets()))
            .expect("MCP configuration app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "mcp-configuration", Default::default())
                .build()
                .expect("MCP configuration webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "mcp_client_configuration".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "activeProjectDir": project.path().display().to_string()
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("MCP configuration response")
        .deserialize::<serde_json::Value>()
        .expect("MCP configuration JSON");

        assert_eq!(response["actionLabel"], "Copy client configuration");
        assert_eq!(response["projectDir"], project.path().display().to_string());
        let configuration: serde_json::Value = serde_json::from_str(
            response["configuration"]
                .as_str()
                .expect("copyable configuration"),
        )
        .expect("configuration JSON");
        assert_eq!(
            configuration["mcpServers"]["video-creater"]["args"],
            serde_json::json!(["--project-dir", project.path()])
        );
    }

    #[test]
    fn skill_repair_command_rejects_mismatched_confirmation_without_writing() {
        let root = tempfile::tempdir().expect("repository root");
        std::fs::write(root.path().join("AGENTS.md"), "project policy\n").expect("write AGENTS.md");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let ids = vec!["video-creater-graphics".to_string()];

        let preview = super::repair_bundled_skills_for_root(
            Some(root.path().to_str().expect("root string")),
            &ids,
            None,
            &registry,
        )
        .expect("preview");
        assert!(preview.operation.is_none());
        assert!(preview.report.is_none());
        assert_eq!(preview.preview.affected_paths.len(), 1);

        let mismatched = vec![root.path().join("wrong/SKILL.md").display().to_string()];
        let error = super::repair_bundled_skills_for_root(
            Some(root.path().to_str().expect("root string")),
            &ids,
            Some(&mismatched),
            &registry,
        )
        .expect_err("mismatched paths must fail");

        assert_eq!(error.code, "skills.confirmationMismatch");
        assert!(!root
            .path()
            .join(".agents/skills/video-creater-graphics/SKILL.md")
            .exists());
        assert!(registry.list_recent().expect("operations").is_empty());
    }

    #[test]
    fn skill_repair_command_repairs_only_confirmed_preview_paths() {
        let root = tempfile::tempdir().expect("repository root");
        std::fs::write(root.path().join("AGENTS.md"), "project policy\n").expect("write AGENTS.md");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let ids = vec!["video-creater-visuals".to_string()];

        let preview = super::repair_bundled_skills_for_root(
            Some(root.path().to_str().expect("root string")),
            &ids,
            None,
            &registry,
        )
        .expect("preview");
        let confirmed_paths = preview
            .preview
            .affected_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        let result = super::repair_bundled_skills_for_root(
            Some(root.path().to_str().expect("root string")),
            &ids,
            Some(&confirmed_paths),
            &registry,
        )
        .expect("confirmed repair");

        let operation = result.operation.expect("shared operation");
        let report = result.report.expect("repair report");
        assert_eq!(operation.kind, SettingsOperationKind::SkillRepair);
        assert_eq!(operation.state, SettingsOperationState::Succeeded);
        assert_eq!(report.repaired_paths, preview.preview.affected_paths);
        assert!(report.verification.iter().any(|item| {
            item.id == "video-creater-visuals"
                && item.load_state
                    == video_creater_lib::settings::skills::SkillLoadState::MatchesBundled
        }));
        assert_eq!(
            std::fs::read_to_string(&report.repaired_paths[0]).expect("repaired skill"),
            video_creater_lib::settings::skills::mandatory_skill_definition(
                "video-creater-visuals"
            )
            .expect("visuals definition")
            .bundled_content
        );
    }

    #[test]
    fn skill_repair_command_is_reachable_through_tauri_ipc_for_preview() {
        let root = tempfile::tempdir().expect("repository root");
        std::fs::write(root.path().join("AGENTS.md"), "project policy\n").expect("write AGENTS.md");
        let registry = super::SettingsOperationRegistry::load(
            root.path().join("settings/operations.json"),
            |_| false,
        )
        .expect("operation registry");
        let app = mock_builder()
            .manage(super::SettingsOperationRegistryState(registry))
            .invoke_handler(tauri::generate_handler![super::repair_bundled_skills])
            .build(mock_context(noop_assets()))
            .expect("skill repair app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "skill-repair", Default::default())
            .build()
            .expect("skill repair webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "repair_bundled_skills".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectRoot": root.path().display().to_string(),
                    "skillIds": ["video-creater-video-pipeline"],
                    "confirmedPaths": null
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("skill repair preview response")
        .deserialize::<serde_json::Value>()
        .expect("skill repair preview JSON");

        assert_eq!(
            response["preview"]["skillIds"][0],
            "video-creater-video-pipeline"
        );
        assert_eq!(
            response["preview"]["affectedPaths"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(response["operation"].is_null());
        assert!(response["report"].is_null());
    }

    #[test]
    fn lists_builtin_and_project_shader_background_templates() {
        let project = tempfile::tempdir().expect("project root");
        let template_dir = project
            .path()
            .join("shader-background-templates/user-neon-v1");
        std::fs::create_dir_all(&template_dir).expect("template dir");
        std::fs::write(
            template_dir.join("template.json"),
            r##"{
  "id": "user-neon-v1",
  "version": 1,
  "title": "User Neon",
  "sourceKind": "user",
  "category": "user",
  "shaderProfileId": "user-neon-v1",
  "defaultDurationSeconds": 3.0,
  "sourceUrl": null,
  "license": "user-provided",
  "shaderRef": "shader.frag",
  "utilityRefs": ["shared/utils.glsl"],
  "placement": { "trackKind": "hyperframe_scene", "defaultStartSeconds": 0.0 },
  "renderContract": { "dimensions": "project", "fps": "project", "alpha": false },
  "preview": {
    "thumbnailKind": "css",
    "accentColor": "#38bdf8",
    "secondaryColor": "#f97316",
    "description": "User-authored neon field"
  },
  "visualTreatment": "user-authored shader background with neon gradients and soft depth",
  "motion": "slow procedural shimmer with stable low-frequency drift",
  "safeZone": "keep essential motion inside 10% margins",
  "avoid": "strobing, external textures, URLs, and unsafe high-frequency noise",
  "agentSummary": "user-neon-v1: User Neon. kind shader_background.",
  "notes": ["Loaded from a project-local user template folder."]
}"##,
        )
        .expect("template config");
        std::fs::write(
            template_dir.join("shader.frag"),
            "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, 0.5 + 0.5 * sin(time + progress), 1.0); }",
        )
        .expect("template shader");

        let templates = list_shader_background_templates(Some(
            project.path().to_str().expect("utf-8 path").to_string(),
        ))
        .expect("catalog");

        assert_eq!(templates.len(), 18);
        assert_eq!(templates[0].id, "shadertoy-octagrams-v1");
        let user = templates
            .iter()
            .find(|template| template.id == "user-neon-v1")
            .expect("user template");
        assert_eq!(user.name, "User Neon");
        assert_eq!(user.source_kind, super::ShaderTemplateSourceKind::User);
        assert!(user
            .config_path
            .contains("shader-background-templates/user-neon-v1/template.json"));
    }

    #[test]
    fn rejects_relative_codex_project_root() {
        let error =
            resolve_codex_project_root(Some("relative/root")).expect_err("relative root must fail");

        assert_eq!(error, "project root must be an absolute path");
    }

    #[test]
    fn split_project_commands_save_and_validate_project_folder() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let project = sample_project();

        let report = save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        assert!(report.manifest_path.ends_with("video-creater.project.json"));
        assert!(project_dir.join("timeline.json").exists());

        let validation = validate_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
        )
        .expect("validate split project");

        assert!(validation.ok);
        assert!(validation.issues.is_empty());
    }

    #[test]
    fn canonical_preview_command_returns_an_ephemeral_prepared_project() {
        let temporary = tempfile::tempdir().expect("temporary project");
        let project = sample_project();

        let prepared = prepare_project_preview_blocking(
            temporary.path().display().to_string(),
            project.clone(),
        )
        .expect("prepare preview");

        assert_eq!(prepared.project.id, project.id);
        assert_eq!(
            prepared.project.timeline.duration_seconds,
            project.timeline.duration_seconds
        );
        assert_eq!(
            prepared.project.timeline.tracks[0].items,
            project.timeline.tracks[0].items
        );
        assert_eq!(project.timeline.tracks[0].id, "track-video");
        assert_eq!(prepared.project.timeline.tracks[0].id, "root:track-video:0");
        assert!(prepared.reports.is_empty());
    }

    #[test]
    fn canonical_preview_waits_for_project_lease_and_reloads_canonical_state() {
        use std::sync::mpsc;

        let temporary = tempfile::tempdir().expect("temporary project");
        let project_dir = temporary.path().join("project");
        let stale_project = sample_project();
        save_split_project_to_folder(project_dir.display().to_string(), stale_project.clone())
            .expect("save split fixture");
        let mut canonical_project = stale_project.clone();
        canonical_project.name = "Settings-promoted name".to_string();
        save_split_project_to_folder(project_dir.display().to_string(), canonical_project.clone())
            .expect("save promoted fixture");
        let lease = video_creater_lib::project::mutation::acquire_split_project_mutation_lease(
            &project_dir,
        )
        .expect("hold settings-compatible project lease");
        let writer_project_dir = project_dir.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = prepare_project_preview_blocking(
                writer_project_dir.display().to_string(),
                stale_project,
            );
            done_tx.send(()).expect("preview completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        drop(lease);
        let prepared = writer.join().expect("preview thread").expect("preview");
        assert_eq!(prepared.project.name, canonical_project.name);
    }

    #[test]
    fn split_project_command_loads_app_server_conversation_history() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let project = sample_project();
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project.clone(),
        )
        .expect("save split project");
        let request = sample_edit_request();
        record_app_server_conversation_turn(
            &project_dir,
            &project.id,
            "thread-new",
            AppServerConversationTurn {
                turn_id: Some("turn-1".to_string()),
                turn_status: Some("completed".to_string()),
                prompt: request.prompt.clone(),
                created_at: request.created_at.clone(),
                request: serde_json::to_value(&request).expect("request json"),
                thread_response: serde_json::json!({ "thread": { "id": "thread-new" } }),
                turn_response: serde_json::json!({ "turn": { "id": "turn-1", "status": "completed" } }),
                has_proposal: true,
                provider: None,
                provider_session_id: None,
            },
        )
        .expect("record conversation");

        let history = load_app_server_conversation_history_from_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
        )
        .expect("load conversation history");

        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].thread_id, "thread-new");
        assert_eq!(history.entries[0].turn_id.as_deref(), Some("turn-1"));
        assert_eq!(history.entries[0].prompt, request.prompt);
        assert!(history.entries[0].has_proposal);
    }

    #[test]
    fn temporal_start_result_command_builds_running_status_action() {
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            "project-1",
            "generated-shot-1",
            JobStatus::Queued,
            "2026-06-23T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-shot-1",
            "generated-shot-1",
            true,
            None,
        ));

        let action = build_temporal_start_result_action(
            job,
            "temporal-run-1".to_string(),
            "2026-06-23T12:01:00Z".to_string(),
        )
        .expect("start result action");

        assert_eq!(
            action,
            ProjectAction::UpdateJobStatus {
                job_id: "generated-shot-1".to_string(),
                status: JobStatus::Running,
                updated_at: "2026-06-23T12:01:00Z".to_string(),
                run_id: Some("temporal-run-1".to_string()),
            }
        );
    }

    #[cfg(not(feature = "temporal-worker"))]
    #[test]
    fn temporal_start_command_reports_unavailable_runtime_without_run_id() {
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            "project-1",
            "generated-shot-1",
            JobStatus::Queued,
            "2026-06-23T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-shot-1",
            "generated-shot-1",
            false,
            None,
        ));

        let result = pollster::block_on(start_temporal_workflow(job)).expect("start result");

        assert_eq!(result.status, "unavailable");
        assert_eq!(
            result.workflow_id,
            "video-creater/project-1/generate-media/generated-shot-1"
        );
        assert_eq!(result.run_id, None);
        assert!(result.message.contains("temporal-worker"));
    }

    #[test]
    fn temporal_worker_environment_report_command_includes_runbook() {
        let report = get_temporal_worker_environment_report();

        assert_eq!(report.task_queue, "video-creater-workflows");
        assert_eq!(report.local_service_target, "http://localhost:7233");
        assert_eq!(report.local_web_ui_url, "http://localhost:8233");
        assert_eq!(report.local_dev_command, "temporal server start-dev");
        assert_eq!(report.feature_name, "temporal-worker");
        assert!(report
            .worker_run_command
            .ends_with("--bin video-creater-temporal-worker"));
        assert!(report.tools.iter().any(|tool| tool.name == "protoc"));
        assert!(report.tools.iter().any(|tool| tool.name == "temporal"));
    }

    #[test]
    fn export_profile_availability_report_command_reports_the_current_runtime_state() {
        let report = get_export_profile_availability_report();

        assert_eq!(
            report
                .iter()
                .map(|profile| profile.profile)
                .collect::<Vec<_>>(),
            vec![
                ExportProfile::Webm,
                ExportProfile::Mp4H264,
                ExportProfile::Mp4H265,
                ExportProfile::ProResMov,
            ]
        );
        assert!(report.iter().all(|profile| {
            if profile.available {
                profile.policy_status
                    == video_creater_lib::project::export_profiles::ExportPolicyStatus::Approved
                    && profile.unavailable_reason.is_none()
            } else {
                profile.unavailable_reason.is_some()
            }
        }));
    }

    #[test]
    fn generation_model_catalog_command_reuses_codex_list_models_payload() {
        let payload = super::list_generation_model_catalog().expect("generation model catalog");

        assert_eq!(payload["loaded"], serde_json::json!(true));
        assert_eq!(
            payload["providerCredentialsExposed"],
            serde_json::json!(false)
        );
        let generation_models = payload["generationModels"]
            .as_array()
            .expect("generation models");
        assert!(generation_models.iter().any(|model| {
            model["provider"] == serde_json::json!("google")
                && model["id"] == serde_json::json!("veo3.1-fast")
                && model["kind"] == serde_json::json!("video")
                && model["durations"] == serde_json::json!([8])
                && model["cancellationCapability"] == serde_json::json!("local")
        }));
        for model_id in [
            "black-forest-labs/flux-schnell",
            "black-forest-labs/flux-dev",
            "black-forest-labs/flux-1.1-pro",
            "black-forest-labs/flux-1.1-pro-ultra",
        ] {
            assert!(generation_models.iter().any(|model| {
                model["provider"] == serde_json::json!("replicate")
                    && model["id"] == serde_json::json!(model_id)
                    && model["kind"] == serde_json::json!("image")
                    && model["responseShape"] == serde_json::json!("images")
                    && model["cancellationCapability"] == serde_json::json!("provider")
                    && model["uiCapabilities"]["maxImages"] == serde_json::json!(4)
            }));
        }
    }

    #[test]
    fn provider_account_status_reports_known_byok_providers_without_balance_endpoints() {
        let fixture_secret = "provider-account-fixture-secret-330d";
        let providers = ["openai", "xai", "elevenlabs", "google", "minimax"];

        for provider in providers {
            let status = super::provider_account_status_with_credential(
                provider.to_string(),
                fixture_secret,
            );
            assert_eq!(status.provider, provider);
            assert_eq!(status.credential_status, "balanceUnavailable");
            assert_eq!(status.account_name, None);
            assert_eq!(status.balance, None);
            assert!(status.detail.contains("credential presence checked"));
            assert!(!status.detail.contains("Unsupported provider"));
            let payload = serde_json::to_value(&status).expect("serialize account status payload");
            assert_provider_payload_secret_free(&payload, fixture_secret);
            assert!(payload.get("credentialEnvVar").is_none());
        }
    }

    #[test]
    fn provider_credential_save_ipc_error_omits_raw_credential() {
        let fixture_secret = "provider-save-ipc-fixture-secret-a590";
        let credential = fixture_secret.repeat(
            (video_creater_lib::provider_credentials::MAX_PROVIDER_CREDENTIAL_BYTES
                / fixture_secret.len())
            .saturating_add(2),
        );
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![super::set_provider_credential])
            .build(mock_context(noop_assets()))
            .expect("provider credential IPC app");
        let webview =
            tauri::WebviewWindowBuilder::new(&app, "provider-credential", Default::default())
                .build()
                .expect("provider credential IPC webview");

        let error = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "set_provider_credential".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "provider": "openai",
                    "credential": credential,
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect_err("oversized credential must be rejected");
        let serialized = serde_json::to_string(&error).expect("serialize IPC error");

        assert!(!serialized.contains(fixture_secret));
        assert!(serialized.contains("provider credential exceeds"));
    }

    #[test]
    fn provider_dependencies_use_enabled_catalog_rows_and_client_model_preferences() {
        let payload = serde_json::json!({
            "generationModels": [
                { "provider": "fal.ai", "id": "video-a" },
                { "provider": "fal.ai", "id": "video-b" },
                { "provider": "openai", "id": "image-a" },
                { "provider": "mock", "id": "mock-upscale-v1" },
                { "provider": "google" }
            ]
        });

        let dependencies = super::provider_model_dependencies_from_payload(
            &payload,
            &["fal.ai:video-b".to_string(), "image-a".to_string()],
        )
        .expect("provider dependencies");

        assert_eq!(
            dependencies,
            vec![
                video_creater_lib::settings::providers::ProviderModelDependency::new(
                    "fal.ai",
                    "fal.ai:video-a",
                )
            ]
        );
    }

    #[test]
    fn account_status_conversion_preserves_labels_without_credential_material() {
        let status = super::ProviderAccountStatus {
            provider: "fal.ai".to_string(),
            credential_status: "available".to_string(),
            account_name: Some("Fixture studio".to_string()),
            balance: Some(super::ProviderAccountBalance {
                current_balance: 12.5,
                currency: "USD".to_string(),
            }),
            detail: "fixture-keychain-secret-61a0 must stay backend-only".to_string(),
            fetched_at: Some("2026-07-17T12:00:00Z".to_string()),
        };

        let validation = super::provider_account_validation(status);
        let health = video_creater_lib::settings::providers::aggregate_provider_health(
            &[
                video_creater_lib::provider_credentials::ProviderCredentialStatus {
                    provider: "fal.ai".to_string(),
                    display_name: "fal.ai".to_string(),
                    configured: true,
                    source:
                        video_creater_lib::provider_credentials::ProviderCredentialSource::Keychain,
                },
            ],
            std::slice::from_ref(&validation),
            &[
                video_creater_lib::settings::providers::ProviderModelDependency::new(
                    "fal.ai",
                    "fal.ai:video-a",
                ),
            ],
            None,
        )
        .expect("aggregate provider health");
        let serialized = serde_json::to_string(&health).expect("serialize provider health");

        assert_eq!(
            validation.validation_state,
            video_creater_lib::settings::providers::ProviderValidationState::Available
        );
        assert_eq!(validation.account_label.as_deref(), Some("Fixture studio"));
        assert_eq!(validation.balance_label.as_deref(), Some("12.50 USD"));
        assert!(!serialized.contains("fixture-keychain-secret-61a0"));
        assert!(!serialized.contains("FAL_KEY"));
    }

    #[test]
    fn app_update_status_command_reflects_persisted_update_policy() {
        let health = get_update_health();
        assert_eq!(health.state, SettingsHealthState::Unavailable);
        assert_eq!(health.installed_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(health.summary, "Updates are not configured for this build");
    }

    #[test]
    fn notification_capability_requires_an_explicit_request_when_undetermined() {
        let capability = notification_capability_for_authorization_status(
            NativeNotificationAuthorizationStatus::NotDetermined,
        );
        assert_eq!(capability.state, SettingsHealthState::ActionRequired);
        assert!(capability.delivery_available);
        assert!(capability.can_request);
        assert_eq!(capability.permission_status, "notDetermined");
    }

    #[test]
    fn notification_capability_disables_delivery_when_native_runtime_is_unavailable() {
        let capability = unavailable_notification_capability();
        assert_eq!(capability.state, SettingsHealthState::Unavailable);
        assert!(!capability.delivery_available);
        assert!(!capability.can_request);
        assert_eq!(capability.permission_status, "unavailable");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn notification_capability_skips_native_probe_for_unbundled_executable() {
        let probe_calls = std::cell::Cell::new(0);

        let capability = notification_capability_for_executable(
            std::path::Path::new("/tmp/video-creater/target/debug/video-creater"),
            || {
                probe_calls.set(probe_calls.get() + 1);
                Ok((NativeNotificationAuthorizationStatus::Authorized, true))
            },
        );

        assert_eq!(probe_calls.get(), 0);
        assert_eq!(capability.state, SettingsHealthState::Unavailable);
        assert!(!capability.delivery_available);
        assert!(!capability.can_request);
        assert_eq!(
            capability.diagnostic_code.as_deref(),
            Some("notifications.nativeUnavailable")
        );
        assert!(capability
            .diagnostic_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("application bundle")));
    }

    #[test]
    fn notification_capability_invokes_native_probe_for_bundled_executable() {
        let probe_calls = std::cell::Cell::new(0);

        let capability = notification_capability_for_executable(
            std::path::Path::new("/Applications/Video Creater.app/Contents/MacOS/video-creater"),
            || {
                probe_calls.set(probe_calls.get() + 1);
                Ok((NativeNotificationAuthorizationStatus::Authorized, true))
            },
        );

        assert_eq!(probe_calls.get(), 1);
        assert_eq!(capability.state, SettingsHealthState::Ready);
        assert!(capability.delivery_available);
        assert!(!capability.can_request);
        assert_eq!(capability.permission_status, "authorized");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_notification_capability_probes_any_executable_for_the_desktop_service() {
        let probe_calls = std::cell::Cell::new(0);
        let capability = notification_capability_for_executable(
            std::path::Path::new("/usr/bin/video-creater"),
            || {
                probe_calls.set(probe_calls.get() + 1);
                super::linux_notification_authorization(Ok(true))
            },
        );
        assert_eq!(probe_calls.get(), 1);
        assert_eq!(capability.state, SettingsHealthState::Ready);
        assert!(capability.delivery_available);
        assert_eq!(capability.permission_status, "authorized");

        let missing = notification_capability_for_executable(
            std::path::Path::new("/usr/bin/video-creater"),
            || super::linux_notification_authorization(Ok(false)),
        );
        assert_eq!(missing.state, SettingsHealthState::Unavailable);
        assert!(!missing.delivery_available);
        assert!(!missing.can_request);
        assert_eq!(missing.permission_status, "unavailable");
        assert!(missing
            .diagnostic_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("org.freedesktop.Notifications")));

        let no_bus = notification_capability_after_permission_request(
            super::linux_notification_authorization(Err(
                "the D-Bus session bus is unavailable".to_string()
            ))
            .map(|_| true),
            || super::linux_notification_authorization(Ok(true)),
        );
        assert_eq!(no_bus.state, SettingsHealthState::Unavailable);
        assert!(!no_bus.delivery_available);

        let granted = notification_capability_after_permission_request(
            super::linux_notification_authorization(Ok(true)).map(|_| true),
            || super::linux_notification_authorization(Ok(true)),
        );
        assert_eq!(granted.state, SettingsHealthState::Ready);
        assert_eq!(granted.permission_status, "authorized");
    }

    #[test]
    fn platform_info_reports_the_compile_target_platform() {
        let payload = serde_json::to_value(super::get_platform_info()).expect("platform info");
        let expected = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "other"
        };
        assert_eq!(payload, serde_json::json!({ "platform": expected }));
    }

    #[test]
    fn notification_capability_reports_authorized_but_disabled_native_delivery() {
        let capability = notification_capability_for_native_settings(
            NativeNotificationAuthorizationStatus::Authorized,
            false,
        );
        assert_eq!(capability.state, SettingsHealthState::ActionRequired);
        assert!(!capability.delivery_available);
        assert!(!capability.can_request);
        assert_eq!(capability.permission_status, "authorized");
        assert_eq!(
            capability.diagnostic_code.as_deref(),
            Some("notifications.deliveryDisabled")
        );
    }

    #[test]
    fn notification_permission_follow_up_failure_never_infers_delivery() {
        let capability = notification_capability_after_permission_request(Ok(true), || {
            Err("native notification settings timed out".to_string())
        });
        assert_eq!(capability.state, SettingsHealthState::Failed);
        assert!(!capability.delivery_available);
        assert!(!capability.can_request);
        assert_eq!(capability.permission_status, "authorized");
        assert_eq!(
            capability.diagnostic_code.as_deref(),
            Some("notifications.settingsQueryFailed")
        );
        assert_eq!(
            capability.diagnostic_detail.as_deref(),
            Some("native notification settings timed out")
        );
    }

    #[test]
    fn temporal_generate_media_failure_command_builds_failed_status_actions() {
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            "project-1",
            "generated-shot-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            "project-1",
            "/tmp/video-creater/project-1",
            "generated-shot-1",
            "generated-shot-1",
            true,
            None,
        ));

        let actions = build_temporal_generate_media_failure_actions(
            job,
            "generated-shot-1".to_string(),
            Some("temporal-run-1".to_string()),
            "2026-06-23T12:05:00Z".to_string(),
        )
        .expect("failure actions");

        assert_eq!(
            actions,
            vec![
                ProjectAction::UpdateJobStatus {
                    job_id: "generated-shot-1".to_string(),
                    status: JobStatus::Failed,
                    updated_at: "2026-06-23T12:05:00Z".to_string(),
                    run_id: Some("temporal-run-1".to_string()),
                },
                ProjectAction::UpdateGeneratedAssetStatus {
                    asset_id: "generated-shot-1".to_string(),
                    status: GeneratedAssetStatus::Failed,
                },
            ]
        );
    }

    #[test]
    fn cancel_provider_request_ignores_process_environment_credentials() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let cancel_url = "http://127.0.0.1:9/requests/fal-request-1/cancel".to_string();
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-shot-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Running,
            name: Some("Cancelable shot".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "cancel this generation".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: Vec::new(),
            created_at: "2026-06-23T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-shot-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.to_str().expect("utf-8 path"),
            "generated-shot-1",
            "generated-shot-1",
            false,
            None,
        ));
        job.provider_request = Some(JobProviderRequest {
            provider: FAL_PROVIDER.to_string(),
            request_id: "fal-request-1".to_string(),
            status_url: "http://127.0.0.1:9/requests/fal-request-1/status".to_string(),
            response_url: "http://127.0.0.1:9/requests/fal-request-1".to_string(),
            cancel_url: cancel_url.clone(),
            submitted_at: "2026-06-23T12:01:00Z".to_string(),
        });
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let isolated_service = format!(
            "com.olhapi.video-creater.settings-acceptance.{}",
            uuid::Uuid::new_v4().simple()
        );
        std::env::set_var(
            video_creater_lib::provider_credentials::PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV,
            isolated_service,
        );
        std::env::set_var("FAL_KEY", "test-fal-key");

        let error = cancel_generate_media_provider_request_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "generated-shot-1".to_string(),
        )
        .expect_err("environment credential must not authorize cancellation");
        std::env::remove_var("FAL_KEY");
        std::env::remove_var(
            video_creater_lib::provider_credentials::PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV,
        );

        assert!(
            error.contains("no credential is configured for provider fal.ai")
                || error.contains("secure credential storage is unavailable"),
            "unexpected cancellation error: {error}"
        );
        assert!(!error.contains("test-fal-key"));
    }

    #[test]
    fn cancel_provider_request_uses_keychain_resolver_and_authenticates_request() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let Some(server) = FakeProviderCancelServer::start() else {
            return;
        };
        let cancel_url = format!("{}/requests/fal-request-1/cancel", server.base_url);
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-shot-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Running,
            name: Some("Cancelable shot".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "cancel this generation".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: Vec::new(),
            created_at: "2026-06-23T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-shot-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.to_str().expect("utf-8 path"),
            "generated-shot-1",
            "generated-shot-1",
            false,
            None,
        ));
        job.provider_request = Some(JobProviderRequest {
            provider: FAL_PROVIDER.to_string(),
            request_id: "fal-request-1".to_string(),
            status_url: format!("{}/requests/fal-request-1/status", server.base_url),
            response_url: format!("{}/requests/fal-request-1", server.base_url),
            cancel_url: cancel_url.clone(),
            submitted_at: "2026-06-23T12:01:00Z".to_string(),
        });
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let keychain_items = std::collections::HashMap::from([(
            FAL_PROVIDER.to_string(),
            "test-fal-keychain-key".to_string(),
        )]);

        let output =
            cancel_generate_media_provider_request_in_split_project_folder_with_credential_resolver(
                project_dir.to_str().expect("utf-8 path").to_string(),
                "generated-shot-1".to_string(),
                |provider| {
                    keychain_items
                        .get(provider)
                        .cloned()
                        .ok_or_else(|| format!("missing isolated Keychain item: {provider}"))
                },
            )
            .expect("cancel provider request");
        let requests = server.join();

        assert_eq!(output.request_id, "fal-request-1");
        assert_eq!(output.cancel_url, cancel_url);
        let request = requests.first().expect("cancel request");
        assert_eq!(request.method, "PUT");
        assert_eq!(request.path, "/requests/fal-request-1/cancel");
        assert_eq!(
            request.authorization.as_deref(),
            Some("Key test-fal-keychain-key")
        );
        assert!(!request.body.contains("test-fal-keychain-key"));
    }

    #[test]
    fn cancel_generate_media_in_process_works_before_provider_metadata_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-local-cancel-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: Some("Locally cancelled shot".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "cancel before provider submission".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: Vec::new(),
            created_at: "2026-07-12T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "generated-local-cancel-1",
            JobStatus::Queued,
            "2026-07-12T12:00:00Z",
        );
        job.start_request = Some(temporal_generate_media_start_request(
            &project.id,
            project_dir.to_str().expect("utf-8 path"),
            "generated-local-cancel-1",
            "generated-local-cancel-1",
            false,
            None,
        ));
        assert!(job.provider_request.is_none());
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let first = cancel_generate_media_in_process_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "generated-local-cancel-1".to_string(),
            "2026-07-12T12:01:00Z".to_string(),
        )
        .expect("cancel before provider metadata");
        assert_eq!(first.outcome, "cancelled");
        assert_eq!(
            first
                .project
                .jobs
                .iter()
                .find(|job| job.id == "generated-local-cancel-1")
                .expect("job")
                .status,
            JobStatus::Cancelled
        );
        assert_eq!(
            first.project.generated_assets.last().expect("asset").status,
            GeneratedAssetStatus::Cancelled
        );

        let second = cancel_generate_media_in_process_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "generated-local-cancel-1".to_string(),
            "2026-07-12T12:02:00Z".to_string(),
        )
        .expect("idempotent cancellation");
        assert_eq!(second.outcome, "alreadyCancelled");
    }

    #[test]
    fn retry_generated_asset_output_download_restores_missing_output_media() {
        let Some(server) = FakeGeneratedOutputServer::start(b"restored-provider-bytes") else {
            return;
        };
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-retry-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some("Recovered generated still".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "recover this missing output".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-retry-1-output".to_string(),
                relative_path: "generated/generated-retry-1/output.png".to_string(),
                source_url: Some(format!("{}/output.png", server.base_url)),
                width: 1024,
                height: 1024,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-07-05T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let mut broken_project = load_split_project(&project_dir).expect("reload split project");
        broken_project
            .media
            .retain(|media| media.id != "generated-retry-1-output");
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            broken_project,
        )
        .expect("save broken split project");

        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&server.base_url)
            .expect("fixture origin");
        let result = retry_generated_asset_output_download_with_policy(
            &project_dir,
            "generated-retry-1",
            "generated-retry-1-output",
            &policy,
        )
        .expect("retry generated output download");

        assert_eq!(
            std::fs::read(project_dir.join("generated/generated-retry-1/output.png"))
                .expect("downloaded output"),
            b"restored-provider-bytes"
        );
        assert!(result
            .project
            .media
            .iter()
            .any(|media| media.id == "generated-retry-1-output"
                && media.relative_path == "generated/generated-retry-1/output.png"
                && media.kind == MediaKind::Generated));
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("media/index.json")));
        let request = server.join();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/output.png");
    }

    #[test]
    fn retry_publication_failure_keeps_original_without_a_canonical_path_gap() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let ops = FaultingRetryOutputFileOps {
            replace_calls: Cell::new(0),
            fail_replace_call: Some(1),
            fail_copy: false,
            fail_sync: false,
            fail_remove: false,
        };

        let error = publish_retry_output_with_rollback(
            &ops,
            &stage,
            &output,
            &backup,
            || -> Result<(), String> { panic!("metadata commit must not run") },
        )
        .unwrap_err();

        assert!(error.contains("publication failed"));
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
        assert!(!backup.exists());
        assert!(!stage.exists());
    }

    #[test]
    fn retry_backup_copy_failure_removes_partial_backup() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let ops = FaultingRetryOutputFileOps {
            replace_calls: Cell::new(0),
            fail_replace_call: None,
            fail_copy: true,
            fail_sync: false,
            fail_remove: false,
        };

        let error = publish_retry_output_with_rollback(
            &ops,
            &stage,
            &output,
            &backup,
            || -> Result<(), String> { panic!("metadata commit must not run") },
        )
        .unwrap_err();

        assert!(error.contains("backup copy failure"));
        assert!(error.contains("partial backup removed"));
        assert!(!backup.exists());
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
    }

    #[test]
    fn retry_backup_sync_and_cleanup_failure_reports_retained_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let ops = FaultingRetryOutputFileOps {
            replace_calls: Cell::new(0),
            fail_replace_call: None,
            fail_copy: false,
            fail_sync: true,
            fail_remove: true,
        };

        let error = publish_retry_output_with_rollback(
            &ops,
            &stage,
            &output,
            &backup,
            || -> Result<(), String> { panic!("metadata commit must not run") },
        )
        .unwrap_err();

        assert!(error.contains("backup sync failure"));
        assert!(error.contains("partial backup cleanup failed"));
        assert!(error.contains(&backup.display().to_string()));
        assert_eq!(std::fs::read(&backup).unwrap(), b"original");
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
    }

    #[test]
    fn retry_metadata_failure_restores_prior_output_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();

        let error = publish_retry_output_with_rollback(
            &SystemRetryOutputFileOps,
            &stage,
            &output,
            &backup,
            || Err::<(), _>("injected metadata failure".into()),
        )
        .unwrap_err();

        assert!(error.contains("prior output restored"));
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
        assert!(!backup.exists());
    }

    #[test]
    fn retry_restore_failure_reports_recoverable_backup_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let ops = FaultingRetryOutputFileOps {
            replace_calls: Cell::new(0),
            fail_replace_call: Some(2),
            fail_copy: false,
            fail_sync: false,
            fail_remove: false,
        };

        let error = publish_retry_output_with_rollback(&ops, &stage, &output, &backup, || {
            Err::<(), _>("injected metadata failure".into())
        })
        .unwrap_err();

        assert!(error.contains("restoration failed"));
        assert!(error.contains(&backup.display().to_string()));
        assert_eq!(std::fs::read(&backup).unwrap(), b"original");
        assert_eq!(std::fs::read(&output).unwrap(), b"replacement");
    }

    #[test]
    fn retry_backup_cleanup_failure_is_observable_and_preserves_backup() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.bin");
        let stage = dir.path().join("stage");
        let backup = dir.path().join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let ops = FaultingRetryOutputFileOps {
            replace_calls: Cell::new(0),
            fail_replace_call: None,
            fail_copy: false,
            fail_sync: false,
            fail_remove: true,
        };

        let error = publish_retry_output_with_rollback(&ops, &stage, &output, &backup, || Ok(()))
            .unwrap_err();

        assert!(error.contains("backup cleanup failed"));
        assert!(backup.exists());
        assert_eq!(std::fs::read(&output).unwrap(), b"replacement");
    }

    #[test]
    fn retry_response_reload_failure_preserves_committed_output_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("split-project");
        save_split_project_to_folder(project_dir.to_string_lossy().into_owned(), sample_project())
            .unwrap();
        let output = project_dir.join("output.bin");
        let stage = project_dir.join("stage");
        let backup = project_dir.join("backup");
        std::fs::write(&output, b"original").unwrap();
        std::fs::write(&stage, b"replacement").unwrap();
        let mut committed_project = load_split_project(&project_dir).unwrap();
        committed_project.name = "committed retry metadata".into();

        let error = publish_retry_output_and_load_response(
            &SystemRetryOutputFileOps,
            &stage,
            &output,
            &backup,
            || {
                save_split_project(&project_dir, &committed_project)
                    .map(|result| result.report)
                    .map_err(|error| error.to_string())
            },
            || {
                Err::<video_creater_lib::project::model::VideoProject, _>(
                    "injected response reload failure".into(),
                )
            },
        )
        .unwrap_err();

        assert!(error.contains("committed, but response reload failed"));
        assert_eq!(std::fs::read(&output).unwrap(), b"replacement");
        assert!(!backup.exists());
        assert_eq!(
            load_split_project(&project_dir).unwrap().name,
            "committed retry metadata"
        );
    }

    #[test]
    fn retry_generated_asset_output_download_rejects_stale_revision_before_publication() {
        let Some(server) = FakeGeneratedOutputServer::start(b"new-provider-bytes") else {
            return;
        };
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-stale-retry".into(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: None,
            target_folder_id: None,
            placement_intent: Some("library".into()),
            prompt: "stale retry".into(),
            model: GenerationModel {
                provider: "fal.ai".into(),
                id: "fal-ai/flux/schnell".into(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-stale-output".into(),
                relative_path: "generated/generated-stale-retry/output.png".into(),
                source_url: Some(format!("{}/output.png", server.base_url)),
                width: 16,
                height: 16,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-09-12T00:00:00Z".into(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(project_dir.to_string_lossy().into_owned(), project)
            .expect("save project");
        let output_path = project_dir.join("generated/generated-stale-retry/output.png");
        std::fs::create_dir_all(output_path.parent().unwrap()).unwrap();
        std::fs::write(&output_path, b"existing-bytes").unwrap();
        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&server.base_url)
            .unwrap();

        let error = retry_generated_asset_output_download_with_policy_and_hook(
            &project_dir,
            "generated-stale-retry",
            "generated-stale-output",
            Some(&policy),
            || Ok(()),
            || {
                let mut changed =
                    load_split_project(&project_dir).map_err(|error| error.to_string())?;
                changed.name = "concurrent change".into();
                save_split_project(&project_dir, &changed).map_err(|error| error.to_string())?;
                Ok(())
            },
        )
        .expect_err("stale retry must fail");

        assert!(error.contains("project revision conflict"));
        assert_eq!(std::fs::read(&output_path).unwrap(), b"existing-bytes");
        assert_eq!(
            load_split_project(&project_dir).unwrap().name,
            "concurrent change"
        );
        assert!(std::fs::read_dir(output_path.parent().unwrap())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("retry-")));
        let _ = server.join();
    }

    #[test]
    fn retry_policy_is_derived_after_provider_change_and_sends_no_unauthorized_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let source_url = format!("http://{}/output.png", listener.local_addr().unwrap());
        let trusted_origin = source_url.clone();
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "provider-race".into(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: None,
            target_folder_id: None,
            placement_intent: Some("library".into()),
            prompt: "provider race".into(),
            model: GenerationModel {
                provider: "replicate".into(),
                id: "model-a".into(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: vec![GeneratedAssetOutput {
                media_id: "provider-race-output".into(),
                relative_path: "generated/provider-race/output.png".into(),
                source_url: Some(source_url),
                width: 16,
                height: 16,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-09-12T00:00:00Z".into(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(project_dir.to_string_lossy().into_owned(), project).unwrap();

        let error = retry_generated_asset_output_download_with_policy_resolver_and_hooks(
            &project_dir,
            "provider-race",
            "provider-race-output",
            |provider| {
                if provider == "replicate" {
                    DownloadNetworkPolicy::public_only()
                        .with_trusted_origin(&trusted_origin)
                        .map_err(|error| error.to_string())
                } else {
                    Ok(DownloadNetworkPolicy::public_only())
                }
            },
            || {
                let mut changed =
                    load_split_project(&project_dir).map_err(|error| error.to_string())?;
                let asset = changed
                    .generated_assets
                    .iter_mut()
                    .find(|asset| asset.id == "provider-race")
                    .unwrap();
                asset.model.provider = "google".into();
                asset.model.id = "model-b".into();
                save_split_project(&project_dir, &changed).map_err(|error| error.to_string())?;
                Ok(())
            },
            || Ok(()),
        )
        .unwrap_err();

        assert!(error.contains("public downloads require HTTPS"));
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[test]
    fn retry_generated_asset_output_download_refreshes_existing_output_media() {
        let Some(server) = FakeGeneratedOutputServer::start(b"refreshed-provider-bytes") else {
            return;
        };
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project
            .media
            .push(video_creater_lib::project::model::MediaAsset {
                id: "generated-refresh-1-output".to_string(),
                name: None,
                relative_path: "generated/generated-refresh-1/output.png".to_string(),
                kind: MediaKind::Generated,
                duration_seconds: 1.0,
                width: Some(1024),
                height: Some(1024),
                fps: Some(1.0),
                folder_id: Some("folder-generated".to_string()),
            });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-refresh-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some("Refresh generated still".to_string()),
            target_folder_id: Some("folder-generated".to_string()),
            placement_intent: Some("library".to_string()),
            prompt: "refresh this existing output".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-refresh-1-output".to_string(),
                relative_path: "generated/generated-refresh-1/output.png".to_string(),
                source_url: Some(format!("{}/output.png", server.base_url)),
                width: 1024,
                height: 1024,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-07-05T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let output_path = project_dir.join("generated/generated-refresh-1/output.png");
        std::fs::create_dir_all(output_path.parent().expect("output parent"))
            .expect("create stale output parent");
        std::fs::write(&output_path, b"stale-local-bytes").expect("write stale output");

        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&server.base_url)
            .expect("fixture origin");
        let result = retry_generated_asset_output_download_with_policy(
            &project_dir,
            "generated-refresh-1",
            "generated-refresh-1-output",
            &policy,
        )
        .expect("retry generated output download");

        assert_eq!(
            std::fs::read(&output_path).expect("refreshed output"),
            b"refreshed-provider-bytes"
        );
        assert_eq!(
            result
                .project
                .media
                .iter()
                .filter(|media| media.id == "generated-refresh-1-output")
                .count(),
            1
        );
        assert!(result
            .project
            .media
            .iter()
            .any(|media| media.id == "generated-refresh-1-output"
                && media.folder_id.as_deref() == Some("folder-generated")));
        let request = server.join();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/output.png");
    }

    #[test]
    fn retry_generated_asset_output_download_repairs_existing_output_media_metadata() {
        let Some(server) = FakeGeneratedOutputServer::start(b"repaired-provider-bytes") else {
            return;
        };
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project
            .media
            .push(video_creater_lib::project::model::MediaAsset {
                id: "generated-repair-1-output".to_string(),
                name: Some("Stale generated output".to_string()),
                relative_path: "generated/stale/output.png".to_string(),
                kind: MediaKind::Image,
                duration_seconds: 99.0,
                width: Some(320),
                height: Some(240),
                fps: None,
                folder_id: None,
            });
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-repair-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some("Repair generated still".to_string()),
            target_folder_id: Some("folder-generated".to_string()),
            placement_intent: Some("library".to_string()),
            prompt: "repair this existing output row".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-repair-1-output".to_string(),
                relative_path: "generated/generated-repair-1/output.png".to_string(),
                source_url: Some(format!("{}/output.png", server.base_url)),
                width: 1024,
                height: 1024,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-07-05T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&server.base_url)
            .expect("fixture origin");
        let result = retry_generated_asset_output_download_with_policy(
            &project_dir,
            "generated-repair-1",
            "generated-repair-1-output",
            &policy,
        )
        .expect("retry generated output download");

        assert_eq!(
            std::fs::read(project_dir.join("generated/generated-repair-1/output.png"))
                .expect("repaired output"),
            b"repaired-provider-bytes"
        );
        let media = result
            .project
            .media
            .iter()
            .find(|media| media.id == "generated-repair-1-output")
            .expect("repaired media row");
        assert_eq!(
            media.relative_path,
            "generated/generated-repair-1/output.png"
        );
        assert_eq!(media.kind, MediaKind::Generated);
        assert_eq!(media.duration_seconds, 1.0);
        assert_eq!(media.width, Some(1024));
        assert_eq!(media.height, Some(1024));
        assert_eq!(media.fps, Some(1.0));
        assert_eq!(media.folder_id.as_deref(), Some("folder-generated"));
        assert_eq!(media.name, None);
        assert_eq!(
            result
                .project
                .media
                .iter()
                .filter(|media| media.id == "generated-repair-1-output")
                .count(),
            1
        );
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("media/index.json")));
        let request = server.join();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/output.png");
    }

    #[test]
    fn retry_generated_asset_output_download_rejects_non_http_provider_source_url() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-file-url-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some("File URL generated still".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "reject this local file source".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-file-url-1-output".to_string(),
                relative_path: "generated/generated-file-url-1/output.png".to_string(),
                source_url: Some("file:///tmp/generated-output.png".to_string()),
                width: 1024,
                height: 1024,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-07-05T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let error = retry_generated_asset_output_download_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "generated-file-url-1".to_string(),
            "generated-file-url-1-output".to_string(),
        )
        .expect_err("non-http generated output source URL should fail");

        assert!(error.contains("only HTTP(S) URLs are allowed"));
        assert!(!project_dir
            .join("generated/generated-file-url-1/output.png")
            .exists());
    }

    #[test]
    fn retry_generated_asset_output_download_rejects_incomplete_generated_asset() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "generated-running-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Running,
            name: Some("Running generated still".to_string()),
            target_folder_id: None,
            placement_intent: Some("library".to_string()),
            prompt: "do not recover this running output".to_string(),
            model: GenerationModel {
                provider: "fal.ai".to_string(),
                id: "fal-ai/flux/schnell".to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
                fps: None,
                aspect_ratio: Some("1:1".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: "generated-running-1-output".to_string(),
                relative_path: "generated/generated-running-1/output.png".to_string(),
                source_url: Some("file:///tmp/generated-output.png".to_string()),
                width: 1024,
                height: 1024,
                duration_seconds: 1.0,
                fps: 1.0,
            }],
            created_at: "2026-07-05T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let error = retry_generated_asset_output_download_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "generated-running-1".to_string(),
            "generated-running-1-output".to_string(),
        )
        .expect_err("incomplete generated output should not be recoverable");

        assert!(error.contains("generated asset generated-running-1 is not completed"));
        assert!(!project_dir
            .join("generated/generated-running-1/output.png")
            .exists());
    }

    #[test]
    fn cancel_render_job_command_persists_terminal_cancelled_status() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project.id,
            "render-draft-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.workflow.as_mut().expect("workflow").run_id =
            Some("render-attempt/test-cancel".to_string());
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let key = RenderAttemptKey::new(
            std::fs::canonicalize(&project_dir).expect("canonical project"),
            &load_split_project(&project_dir).expect("project").id,
            "render-draft-1",
            "render-attempt/test-cancel",
        )
        .expect("attempt key");
        let _attempt = video_creater_lib::render_pipeline::cancel::register_render_attempt(key)
            .expect("register attempt");

        let result = cancel_render_job_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "render-draft-1".to_string(),
            "render-attempt/test-cancel".to_string(),
            "2026-06-23T12:01:00Z".to_string(),
        )
        .expect("cancel render job");

        let job = result
            .project
            .jobs
            .iter()
            .find(|job| job.id == "render-draft-1")
            .expect("render job");
        assert_eq!(job.status, JobStatus::Cancelled);
        assert_eq!(job.updated_at, "2026-06-23T12:01:00Z");
    }

    #[test]
    fn cancel_render_job_command_signals_active_render_cancellation() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project.id,
            "render-draft-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.workflow.as_mut().expect("workflow").run_id =
            Some("render-attempt/test-signal".to_string());
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let key = RenderAttemptKey::new(
            std::fs::canonicalize(&project_dir).expect("canonical project"),
            &load_split_project(&project_dir).expect("project").id,
            "render-draft-1",
            "render-attempt/test-signal",
        )
        .expect("attempt key");
        let attempt = video_creater_lib::render_pipeline::cancel::register_render_attempt(key)
            .expect("register attempt");
        let token = attempt.token();
        let project_lease =
            video_creater_lib::project::mutation::acquire_split_project_mutation_lease(
                &project_dir,
            )
            .expect("lease");
        let command_project_dir = project_dir.clone();
        let cancellation = std::thread::spawn(move || {
            cancel_render_job_in_split_project_folder_blocking(
                command_project_dir.to_string_lossy().into_owned(),
                "render-draft-1".to_string(),
                "render-attempt/test-signal".to_string(),
                "2026-06-23T12:01:00Z".to_string(),
            )
        });

        assert!(
            token.wait_timeout(std::time::Duration::from_secs(1)),
            "cancellation signal must not wait for the project mutation lease"
        );
        drop(project_lease);
        cancellation
            .join()
            .expect("cancellation command joins")
            .expect("cancel render job");
    }

    #[test]
    fn cancel_render_job_command_succeeds_when_the_render_records_the_cancellation_first() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            &project.id,
            "export-mp4H264-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.workflow.as_mut().expect("workflow").run_id =
            Some("render-attempt/test-race".to_string());
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let key = RenderAttemptKey::new(
            std::fs::canonicalize(&project_dir).expect("canonical project"),
            &load_split_project(&project_dir).expect("project").id,
            "export-mp4H264-1",
            "render-attempt/test-race",
        )
        .expect("attempt key");
        let attempt = video_creater_lib::render_pipeline::cancel::register_render_attempt(key)
            .expect("register attempt");
        let token = attempt.token();
        // The render holds the project lease while it runs.
        let render_lease =
            video_creater_lib::project::mutation::acquire_split_project_mutation_lease(
                &project_dir,
            )
            .expect("lease");
        let command_project_dir = project_dir.clone();
        let cancellation = std::thread::spawn(move || {
            cancel_render_job_in_split_project_folder_blocking(
                command_project_dir.to_string_lossy().into_owned(),
                "export-mp4H264-1".to_string(),
                "render-attempt/test-race".to_string(),
                "2026-06-23T12:01:00Z".to_string(),
            )
        });
        assert!(token.wait_timeout(std::time::Duration::from_secs(5)));
        // The stopped render records its own cancellation before releasing the lease.
        video_creater_lib::project::split::apply_project_action_to_split_project(
            &project_dir,
            ProjectAction::UpdateJobStatus {
                job_id: "export-mp4H264-1".to_string(),
                status: JobStatus::Cancelled,
                updated_at: "2026-06-23T12:00:30Z".to_string(),
                run_id: Some("render-attempt/test-race".to_string()),
            },
        )
        .expect("render records its cancellation");
        drop(attempt);
        drop(render_lease);

        let result = cancellation
            .join()
            .expect("cancellation command joins")
            .expect("an already recorded cancellation is a successful cancel");
        let job = result
            .project
            .jobs
            .iter()
            .find(|job| job.id == "export-mp4H264-1")
            .expect("export job");
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn reveal_export_artifact_command_refuses_unrecorded_paths_in_a_saved_project() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let project_dir_text = project_dir.to_str().expect("utf-8 path").to_string();
        save_split_project_to_folder(project_dir_text.clone(), sample_project())
            .expect("save split project");

        let error = super::reveal_export_artifact_in_split_project_folder_blocking(
            project_dir_text,
            "video-creater.project.json".to_string(),
        )
        .expect_err("the manifest is not a recorded export");
        assert_eq!(error, "This file isn't a recorded export of this project.");
    }

    #[test]
    fn cancel_render_rejects_terminal_non_render_stale_and_inactive_attempts() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        for (id, kind, status, attempt) in [
            ("completed", "render_draft", JobStatus::Completed, "done"),
            ("failed", "export_media", JobStatus::Failed, "failed"),
            (
                "generation",
                "generate_media",
                JobStatus::Running,
                "generation",
            ),
            ("running", "render_draft", JobStatus::Running, "current"),
        ] {
            let mut job = temporal_job_summary(
                TemporalWorkflowKind::RenderDraft,
                &project.id,
                id,
                status,
                "2026-06-23T12:00:00Z",
            );
            job.kind = kind.to_string();
            job.workflow.as_mut().expect("workflow").run_id =
                Some(format!("render-attempt/{attempt}"));
            project.jobs.push(job);
        }
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project.clone(),
        )
        .expect("save project");

        for (job_id, attempt_id) in [
            ("completed", "render-attempt/done"),
            ("failed", "render-attempt/failed"),
            ("generation", "render-attempt/generation"),
            ("running", "render-attempt/stale"),
            ("running", "render-attempt/current"),
            ("missing", "render-attempt/missing"),
        ] {
            cancel_render_job_in_split_project_folder_blocking(
                project_dir.to_string_lossy().into_owned(),
                job_id.to_string(),
                attempt_id.to_string(),
                "2026-06-23T12:01:00Z".to_string(),
            )
            .expect_err("ineligible cancellation must fail");
        }

        let reloaded = load_split_project(&project_dir).expect("reload project");
        assert_eq!(reloaded.jobs, project.jobs);
    }

    #[test]
    fn loading_split_project_recovers_interrupted_local_render_for_retry() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project.id,
            "render-crashed-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        ));
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let render_dir = project_dir.join("renders/render-crashed-1");
        std::fs::create_dir_all(&render_dir).expect("create interrupted render directory");
        std::fs::write(render_dir.join("output.mp4"), b"unvalidated-partial-output")
            .expect("write interrupted output");

        let recovered =
            load_split_project_from_folder(project_dir.to_str().expect("utf-8 path").to_string())
                .expect("recover interrupted render while loading project");

        let job = recovered
            .jobs
            .iter()
            .find(|job| job.id == "render-crashed-1")
            .expect("recovered render job");
        assert_eq!(job.status, JobStatus::Failed);
        assert!(!render_dir.join("output.mp4").exists());
        assert!(render_dir.join("output.mp4.interrupted").is_file());
        let recovery_log =
            std::fs::read_to_string(render_dir.join("recovery.log")).expect("read recovery log");
        assert!(recovery_log.contains("local app process stopped"));
        assert!(recovery_log.contains("Retry the render"));

        let reloaded =
            load_split_project_from_folder(project_dir.to_str().expect("utf-8 path").to_string())
                .expect("reloading recovered project must be idempotent");
        assert_eq!(
            reloaded
                .jobs
                .iter()
                .find(|job| job.id == "render-crashed-1")
                .expect("reloaded job")
                .status,
            JobStatus::Failed
        );
    }

    #[test]
    fn loading_split_project_does_not_recover_a_live_local_render() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &project.id,
            "render-live-1",
            JobStatus::Running,
            "2026-06-23T12:00:00Z",
        );
        job.workflow.as_mut().expect("workflow").run_id = Some("render-attempt/live".to_string());
        project.jobs.push(job);
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let loaded_project = load_split_project(&project_dir).expect("project");
        let key = RenderAttemptKey::new(
            std::fs::canonicalize(&project_dir).expect("canonical project"),
            &loaded_project.id,
            "render-live-1",
            "render-attempt/live",
        )
        .expect("attempt key");
        let token = video_creater_lib::render_pipeline::cancel::register_render_attempt(key)
            .expect("register attempt");

        let loaded =
            load_split_project_from_folder(project_dir.to_str().expect("utf-8 path").to_string())
                .expect("load while render remains active");

        assert_eq!(
            loaded
                .jobs
                .iter()
                .find(|job| job.id == "render-live-1")
                .expect("live render job")
                .status,
            JobStatus::Running
        );
        assert!(!project_dir
            .join("renders/render-live-1/recovery.log")
            .exists());
        drop(token);
    }

    #[test]
    fn loading_split_project_fails_queued_renders_and_frame_captures_no_attempt_still_runs() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        for (id, kind, status) in [
            ("export-webm-stale", "render_draft", JobStatus::Queued),
            (
                "capture-stale",
                "captureCanonicalPreviewFrame",
                JobStatus::Running,
            ),
        ] {
            let mut job = temporal_job_summary(
                TemporalWorkflowKind::RenderDraft,
                &project.id,
                id,
                status,
                "2026-09-13T12:00:00Z",
            );
            job.kind = kind.to_string();
            job.workflow.as_mut().expect("workflow").run_id = Some(format!("render-attempt/{id}"));
            project.jobs.push(job);
        }
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let loaded =
            load_split_project_from_folder(project_dir.to_str().expect("utf-8 path").to_string())
                .expect("load project");

        for id in ["export-webm-stale", "capture-stale"] {
            let job = loaded
                .jobs
                .iter()
                .find(|job| job.id == id)
                .expect("stale job");
            assert_eq!(job.status, JobStatus::Failed, "{id}");
            let recovery_log =
                std::fs::read_to_string(project_dir.join(format!("renders/{id}/recovery.log")))
                    .expect("read recovery log");
            assert!(recovery_log.starts_with("Stopped when the app closed."));
        }
    }

    #[test]
    fn loading_split_project_leaves_temporal_jobs_to_their_worker() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let temporal_jobs = [
            (
                TemporalWorkflowKind::ExportMedia,
                "export-mp4H264-temporal",
                JobStatus::Running,
                Some("01a09ad3-75e2-7bca-94a3-1cd878a60c43"),
            ),
            (
                TemporalWorkflowKind::ExportMedia,
                "export-mp4H264-awaiting-start",
                JobStatus::Queued,
                None,
            ),
            (
                TemporalWorkflowKind::RenderDraft,
                "render-draft-temporal",
                JobStatus::Progress,
                Some("01a09ada-3ec7-7154-bc39-9f85f60ad829"),
            ),
            (
                TemporalWorkflowKind::GenerateMedia,
                "generate-temporal",
                JobStatus::Running,
                Some("01a09aec-d7c2-76da-912b-bbbbb9c042fa"),
            ),
        ];
        for (kind, id, status, run_id) in &temporal_jobs {
            let mut job = temporal_job_summary(
                *kind,
                &project.id,
                id,
                status.clone(),
                "2026-09-13T12:00:00Z",
            );
            job.workflow.as_mut().expect("workflow").run_id = run_id.map(str::to_string);
            project.jobs.push(job);
        }
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let saved = load_split_project(&project_dir).expect("saved project");

        let loaded =
            load_split_project_from_folder(project_dir.to_str().expect("utf-8 path").to_string())
                .expect("load project");

        assert_eq!(loaded.jobs, saved.jobs);
        for (_, id, _, _) in temporal_jobs {
            assert!(!project_dir
                .join(format!("renders/{id}/recovery.log"))
                .exists());
        }
    }

    #[test]
    fn nle_xml_export_command_records_temporal_start_request() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let project = sample_project();
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let result = export_nle_xml_to_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            NleXmlFormat::PremiereXmeml,
            "nle-export-1".to_string(),
            "2026-06-23T12:05:00Z".to_string(),
            None,
        )
        .expect("export nle xml");

        let start_request = result.job.start_request.expect("start request");
        assert_eq!(
            start_request.workflow_id,
            "video-creater/project-test/export-nle-xml/nle-export-1"
        );
        assert_eq!(
            start_request.workflow_type,
            "VideoCreaterExportNleXmlWorkflow"
        );
        assert_eq!(start_request.task_queue, "video-creater-workflows");
        assert_eq!(
            start_request.input["format"],
            serde_json::json!("premiereXmeml")
        );
        assert_eq!(
            start_request.input["outputPath"],
            serde_json::json!("exports/project-test-premiere.xml")
        );
        assert_eq!(
            start_request.activity_types,
            vec![
                "BuildNleXml".to_string(),
                "ValidateNleXml".to_string(),
                "WriteExportArtifact".to_string(),
                "AttachExportReport".to_string(),
            ]
        );
    }

    #[test]
    fn palmier_project_export_command_writes_package_without_temporal() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            sample_project(),
        )
        .expect("save split project");

        let result = write_palmier_project_package(
            project_dir.to_str().expect("utf-8 path"),
            "export-palmierProject-1".to_string(),
            "exports/project-test-palmierProject-1.palmier".to_string(),
            "2026-09-13T12:00:00Z".to_string(),
        )
        .expect("export Palmier project package");

        assert!(std::path::Path::new(&result.export_path).is_dir());
        assert_eq!(result.job.status, JobStatus::Completed);
        assert_eq!(
            result
                .job
                .start_request
                .expect("start request")
                .workflow_type,
            "VideoCreaterExportMediaWorkflow"
        );
        assert!(result.project.export_artifacts.iter().any(|artifact| {
            artifact.format == "palmierProject"
                && artifact.path == "exports/project-test-palmierProject-1.palmier"
        }));

        let error = write_palmier_project_package(
            project_dir.to_str().expect("utf-8 path"),
            "export-palmierProject-2".to_string(),
            "../outside.palmier".to_string(),
            "2026-09-13T12:01:00Z".to_string(),
        )
        .expect_err("reject output outside exports");
        assert!(error.contains("outputPath"), "{error}");
        let project = load_split_project(&project_dir).expect("reload project");
        let failed = project
            .jobs
            .iter()
            .find(|job| job.id == "export-palmierProject-2")
            .expect("failed job recorded");
        assert_eq!(failed.status, JobStatus::Failed);
    }

    #[test]
    fn nle_xml_export_command_selects_an_explicit_alternate_timeline() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        let mut alternate_timeline = project.timeline.clone();
        alternate_timeline.tracks[0].items[0].label = "Alternate opening".to_string();
        project
            .timelines
            .push(video_creater_lib::project::model::ProjectTimeline {
                id: "alternate".to_string(),
                name: "Alternate cut".to_string(),
                timeline: alternate_timeline,
            });
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let result = export_nle_xml_to_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            NleXmlFormat::PremiereXmeml,
            "nle-export-alternate".to_string(),
            "2026-07-10T00:00:00Z".to_string(),
            Some("alternate".to_string()),
        )
        .expect("export alternate NLE XML");

        let xml = std::fs::read_to_string(&result.export_path).expect("read NLE XML");
        assert!(xml.contains("Alternate opening"));
        assert_eq!(
            result.job.start_request.expect("start request").input["timelineId"],
            serde_json::json!("alternate")
        );
    }

    #[test]
    fn temporal_export_nle_xml_start_request_command_returns_replayable_input() {
        let request = build_temporal_export_nle_xml_start_request(
            "Project A".to_string(),
            "/tmp/video-creater/Project A".to_string(),
            "nle-export-1".to_string(),
            NleXmlFormat::DavinciFcpxml,
            "exports/project-a-davinci.fcpxml".to_string(),
        );

        assert_eq!(
            request.workflow_id,
            "video-creater/project-a/export-nle-xml/nle-export-1"
        );
        assert_eq!(request.workflow_type, "VideoCreaterExportNleXmlWorkflow");
        assert_eq!(request.task_queue, "video-creater-workflows");
        assert_eq!(request.input["projectId"], serde_json::json!("Project A"));
        assert_eq!(
            request.input["projectDir"],
            serde_json::json!("/tmp/video-creater/Project A")
        );
        assert_eq!(request.input["jobId"], serde_json::json!("nle-export-1"));
        assert_eq!(request.input["format"], serde_json::json!("davinciFcpxml"));
        assert_eq!(
            request.input["outputPath"],
            serde_json::json!("exports/project-a-davinci.fcpxml")
        );
        assert_eq!(
            request.search_attributes["workflowKind"],
            serde_json::json!("export_nle_xml")
        );
        assert_eq!(
            request.activity_types,
            vec![
                "BuildNleXml".to_string(),
                "ValidateNleXml".to_string(),
                "WriteExportArtifact".to_string(),
                "AttachExportReport".to_string(),
            ]
        );
        assert_eq!(request.id_reuse_policy, "rejectDuplicate");

        let serialized = serde_json::to_string(&request)
            .expect("serialize start request")
            .to_lowercase();
        assert!(!serialized.contains("credential"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("fal_key"));
    }

    #[test]
    fn temporal_transcribe_media_start_request_command_returns_replayable_input() {
        let request = build_temporal_transcribe_media_start_request(
            "project-1".to_string(),
            "/tmp/video-project".to_string(),
            "media-1".to_string(),
            "transcribe-job-1".to_string(),
            "en".to_string(),
        );

        assert_eq!(
            request.workflow_id,
            "video-creater/project-1/transcribe-media/transcribe-job-1"
        );
        assert_eq!(request.workflow_type, "VideoCreaterTranscribeMediaWorkflow");
        assert_eq!(request.task_queue, "video-creater-workflows");
        assert_eq!(request.input["projectId"], serde_json::json!("project-1"));
        assert_eq!(
            request.input["projectDir"],
            serde_json::json!("/tmp/video-project")
        );
        assert_eq!(request.input["mediaId"], serde_json::json!("media-1"));
        assert_eq!(
            request.input["jobId"],
            serde_json::json!("transcribe-job-1")
        );
        assert_eq!(request.input["languageMode"], serde_json::json!("en"));
        assert_eq!(
            request.search_attributes["workflowKind"],
            serde_json::json!("transcribe_media")
        );
        assert_eq!(
            request.activity_types,
            vec![
                "ProbeMedia".to_string(),
                "RunTranscription".to_string(),
                "StoreTranscript".to_string(),
            ]
        );
    }

    #[test]
    fn mock_generation_command_completes_queued_split_project_asset() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "mock-shot-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "mock product video".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_TEXT_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(4.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-23T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "mock-shot-1",
            JobStatus::Queued,
            "2026-06-23T12:00:00Z",
        ));
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let result = complete_mock_generated_asset_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "mock-shot-1".to_string(),
            "2026-06-23T12:05:00Z".to_string(),
            None,
        )
        .expect("complete mock generation");

        let generated = result
            .project
            .generated_assets
            .iter()
            .find(|asset| asset.id == "mock-shot-1")
            .expect("generated asset");
        assert_eq!(generated.status, GeneratedAssetStatus::Completed);
        assert_eq!(generated.outputs.len(), 1);
        assert!(result
            .project
            .media
            .iter()
            .any(|media| media.id == "mock-shot-1-mock-output"));
    }

    #[test]
    fn mock_generation_command_can_complete_and_replace_timeline_item() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.generated_assets.push(GeneratedAsset {
            schema_version: 1,
            id: "mock-shot-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "mock product video".to_string(),
            model: GenerationModel {
                provider: FAL_PROVIDER.to_string(),
                id: FAL_WAN_TEXT_TO_VIDEO_MODEL_ID.to_string(),
            },
            references: GeneratedAssetReferences {
                media_ids: Vec::new(),
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: GeneratedAssetSettings {
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(4.0),
                fps: Some(24.0),
                aspect_ratio: Some("16:9".to_string()),
                resolution: None,
                generate_audio: None,
                ..GeneratedAssetSettings::default()
            },
            outputs: Vec::new(),
            created_at: "2026-06-23T12:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
        project.jobs.push(temporal_job_summary(
            TemporalWorkflowKind::GenerateMedia,
            &project.id,
            "mock-shot-1",
            JobStatus::Queued,
            "2026-06-23T12:00:00Z",
        ));
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let result = complete_mock_generated_asset_in_split_project_folder_blocking(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "mock-shot-1".to_string(),
            "2026-06-23T12:05:00Z".to_string(),
            Some("item-1".to_string()),
        )
        .expect("complete mock generation and replace timeline item");

        let item = result
            .project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("timeline item");
        assert_eq!(
            item.source,
            TimelineSource::Media {
                media_id: "mock-shot-1-mock-output".to_string()
            }
        );
        assert_eq!(
            item.properties["generatedAssetId"],
            serde_json::json!("mock-shot-1")
        );
    }

    #[test]
    fn temporal_codex_edit_start_request_command_returns_replayable_input_without_proposal() {
        let project_root = tempfile::tempdir().expect("project root");
        let canonical_project_root = project_root
            .path()
            .canonicalize()
            .expect("canonical project root");
        let request = build_temporal_codex_edit_start_request(
            "Project A".to_string(),
            Some(
                project_root
                    .path()
                    .to_str()
                    .expect("project root utf8")
                    .to_string(),
            ),
            "/tmp/video-creater/Project A".to_string(),
            "codex-edit-1".to_string(),
            sample_edit_request(),
        )
        .expect("codex edit start request");

        assert_eq!(
            request.workflow_id,
            "video-creater/project-a/codex-edit/codex-edit-1"
        );
        assert_eq!(request.workflow_type, "VideoCreaterCodexEditWorkflow");
        assert_eq!(request.task_queue, "video-creater-workflows");
        assert_eq!(request.input["projectId"], serde_json::json!("Project A"));
        assert_eq!(
            request.input["projectRoot"],
            serde_json::json!(canonical_project_root
                .to_str()
                .expect("canonical project root utf8"))
        );
        assert_eq!(
            request.input["projectDir"],
            serde_json::json!("/tmp/video-creater/Project A")
        );
        assert_eq!(request.input["jobId"], serde_json::json!("codex-edit-1"));
        assert_eq!(
            request.input["request"]["mediaId"],
            serde_json::json!("media-1")
        );
        assert_eq!(
            request.input["request"]["preset"],
            serde_json::json!("trailer_cut")
        );
        assert!(request.input.get("proposal").is_none());
        assert_eq!(
            request.search_attributes["workflowKind"],
            serde_json::json!("codex_edit")
        );
        assert_eq!(
            request.activity_types,
            vec![
                "CollectProjectContext".to_string(),
                "RequestCodexProposal".to_string(),
                "ValidateProjectActions".to_string(),
                "PersistAcceptedProposal".to_string(),
                "AttachCodexEditFailure".to_string(),
            ]
        );
    }

    #[test]
    fn codex_turn_uses_current_split_files_when_project_dir_is_supplied() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut stale_project = sample_project();
        stale_project.name = "Stale UI Project".to_string();
        let mut file_project = sample_project();
        file_project.schema_version = 2;
        file_project.name = "Current Split Project".to_string();

        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            file_project,
        )
        .expect("save split project");

        let selected_project =
            codex_project_for_turn(stale_project, Some(&project_dir)).expect("codex project");

        assert_eq!(selected_project.name, "Current Split Project");
        assert_eq!(selected_project.schema_version, 2);
    }

    #[test]
    fn codex_conversation_cancel_command_reports_whether_a_turn_was_stopped() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let mut project = sample_project();
        project.schema_version = 2;
        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");
        let root = dir.path().to_str().expect("utf-8 root").to_string();
        let folder = project_dir.to_str().expect("utf-8 path").to_string();
        let cancel =
            || cancel_codex_conversation_edit_for_project(Some(root.clone()), Some(folder.clone()));

        assert_eq!(cancel(), Ok(false));
        let (guard, token) =
            register_codex_project_turn(dir.path(), Some(&project_dir)).expect("register turn");
        assert_eq!(cancel(), Ok(true));
        assert!(token.is_cancelled());
        drop(guard);
        assert_eq!(cancel(), Ok(false));
        assert!(
            cancel_codex_conversation_edit_for_project(Some(root), Some("relative".into()))
                .is_err()
        );
    }

    #[test]
    fn media_analysis_report_command_persists_derived_moments_and_silence_to_split_project() {
        let dir = tempfile::tempdir().expect("project parent");
        let project_dir = dir.path().join("split-project");
        let project = sample_project();

        save_split_project_to_folder(
            project_dir.to_str().expect("utf-8 path").to_string(),
            project,
        )
        .expect("save split project");

        let report = VideoInspectionReport {
            kind: "video".to_string(),
            path: project_dir.join("media/input.mp4"),
            size_bytes: 8_000_000,
            media: MediaProbe {
                container_name: Some("mp4".to_string()),
                duration_seconds: Some(12.0),
                size_bytes: Some(8_000_000),
                video: Some(VideoProbe {
                    codec_name: Some("h264".to_string()),
                    width: Some(1920),
                    height: Some(1080),
                    fps: Some(60.0),
                }),
                audio: Some(AudioProbe {
                    codec_name: Some("aac".to_string()),
                }),
            },
            warnings: Vec::new(),
        };

        let result = attach_media_analysis_report_to_split_project_folder(
            &project_dir,
            "media-1",
            &report,
            None,
            &[],
            &[
                AudioWindowMetrics {
                    start_seconds: 0.0,
                    end_seconds: 1.5,
                    mean_volume_db: -12.0,
                    max_volume_db: -3.0,
                },
                AudioWindowMetrics {
                    start_seconds: 4.0,
                    end_seconds: 6.0,
                    mean_volume_db: -49.0,
                    max_volume_db: -42.0,
                },
                AudioWindowMetrics {
                    start_seconds: 8.0,
                    end_seconds: 9.5,
                    mean_volume_db: -13.0,
                    max_volume_db: -4.0,
                },
            ],
        )
        .expect("attach media analysis");

        assert_eq!(result.moments.len(), 2);
        assert_eq!(result.moments[0].source_in, 0.0);
        assert_eq!(result.moments[1].source_in, 8.0);
        assert_eq!(result.silence_ranges.len(), 1);
        assert_eq!(result.silence_ranges[0].source_in, 4.0);
        assert_eq!(result.silence_ranges[0].source_out, 6.0);
        assert_eq!(result.project.media_analysis, result.moments);
        assert_eq!(result.project.media_silence_ranges, result.silence_ranges);
        let persisted = load_split_project(&project_dir).expect("load analyzed project");
        assert_eq!(result.project, persisted);
        video_creater_lib::project::split::replace_split_project_if_revision(
            &project_dir,
            result.project.clone(),
            result.project.content_revision,
        )
        .expect("subsequent CAS save");
        assert!(result
            .write_report
            .written_files
            .iter()
            .any(|path| path.ends_with("media/index.json")));

        let reloaded =
            load_split_project(&project_dir).expect("reload split project with media analysis");
        assert_eq!(reloaded.media_analysis, result.moments);
        assert_eq!(reloaded.media_silence_ranges, result.silence_ranges);
    }

    #[test]
    fn codex_local_tool_commands_expose_manifest_and_dispatcher() {
        let tools = list_codex_local_tools();
        assert!(tools
            .iter()
            .any(|tool| tool.name == "video_creater.project_context"));

        let project = sample_project();
        let expected_project_id = project.id.clone();
        let result = call_codex_local_tool_blocking(
            None,
            project,
            "video_creater.project_context".to_string(),
            serde_json::json!({}),
        )
        .expect("project context tool");

        assert_eq!(result.tool_name, "video_creater.project_context");
        assert_eq!(
            result.payload["project"]["id"],
            serde_json::json!(expected_project_id)
        );
    }

    struct FakeProviderCancelServer {
        base_url: String,
        handle: thread::JoinHandle<Vec<FakeProviderRequest>>,
    }

    struct FakeGeneratedOutputServer {
        base_url: String,
        handle: thread::JoinHandle<FakeProviderRequest>,
    }

    #[derive(Debug)]
    struct FakeProviderRequest {
        method: String,
        path: String,
        authorization: Option<String>,
        body: String,
    }

    impl FakeProviderCancelServer {
        fn start() -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind fake provider cancel server: {error}"),
            };
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let handle = thread::spawn(move || {
                let mut requests = Vec::new();
                for stream in listener.incoming().take(1) {
                    let mut stream = stream.expect("fake provider connection");
                    let request = read_fake_provider_request(&mut stream);
                    let body =
                        serde_json::json!({ "status": "CANCELLATION_REQUESTED" }).to_string();
                    let response = format!(
                        "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write fake provider response");
                    requests.push(request);
                }
                requests
            });

            Some(Self { base_url, handle })
        }

        fn join(self) -> Vec<FakeProviderRequest> {
            self.handle.join().expect("fake provider server join")
        }
    }

    impl FakeGeneratedOutputServer {
        fn start(body: &'static [u8]) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind fake generated output server: {error}"),
            };
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("fake output connection");
                let request = read_fake_provider_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write fake output headers");
                stream.write_all(body).expect("write fake output body");
                request
            });

            Some(Self { base_url, handle })
        }

        fn join(self) -> FakeProviderRequest {
            self.handle.join().expect("fake output server join")
        }
    }

    struct FaultingRetryOutputFileOps {
        replace_calls: Cell<usize>,
        fail_replace_call: Option<usize>,
        fail_copy: bool,
        fail_sync: bool,
        fail_remove: bool,
    }

    impl RetryOutputFileOps for FaultingRetryOutputFileOps {
        fn exists(&self, path: &Path) -> Result<bool, String> {
            path.try_exists().map_err(|error| error.to_string())
        }

        fn copy_backup(&self, source: &Path, backup: &Path) -> Result<(), String> {
            if self.fail_copy {
                std::fs::write(backup, b"partial backup").map_err(|error| error.to_string())?;
                return Err("injected backup copy failure".into());
            }
            std::fs::copy(source, backup).map_err(|error| error.to_string())?;
            Ok(())
        }

        fn sync_backup(&self, _backup: &Path) -> Result<(), String> {
            if self.fail_sync {
                Err("injected backup sync failure".into())
            } else {
                Ok(())
            }
        }

        fn replace(&self, source: &Path, destination: &Path) -> Result<(), String> {
            let call = self.replace_calls.get() + 1;
            self.replace_calls.set(call);
            if self.fail_replace_call == Some(call) {
                return Err(format!("injected replace failure {call}"));
            }
            std::fs::rename(source, destination).map_err(|error| error.to_string())
        }

        fn remove(&self, path: &Path) -> Result<(), String> {
            if self.fail_remove {
                return Err("injected remove failure".into());
            }
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        }
    }

    fn read_fake_provider_request(stream: &mut TcpStream) -> FakeProviderRequest {
        let mut buffer = [0_u8; 4096];
        let mut bytes = Vec::new();
        loop {
            let read = stream
                .read(&mut buffer)
                .expect("read fake provider request");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if fake_provider_request_body_complete(&bytes) {
                break;
            }
        }
        let request = String::from_utf8(bytes).expect("utf8 fake provider request");
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

        FakeProviderRequest {
            method,
            path,
            authorization,
            body: body.to_string(),
        }
    }

    fn fake_provider_request_body_complete(bytes: &[u8]) -> bool {
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

    #[test]
    fn import_media_to_project_names_media_from_the_names_map() {
        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save project");
        let source = project_dir.join("renders/save-range-1/still.png");
        std::fs::create_dir_all(source.parent().expect("render dir")).expect("render dir");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
            .save(&source)
            .expect("write source image");
        let source_text = source.display().to_string();

        let result = tauri::async_runtime::block_on(super::import_media_to_project(
            project_dir.display().to_string(),
            project,
            vec![source_text.clone()],
            Some(std::collections::BTreeMap::from([(
                source_text,
                "Edison Restoration Demo 00:04–00:09".to_string(),
            )])),
        ))
        .expect("named import");

        assert_eq!(
            result.imported[0].name.as_deref(),
            Some("Edison Restoration Demo 00:04–00:09")
        );
    }

    #[test]
    fn project_writes_queue_behind_a_held_project_lease_in_order() {
        use std::future::Future;
        use std::task::{Context, Waker};

        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save project");
        let project_dir_text = project_dir.to_str().expect("utf-8 path").to_string();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let lease_dir = project_dir.clone();
        let holder = std::thread::spawn(move || {
            let lease = video_creater_lib::project::mutation::acquire_split_project_mutation_lease(
                &lease_dir,
            )
            .expect("render lease");
            held_tx.send(()).expect("signal held lease");
            let _ = release_rx.recv();
            drop(lease);
        });
        held_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("lease held");

        let (queued_tx, queued_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            let mut context = Context::from_waker(Waker::noop());
            let mut writes = ["A", "B", "C"]
                .into_iter()
                .map(|name| {
                    let mut write = Box::pin(super::apply_project_actions_to_split_project_folder(
                        project_dir_text.clone(),
                        vec![ProjectAction::RenameMedia {
                            media_id: "media-1".to_string(),
                            name: name.to_string(),
                        }],
                        None,
                    ));
                    assert!(
                        write.as_mut().poll(&mut context).is_pending(),
                        "write {name} must wait behind the held project lease"
                    );
                    write
                })
                .collect::<Vec<_>>();
            queued_tx.send(()).expect("signal queued writes");
            writes
                .iter_mut()
                .map(|write| tauri::async_runtime::block_on(write.as_mut()))
                .collect::<Vec<_>>()
        });
        queued_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("writes queued");
        release_tx.send(()).expect("release lease");
        let results = writer.join().expect("writer thread");
        holder.join().expect("lease holder");

        let revisions = results
            .into_iter()
            .map(|result| {
                result
                    .expect("queued write succeeds")
                    .project
                    .content_revision
            })
            .collect::<Vec<_>>();
        assert!(
            revisions.windows(2).all(|pair| pair[0] < pair[1]),
            "revisions must increase in submission order: {revisions:?}"
        );
        let project = load_split_project(&project_dir).expect("reload project");
        let media = project
            .media
            .iter()
            .find(|media| media.id == "media-1")
            .expect("media-1");
        assert_eq!(media.name.as_deref(), Some("C"));
    }

    fn project_with_export_job(
        status: JobStatus,
        job_id: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        project.jobs.push(export_job_with_run_id(job_id, status));
        save_split_project(&project_dir, &project).expect("save project");
        (temp, project_dir)
    }

    fn export_job_with_run_id(
        job_id: &str,
        status: JobStatus,
    ) -> video_creater_lib::project::model::JobSummary {
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            "project-1",
            job_id,
            status,
            "2026-09-16T10:00:00Z",
        );
        job.start_request = Some(
            video_creater_lib::workflows::temporal_workflow_start_request(
                TemporalWorkflowKind::ExportMedia,
                "project-1",
                job_id,
                serde_json::json!({}),
            ),
        );
        if let Some(workflow) = job.workflow.as_mut() {
            workflow.run_id = Some("temporal-run-1".to_string());
        }
        job
    }

    #[test]
    fn load_job_progress_returns_snapshots_while_the_project_lease_is_held() {
        let (_temp, project_dir) = project_with_export_job(JobStatus::Running, "export-progress");
        assert!(
            video_creater_lib::project::job_progress::JobProgressReporter::new(
                &project_dir,
                "export-progress"
            )
            .report(0.4)
        );
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let lease_dir = project_dir.clone();
        let holder = std::thread::spawn(move || {
            let storage = video_creater_lib::settings::storage::acquire_storage_mutation_lease()
                .expect("storage lease");
            let project =
                video_creater_lib::project::mutation::acquire_split_project_mutation_lease(
                    &lease_dir,
                )
                .expect("project lease");
            held_tx.send(()).expect("signal held leases");
            let _ = release_rx.recv();
            drop(project);
            drop(storage);
        });
        held_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("leases held");

        let (read_tx, read_rx) = std::sync::mpsc::channel();
        let project_dir_text = project_dir.to_str().expect("utf-8 path").to_string();
        std::thread::spawn(move || {
            let _ = read_tx.send(tauri::async_runtime::block_on(
                super::load_job_progress_from_split_project_folder(project_dir_text),
            ));
        });
        let snapshots = read_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("progress loads while both leases are held")
            .expect("progress snapshots");
        release_tx.send(()).expect("release leases");
        holder.join().expect("lease holder");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].job_id, "export-progress");
        assert_eq!(snapshots[0].progress, 0.4);
    }

    #[test]
    fn project_load_removes_progress_for_finished_jobs() {
        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        project
            .jobs
            .push(export_job_with_run_id("export-done", JobStatus::Completed));
        project
            .jobs
            .push(export_job_with_run_id("export-running", JobStatus::Running));
        save_split_project(&project_dir, &project).expect("save project");
        for job_id in ["export-done", "export-running"] {
            assert!(
                video_creater_lib::project::job_progress::JobProgressReporter::new(
                    &project_dir,
                    job_id
                )
                .report(0.5)
            );
        }

        super::load_split_project_from_folder_impl(
            project_dir.to_str().expect("utf-8 path").to_string(),
        )
        .expect("load project");

        let remaining =
            video_creater_lib::project::job_progress::read_job_progress_snapshots(&project_dir)
                .expect("read progress")
                .into_iter()
                .map(|snapshot| snapshot.job_id)
                .collect::<Vec<_>>();
        assert_eq!(remaining, vec!["export-running".to_string()]);
    }

    #[cfg(not(feature = "temporal-worker"))]
    #[test]
    fn reconcile_command_without_temporal_feature_reports_unavailable() {
        let (_temp, project_dir) = project_with_export_job(JobStatus::Running, "export-1");

        let result = tauri::async_runtime::block_on(super::reconcile_temporal_jobs(
            project_dir.to_str().expect("utf-8 path").to_string(),
            "2026-09-16T12:00:00Z".to_string(),
            true,
        ))
        .expect("reconciliation result");

        assert!(!result.service_reachable);
        assert_eq!(
            result.detail.as_deref(),
            Some("Workflow service unreachable")
        );
        assert!(result.project.is_none());
    }

    #[test]
    fn reconcile_command_serializes_camel_case_result() {
        struct RestoreTemporalAddress(Option<String>);
        impl Drop for RestoreTemporalAddress {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(address) => std::env::set_var("TEMPORAL_ADDRESS", address),
                    None => std::env::remove_var("TEMPORAL_ADDRESS"),
                }
            }
        }
        let _restore = RestoreTemporalAddress(std::env::var("TEMPORAL_ADDRESS").ok());
        std::env::set_var("TEMPORAL_ADDRESS", "http://127.0.0.1:1");
        let (_temp, project_dir) = project_with_export_job(JobStatus::Running, "export-1");
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                super::reconcile_temporal_jobs_in_split_project_folder
            ])
            .build(mock_context(noop_assets()))
            .expect("reconcile test app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "reconcile", Default::default())
            .build()
            .expect("reconcile test webview");

        let response = get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "reconcile_temporal_jobs_in_split_project_folder".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().expect("invoke URL"),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                    "projectDir": project_dir.to_str().expect("utf-8 path"),
                    "updatedAt": "2026-09-16T12:00:00Z",
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("reconcile response");

        assert_eq!(
            response
                .deserialize::<serde_json::Value>()
                .expect("reconcile response JSON"),
            serde_json::json!({
                "project": null,
                "failedJobIds": [],
                "serviceReachable": false,
                "detail": "Workflow service unreachable",
            })
        );
        let reloaded = load_split_project(&project_dir).expect("reload project");
        assert_eq!(
            reloaded
                .jobs
                .iter()
                .find(|job| job.id == "export-1")
                .expect("job")
                .status,
            JobStatus::Running
        );
    }
}
