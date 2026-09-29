use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::agent::codex_transport::CodexTurnTransport;
use crate::agent::turn::{
    agent_session_for_next_turn, build_agent_conversation_request, finish_agent_conversation_turn,
};
use crate::agent::{AgentBackendKind, AgentTurnTransport};
use crate::codex::app_server::{
    bundled_codex_app_server_command, codex_app_server_deadline, StdioCodexAppServerTransport,
};
use crate::codex::context::bundled_host_skill_bundle;
use crate::codex::turn_cancel::{
    close_codex_turn_cancellation, register_codex_project_turn,
    request_codex_project_turn_cancellation,
};

use crate::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    undo_codex_conversation_edit, CodexConversationApplyRequest, CodexConversationEditProposal,
    CodexConversationEditRequest,
};
use crate::codex::proposal::{CodexProposalClip, CodexRenderReview};
use crate::edit::render_plan::{ExportEncodeTier, RenderQuality};
use crate::generation::cancel::{
    request_generation_cancellation, GenerationCancellationRequestOutcome,
};
use crate::project::action::ProjectAction;
use crate::project::export_destination::ExportOutputRequest;
use crate::project::export_options::{ExportRenderOptions, JobExportSettings};
use crate::project::export_profiles::{mp4_export_profile_availability_report, ExportProfile};
use crate::project::job_progress::read_job_progress_snapshots;
use crate::project::model::{GeneratedAssetStatus, JobStatus, MediaKind, VideoProject};
use crate::project::split::{
    apply_agent_session_action, apply_project_action_to_split_project,
    apply_project_actions_to_split_project, load_agent_session_manifest,
    load_app_server_conversation_history, load_split_project, record_app_server_conversation_turn,
    replace_split_project_if_revision, resolve_project_relative_path, save_split_project,
    AgentSessionAction, ProjectActionWriteResult, ProjectWriteReport,
};
use crate::provider_credentials::resolve_provider_credential;
use crate::render_pipeline::cancel::{
    request_render_cancellation_by_locator, RenderCancellationOutcome,
};
use crate::render_pipeline::project_export::{
    load_project_render_pipeline_report, render_media_export_to_split_project_folder,
    render_prepared_preview_frame_to_split_project_folder, MediaExportRequest,
};
use crate::timeline_filmstrip::{request_cache_key, source_fingerprint, TimelineFilmstripReport};
use crate::workflows::{
    temporal_generate_media_cancel_provider_activity_with_client,
    temporal_generate_media_cancellation_actions, temporal_job_summary, TemporalWorkflowKind,
};

use super::project_catalog::ProjectCatalog;
use super::rpc::{RpcDispatcher, RpcEnvelope};

pub struct HostDispatcher {
    preferences: Mutex<Value>,
    projects: Option<ProjectCatalog>,
    agent_fixture: bool,
}

impl Default for HostDispatcher {
    fn default() -> Self {
        Self {
            preferences: Mutex::new(default_preferences()),
            projects: None,
            agent_fixture: false,
        }
    }
}

impl RpcDispatcher for HostDispatcher {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String> {
        match request.operation.as_str() {
            "get_platform_info" => Ok(json!({ "platform": host_platform() })),
            "get_remote_access_status" => Ok(json!({
                "serviceState": "running",
                "tailscale": crate::remote_access::detect_and_verify(
                    crate::web_host::config::DEFAULT_HOST_PORT
                ),
                "managementAvailable": false,
                "detail": "Remote access is managed on the host machine."
            })),
            "get_app_preferences" => Ok(self
                .preferences
                .lock()
                .map_err(|_| "preferences lock failed".to_string())?
                .clone()),
            "update_app_preferences" => self.update_preferences(&request.payload),
            "list_transcription_models" => Ok(json!([])),
            "get_active_transcription_model" => Ok(Value::Null),
            "get_transcription_runtime_status" => Ok(json!({
                "selection": "unavailable",
                "detail": "No transcription runtime is selected."
            })),
            "get_production_speech_model_status" => Ok(json!({
                "ready": false,
                "models": [],
                "detail": "Speech models are not installed."
            })),
            "get_export_profile_availability_report" => {
                serde_json::to_value(mp4_export_profile_availability_report())
                    .map_err(|_| "export availability response failed".to_string())
            }
            "remote_list_projects" => serde_json::to_value(self.projects()?.list()?)
                .map_err(|_| "project catalog response failed".to_string()),
            "remote_create_project" => self.create_project(&request.payload),
            "load_split_project_from_folder" => {
                let path = self.resolve_project(request)?;
                serde_json::to_value(
                    load_split_project(&path).map_err(|_| "project could not be loaded")?,
                )
                .map_err(|_| "project response failed".to_string())
            }
            "save_split_project_to_folder" => self.save_project(request),
            "apply_project_action_to_split_project_folder" => {
                self.apply_project_actions(request, false)
            }
            "apply_project_actions_to_split_project_folder" => {
                self.apply_project_actions(request, true)
            }
            "load_job_progress_from_split_project_folder" => {
                let path = self.resolve_project(request)?;
                serde_json::to_value(read_job_progress_snapshots(&path)?)
                    .map_err(|_| "job progress response failed".to_string())
            }
            "reconcile_temporal_jobs_in_split_project_folder" => {
                let path = self.resolve_project(request)?;
                let project =
                    load_split_project(&path).map_err(|_| "project could not be loaded")?;
                Ok(json!({
                    "project": project,
                    "failedJobIds": [],
                    "serviceReachable": true,
                    "detail": null
                }))
            }
            "load_render_pipeline_report_from_split_project_folder" => {
                let path = self.resolve_project(request)?;
                let job_id = required_string(&request.payload, "jobId")?;
                serde_json::to_value(
                    load_project_render_pipeline_report(&path, job_id)
                        .map_err(pipeline_error_message)?,
                )
                .map_err(|_| "render report response failed".to_string())
            }
            "render_media_to_split_project_folder" => self.render_media(request),
            "cache_timeline_filmstrip_in_split_project_folder" => {
                self.cache_timeline_filmstrip(request)
            }
            "capture_canonical_preview_frame_in_split_project_folder" => {
                self.capture_canonical_preview_frame(request)
            }
            "load_agent_sessions_from_split_project_folder" => {
                let path = self.resolve_project(request)?;
                serde_json::to_value(
                    load_agent_session_manifest(&path)
                        .map_err(|_| "agent sessions could not be loaded")?,
                )
                .map_err(|_| "agent sessions response failed".to_string())
            }
            "load_app_server_conversation_history_from_split_project_folder" => {
                let path = self.resolve_project(request)?;
                serde_json::to_value(
                    load_app_server_conversation_history(&path)
                        .map_err(|_| "agent history could not be loaded")?,
                )
                .map_err(|_| "agent history response failed".to_string())
            }
            "apply_agent_session_action_to_split_project_folder" => {
                self.apply_agent_session_action(request)
            }
            "start_codex_conversation_edit_for_project" => self.start_agent(request),
            "apply_codex_conversation_proposal" => self.apply_agent_proposal(request),
            "undo_latest_codex_conversation_edit" => self.undo_agent_proposal(request),
            "cancel_codex_conversation_edit_for_project" => self.cancel_agent(request),
            "cancel_codex_video_edit_for_project" => self.cancel_agent(request),
            "cancel_generate_media_in_process" => self.cancel_generation(request),
            "cancel_generate_media_provider_request_in_split_project_folder" => {
                self.cancel_generation_provider(request)
            }
            "cancel_render_job_in_split_project_folder" => self.cancel_render(request),
            "validate_split_project_folder" => {
                let path = self.resolve_project(request)?;
                serde_json::to_value(
                    crate::project::split::validate_split_project(&path)
                        .map_err(|_| "project could not be validated")?,
                )
                .map_err(|_| "project response failed".to_string())
            }
            operation => Err(format!("remote operation is not implemented: {operation}")),
        }
    }
}

impl HostDispatcher {
    pub fn with_project_catalog(projects: ProjectCatalog) -> Self {
        Self {
            preferences: Mutex::new(default_preferences()),
            projects: Some(projects),
            agent_fixture: remote_agent_fixture_enabled(),
        }
    }

    #[doc(hidden)]
    pub fn with_project_catalog_and_agent_fixture(
        projects: ProjectCatalog,
        agent_fixture: bool,
    ) -> Self {
        Self {
            preferences: Mutex::new(default_preferences()),
            projects: Some(projects),
            agent_fixture,
        }
    }

    fn update_preferences(&self, payload: &Value) -> Result<Value, String> {
        let patch = payload
            .get("patch")
            .and_then(Value::as_object)
            .ok_or_else(|| "preferences patch is required".to_string())?;
        let mut preferences = self
            .preferences
            .lock()
            .map_err(|_| "preferences lock failed".to_string())?;
        let target = preferences
            .as_object_mut()
            .ok_or_else(|| "preferences state is invalid".to_string())?;
        for (key, value) in patch {
            if key != "schemaVersion" {
                target.insert(key.clone(), value.clone());
            }
        }
        Ok(preferences.clone())
    }

    fn projects(&self) -> Result<&ProjectCatalog, String> {
        self.projects
            .as_ref()
            .ok_or_else(|| "project catalog is not configured".to_string())
    }

    fn resolve_project(&self, request: &RpcEnvelope) -> Result<std::path::PathBuf, String> {
        self.projects()?.resolve(
            request
                .project_id
                .as_deref()
                .ok_or_else(|| "project ID is required".to_string())?,
        )
    }

    fn create_project(&self, payload: &Value) -> Result<Value, String> {
        let mut project: VideoProject = serde_json::from_value(
            payload
                .get("project")
                .cloned()
                .ok_or_else(|| "project is required".to_string())?,
        )
        .map_err(|_| "project is invalid".to_string())?;
        project.id = format!("project-{}", uuid::Uuid::new_v4().simple());
        let path = self.projects()?.create_path()?;
        let saved =
            save_split_project(&path, &project).map_err(|_| "project could not be saved")?;
        let catalog_project_id = self.projects()?.id_for_path(&path)?;
        Ok(json!({"catalogProjectId": catalog_project_id, "project": saved.project}))
    }

    fn save_project(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let project: VideoProject = serde_json::from_value(
            request
                .payload
                .get("project")
                .cloned()
                .ok_or_else(|| "project is required".to_string())?,
        )
        .map_err(|_| "project is invalid".to_string())?;
        let expected_revision = request
            .expected_revision
            .ok_or_else(|| "expected revision is required".to_string())?;
        serde_json::to_value(
            replace_split_project_if_revision(&path, project, expected_revision)
                .map_err(|_| "project could not be saved")?,
        )
        .map_err(|_| "project response failed".to_string())
    }

    fn apply_project_actions(
        &self,
        request: &RpcEnvelope,
        multiple: bool,
    ) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let canonical = load_split_project(&path).map_err(|_| "project could not be loaded")?;
        let expected_revision = request
            .expected_revision
            .ok_or_else(|| "expected revision is required".to_string())?;
        if canonical.content_revision != expected_revision {
            return Err("project revision conflict".into());
        }
        let actions: Vec<ProjectAction> = if multiple {
            serde_json::from_value(
                request
                    .payload
                    .get("actions")
                    .cloned()
                    .ok_or_else(|| "project actions are required".to_string())?,
            )
            .map_err(|_| "project actions are invalid".to_string())?
        } else {
            vec![serde_json::from_value(
                request
                    .payload
                    .get("action")
                    .cloned()
                    .ok_or_else(|| "project action is required".to_string())?,
            )
            .map_err(|_| "project action is invalid".to_string())?]
        };
        serde_json::to_value(
            apply_project_actions_to_split_project(&path, actions)
                .map_err(|_| "project actions could not be applied")?,
        )
        .map_err(|_| "project response failed".to_string())
    }

    fn cache_timeline_filmstrip(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let input: FilmstripRequest = serde_json::from_value(request.payload.clone())
            .map_err(|_| "timeline filmstrip request is invalid".to_string())?;
        let project = load_split_project(&path).map_err(|_| "project could not be loaded")?;
        let media = project
            .media
            .iter()
            .find(|media| media.id == input.media_id)
            .ok_or_else(|| format!("media was not found: {}", input.media_id))?;
        let source = resolve_project_relative_path(&path, &media.relative_path)
            .map_err(|_| "media source could not be resolved".to_string())?;
        if !input.source_in.is_finite()
            || !input.source_out.is_finite()
            || input.source_in < 0.0
            || input.source_out <= input.source_in
            || !input.speed.is_finite()
            || input.speed <= 0.0
            || !input.clip_pixel_width.is_finite()
            || input.clip_pixel_width <= 0.0
        {
            return Err("timeline filmstrip request is invalid".into());
        }
        let source_out = if media.duration_seconds > 0.0 {
            input
                .source_out
                .min((media.duration_seconds - 0.001).max(input.source_in + 0.001))
        } else {
            input.source_out
        };
        let fingerprint = source_fingerprint(&source)?;
        let report = TimelineFilmstripReport {
            media_id: media.id.clone(),
            source_fingerprint: fingerprint.clone(),
            cache_key: request_cache_key(
                &fingerprint,
                input.source_in,
                source_out,
                input.speed,
                input.zoom_bucket,
                input.height_bucket,
                input.clip_pixel_width,
            ),
            cache_hit: false,
            source_in: input.source_in,
            source_out,
            speed: input.speed,
            zoom_bucket: input.zoom_bucket,
            height_bucket: input.height_bucket,
            sampling_policy: "remote-solid-fill-v1".into(),
            frames: Vec::new(),
        };
        serde_json::to_value(report).map_err(|_| "timeline filmstrip response failed".to_string())
    }

    fn capture_canonical_preview_frame(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let input: CanonicalPreviewRequest = serde_json::from_value(request.payload.clone())
            .map_err(|_| "canonical preview request is invalid".to_string())?;
        serde_json::to_value(
            render_prepared_preview_frame_to_split_project_folder(
                &path,
                input.playhead_seconds,
                &input.job_id,
                &input.updated_at,
            )
            .map_err(pipeline_error_message)?,
        )
        .map_err(|_| "canonical preview response failed".to_string())
    }

    fn ensure_revision(
        &self,
        request: &RpcEnvelope,
        path: &std::path::Path,
    ) -> Result<VideoProject, String> {
        let canonical = load_split_project(path).map_err(|_| "project could not be loaded")?;
        let expected = request
            .expected_revision
            .ok_or_else(|| "expected revision is required".to_string())?;
        if canonical.content_revision != expected {
            return Err("project revision conflict".into());
        }
        Ok(canonical)
    }

    fn render_media(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let canonical = self.ensure_revision(request, &path)?;
        let input: RenderMediaRequest = serde_json::from_value(request.payload.clone())
            .map_err(|_| "render request is invalid".to_string())?;
        if input.project_id != canonical.id {
            return Err("render project ID does not match the canonical project".into());
        }
        let options =
            ExportRenderOptions::new(input.profile, input.quality, input.width, input.height)
                .and_then(|options| options.with_fps(input.fps))
                .and_then(|options| options.with_encode_tier(input.encode_tier.unwrap_or_default()))
                .map_err(|error| error.to_string())?;
        let range_seconds = match (input.range_start_seconds, input.range_end_seconds) {
            (Some(start), Some(end)) => Some((start, end)),
            (None, None) => None,
            _ => return Err("render range bounds must be provided together".into()),
        };
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::RenderDraft,
            &canonical.id,
            &input.job_id,
            JobStatus::Queued,
            &input.updated_at,
        );
        if let Some(output) = input.output.clone() {
            job.export_settings = Some(JobExportSettings::for_export(
                options,
                input.export_settings,
                output,
            ));
        }
        let result = render_media_export_to_split_project_folder(MediaExportRequest {
            project_dir: &path,
            project_id: &canonical.id,
            options,
            job,
            updated_at: &input.updated_at,
            run_id: Some(input.attempt_id),
            range_seconds,
            timeline_id: input.timeline_id.as_deref(),
            output: input.output.as_ref(),
        })
        .map_err(pipeline_error_message)?;
        serde_json::to_value(result).map_err(|_| "render response failed".to_string())
    }

    fn apply_agent_session_action(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        self.ensure_revision(request, &path)?;
        let project_id = required_string(&request.payload, "projectId")?;
        let action: AgentSessionAction = serde_json::from_value(
            request
                .payload
                .get("action")
                .cloned()
                .ok_or_else(|| "agent session action is required".to_string())?,
        )
        .map_err(|_| "agent session action is invalid".to_string())?;
        serde_json::to_value(
            apply_agent_session_action(&path, project_id, action)
                .map_err(|_| "agent session action failed")?,
        )
        .map_err(|_| "agent session response failed".to_string())
    }

    fn start_agent(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let mut project = self.ensure_revision(request, &path)?;
        let expected_revision = project.content_revision;
        let turn: CodexConversationEditRequest = serde_json::from_value(
            request
                .payload
                .get("request")
                .cloned()
                .ok_or_else(|| "agent request is required".to_string())?,
        )
        .map_err(|_| "agent request is invalid".to_string())?;
        turn.validate(&project).map_err(|error| error.to_string())?;
        if !self.agent_fixture {
            let skills = bundled_host_skill_bundle();
            let root = path.as_path();
            let (cancellation_guard, cancellation) = register_codex_project_turn(root, Some(&path))
                .map_err(|error| error.to_string())?;
            let deadline = codex_app_server_deadline();
            let command = bundled_codex_app_server_command().map_err(|error| error.to_string())?;
            let mut app_server = StdioCodexAppServerTransport::spawn_until(
                &command,
                None,
                deadline,
                Some(&cancellation),
            )
            .map_err(|error| error.to_string())?;
            let turn_request = build_agent_conversation_request(
                &project,
                &turn,
                root,
                Some(&path),
                &skills,
                agent_session_for_next_turn(AgentBackendKind::Codex, &project, Some(&path))
                    .map_err(|error| error.to_string())?,
            );
            let outcome = CodexTurnTransport::new(&mut app_server, 1)
                .run_conversation_turn(&turn_request, deadline, Some(&cancellation))
                .map_err(|error| error.to_string())?;
            close_codex_turn_cancellation(cancellation_guard, &cancellation)
                .map_err(|error| error.to_string())?;
            let completed =
                finish_agent_conversation_turn(AgentBackendKind::Codex, &project, &turn, &outcome);
            project.codex_thread_id = Some(completed.thread_id.clone());
            project = replace_split_project_if_revision(&path, project, expected_revision)
                .map_err(|error| error.to_string())?
                .project;
            record_app_server_conversation_turn(
                &path,
                &project.id,
                &completed.thread_id,
                completed.record,
            )
            .map_err(|error| error.to_string())?;
            return Ok(json!({
                "project": project,
                "threadId": completed.thread_id,
                "threadResponse": completed.thread_response,
                "turnResponse": completed.turn_response,
                "proposal": completed.proposal,
                "preparedProposal": completed.prepared_proposal,
                "proposalValidationIssues": completed.proposal_validation_issues
            }));
        }
        let proposal = fixture_edl_proposal(&project)?;
        let prepared = prepare_codex_conversation_proposal(&project, &proposal)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "project": project,
            "threadId": "remote-e2e-fixture-thread",
            "threadResponse": {
                "provider": "fixture",
                "requiredSkills": ["video-creater-video-pipeline"]
            },
            "turnResponse": {"status":"completed"},
            "proposal": proposal,
            "preparedProposal": prepared,
            "proposalValidationIssues": null
        }))
    }

    fn cancel_agent(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let root = path.as_path();
        request_codex_project_turn_cancellation(root, Some(&path))
            .map(Value::Bool)
            .map_err(|error| error.to_string())
    }

    fn cancel_generation(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let project = load_split_project(&path).map_err(|_| "project could not be loaded")?;
        let job_id = required_string(&request.payload, "jobId")?;
        let updated_at = required_string(&request.payload, "updatedAt")?;
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
            .and_then(Value::as_str)
            .ok_or_else(|| "job start request is missing assetId".to_string())?;
        let asset = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| format!("generation asset was not found: {asset_id}"))?;

        if job.status == JobStatus::Cancelled || asset.status == GeneratedAssetStatus::Cancelled {
            let _ = request_generation_cancellation(&project.id, job_id);
            return Ok(json!({"outcome": "alreadyCancelled", "project": project}));
        }
        if matches!(job.status, JobStatus::Completed | JobStatus::Failed)
            || matches!(
                asset.status,
                GeneratedAssetStatus::Completed | GeneratedAssetStatus::Failed
            )
        {
            return Ok(json!({"outcome": "alreadyTerminal", "project": project}));
        }

        let outcome = request_generation_cancellation(&project.id, job_id);
        if outcome == GenerationCancellationRequestOutcome::TooLate {
            return Ok(json!({"outcome": "alreadyTerminal", "project": project}));
        }
        let actions = temporal_generate_media_cancellation_actions(
            job,
            asset_id,
            job.workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            updated_at,
        )
        .map_err(|error| error.to_string())?;
        let write = apply_project_actions_to_split_project(&path, actions)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "outcome": if outcome == GenerationCancellationRequestOutcome::AlreadyRequested {
                "alreadyCancelled"
            } else {
                "cancelled"
            },
            "project": write.project
        }))
    }

    fn cancel_generation_provider(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let project = load_split_project(&path).map_err(|_| "project could not be loaded")?;
        let job_id = required_string(&request.payload, "jobId")?;
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
            .and_then(Value::as_str)
            .ok_or_else(|| "job start request is missing assetId".to_string())?;
        let provider = project
            .generated_assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .map(|asset| asset.model.provider.trim())
            .filter(|provider| !provider.is_empty())
            .ok_or_else(|| format!("generation asset was not found: {asset_id}"))?;
        if provider_request.provider != provider {
            return Err("provider request metadata does not match the generated asset".into());
        }
        if !matches!(provider, "fal.ai" | "replicate") {
            return Err(format!(
                "provider-side cancellation is unavailable for {provider}; use local cancellation"
            ));
        }
        let credential = resolve_provider_credential(provider)
            .map(|credential| credential.into_secret())
            .map_err(|error| error.to_string())?;
        let client = reqwest::blocking::Client::new();
        serde_json::to_value(
            temporal_generate_media_cancel_provider_activity_with_client(
                &client,
                &provider_request.cancel_url,
                &credential,
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "provider cancellation response failed".to_string())
    }

    fn cancel_render(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        let job_id = required_string(&request.payload, "jobId")?.to_string();
        let attempt_id = required_string(&request.payload, "attemptId")?.to_string();
        let updated_at = required_string(&request.payload, "updatedAt")?.to_string();
        // Signal first: the renderer may hold the project mutation lease while this request runs.
        let outcome = request_render_cancellation_by_locator(&path, &job_id, &attempt_id);
        serde_json::to_value(record_render_cancellation_outcome(
            &path, outcome, job_id, attempt_id, updated_at,
        )?)
        .map_err(|_| "render cancellation response failed".to_string())
    }

    fn apply_agent_proposal(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        self.ensure_revision(request, &path)?;
        let input: ApplyAgentProposalRequest = serde_json::from_value(request.payload.clone())
            .map_err(|_| "agent apply request is invalid".to_string())?;
        serde_json::to_value(
            apply_codex_conversation_proposal(
                &path,
                CodexConversationApplyRequest {
                    proposal: input.proposal,
                    action_ids: input.action_ids,
                    review_approved: input.review_approved,
                    session_id: input.session_id,
                },
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "agent apply response failed".to_string())
    }

    fn undo_agent_proposal(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let path = self.resolve_project(request)?;
        self.ensure_revision(request, &path)?;
        let history_entry_id = request
            .payload
            .get("historyEntryId")
            .and_then(Value::as_str);
        serde_json::to_value(
            undo_codex_conversation_edit(&path, history_entry_id)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "agent undo response failed".to_string())
    }
}

fn already_cancelled_render_write(
    project_dir: &std::path::Path,
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
            manifest_path: crate::project::split::split_project_manifest_path(project_dir)
                .display()
                .to_string(),
            written_files: Vec::new(),
            removed_files: Vec::new(),
            recovery_pending: false,
        },
    })
}

fn record_render_cancellation_outcome(
    project_dir: &std::path::Path,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderMediaRequest {
    project_id: String,
    profile: ExportProfile,
    quality: RenderQuality,
    width: u32,
    height: u32,
    job_id: String,
    attempt_id: String,
    updated_at: String,
    #[serde(default)]
    range_start_seconds: Option<f64>,
    #[serde(default)]
    range_end_seconds: Option<f64>,
    #[serde(default)]
    timeline_id: Option<String>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    encode_tier: Option<ExportEncodeTier>,
    #[serde(default)]
    output: Option<ExportOutputRequest>,
    #[serde(default)]
    export_settings: Option<JobExportSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyAgentProposalRequest {
    proposal: CodexConversationEditProposal,
    action_ids: Vec<String>,
    #[serde(default)]
    review_approved: bool,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilmstripRequest {
    media_id: String,
    source_in: f64,
    source_out: f64,
    speed: f64,
    zoom_bucket: u32,
    height_bucket: u32,
    clip_pixel_width: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalPreviewRequest {
    playhead_seconds: f64,
    job_id: String,
    updated_at: String,
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{key} is required"))
}

fn pipeline_error_message(errors: Vec<crate::render_pipeline::error::PipelineError>) -> String {
    errors
        .into_iter()
        .map(|error| error.message)
        .collect::<Vec<_>>()
        .join("; ")
}

fn remote_agent_fixture_enabled() -> bool {
    debug_agent_fixture_enabled()
}

#[cfg(debug_assertions)]
fn debug_agent_fixture_enabled() -> bool {
    std::env::var("VIDEO_CREATER_REMOTE_E2E").as_deref() == Ok("1")
        && std::env::var("VIDEO_CREATER_REMOTE_AGENT_FIXTURE").as_deref() == Ok("1")
}

#[cfg(not(debug_assertions))]
fn debug_agent_fixture_enabled() -> bool {
    false
}

fn fixture_edl_proposal(project: &VideoProject) -> Result<CodexConversationEditProposal, String> {
    let eligible = |media: &&crate::project::model::MediaAsset| {
        media.duration_seconds.is_finite() && media.duration_seconds > 0.25
    };
    let media = project
        .media
        .iter()
        .filter(eligible)
        .find(|media| media.kind == MediaKind::Video)
        .or_else(|| {
            project
                .media
                .iter()
                .filter(eligible)
                .find(|media| media.kind == MediaKind::Audio)
        })
        .ok_or_else(|| "the fixture agent needs imported audio or video".to_string())?;
    let (source_in, source_out) = if media.duration_seconds > 0.5 {
        (0.1, (media.duration_seconds - 0.1).min(0.9))
    } else {
        (0.0, media.duration_seconds)
    };
    let duration = source_out - source_in;
    if duration <= 0.0 {
        return Err("the fixture media has no selectable EDL range".into());
    }
    Ok(CodexConversationEditProposal {
        summary: "Build a concise rough cut from a selected source range.".into(),
        edl: vec![CodexProposalClip {
            media_id: media.id.clone(),
            source_in,
            source_out,
            reason:
                "A bounded primary-source selection proves the edit is an EDL, not a pass-through."
                    .into(),
        }],
        // Rust materializes the primary timeline items from this EDL. Keeping the explicit
        // action list empty prevents an agent from smuggling a second, non-EDL primary clip in.
        project_actions: Vec::new(),
        render_review: Some(CodexRenderReview {
            duration_seconds: duration,
            stream_check_required: true,
            caption_alignment_required: false,
            overlay_timing_required: false,
            visual_frame_evidence_required: false,
            artifact_paths_required: true,
            log_reference_required: true,
        }),
    })
}

fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "other"
    }
}

fn default_preferences() -> Value {
    json!({
        "schemaVersion": 2,
        "projectLocation": { "mode": "ask" },
        "requireProviderUploadConfirmation": true,
        "renderCompletionNotifications": false,
        "newProjectDefaults": {
            "width": 1920,
            "height": 1080,
            "fps": 30,
            "loudnessLufs": -14,
            "captions": "burn_in"
        },
        "enabledGenerationModelIds": [],
        "generationExecutionBackend": "inProcess",
        "agentBackend": "automatic",
        "claudeModel": "sonnet",
        "claudeExecutablePath": ""
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_host::registry::WEB_HOST_DISPATCHED_OPERATIONS;

    #[test]
    fn every_advertised_operation_has_a_dispatch_arm() {
        let dispatcher = HostDispatcher::default();
        for operation in WEB_HOST_DISPATCHED_OPERATIONS {
            let result = dispatcher.dispatch(&RpcEnvelope {
                request_id: "coverage".into(),
                operation: (*operation).into(),
                project_id: Some("opaque-project".into()),
                expected_revision: Some(1),
                editor_lease_token: Some("lease".into()),
                payload: json!({}),
            });
            assert!(
                !matches!(&result, Err(error) if error.contains("remote operation is not implemented")),
                "advertised operation has no dispatcher: {operation}"
            );
        }
    }
}
