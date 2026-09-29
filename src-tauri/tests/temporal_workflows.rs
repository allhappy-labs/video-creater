use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::thread;
use std::time::Duration;
use video_creater_lib::codex::app_server::{CodexAppServerError, CodexAppServerTransport};
use video_creater_lib::codex::proposal::{CodexEditProposal, CodexProposalClip, CodexRenderReview};
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::edit::render_plan::{RenderQuality, RenderQualityProfile};
use video_creater_lib::generation::fal::{FalGenerationRunOptions, FalQueueStatusKind};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAsset, MediaFolder,
    MediaKind, ProjectExportArtifactKind, TimelineItem, TimelineItemKind, TimelineSource,
    Transcript, TranscriptWord, VideoProject,
};
use video_creater_lib::project::nle_export::NleXmlFormat;
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::render_runtime::start_render_process_runtime;
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime;
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::job::{
    TemporalTranscribeProbeOutput, TemporalTranscribeProbeStatus,
};
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::model::ModelInstallStatus;
use video_creater_lib::transcription::model::{
    catalog_entry_runtime_id, parakeet_v3_catalog_entry, safe_model_dir_name,
    TranscriptionModelCatalogEntry, FLUID_AUDIO_COREML_RUNTIME_ID,
    FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME,
};
use video_creater_lib::transcription::runtime::{
    FixtureTranscriptionBackend, FixtureTranscriptionRuntime, RuntimeCapability, TranscriptToken,
    TranscriptionBackend, TranscriptionRuntimeError, TranscriptionRuntimeJob,
    TranscriptionRuntimeOutput, TranscriptionRuntimeRegistry,
};
use video_creater_lib::transcription::store::TranscriptionModelStore;
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::store::COREML_INSPECTION_FILE_NAME;
use video_creater_lib::workflows::{
    temporal_codex_edit_attach_failure_activity_value,
    temporal_codex_edit_collect_project_context_activity_value,
    temporal_codex_edit_failure_actions, temporal_codex_edit_failure_activity_input_value,
    temporal_codex_edit_persist_accepted_proposal_activity_value,
    temporal_codex_edit_proposal_activity_input_value,
    temporal_codex_edit_request_proposal_activity_with_transport,
    temporal_codex_edit_start_request, temporal_codex_edit_validate_project_actions_activity_value,
    temporal_export_media_attach_export_report_activity_value, temporal_export_media_start_request,
    temporal_export_media_start_request_with_options,
    temporal_export_media_workflow_activity_plan_value,
    temporal_export_media_write_artifact_activity_value,
    temporal_export_nle_xml_attach_export_report_activity_value,
    temporal_export_nle_xml_build_activity_value, temporal_export_nle_xml_start_request,
    temporal_export_nle_xml_start_request_with_overwrite,
    temporal_export_nle_xml_validate_activity_value,
    temporal_export_nle_xml_write_artifact_activity_value,
    temporal_export_project_bundle_start_request,
    temporal_export_project_bundle_write_activity_value,
    temporal_generate_media_attach_failure_activity_value, temporal_generate_media_failure_actions,
    temporal_generate_media_fal_queue_submission,
    temporal_generate_media_fal_run_submission_with_client,
    temporal_generate_media_mock_completion_actions, temporal_generate_media_start_request,
    temporal_generate_media_workflow_input, temporal_job_summary,
    temporal_render_build_plan_activity_value, temporal_render_draft_start_request,
    temporal_render_media_activity_input_value, temporal_render_workflow_activity_plan,
    temporal_start_result_action, temporal_transcribe_media_start_request,
    temporal_transcribe_run_activity_value, temporal_transcribe_run_activity_value_with_backend,
    temporal_transcribe_run_activity_value_with_runtime_registry,
    temporal_transcribe_run_activity_value_with_runtime_registry_and_store,
    temporal_transcribe_store_activity_value, temporal_transcribe_workflow_activity_plan_value,
    temporal_validate_rendered_media_activity_value, temporal_worker_manifest,
    temporal_worker_registration_plan, temporal_workflow_client_start_plan, temporal_workflow_spec,
    temporal_workflow_started_start_result, temporal_workflow_unavailable_start_result,
    FalWorkflowExecution, TemporalGenerateMediaBrief, TemporalStartResultError,
    TemporalWorkflowInputError, TemporalWorkflowKind, VIDEO_CREATER_TEMPORAL_TASK_QUEUE,
};
#[cfg(target_os = "macos")]
use video_creater_lib::workflows::{
    temporal_transcribe_probe_media_activity_value_with_store,
    temporal_transcribe_probe_media_activity_value_with_store_and_registry,
};

#[path = "temporal_workflows/export_destination.rs"]
mod export_destination;

#[derive(Default)]
struct RecordingCodexTransport {
    requests: Vec<Value>,
    responses: Vec<Value>,
    messages: std::collections::VecDeque<video_creater_lib::codex::app_server::AppServerMessage>,
    mutate_project_on_turn: Option<std::path::PathBuf>,
}

impl RecordingCodexTransport {
    fn with_responses(responses: Vec<Value>) -> Self {
        Self {
            requests: Vec::new(),
            responses: responses.into_iter().rev().collect(),
            messages: Default::default(),
            mutate_project_on_turn: None,
        }
    }
}

impl CodexAppServerTransport for RecordingCodexTransport {
    fn send(&mut self, request: Value) -> Result<(), CodexAppServerError> {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let thread_id = request
            .pointer("/params/threadId")
            .cloned()
            .unwrap_or(json!("thread-workflow"));
        self.requests.push(request);
        let result = self.responses.pop().ok_or_else(|| {
            CodexAppServerError::Transport("missing fake Codex response".to_string())
        })?;
        if method == "turn/start" {
            if let Some(project_dir) = self.mutate_project_on_turn.take() {
                let mut canonical = load_split_project(&project_dir).unwrap();
                canonical.name = "Concurrent edit".to_string();
                save_split_project(&project_dir, &canonical).unwrap();
            }
            self.messages.push_back(
                video_creater_lib::codex::app_server::AppServerMessage::Response {
                    id,
                    result: json!({"turn":{"id":"turn-fixture","items":[],"status":"inProgress"}}),
                },
            );
            self.messages.push_back(video_creater_lib::codex::app_server::AppServerMessage::Notification { method:"turn/completed".into(), params:json!({"threadId":thread_id,"turn":{"id":"turn-fixture","status":"completed","items":[{"id":"message-fixture","type":"agentMessage","phase":"final_answer","text":serde_json::to_string(&result).unwrap()}]}}) });
        } else {
            self.messages.push_back(
                video_creater_lib::codex::app_server::AppServerMessage::Response { id, result },
            );
        }
        Ok(())
    }
    fn recv_until(
        &mut self,
        _deadline: std::time::Instant,
    ) -> Result<video_creater_lib::codex::app_server::AppServerMessage, CodexAppServerError> {
        self.messages
            .pop_front()
            .ok_or_else(|| CodexAppServerError::Transport("missing fake Codex response".into()))
    }
    fn terminate(
        &mut self,
    ) -> Result<video_creater_lib::codex::app_server::AppServerCleanupReport, CodexAppServerError>
    {
        Ok(Default::default())
    }
}

#[derive(Clone)]
struct RuntimeIdFixtureTranscriptionBackend {
    runtime_id: String,
    tokens: Vec<TranscriptToken>,
}

impl TranscriptionBackend for RuntimeIdFixtureTranscriptionBackend {
    fn transcribe(
        &self,
        _job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        Ok(TranscriptionRuntimeOutput {
            runtime_id: self.runtime_id.clone(),
            model_id: model_id.to_string(),
            tokens: self.tokens.clone(),
        })
    }
}

#[test]
fn temporal_job_summary_serializes_workflow_metadata() {
    let summary = temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        "Project A",
        "Draft Render 1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );

    let value = serde_json::to_value(summary).expect("serialize summary");

    assert_eq!(
        value,
        json!({
            "id": "Draft Render 1",
            "kind": "render_draft",
            "status": "queued",
            "updatedAt": "2026-06-23T12:00:00Z",
            "workflow": {
                "workflowId": "video-creater/project-a/render-draft/draft-render-1",
                "workflowType": "VideoCreaterRenderDraftWorkflow",
                "taskQueue": "video-creater-workflows",
                "runId": null,
                "activityTypes": [
                    "BuildRenderPlan",
                    "RenderMedia",
                    "ValidateRenderedMedia",
                    "AttachRenderReport"
                ]
            }
        })
    );
}

#[test]
fn temporal_workflow_spec_maps_media_generation_to_registered_types() {
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
    );

    assert_eq!(spec.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(spec.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(
        spec.workflow_id,
        "video-creater/project-1/generate-media/generated-shot-1"
    );
    assert_eq!(
        spec.activity_types,
        vec![
            "PrepareProviderInputs",
            "BuildFalGenerationRequest",
            "RunMediaProviderGeneration",
            "AttachGeneratedAssetFailure",
        ]
    );
}

#[test]
fn temporal_generate_media_start_request_serializes_temporal_client_payload() {
    let request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        true,
        None,
    );

    let value = serde_json::to_value(request).expect("serialize start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/generate-media/generated-shot-job-1",
            "workflowType": "VideoCreaterGenerateMediaWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectDir": "/tmp/video-creater/Project A",
                "assetId": "generated-shot-1",
                "jobId": "generated-shot-job-1",
                "mockMode": true
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "generated-shot-job-1",
                "workflowKind": "generate_media"
            },
            "activityTypes": [
                "PrepareProviderInputs",
                "BuildFalGenerationRequest",
                "RunMediaProviderGeneration",
                "AttachGeneratedAssetFailure"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let serialized = value.to_string();
    assert!(!serialized.contains("FAL_KEY"));
    assert!(value["input"]
        .get(concat!("providerCredential", "EnvVar"))
        .is_none());
}

#[test]
fn temporal_transcribe_media_start_request_contains_replayable_input() {
    let request = temporal_transcribe_media_start_request(
        "project-1",
        "/tmp/video-project",
        "media-1",
        "transcribe-job-1",
        "en",
    );

    assert_eq!(request.workflow_type, "VideoCreaterTranscribeMediaWorkflow");
    assert_eq!(
        request.workflow_id,
        "video-creater/project-1/transcribe-media/transcribe-job-1"
    );
    assert_eq!(request.input["projectId"], "project-1");
    assert_eq!(request.input["projectDir"], "/tmp/video-project");
    assert_eq!(request.input["mediaId"], "media-1");
    assert_eq!(request.input["jobId"], "transcribe-job-1");
    assert_eq!(request.input["languageMode"], "en");
    assert_eq!(
        request.activity_types,
        vec!["ProbeMedia", "RunTranscription", "StoreTranscript"]
    );
}

#[test]
fn run_transcription_activity_writes_runtime_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let output = temporal_transcribe_run_activity_value_with_backend(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &FixtureTranscriptionBackend {
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: None,
            }],
        },
        &artifact_dir,
    )
    .expect("run output");

    assert_eq!(output["tokenCount"], 1);
    assert_eq!(output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output["modelId"], "nvidia/parakeet-tdt-0.6b-v3");
    let artifact_path = output["artifactPath"].as_str().expect("artifact path");
    assert!(Path::new(artifact_path).starts_with(&artifact_dir));
    let artifact: Value =
        serde_json::from_str(&fs::read_to_string(artifact_path).expect("read artifact"))
            .expect("artifact json");
    assert_eq!(artifact["schemaVersion"], 1);
    assert_eq!(artifact["mediaId"], "media-1");
    assert_eq!(artifact["engine"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(artifact["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(artifact["modelId"], "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(artifact["languageMode"], "en");
    assert_eq!(artifact["tokens"][0]["token"], "Hello");
    assert_eq!(artifact["tokens"][0]["start"], 0.1);
    assert_eq!(artifact["tokens"][0]["end"], 0.4);
}

#[test]
fn run_transcription_activity_dispatches_by_runtime_registry() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
        capability: RuntimeCapability::Ready,
        supported: true,
        tokens: vec![TranscriptToken {
            token: "Hello".to_string(),
            start: 0.1,
            end: 0.4,
            confidence: Some(0.91),
        }],
    })]);

    let output = temporal_transcribe_run_activity_value_with_runtime_registry(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &registry,
        &artifact_dir,
    )
    .expect("run output");

    assert_eq!(output["tokenCount"], 1);
    assert_eq!(output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output["modelId"], "nvidia/parakeet-tdt-0.6b-v3");
    let artifact_path = output["artifactPath"].as_str().expect("artifact path");
    assert!(Path::new(artifact_path).starts_with(&artifact_dir));
    let artifact: Value =
        serde_json::from_str(&fs::read_to_string(artifact_path).expect("read artifact"))
            .expect("artifact json");
    assert_eq!(artifact["engine"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(artifact["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(artifact["tokens"][0]["token"], "Hello");
    assert_eq!(artifact["tokens"][0]["start"], 0.1);
    assert_eq!(artifact["tokens"][0]["end"], 0.4);
    assert_eq!(artifact["tokens"][0]["confidence"], 0.91);
}

#[test]
fn run_transcription_activity_rejects_tampered_source_path_before_writing_artifact() {
    let project_dir = tempfile::tempdir().expect("temp project");
    let model_root = tempfile::tempdir().expect("model root");
    let source_path = project_dir.path().join("media/source.mov");
    let tampered_source_path = project_dir.path().join("media/other.mov");
    fs::create_dir_all(source_path.parent().expect("media parent")).expect("media parent");
    fs::write(&source_path, b"media").expect("source media");
    fs::write(&tampered_source_path, b"other media").expect("other media");

    let mut project = sample_project();
    project.id = "project-1".to_string();
    project.media.push(MediaAsset {
        id: "media-target".to_string(),
        name: None,
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save project");

    let entry = parakeet_v3_catalog_entry();
    let runtime_model_dir = model_root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    write_required_model_files(&entry, &runtime_model_dir);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    store.verify(entry.id).expect("verify model");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
        capability: RuntimeCapability::Ready,
        supported: true,
        tokens: vec![TranscriptToken {
            token: "Hello".to_string(),
            start: 0.1,
            end: 0.4,
            confidence: Some(0.91),
        }],
    })]);
    let artifact_dir = project_dir.path().join("workflow-artifacts/transcribe-1");

    let error = temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "media-target",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": tampered_source_path.display().to_string(),
            "artifactPath": project_dir.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": entry.id,
            "modelPath": runtime_model_dir.display().to_string(),
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &registry,
        &store,
        &artifact_dir,
    )
    .expect_err("tampered source path should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("sourcePath".to_string())
    );
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_tampered_model_path_before_writing_artifact() {
    let project_dir = tempfile::tempdir().expect("temp project");
    let model_root = tempfile::tempdir().expect("model root");
    let source_path = project_dir.path().join("media/source.mov");
    fs::create_dir_all(source_path.parent().expect("media parent")).expect("media parent");
    fs::write(&source_path, b"media").expect("source media");

    let mut project = sample_project();
    project.id = "project-1".to_string();
    project.media.push(MediaAsset {
        id: "media-target".to_string(),
        name: None,
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save project");

    // The platform catalog entry owns the runtime: FluidAudio Core ML on macOS, sherpa-onnx on Linux.
    let entry = parakeet_v3_catalog_entry();
    let runtime_id = catalog_entry_runtime_id(&entry);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    let runtime_model_dir = store
        .runtime_model_dir(entry.id, runtime_id)
        .expect("platform runtime model dir");
    write_required_model_files(&entry, &runtime_model_dir);
    store.verify(entry.id).expect("verify model");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id,
        capability: RuntimeCapability::Ready,
        supported: true,
        tokens: vec![TranscriptToken {
            token: "Hello".to_string(),
            start: 0.1,
            end: 0.4,
            confidence: Some(0.91),
        }],
    })]);
    let artifact_dir = project_dir.path().join("workflow-artifacts/transcribe-1");
    let tampered_model_dir = model_root.path().join("other-runtime-model");
    fs::create_dir_all(&tampered_model_dir).expect("tampered model dir");

    let error = temporal_transcribe_run_activity_value_with_runtime_registry_and_store(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "media-target",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": source_path.display().to_string(),
            "artifactPath": project_dir.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": entry.id,
            "modelPath": tampered_model_dir.display().to_string(),
            "runtimeId": runtime_id
        }),
        &registry,
        &store,
        &artifact_dir,
    )
    .expect_err("tampered model path should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("modelPath".to_string())
    );
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_unknown_registry_runtime_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");
    let registry = TranscriptionRuntimeRegistry::new(Vec::new());

    let error = temporal_transcribe_run_activity_value_with_runtime_registry(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": "unknown_runtime"
        }),
        &registry,
        &artifact_dir,
    )
    .expect_err("unknown runtime should not produce a runtime artifact");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error
        .to_string()
        .contains("unknown runtime `unknown_runtime`"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_unsupported_registry_model_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
        capability: RuntimeCapability::Ready,
        supported: false,
        tokens: vec![TranscriptToken {
            token: "Hello".to_string(),
            start: 0.1,
            end: 0.4,
            confidence: Some(0.91),
        }],
    })]);

    let error = temporal_transcribe_run_activity_value_with_runtime_registry(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &registry,
        &artifact_dir,
    )
    .expect_err("unsupported installed model should not produce a runtime artifact");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error
        .to_string()
        .contains("does not support installed model"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_empty_runtime_tokens_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let error = temporal_transcribe_run_activity_value_with_backend(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &FixtureTranscriptionBackend { tokens: Vec::new() },
        &artifact_dir,
    )
    .expect_err("empty tokens should not produce a runtime artifact");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error
        .to_string()
        .contains("parakeet transcript tokens are missing"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_no_usable_runtime_tokens_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let error = temporal_transcribe_run_activity_value_with_backend(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &FixtureTranscriptionBackend {
            tokens: vec![TranscriptToken {
                token: "[noise]".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: None,
            }],
        },
        &artifact_dir,
    )
    .expect_err("cleaned tokens should not produce a runtime artifact");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error
        .to_string()
        .contains("transcript artifact has no usable tokens after cleaning"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_mismatched_runtime_id_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let error = temporal_transcribe_run_activity_value_with_backend(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &RuntimeIdFixtureTranscriptionBackend {
            runtime_id: "other_runtime".to_string(),
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: None,
            }],
        },
        &artifact_dir,
    )
    .expect_err("mismatched runtime id should not produce a runtime artifact");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("runtimeId".to_string())
    );
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_blank_backend_runtime_id_before_writing_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let error = temporal_transcribe_run_activity_value_with_backend(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mov",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &RuntimeIdFixtureTranscriptionBackend {
            runtime_id: " ".to_string(),
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: None,
            }],
        },
        &artifact_dir,
    )
    .expect_err("blank runtime id should not produce a runtime artifact");

    assert_eq!(
        error,
        TemporalWorkflowInputError::BlankField("runtimeId".to_string())
    );
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[test]
fn run_transcription_activity_rejects_unsafe_default_artifact_job_id() {
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = temporal_transcribe_run_activity_value(json!({
        "status": "ready",
        "projectId": "project-1",
        "projectDir": project_dir.path().display().to_string(),
        "mediaId": "media-1",
        "jobId": "../escape",
        "languageMode": "en",
        "sourcePath": "/tmp/source.mov",
        "artifactPath": project_dir.path().join("transcripts/escape-transcript.json").display().to_string(),
        "mediaKind": "video",
        "mediaRelativePath": "media/source.mov",
        "mediaDurationSeconds": 12.0,
        "mediaWidth": 1920,
        "mediaHeight": 1080,
        "mediaFps": 30.0,
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "modelPath": "/tmp/model",
        "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
    }))
    .expect_err("unsafe job id should not create a default artifact path");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
    );
    assert!(!project_dir.path().join("workflow-artifacts").exists());
}

#[test]
fn store_transcript_activity_replaces_project_transcript() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    project.transcripts.clear();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("old".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });
    save_split_project(root.path(), &project).expect("save project");

    let artifact_path = root
        .path()
        .join("workflow-artifacts/transcribe-1/transcribe-1-transcript.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact parent")).expect("artifact parent");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    let output = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1
        }),
        &artifact_path,
    ))
    .expect("store output");

    assert_eq!(output["transcriptId"], "transcript-media-1");
    assert_eq!(output["wordCount"], 1);
    assert_eq!(output["segmentCount"], 1);
    let stored = load_split_project(root.path()).expect("load stored project");
    assert_eq!(stored.transcripts.len(), 1);
    assert_eq!(
        stored.transcripts[0].engine.as_deref(),
        Some(FLUID_AUDIO_COREML_RUNTIME_ID)
    );
    assert_eq!(
        stored.transcripts[0].raw_artifact_path.as_deref(),
        Some("workflow-artifacts/transcribe-1/transcribe-1-transcript.json")
    );
    assert_eq!(stored.transcripts[0].words[0].text, "Hello");
}

#[test]
fn store_transcript_activity_marks_temporal_job_completed() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        "project-1",
        "transcribe-1",
        JobStatus::Running,
        "2026-06-28T00:00:00Z",
    ));
    save_split_project(root.path(), &project).expect("save project");

    let artifact_path = root
        .path()
        .join("workflow-artifacts/transcribe-1/transcribe-1-transcript.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact parent")).expect("artifact parent");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1,
            "updatedAt": "2026-06-28T00:01:00Z",
            "runId": "temporal-run-1"
        }),
        &artifact_path,
    ))
    .expect("store output");

    let stored = load_split_project(root.path()).expect("load stored project");
    let job = stored
        .jobs
        .iter()
        .find(|job| job.id == "transcribe-1")
        .expect("transcribe job");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.updated_at, "2026-06-28T00:01:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-1")
    );
}

#[test]
fn store_transcript_activity_rejects_artifact_outside_project() {
    let root = tempfile::tempdir().expect("temp project");
    let outside = tempfile::tempdir().expect("outside artifact dir");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    save_split_project(root.path(), &project).expect("save project");
    let artifact_path = outside.path().join("artifact.json");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write outside artifact");

    let error = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1
        }),
        &artifact_path,
    ))
    .expect_err("outside artifact should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("artifactPath".to_string())
    );
}

#[test]
fn store_transcript_activity_rejects_absolute_artifact_escape_with_parent_dir() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    save_split_project(root.path(), &project).expect("save project");
    let outside_path = root
        .path()
        .parent()
        .expect("project parent")
        .join("outside-transcript-artifact.json");
    fs::write(
        &outside_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write outside artifact");

    let escaped_artifact_path = root
        .path()
        .join("../outside-transcript-artifact.json")
        .display()
        .to_string();
    let error = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": escaped_artifact_path,
            "tokenCount": 1
        }),
        &outside_path,
    ))
    .expect_err("parent-dir absolute artifact escape should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("artifactPath".to_string())
    );
}

#[test]
fn store_transcript_activity_rejects_missing_project_media() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    save_split_project(root.path(), &project).expect("save project");

    let artifact_path = root
        .path()
        .join("workflow-artifacts/transcribe-1/transcribe-1-transcript.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact parent")).expect("artifact parent");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "missing-media",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    let error = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "missing-media",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1
        }),
        &artifact_path,
    ))
    .expect_err("missing project media should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("mediaId".to_string())
    );
}

#[test]
fn store_transcript_activity_rejects_artifact_model_mismatch() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    save_split_project(root.path(), &project).expect("save project");

    let artifact_path = root
        .path()
        .join("workflow-artifacts/transcribe-1/transcribe-1-transcript.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact parent")).expect("artifact parent");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "other-model",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    let error = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1
        }),
        &artifact_path,
    ))
    .expect_err("artifact model mismatch should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("modelId".to_string())
    );
}

#[test]
fn store_transcript_activity_rejects_artifact_token_count_mismatch() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = sample_project();
    project.id = "project-1".to_string();
    save_split_project(root.path(), &project).expect("save project");

    let artifact_path = root
        .path()
        .join("workflow-artifacts/transcribe-1/transcribe-1-transcript.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact parent")).expect("artifact parent");
    fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    let error = temporal_transcribe_store_activity_value(transcribe_store_input(
        json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 2
        }),
        &artifact_path,
    ))
    .expect_err("artifact token count mismatch should be rejected");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("tokenCount".to_string())
    );
}

#[test]
fn transcribe_workflow_activity_plan_is_not_placeholder() {
    let plan = temporal_transcribe_workflow_activity_plan_value(
        json!({
            "projectId": "project-1",
            "projectDir": "/tmp/project",
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en"
        }),
        "2026-06-28T00:00:00Z",
        Some("run-1"),
    )
    .expect("plan");

    assert_eq!(plan["status"], "planned");
    assert_eq!(plan["workflowType"], "VideoCreaterTranscribeMediaWorkflow");
    assert_eq!(
        plan["activityTypes"],
        json!(["ProbeMedia", "RunTranscription", "StoreTranscript"])
    );
    assert_eq!(plan["probeMediaInput"]["mediaId"], "media-1");
    assert_eq!(plan["runTranscriptionInputFrom"], "ProbeMedia");
    assert_eq!(plan["storeTranscriptInputFrom"], "RunTranscription");
    assert_eq!(plan["updatedAt"], "2026-06-28T00:00:00Z");
    assert_eq!(plan["runId"], "run-1");
}

#[cfg(target_os = "macos")]
#[test]
fn registry_coreml_run_activity_reports_not_ready_without_coreml_inspection_report() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_dir = tempfile::tempdir().expect("model dir");
    let helper_path = project_dir
        .path()
        .join("video-creater-fluidaudio-transcribe");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ] {
        fs::create_dir_all(model_dir.path().join(bundle)).expect("model bundle dir");
    }
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FluidAudioCoreMlRuntime::new(
        helper_path,
    ))]);
    let artifact_dir = project_dir.path().join("workflow-artifacts/transcribe-1");

    let error = temporal_transcribe_run_activity_value_with_runtime_registry(
        json!({
        "status": "ready",
        "projectId": "project-1",
        "projectDir": project_dir.path().display().to_string(),
        "mediaId": "media-1",
        "jobId": "transcribe-1",
        "languageMode": "en",
        "sourcePath": "/tmp/source.mov",
        "artifactPath": project_dir.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
        "mediaKind": "video",
        "mediaRelativePath": "media/source.mov",
        "mediaDurationSeconds": 12.0,
        "mediaWidth": 1920,
        "mediaHeight": 1080,
        "mediaFps": 30.0,
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "modelPath": model_dir.path().display().to_string(),
        "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &registry,
        &artifact_dir,
    )
    .expect_err("Core ML runtime should not be ready without inspection report");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error.to_string().contains("not ready"));
    assert!(error
        .to_string()
        .contains("missing Core ML inspection report"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn registry_coreml_run_activity_reports_not_ready_without_compiled_model_bundles() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_dir = tempfile::tempdir().expect("model dir");
    let helper_path = project_dir
        .path()
        .join("video-creater-fluidaudio-transcribe");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
    ] {
        fs::create_dir_all(model_dir.path().join(bundle)).expect("model bundle dir");
    }
    fs::write(
        model_dir.path().join(COREML_INSPECTION_FILE_NAME),
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "bundles": []
        }))
        .expect("inspection report json"),
    )
    .expect("inspection report");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FluidAudioCoreMlRuntime::new(
        helper_path,
    ))]);
    let artifact_dir = project_dir.path().join("workflow-artifacts/transcribe-1");

    let error = temporal_transcribe_run_activity_value_with_runtime_registry(
        json!({
        "status": "ready",
        "projectId": "project-1",
        "projectDir": project_dir.path().display().to_string(),
        "mediaId": "media-1",
        "jobId": "transcribe-1",
        "languageMode": "en",
        "sourcePath": "/tmp/source.mov",
        "artifactPath": project_dir.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
        "mediaKind": "video",
        "mediaRelativePath": "media/source.mov",
        "mediaDurationSeconds": 12.0,
        "mediaWidth": 1920,
        "mediaHeight": 1080,
        "mediaFps": 30.0,
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "modelPath": model_dir.path().display().to_string(),
        "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID
        }),
        &registry,
        &artifact_dir,
    )
    .expect_err("Core ML runtime should not be ready without required bundles");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error.to_string().contains("not ready"));
    assert!(error.to_string().contains("missing Core ML bundle"));
    assert!(error.to_string().contains("JointDecisionv3.mlmodelc"));
    assert!(!artifact_dir.join("transcribe-1-transcript.json").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn transcribe_probe_activity_resolves_media_and_global_model() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_root = tempfile::tempdir().expect("model root");
    let media_path = project_dir.path().join("media/source.mov");
    fs::create_dir_all(media_path.parent().expect("media parent")).expect("media parent");
    fs::write(&media_path, b"media").expect("media file");

    let mut project = sample_project();
    project.id = "project-probe".to_string();
    project.media.push(MediaAsset {
        id: "target-media".to_string(),
        name: None,
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save split project");

    let entry = parakeet_v3_catalog_entry();
    let model_dir = model_root.path().join(safe_model_dir_name(entry.id));
    let runtime_model_dir = model_dir.join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    write_required_model_files(&entry, &runtime_model_dir);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    let status = store.verify(entry.id).expect("verify model");
    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let expected_runtime_model_dir = store
        .runtime_model_dir(entry.id, FLUID_AUDIO_COREML_RUNTIME_ID)
        .expect("runtime model dir");

    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
        capability: RuntimeCapability::Ready,
        supported: true,
        tokens: Vec::new(),
    })]);
    let output = temporal_transcribe_probe_media_activity_value_with_store_and_registry(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "transcribe-job-1",
            "languageMode": "en"
        }),
        &store,
        &registry,
    )
    .expect("probe media");

    assert_eq!(output["status"], "ready");
    assert_eq!(output["mediaId"], "target-media");
    assert_eq!(output["modelId"], entry.id);
    assert_eq!(output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output["sourcePath"], media_path.display().to_string());
    assert_eq!(
        output["modelPath"],
        expected_runtime_model_dir.display().to_string()
    );
    assert_eq!(
        output["artifactPath"],
        project_dir
            .path()
            .join("transcripts/transcribe-job-1-transcript.json")
            .display()
            .to_string()
    );
    assert_eq!(output["mediaKind"], "video");
    assert_eq!(output["mediaRelativePath"], "media/source.mov");
    assert_eq!(output["mediaDurationSeconds"], 12.0);
    assert_eq!(output["mediaWidth"], 1920);
    assert_eq!(output["mediaHeight"], 1080);
    assert_eq!(output["mediaFps"], 30.0);

    let typed_output: TemporalTranscribeProbeOutput =
        serde_json::from_value(output.clone()).expect("typed probe output");
    assert_eq!(typed_output.status, TemporalTranscribeProbeStatus::Ready);

    let artifact_dir = project_dir.path().join("workflow-artifacts");
    let run_output = temporal_transcribe_run_activity_value_with_backend(
        output,
        &FixtureTranscriptionBackend {
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: None,
            }],
        },
        &artifact_dir,
    )
    .expect("run transcription with normalized runtime");
    assert_eq!(run_output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
}

#[cfg(target_os = "macos")]
#[test]
fn transcribe_probe_activity_selects_installed_model_through_registry() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_root = tempfile::tempdir().expect("model root");
    let media_path = project_dir.path().join("media/source.mov");
    fs::create_dir_all(media_path.parent().expect("media parent")).expect("media parent");
    fs::write(&media_path, b"media").expect("media file");

    let mut project = sample_project();
    project.id = "project-probe".to_string();
    project.media.push(MediaAsset {
        id: "target-media".to_string(),
        name: None,
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save split project");

    let entry = parakeet_v3_catalog_entry();
    let runtime_model_dir = model_root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    write_required_model_files(&entry, &runtime_model_dir);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    store.verify(entry.id).expect("verify model");
    let expected_runtime_model_dir = store
        .runtime_model_dir(entry.id, FLUID_AUDIO_COREML_RUNTIME_ID)
        .expect("runtime model dir");
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FixtureTranscriptionRuntime {
        runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
        capability: RuntimeCapability::Ready,
        supported: true,
        tokens: Vec::new(),
    })]);

    let output = temporal_transcribe_probe_media_activity_value_with_store_and_registry(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "transcribe-job-1",
            "languageMode": "en"
        }),
        &store,
        &registry,
    )
    .expect("probe media");

    assert_eq!(output["status"], "ready");
    assert_eq!(output["runtimeId"], FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output["modelId"], entry.id);
    assert_eq!(
        output["modelPath"],
        expected_runtime_model_dir.display().to_string()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn transcribe_probe_activity_reports_missing_coreml_inspection_detail() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_root = tempfile::tempdir().expect("model root");
    let media_path = project_dir.path().join("media/source.mov");
    fs::create_dir_all(media_path.parent().expect("media parent")).expect("media parent");
    fs::write(&media_path, b"media").expect("media file");

    let mut project = sample_project();
    project.id = "project-probe".to_string();
    project.media.push(MediaAsset {
        id: "target-media".to_string(),
        name: None,
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save split project");

    let entry = parakeet_v3_catalog_entry();
    let runtime_model_dir = model_root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    write_required_model_files(&entry, &runtime_model_dir);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    store.verify(entry.id).expect("verify model");
    fs::remove_file(runtime_model_dir.join(COREML_INSPECTION_FILE_NAME))
        .expect("remove inspection report");
    let helper_path = project_dir
        .path()
        .join("video-creater-fluidaudio-transcribe");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    let registry = TranscriptionRuntimeRegistry::new(vec![Box::new(FluidAudioCoreMlRuntime::new(
        helper_path,
    ))]);

    let error = temporal_transcribe_probe_media_activity_value_with_store_and_registry(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "transcribe-job-1",
            "languageMode": "en"
        }),
        &store,
        &registry,
    )
    .expect_err("probe should report missing Core ML inspection report");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::Transcription(_)
    ));
    assert!(error.to_string().contains("not ready"));
    assert!(error
        .to_string()
        .contains("missing Core ML inspection report"));
}

#[cfg(target_os = "macos")]
#[test]
fn transcribe_probe_activity_rejects_unsafe_project_paths() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_root = tempfile::tempdir().expect("model root");

    let mut project = sample_project();
    project.id = "project-probe".to_string();
    project.media.push(MediaAsset {
        id: "target-media".to_string(),
        name: None,
        relative_path: "../outside.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    save_split_project(project_dir.path(), &project).expect("save split project");

    let entry = parakeet_v3_catalog_entry();
    let model_dir = model_root.path().join(safe_model_dir_name(entry.id));
    let runtime_model_dir = model_dir.join(FLUID_AUDIO_COREML_RUNTIME_MODEL_DIR_NAME);
    write_required_model_files(&entry, &runtime_model_dir);
    let store = TranscriptionModelStore::new(model_root.path().to_path_buf());
    store.verify(entry.id).expect("verify model");

    let media_path_error = temporal_transcribe_probe_media_activity_value_with_store(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "transcribe-job-1",
            "languageMode": "en"
        }),
        &store,
    )
    .expect_err("unsafe media path should fail");
    assert_eq!(
        media_path_error,
        TemporalWorkflowInputError::MismatchedInputField("media.relativePath".to_string())
    );

    project
        .media
        .iter_mut()
        .find(|media| media.id == "target-media")
        .expect("target media")
        .relative_path = "media/source.mov".to_string();
    save_split_project(project_dir.path(), &project).expect("save split project");

    let missing_file_error = temporal_transcribe_probe_media_activity_value_with_store(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "transcribe-job-1",
            "languageMode": "en"
        }),
        &store,
    )
    .expect_err("missing media file should fail");
    assert_eq!(
        missing_file_error,
        TemporalWorkflowInputError::MismatchedInputField("media.relativePath".to_string())
    );

    let job_id_error = temporal_transcribe_probe_media_activity_value_with_store(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": "../escape",
            "languageMode": "en"
        }),
        &store,
    )
    .expect_err("unsafe job id should fail");
    assert_eq!(
        job_id_error,
        TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
    );

    let padded_job_id_error = temporal_transcribe_probe_media_activity_value_with_store(
        json!({
            "projectId": "project-probe",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "target-media",
            "jobId": " transcribe-job-1 ",
            "languageMode": "en"
        }),
        &store,
    )
    .expect_err("padded job id should fail");
    assert_eq!(
        padded_job_id_error,
        TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
    );
}

#[test]
fn temporal_generate_media_start_request_serializes_generation_brief_without_secret() {
    let request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        true,
        Some(TemporalGenerateMediaBrief {
            name: Some("Hero product reveal".to_string()),
            target_folder_id: Some("folder-generated".to_string()),
            placement_intent: Some("timeline".to_string()),
            prompt: Some("floating product shot with soft backlight".to_string()),
            model: Some(json!({
                "provider": "fal.ai",
                "id": "fal-ai/wan-25-preview/text-to-video"
            })),
            references: Some(json!({
                "mediaIds": ["media-4"],
                "firstFrameMediaId": "media-1",
                "lastFrameMediaId": "media-3"
            })),
            settings: Some(json!({
                "width": 1080,
                "height": 1920,
                "durationSeconds": 8,
                "fps": 24,
                "aspectRatio": "9:16"
            })),
        }),
    );

    let value = serde_json::to_value(request).expect("serialize start request");

    assert_eq!(value["input"]["name"], json!("Hero product reveal"));
    assert_eq!(value["input"]["targetFolderId"], json!("folder-generated"));
    assert_eq!(value["input"]["placementIntent"], json!("timeline"));
    assert_eq!(
        value["input"]["prompt"],
        json!("floating product shot with soft backlight")
    );
    assert_eq!(
        value["input"]["model"],
        json!({
            "provider": "fal.ai",
            "id": "fal-ai/wan-25-preview/text-to-video"
        })
    );
    assert_eq!(
        value["input"]["references"],
        json!({
            "mediaIds": ["media-4"],
            "firstFrameMediaId": "media-1",
            "lastFrameMediaId": "media-3"
        })
    );
    assert_eq!(
        value["input"]["settings"],
        json!({
            "width": 1080,
            "height": 1920,
            "durationSeconds": 8,
            "fps": 24,
            "aspectRatio": "9:16"
        })
    );
    assert!(value["input"]
        .get(concat!("providerCredential", "EnvVar"))
        .is_none());
    assert!(value["input"].get("providerCredential").is_none());
    assert!(value["input"].get("providerApiKey").is_none());
}

#[test]
fn temporal_generate_media_workflow_input_parses_replayable_start_request() {
    let request = temporal_generate_media_start_request(
        " Project A ",
        " /tmp/video-creater/Project A ",
        " generated-shot-1 ",
        " generated-shot-job-1 ",
        false,
        Some(TemporalGenerateMediaBrief {
            name: Some("Hero product reveal".to_string()),
            target_folder_id: None,
            placement_intent: Some("replace:item-1".to_string()),
            prompt: Some("floating product shot with soft backlight".to_string()),
            model: Some(json!({
                "provider": "fal.ai",
                "id": "fal-ai/wan-25-preview/text-to-video"
            })),
            references: None,
            settings: None,
        }),
    );

    let input = temporal_generate_media_workflow_input(&request).expect("parse workflow input");

    assert_eq!(input.project_id, "Project A");
    assert_eq!(input.project_dir, "/tmp/video-creater/Project A");
    assert_eq!(input.asset_id, "generated-shot-1");
    assert_eq!(input.job_id, "generated-shot-job-1");
    assert!(!input.mock_mode);
    assert_eq!(input.name.as_deref(), Some("Hero product reveal"));
    assert_eq!(input.placement_intent.as_deref(), Some("replace:item-1"));
    assert_eq!(
        input.prompt.as_deref(),
        Some("floating product shot with soft backlight")
    );
    assert_eq!(
        input.model,
        Some(json!({
            "provider": "fal.ai",
            "id": "fal-ai/wan-25-preview/text-to-video"
        }))
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_secret_material() {
    let mut request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    request.input[concat!("providerCredential", "EnvVar")] =
        json!("literal-secret-value:should-not-persist");

    let error =
        temporal_generate_media_workflow_input(&request).expect_err("secret material should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::ForbiddenCredentialField(
            concat!("providerCredential", "EnvVar").to_string()
        )
    );
    assert!(!error.to_string().contains("should-not-persist"));
}

#[test]
fn temporal_generate_media_workflow_input_rejects_blank_required_fields() {
    let request = temporal_generate_media_start_request(
        "Project A",
        "   ",
        "generated-shot-1",
        "generated-shot-job-1",
        true,
        None,
    );

    let error = temporal_generate_media_workflow_input(&request)
        .expect_err("blank project dir should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::BlankField("projectDir".to_string())
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_non_generate_media_start_request() {
    let generate_request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    let mut export_request = temporal_export_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-job-1",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        "exports/project-a.mp4",
    );
    export_request.input = generate_request.input;

    let error = temporal_generate_media_workflow_input(&export_request)
        .expect_err("wrong workflow type should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("workflowType".to_string())
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_mismatched_workflow_id() {
    let mut request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    request.workflow_id = "video-creater/project-a/render-draft/generated-shot-job-1".to_string();

    let error = temporal_generate_media_workflow_input(&request)
        .expect_err("wrong workflow id should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("workflowId".to_string())
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_mismatched_search_project_id() {
    let mut request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    request.search_attributes["projectId"] = json!("Other Project");

    let error = temporal_generate_media_workflow_input(&request)
        .expect_err("mismatched search project id should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("searchAttributes.projectId".to_string())
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_mismatched_search_job_id() {
    let mut request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    request.search_attributes["jobId"] = json!("other-job");

    let error = temporal_generate_media_workflow_input(&request)
        .expect_err("mismatched search job id should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("searchAttributes.jobId".to_string())
    );
}

#[test]
fn temporal_generate_media_workflow_input_rejects_mismatched_search_workflow_kind() {
    let mut request = temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-job-1",
        false,
        None,
    );
    request.search_attributes["workflowKind"] = json!("render_draft");

    let error = temporal_generate_media_workflow_input(&request)
        .expect_err("mismatched search workflow kind should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField(
            "searchAttributes.workflowKind".to_string()
        )
    );
}

#[test]
fn temporal_generate_media_fal_queue_submission_uses_stable_provider_id() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Live".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    );
    let submission = temporal_generate_media_fal_queue_submission(&project, &start_request)
        .expect("fal queue submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        "https://queue.fal.run/fal-ai/wan-25-preview/text-to-video"
    );
    assert_eq!(submission.endpoint, "fal-ai/wan-25-preview/text-to-video");
    assert_eq!(submission.provider, "fal.ai");
    assert_eq!(
        submission.input["prompt"],
        json!("floating product shot with crisp rim light")
    );
    assert_eq!(submission.input["aspect_ratio"], json!("16:9"));
    assert!(serde_json::to_value(&submission)
        .expect("serialize submission")
        .get("authEnvVar")
        .is_none());
    assert!(submission.input.get("providerCredential").is_none());
    assert!(submission.input.get("providerApiKey").is_none());
}

#[test]
fn temporal_generate_media_fal_queue_submission_rejects_mock_mode() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Live".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        true,
        None,
    );

    let error = temporal_generate_media_fal_queue_submission(&project, &start_request)
        .expect_err("mock mode should not build live fal submission");

    assert_eq!(error, TemporalWorkflowInputError::LiveModeRequired);
}

#[test]
fn temporal_generate_media_fal_run_submission_downloads_result_and_builds_actions() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Live".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    let mut asset = generated_asset("generated-shot-1");
    asset.status = GeneratedAssetStatus::Running;
    project.generated_assets.push(asset);
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        "generated-shot-1",
        JobStatus::Running,
        "2026-06-23T12:00:00Z",
    ));
    save_split_project(project_dir.path(), &project).expect("save split project");
    let start_request = temporal_generate_media_start_request(
        "project-1",
        project_dir.path().to_str().expect("project dir utf8"),
        "generated-shot-1",
        "generated-shot-1",
        false,
        Some(TemporalGenerateMediaBrief {
            name: None,
            target_folder_id: None,
            placement_intent: Some("replace:item-1".to_string()),
            prompt: None,
            model: None,
            references: None,
            settings: None,
        }),
    );
    let mut submission = temporal_generate_media_fal_queue_submission(&project, &start_request)
        .expect("fal queue submission");
    let Some((base_url, request_handle)) = spawn_temporal_fal_run_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/wan-25-preview/text-to-video");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test client");
    let run = temporal_generate_media_fal_run_submission_with_client(
        &client,
        project_dir.path(),
        &project,
        &start_request,
        &submission,
        FalWorkflowExecution {
            credential: "unit-test-token",
            updated_at: "2026-06-23T12:05:00Z",
            run_id: Some("fal-run-123"),
            options: FalGenerationRunOptions {
                max_status_polls: 2,
                poll_interval: Duration::from_millis(0),
            },
        },
    )
    .expect("run fal generation");
    let requests = request_handle.join().expect("request capture");

    assert_eq!(run.request_id, "req-123");
    assert_eq!(run.status.status, FalQueueStatusKind::Completed);
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-shot-1/fal-output.mp4")
    );
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("downloaded output"),
        b"result-video-bytes"
    );
    assert!(matches!(
        &run.completion.actions[0],
        ProjectAction::CompleteGeneratedAsset {
            asset_id,
            replacement: Some(replacement),
            ..
        } if asset_id == "generated-shot-1"
            && replacement.item_id == "item-1"
            && replacement.media_id == "generated-shot-1-fal-output"
    ));
    assert!(matches!(
        &run.completion.actions[1],
        ProjectAction::UpdateJobStatus {
            job_id,
            status,
            run_id,
            ..
        } if job_id == "generated-shot-1"
            && status == &JobStatus::Completed
            && run_id.as_deref() == Some("fal-run-123")
    ));
    assert!(requests[0].starts_with("POST /fal-ai/wan-25-preview/text-to-video HTTP/1.1"));
    assert!(requests[1].starts_with(
        "GET /fal-ai/wan-25-preview/text-to-video/requests/req-123/status?logs=1 HTTP/1.1"
    ));
    assert!(requests[2].starts_with(
        "GET /fal-ai/wan-25-preview/text-to-video/requests/req-123/response HTTP/1.1"
    ));
    assert!(requests[3].starts_with("GET /generated/hero.mp4 HTTP/1.1"));
    assert!(requests[..3]
        .iter()
        .all(|request| request.contains("authorization: Key unit-test-token")));
    assert!(requests
        .iter()
        .all(|request| !request_body(request).contains("unit-test-token")));
}

#[test]
fn temporal_export_media_start_request_serializes_policy_checked_payload_without_secret() {
    let options = video_creater_lib::project::export_options::ExportRenderOptions::new(
        ExportProfile::Mp4H264,
        RenderQuality::Draft,
        1280,
        720,
    )
    .expect("explicit export options");
    let request = temporal_export_media_start_request_with_options(
        "Project A",
        "/tmp/video-creater/Project A",
        "mp4-export-1",
        options,
        "exports/project-a-h264.mp4",
    );

    let value = serde_json::to_value(request).expect("serialize export start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/export-media/mp4-export-1",
            "workflowType": "VideoCreaterExportMediaWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectDir": "/tmp/video-creater/Project A",
                "jobId": "mp4-export-1",
                "profile": "mp4H264",
                "quality": "draft",
                "width": 1280,
                "height": 720,
                "outputPath": "exports/project-a-h264.mp4",
                "validation": {
                    "container": "mp4",
                    "extension": "mp4",
                    "mimeType": "video/mp4",
                    "videoCodec": "h264",
                    "audioCodec": "aac",
                    "requireVideoStream": true,
                    "requireAudioStreamWhenTimelineHasAudio": true
                }
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "mp4-export-1",
                "workflowKind": "export_media"
            },
            "activityTypes": [
                "BuildRenderPlan",
                "ValidateExportProfile",
                "RenderMedia",
                "ValidateRenderedMedia",
                "AttachRenderReport"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let serialized = value.to_string().to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
}

#[test]
fn temporal_export_media_start_request_serializes_palmier_package_payload_without_secret() {
    let request = temporal_export_project_bundle_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "palmier-export-1",
        "exports/project-a.palmier",
        true,
    );

    let value = serde_json::to_value(request).expect("serialize Palmier package start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/export-media/palmier-export-1",
            "workflowType": "VideoCreaterExportMediaWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectDir": "/tmp/video-creater/Project A",
                "jobId": "palmier-export-1",
                "profile": "palmierProject",
                "outputPath": "exports/project-a.palmier",
                "validation": {
                    "container": "palmier",
                    "extension": "palmier",
                    "mimeType": "application/vnd.video-creater.project",
                    "videoCodec": null,
                    "audioCodec": null,
                    "requireVideoStream": false,
                    "requireAudioStreamWhenTimelineHasAudio": false,
                    "packageKind": "splitProjectBundle"
                }
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "palmier-export-1",
                "workflowKind": "export_media"
            },
            "activityTypes": [
                "WriteExportArtifact",
                "AttachExportReport"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let serialized = value.to_string().to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
}

#[test]
fn temporal_export_nle_xml_start_request_serializes_format_payload_without_secret() {
    let request = temporal_export_nle_xml_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/project-a-premiere.xml",
    );

    let value = serde_json::to_value(request).expect("serialize nle export start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/export-nle-xml/nle-export-1",
            "workflowType": "VideoCreaterExportNleXmlWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectDir": "/tmp/video-creater/Project A",
                "jobId": "nle-export-1",
                "format": "premiereXmeml",
                "outputPath": "exports/project-a-premiere.xml"
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "nle-export-1",
                "workflowKind": "export_nle_xml"
            },
            "activityTypes": [
                "BuildNleXml",
                "ValidateNleXml",
                "WriteExportArtifact",
                "AttachExportReport"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let serialized = value.to_string().to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
}

#[test]
fn temporal_export_nle_xml_build_activity_loads_split_project_timeline() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let start_request = temporal_export_nle_xml_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/project-test-premiere.xml",
    );

    let output = temporal_export_nle_xml_build_activity_value(json!({
        "startRequest": start_request,
    }))
    .expect("build nle xml activity");

    assert_eq!(output["projectId"], "project-test");
    assert_eq!(output["jobId"], "nle-export-1");
    assert_eq!(output["format"], "premiereXmeml");
    assert_eq!(output["filename"], "project-test-premiere.xml");
    assert_eq!(output["outputPath"], "exports/project-test-premiere.xml");
    assert_eq!(output["mimeType"], "application/xml");
    assert!(output["xml"]
        .as_str()
        .expect("xml string")
        .contains("<xmeml version=\"5\">"));
    assert!(!serde_json::to_string(&output)
        .expect("serialize output")
        .to_lowercase()
        .contains("secret"));
}

#[test]
fn temporal_export_nle_xml_validate_activity_accepts_built_export_payload() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let start_request = temporal_export_nle_xml_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/project-test-premiere.xml",
    );
    let build_output = temporal_export_nle_xml_build_activity_value(json!({
        "startRequest": start_request,
    }))
    .expect("build nle xml activity");

    let validation = temporal_export_nle_xml_validate_activity_value(build_output)
        .expect("validate nle xml activity");

    assert_eq!(validation["status"], "validated");
    assert_eq!(validation["projectId"], "project-test");
    assert_eq!(validation["jobId"], "nle-export-1");
    assert_eq!(validation["format"], "premiereXmeml");
    assert_eq!(
        validation["outputPath"],
        "exports/project-test-premiere.xml"
    );
}

#[test]
fn temporal_export_nle_xml_write_and_attach_record_project_artifact() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let start_request = temporal_export_nle_xml_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/project-test-premiere.xml",
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportNleXml,
        &project.id,
        "nle-export-1",
        JobStatus::Running,
        "2026-06-26T12:00:00Z",
    );
    job.start_request = Some(start_request.clone());
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");
    let build_output = temporal_export_nle_xml_build_activity_value(json!({
        "startRequest": start_request,
    }))
    .expect("build nle xml activity");

    let write_output = temporal_export_nle_xml_write_artifact_activity_value(build_output)
        .expect("write nle xml artifact");
    let attach_output = temporal_export_nle_xml_attach_export_report_activity_value(json!({
        "writeOutput": write_output,
        "createdAt": "2026-06-26T12:01:00Z",
        "runId": "temporal-run-1",
    }))
    .expect("attach nle xml export report");
    let export_path = project_dir.path().join("exports/project-test-premiere.xml");
    let reloaded = load_split_project(project_dir.path()).expect("reload split project");
    let artifact = reloaded
        .export_artifacts
        .iter()
        .find(|artifact| artifact.id == "nle-export-1")
        .expect("recorded export artifact");
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "nle-export-1")
        .expect("completed nle export job");

    assert!(export_path.exists());
    assert!(std::fs::read_to_string(export_path)
        .expect("export xml")
        .contains("<xmeml version=\"5\">"));
    assert_eq!(artifact.format, "premiereXmeml");
    assert_eq!(artifact.path, "exports/project-test-premiere.xml");
    assert_eq!(artifact.job_id.as_deref(), Some("nle-export-1"));
    assert_eq!(artifact.created_at, "2026-06-26T12:01:00Z");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-1")
    );
    assert_eq!(attach_output["status"], "attached");
    assert_eq!(
        attach_output["artifactPath"],
        "exports/project-test-premiere.xml"
    );
    assert!(attach_output["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("written file")
            .ends_with("exports/nle-export-1/artifact.json")));
}

#[test]
fn temporal_export_nle_xml_write_respects_overwrite_false() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let existing_path = project_dir.path().join("exports/project-test-premiere.xml");
    fs::create_dir_all(existing_path.parent().expect("exports dir")).expect("create exports dir");
    fs::write(&existing_path, b"existing xml").expect("write existing export");
    let start_request = temporal_export_nle_xml_start_request_with_overwrite(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/project-test-premiere.xml",
        false,
    );
    let build_output = temporal_export_nle_xml_build_activity_value(json!({
        "startRequest": start_request,
    }))
    .expect("build nle xml activity");

    let error = temporal_export_nle_xml_write_artifact_activity_value(build_output)
        .expect_err("overwrite=false should reject an existing NLE export");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("overwrite".to_string())
    );
    assert_eq!(
        fs::read(&existing_path).expect("existing export preserved"),
        b"existing xml"
    );
}

#[test]
fn temporal_export_project_bundle_write_activity_copies_package_and_records_artifact() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media_folders = vec![
        MediaFolder {
            id: "folder-root".to_string(),
            name: "Root folder".to_string(),
            parent_id: None,
        },
        MediaFolder {
            id: "folder-child".to_string(),
            name: "Child folder".to_string(),
            parent_id: Some("folder-root".to_string()),
        },
    ];
    let media = project
        .media
        .iter_mut()
        .find(|asset| asset.id == "media-1")
        .expect("media-1 fixture asset");
    media.duration_seconds = 4.0;
    media.folder_id = Some("folder-child".to_string());
    let mut generated = generated_asset("generated-log-1");
    generated.status = GeneratedAssetStatus::Completed;
    generated.settings.resolution = Some("720p".to_string());
    generated.outputs = vec![GeneratedAssetOutput {
        media_id: "generated-output-1".to_string(),
        relative_path: "generated/generated-log-1/mock-output.mp4".to_string(),
        source_url: Some("https://provider.example/generated-output.mp4".to_string()),
        width: 1280,
        height: 720,
        duration_seconds: 4.0,
        fps: 24.0,
    }];
    project.generated_assets.push(generated);
    project.media.push(MediaAsset {
        id: "generated-output-1".to_string(),
        name: Some("Generated product push".to_string()),
        relative_path: "generated/generated-log-1/mock-output.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 4.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: Some("folder-child".to_string()),
    });
    let start_request = temporal_export_media_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "palmier-export-1",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        "exports/ignored.mp4",
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "palmier-export-1",
        JobStatus::Running,
        "2026-07-02T12:00:00Z",
    );
    job.start_request = Some(start_request.clone());
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");
    fs::create_dir_all(project_dir.path().join("media")).expect("create media dir");
    fs::write(project_dir.path().join("media/input.mp4"), b"video bytes")
        .expect("write source media");

    let output = temporal_export_project_bundle_write_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "palmier-export-1",
        "profile": "palmierProject",
        "outputPath": "exports/test-project.palmier",
        "createdAt": "2026-07-02T12:01:00Z",
        "runId": "temporal-run-palmier-1"
    }))
    .expect("write Palmier project package");

    let package_dir = project_dir.path().join("exports/test-project.palmier");
    let bundled = load_split_project(&package_dir).expect("load exported project package");
    let reloaded = load_split_project(project_dir.path()).expect("reload source project");
    let artifact = reloaded
        .export_artifacts
        .iter()
        .find(|artifact| artifact.id == "palmier-export-1")
        .expect("recorded project bundle artifact");
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "palmier-export-1")
        .expect("completed project bundle job");

    assert_eq!(output["status"], json!("exported"));
    assert_eq!(
        output["artifactPath"],
        json!("exports/test-project.palmier")
    );
    assert_eq!(bundled.id, "project-test");
    assert!(package_dir.join("project.json").exists());
    assert!(package_dir.join("media.json").exists());
    assert!(package_dir.join("generation-log.json").exists());
    let palmier_timeline: Value =
        serde_json::from_slice(&fs::read(package_dir.join("project.json")).expect("timeline json"))
            .expect("parse palmier timeline");
    let palmier_media: Value =
        serde_json::from_slice(&fs::read(package_dir.join("media.json")).expect("media json"))
            .expect("parse palmier media manifest");
    let palmier_generation_log: Value = serde_json::from_slice(
        &fs::read(package_dir.join("generation-log.json")).expect("generation log json"),
    )
    .expect("parse palmier generation log");
    assert_eq!(palmier_timeline["durationSeconds"], json!(4.0));
    assert_eq!(palmier_media["version"], json!(2));
    let palmier_entry = palmier_media["entries"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["id"].as_str() == Some("media-1"))
        })
        .expect("media-1 Palmier manifest entry");
    assert_eq!(palmier_entry["type"], json!("video"));
    assert_eq!(palmier_entry["duration"], json!(4.0));
    assert_eq!(palmier_entry["folderId"], json!("folder-child"));
    assert_eq!(
        palmier_media["folders"],
        json!([
            { "id": "folder-root", "name": "Root folder" },
            { "id": "folder-child", "name": "Child folder", "parentFolderId": "folder-root" }
        ])
    );
    assert_eq!(
        palmier_entry["source"],
        json!({ "project": { "relativePath": "media/input.mp4" } })
    );
    let generated_entry = palmier_media["entries"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["id"].as_str() == Some("generated-output-1"))
        })
        .expect("generated output Palmier manifest entry");
    assert_eq!(
        generated_entry["generationInput"],
        json!({
            "prompt": "floating product shot with crisp rim light",
            "model": "fal-ai/wan-25-preview/text-to-video",
            "duration": 4,
            "aspectRatio": "16:9",
            "resolution": "720p",
            "createdAt": "2026-06-23T12:00:00Z",
            "outputIndex": 0,
            "resultURLs": ["https://provider.example/generated-output.mp4"]
        })
    );
    assert!(palmier_generation_log.get("generatedAssets").is_none());
    assert_eq!(
        palmier_generation_log,
        json!({
            "version": 1,
            "entries": [
                {
                    "id": "generated-log-1",
                    "model": "fal-ai/wan-25-preview/text-to-video",
                    "createdAt": "2026-06-23T12:00:00Z"
                }
            ]
        })
    );
    assert_eq!(
        fs::read(package_dir.join("media/input.mp4")).expect("bundled media bytes"),
        b"video bytes"
    );
    assert_eq!(artifact.kind, ProjectExportArtifactKind::ProjectBundle);
    assert_eq!(artifact.format, "palmierProject");
    assert_eq!(artifact.path, "exports/test-project.palmier");
    assert_eq!(artifact.mime_type, "application/vnd.video-creater.project");
    assert_eq!(artifact.created_at, "2026-07-02T12:01:00Z");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-palmier-1")
    );
}

#[test]
fn temporal_export_project_bundle_write_respects_overwrite_false() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "palmier-export-no-overwrite",
        JobStatus::Running,
        "2026-07-02T12:00:00Z",
    );
    job.start_request = Some(temporal_export_media_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "palmier-export-no-overwrite",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        "exports/ignored.mp4",
    ));
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");
    let existing_path = project_dir.path().join("exports/existing.palmier");
    fs::create_dir_all(&existing_path).expect("create existing bundle dir");
    fs::write(existing_path.join("sentinel.txt"), b"existing bundle")
        .expect("write existing bundle sentinel");

    let error = temporal_export_project_bundle_write_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "palmier-export-no-overwrite",
        "profile": "palmierProject",
        "outputPath": "exports/existing.palmier",
        "overwrite": false,
        "createdAt": "2026-07-02T12:01:00Z"
    }))
    .expect_err("overwrite=false should reject an existing project bundle");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("overwrite".to_string())
    );
    assert_eq!(
        fs::read(existing_path.join("sentinel.txt")).expect("existing bundle preserved"),
        b"existing bundle"
    );
}

#[test]
fn temporal_export_project_bundle_rejects_paths_outside_exports_without_writing() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "palmier-export-unsafe",
        JobStatus::Running,
        "2026-07-02T12:00:00Z",
    );
    job.start_request = Some(temporal_export_media_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "palmier-export-unsafe",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        "exports/ignored.mp4",
    ));
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");

    let error = temporal_export_project_bundle_write_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "palmier-export-unsafe",
        "profile": "palmierProject",
        "outputPath": "unsafe.palmier",
        "createdAt": "2026-07-02T12:01:00Z"
    }))
    .expect_err("project bundle path outside exports should fail before writing");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("outputPath".to_string())
    );
    assert!(!project_dir.path().join("unsafe.palmier").exists());
}

#[test]
fn temporal_export_nle_xml_build_activity_rejects_output_path_mismatch() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let start_request = temporal_export_nle_xml_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "nle-export-1",
        NleXmlFormat::PremiereXmeml,
        "exports/wrong.xml",
    );

    let error = temporal_export_nle_xml_build_activity_value(json!({
        "startRequest": start_request,
    }))
    .expect_err("mismatched output path should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("outputPath".to_string())
    );
}

#[test]
fn temporal_render_build_plan_activity_loads_split_project_timeline() {
    let project_dir = tempfile::tempdir().expect("temp split project");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(5.0));
    save_split_project(project_dir.path(), &project).expect("save split project");

    let output = temporal_render_build_plan_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "render-draft-1",
        "profile": "draftWebm"
    }))
    .expect("build render plan activity");

    assert_eq!(output["status"], json!("built"));
    assert_eq!(output["projectId"], json!("project-test"));
    assert_eq!(output["jobId"], json!("render-draft-1"));
    assert_eq!(output["profile"], json!("draftWebm"));
    assert_eq!(output["renderPlan"]["clips"][0]["sourceIn"], json!(1.0));
    assert_eq!(output["renderPlan"]["clips"][0]["sourceOut"], json!(5.0));
    assert!(output["renderPlan"]["outputPath"]
        .as_str()
        .expect("output path")
        .ends_with("renders/render-draft-1/output.webm"));
}

#[test]
fn temporal_render_draft_start_request_serializes_webm_profile_payload_without_secret() {
    let request = temporal_render_draft_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "render-draft-1",
        RenderQualityProfile::DraftWebm,
    );

    let value = serde_json::to_value(request).expect("serialize render draft start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/render-draft/render-draft-1",
            "workflowType": "VideoCreaterRenderDraftWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectDir": "/tmp/video-creater/Project A",
                "jobId": "render-draft-1",
                "profile": "draftWebm"
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "render-draft-1",
                "workflowKind": "render_draft"
            },
            "activityTypes": [
                "BuildRenderPlan",
                "RenderMedia",
                "ValidateRenderedMedia",
                "AttachRenderReport"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let serialized = value.to_string().to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("mp4h264"));
}

#[test]
fn temporal_render_workflow_activity_plan_sequences_real_render_chain() {
    let start_request = temporal_render_draft_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "render-draft-1",
        RenderQualityProfile::DraftWebm,
    );

    let plan = temporal_render_workflow_activity_plan(
        TemporalWorkflowKind::RenderDraft,
        start_request.input.clone(),
        "2026-06-27T12:00:00Z",
        Some("temporal-run-render-1"),
    )
    .expect("render workflow activity plan");

    assert_eq!(plan.workflow_type, "VideoCreaterRenderDraftWorkflow");
    assert_eq!(plan.status, "planned");
    assert_eq!(
        plan.activity_types,
        vec![
            "BuildRenderPlan".to_string(),
            "RenderMedia".to_string(),
            "ValidateRenderedMedia".to_string(),
            "AttachRenderReport".to_string(),
        ]
    );
    assert_eq!(plan.build_render_plan_input["profile"], json!("draftWebm"));
    assert_eq!(
        plan.build_render_plan_input["projectId"],
        json!("Project A")
    );
    assert_eq!(
        plan.build_render_plan_input["jobId"],
        json!("render-draft-1")
    );
    assert_eq!(plan.render_media_input_from, "BuildRenderPlan");
    assert_eq!(plan.validate_rendered_media_input_from, "RenderMedia");
    assert_eq!(
        plan.attach_render_report_input_from,
        "ValidateRenderedMedia"
    );
    assert_eq!(plan.updated_at, "2026-06-27T12:00:00Z");
    assert_eq!(plan.run_id.as_deref(), Some("temporal-run-render-1"));
}

#[test]
fn temporal_export_media_workflow_activity_plan_sequences_palmier_package_export() {
    let input = json!({
        "projectId": "Project A",
        "projectDir": "/tmp/video-creater/Project A",
        "jobId": "palmier-export-1",
        "profile": "palmierProject",
        "outputPath": "exports/project-a.palmier",
        "validation": {
            "container": "palmier",
            "extension": "palmier",
            "mimeType": "application/vnd.video-creater.project",
            "videoCodec": null,
            "audioCodec": null,
            "requireVideoStream": false,
            "requireAudioStreamWhenTimelineHasAudio": false,
            "packageKind": "splitProjectBundle"
        }
    });

    let plan = temporal_export_media_workflow_activity_plan_value(
        input,
        "2026-07-02T12:00:00Z",
        Some("temporal-run-palmier-1"),
    )
    .expect("Palmier package export workflow activity plan");

    assert_eq!(plan["status"], json!("planned"));
    assert_eq!(
        plan["workflowType"],
        json!("VideoCreaterExportMediaWorkflow")
    );
    assert_eq!(
        plan["activityTypes"],
        json!(["WriteExportArtifact", "AttachExportReport"])
    );
    assert_eq!(
        plan["writeExportArtifactInput"],
        json!({
            "projectId": "Project A",
            "projectDir": "/tmp/video-creater/Project A",
            "jobId": "palmier-export-1",
            "profile": "palmierProject",
            "outputPath": "exports/project-a.palmier",
            "overwrite": true,
            "createdAt": "2026-07-02T12:00:00Z",
            "runId": "temporal-run-palmier-1"
        })
    );
    assert_eq!(
        plan["attachExportReportInputFrom"],
        json!("WriteExportArtifact")
    );
}

#[test]
fn temporal_export_media_codec_profile_uses_native_gstreamer_render() {
    // Planning a codec export checks profile availability against the curated GStreamer runtime,
    // which the app starts during launch before any workflow consumer runs.
    start_render_process_runtime().expect("initialize curated render runtime");
    let start_request = temporal_export_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "mp4-export-1",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1920, 1080)
            .expect("explicit export options"),
        "exports/project-a.mp4",
    );
    assert_eq!(start_request.input["profile"], json!("mp4H264"));
    assert_eq!(start_request.input["quality"], json!("final"));
    assert_eq!(start_request.input["width"], json!(1920));
    assert_eq!(start_request.input["height"], json!(1080));
    assert!(start_request.input.get("exportProfile").is_none());
    assert_ne!(start_request.input["profile"], json!("finalWebm"));

    let plan = temporal_export_media_workflow_activity_plan_value(
        start_request.input,
        "2026-07-02T12:00:00Z",
        Some("temporal-run-media-1"),
    )
    .expect("MP4 export workflow activity plan");

    assert_eq!(plan["status"], json!("planned"));
    assert_eq!(
        plan["workflowType"],
        json!("VideoCreaterExportMediaWorkflow")
    );
    assert_eq!(plan["profile"], json!("mp4H264"));
    assert_eq!(
        plan["activityTypes"],
        json!([
            "BuildRenderPlan",
            "ValidateExportProfile",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
            "WriteExportArtifact",
            "AttachExportReport"
        ])
    );
    assert_eq!(
        plan["buildRenderPlanInput"],
        json!({
            "projectId": "Project A",
            "projectDir": "/tmp/video-creater/Project A",
            "jobId": "mp4-export-1",
            "profile": "mp4H264",
            "quality": "final",
            "width": 1920,
            "height": 1080
        })
    );
    assert!(plan["buildRenderPlanInput"].get("exportProfile").is_none());
    assert_eq!(
        plan["validateExportProfileInput"]["profile"],
        json!("mp4H264")
    );
    assert_eq!(
        plan["renderMediaInput"],
        json!({
            "projectId": "Project A",
            "projectDir": "/tmp/video-creater/Project A",
            "jobId": "mp4-export-1",
            "profile": "mp4H264",
            "quality": "final",
            "width": 1920,
            "height": 1080,
            "updatedAt": "2026-07-02T12:00:00Z",
            "runId": "temporal-run-media-1"
        })
    );
    assert!(plan["renderMediaInput"].get("exportProfile").is_none());
    assert_ne!(plan["renderMediaInput"]["profile"], json!("finalWebm"));
    assert_eq!(
        plan["validateRenderedMediaInput"],
        json!({
            "profile": "mp4H264",
            "quality": "final",
            "width": 1920,
            "height": 1080,
            "outputPath": "exports/project-a.mp4",
            "validation": {
                "container": "mp4",
                "extension": "mp4",
                "mimeType": "video/mp4",
                "videoCodec": "h264",
                "audioCodec": "aac",
                "requireVideoStream": true,
                "requireAudioStreamWhenTimelineHasAudio": true
            }
        })
    );
    assert_eq!(
        plan["writeExportArtifactBaseInput"],
        json!({
            "projectId": "Project A",
            "projectDir": "/tmp/video-creater/Project A",
            "jobId": "mp4-export-1",
            "profile": "mp4H264",
            "quality": "final",
            "width": 1920,
            "height": 1080,
            "outputPath": "exports/project-a.mp4",
            "overwrite": true,
            "createdAt": "2026-07-02T12:00:00Z",
            "runId": "temporal-run-media-1",
            "validation": {
                "container": "mp4",
                "extension": "mp4",
                "mimeType": "video/mp4",
                "videoCodec": "h264",
                "audioCodec": "aac",
                "requireVideoStream": true,
                "requireAudioStreamWhenTimelineHasAudio": true
            }
        })
    );
    assert!(plan.get("writeExportArtifactInputFrom").is_none());
}

#[test]
fn temporal_validate_rendered_media_rejects_output_that_does_not_match_export_profile() {
    let input = json!({
        "profile": "mp4H264",
        "outputPath": "exports/project-a.mp4",
        "validation": {
            "container": "mp4",
            "extension": "mp4",
            "mimeType": "video/mp4",
            "videoCodec": "h264",
            "audioCodec": "aac",
            "requireVideoStream": true,
            "requireAudioStreamWhenTimelineHasAudio": true
        },
        "renderReport": {
            "summary": {
                "outputPath": "renders/mp4-export-1/output.webm"
            }
        },
        "projectRenderReport": {
            "id": "mp4-export-1"
        }
    });

    let error = temporal_validate_rendered_media_activity_value(input)
        .expect_err("MP4 profile must not validate a WebM render output");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("renderReport.summary.outputPath".into())
    );
}

#[test]
fn temporal_export_media_write_artifact_requires_rendered_output_after_native_routing() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(temporal_job_summary(
        video_creater_lib::workflows::TemporalWorkflowKind::ExportMedia,
        &project.id,
        "export-mp4-legacy-write",
        JobStatus::Queued,
        "2026-07-02T12:00:00Z",
    ));
    save_split_project(project_dir.path(), &project).expect("save split project");

    let error = temporal_export_media_write_artifact_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path().display().to_string(),
        "jobId": "export-mp4-legacy-write",
        "profile": "mp4H264",
        "outputPath": "exports/final.mp4",
        "createdAt": "2026-07-02T12:00:00Z",
        "validation": {
            "container": "mp4",
            "extension": "mp4",
            "mimeType": "video/mp4",
            "videoCodec": "h264",
            "audioCodec": "aac",
            "requireVideoStream": true,
            "requireAudioStreamWhenTimelineHasAudio": true
        },
        "validationOutput": {
            "status": "validated",
            "renderReport": {
                "summary": {
                    "outputPath": "renders/export-mp4-no-overwrite/output.webm"
                }
            },
            "projectRenderReport": {
                "id": "export-mp4-no-overwrite"
            }
        }
    }))
    .expect_err("native artifact writing requires the validated rendered output");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField(
            "validationOutput.renderReport.summary.outputPath".to_string()
        )
    );
}

#[test]
fn temporal_export_media_writer_materializes_validated_webm_without_secondary_encoder() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "export-webm-copy",
        JobStatus::Running,
        "2026-07-14T12:00:00Z",
    ));
    save_split_project(project_dir.path(), &project).expect("save split project");
    let source_relative = "renders/export-webm-copy/output.webm";
    let source = project_dir.path().join(source_relative);
    fs::create_dir_all(source.parent().expect("render dir")).expect("create render dir");
    fs::write(&source, b"validated webm bytes").expect("write validated render");
    let validation = temporal_export_media_start_request(
        &project.id,
        project_dir.path().to_str().expect("project path"),
        "export-webm-copy",
        ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Final, 1280, 720)
            .expect("WebM options"),
        "exports/final.webm",
    )
    .input["validation"]
        .clone();

    let write_output = temporal_export_media_write_artifact_activity_value(json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "export-webm-copy",
        "profile": "webm",
        "quality": "final",
        "width": 1280,
        "height": 720,
        "outputPath": "exports/final.webm",
        "overwrite": true,
        "createdAt": "2026-07-14T12:01:00Z",
        "runId": "export-webm-copy-run",
        "validation": validation,
        "validationOutput": {
            "status": "validated",
            "renderReport": { "summary": { "outputPath": source_relative } },
            "projectRenderReport": { "id": "export-webm-copy" }
        }
    }))
    .expect("materialize validated WebM artifact");
    assert!(write_output.get("command").is_none());
    assert_eq!(write_output["materialization"]["kind"], "atomicCopy");
    assert_eq!(write_output["materialization"]["byteIdentical"], true);
    let attach_output = temporal_export_media_attach_export_report_activity_value(json!({
        "writeOutput": write_output,
        "createdAt": "2026-07-14T12:01:00Z",
        "runId": "export-webm-copy-run"
    }))
    .expect("attach WebM export report");

    assert_eq!(
        fs::read(&source).expect("source preserved"),
        b"validated webm bytes"
    );
    assert_eq!(
        fs::read(project_dir.path().join("exports/final.webm")).expect("final artifact"),
        b"validated webm bytes"
    );
    assert!(attach_output["artifactPath"] == "exports/final.webm");
    assert!(attach_output["writtenPath"]
        .as_str()
        .expect("written path")
        .ends_with("exports/final.webm"));
    let reloaded = load_split_project(project_dir.path()).expect("reload project");
    let artifact = reloaded
        .export_artifacts
        .iter()
        .find(|artifact| artifact.id == "export-webm-copy")
        .expect("recorded WebM artifact");
    assert_eq!(artifact.kind, ProjectExportArtifactKind::Webm);
    assert_eq!(artifact.path, "exports/final.webm");
}

#[test]
fn temporal_export_media_writer_respects_overwrite_for_validated_h264_copy() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "export-h264-copy",
        JobStatus::Running,
        "2026-07-14T12:00:00Z",
    ));
    save_split_project(project_dir.path(), &project).expect("save split project");
    let source_relative = "renders/export-h264-copy/output.mp4";
    let source = project_dir.path().join(source_relative);
    fs::create_dir_all(source.parent().expect("render dir")).expect("create render dir");
    fs::write(&source, b"validated h264 bytes").expect("write validated render");
    let destination = project_dir.path().join("exports/final.mp4");
    fs::create_dir_all(destination.parent().expect("exports dir")).expect("create exports dir");
    fs::write(&destination, b"existing artifact").expect("write existing artifact");
    let input = json!({
        "projectId": project.id,
        "projectDir": project_dir.path(),
        "jobId": "export-h264-copy",
        "profile": "mp4H264",
        "quality": "final",
        "width": 1920,
        "height": 1080,
        "outputPath": "exports/final.mp4",
        "overwrite": false,
        "createdAt": "2026-07-14T12:01:00Z",
        "validation": {
            "container": "mp4",
            "extension": "mp4",
            "mimeType": "video/mp4",
            "videoCodec": "h264",
            "audioCodec": "aac",
            "requireVideoStream": true,
            "requireAudioStreamWhenTimelineHasAudio": true
        },
        "validationOutput": {
            "status": "validated",
            "renderReport": { "summary": { "outputPath": source_relative } },
            "projectRenderReport": { "id": "export-h264-copy" }
        }
    });

    let error = temporal_export_media_write_artifact_activity_value(input.clone())
        .expect_err("overwrite=false rejects existing artifact");
    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("overwrite".to_string())
    );
    assert_eq!(
        fs::read(&destination).expect("existing artifact"),
        b"existing artifact"
    );

    let mut overwrite_input = input;
    overwrite_input["overwrite"] = json!(true);
    let write_output = temporal_export_media_write_artifact_activity_value(overwrite_input)
        .expect("replace with validated H.264 artifact");
    assert_eq!(
        fs::read(&source).expect("source preserved"),
        b"validated h264 bytes"
    );
    assert_eq!(
        fs::read(&destination).expect("replaced artifact"),
        b"validated h264 bytes"
    );
    assert_eq!(write_output["artifactPath"], "exports/final.mp4");
    assert!(write_output.get("command").is_none());
    assert_eq!(write_output["materialization"]["byteIdentical"], true);
}

#[test]
fn temporal_render_media_activity_input_carries_workflow_runtime_context() {
    let build_output = json!({
        "status": "built",
        "projectId": "Project A",
        "projectDir": "/tmp/video-creater/Project A",
        "jobId": "render-draft-1",
        "profile": "draftWebm",
        "renderPlan": {
            "inputPath": "/tmp/video-creater/Project A/media/source.mp4",
            "outputPath": "/tmp/video-creater/Project A/renders/render-draft-1/output.webm",
            "width": 1920,
            "height": 1080,
            "fps": 24.0,
            "qualityProfile": "draftWebm",
            "clips": [{ "sourceIn": 1.0, "sourceOut": 5.0 }]
        }
    });

    let input = temporal_render_media_activity_input_value(
        build_output,
        "2026-06-27T12:00:00Z",
        Some("temporal-run-render-1"),
    )
    .expect("render media activity input");

    assert_eq!(input["updatedAt"], json!("2026-06-27T12:00:00Z"));
    assert_eq!(input["runId"], json!("temporal-run-render-1"));
    assert_eq!(input["jobId"], json!("render-draft-1"));
    assert_eq!(input["profile"], json!("draftWebm"));
    assert_eq!(input["renderPlan"]["clips"][0]["sourceIn"], json!(1.0));
}

#[test]
fn temporal_codex_edit_start_request_serializes_replayable_request_without_secret() {
    let request = temporal_codex_edit_start_request(
        "Project A",
        "/tmp/video-creater/repo",
        "/tmp/video-creater/Project A",
        "codex-edit-1",
        temporal_codex_request(),
    );

    let value = serde_json::to_value(request).expect("serialize codex edit start request");

    assert_eq!(
        value,
        json!({
            "workflowId": "video-creater/project-a/codex-edit/codex-edit-1",
            "workflowType": "VideoCreaterCodexEditWorkflow",
            "taskQueue": "video-creater-workflows",
            "input": {
                "projectId": "Project A",
                "projectRoot": "/tmp/video-creater/repo",
                "projectDir": "/tmp/video-creater/Project A",
                "jobId": "codex-edit-1",
                "request": {
                    "mediaId": "media-1",
                    "preset": "trailer_cut",
                    "prompt": "Make a sharp trailer cut with bold captions",
                    "targetDurationSeconds": 45.0,
                    "languageMode": "en",
                    "captionStyle": "bold",
                    "createdAt": "2026-06-26T12:00:00Z"
                }
            },
            "searchAttributes": {
                "projectId": "Project A",
                "jobId": "codex-edit-1",
                "workflowKind": "codex_edit"
            },
            "activityTypes": [
                "CollectProjectContext",
                "RequestCodexProposal",
                "ValidateProjectActions",
                "PersistAcceptedProposal",
                "AttachCodexEditFailure"
            ],
            "idReusePolicy": "rejectDuplicate"
        })
    );

    let input = value.get("input").expect("start request input").to_string();
    let serialized = input.to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("proposal"));
}

#[test]
fn temporal_codex_edit_proposal_activity_input_carries_request_and_proposal_for_worker_chain() {
    let start_request = temporal_codex_edit_start_request(
        "Project A",
        "/tmp/video-creater/repo",
        "/tmp/video-creater/Project A",
        "codex-edit-1",
        temporal_codex_request(),
    );
    let proposal = temporal_codex_valid_proposal();

    let input = temporal_codex_edit_proposal_activity_input_value(
        &start_request.input,
        json!({
            "status": "completed",
            "proposal": proposal,
        }),
        "2026-06-26T12:02:00Z",
        Some("temporal-run-codex-1"),
    )
    .expect("codex proposal activity input");

    assert_eq!(input["projectId"], "Project A");
    assert_eq!(input["projectRoot"], "/tmp/video-creater/repo");
    assert_eq!(input["projectDir"], "/tmp/video-creater/Project A");
    assert_eq!(input["jobId"], "codex-edit-1");
    assert_eq!(input["request"]["mediaId"], "media-1");
    assert_eq!(input["request"]["preset"], "trailer_cut");
    assert_eq!(input["proposal"]["mediaId"], "media-1");
    assert_eq!(input["proposal"]["clips"][0]["sourceIn"], 1.0);
    assert_eq!(input["proposal"]["clips"][0]["sourceOut"], 46.0);
    assert_eq!(input["updatedAt"], "2026-06-26T12:02:00Z");
    assert_eq!(input["runId"], "temporal-run-codex-1");
}

#[test]
fn temporal_codex_edit_collect_project_context_loads_split_project_context() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = temporal_codex_project();
    save_split_project(project_dir.path(), &project).expect("save split project");

    let output = temporal_codex_edit_collect_project_context_activity_value(json!({
        "projectId": "project-codex",
        "projectRoot": "/tmp/video-creater/repo",
        "projectDir": project_dir.path().to_str().expect("project dir utf8"),
        "jobId": "codex-edit-1",
        "request": temporal_codex_request(),
    }))
    .expect("collect codex project context");

    assert_eq!(output["status"], "collected");
    assert_eq!(output["projectId"], "project-codex");
    assert_eq!(output["projectRoot"], "/tmp/video-creater/repo");
    assert_eq!(output["projectDir"], project_dir.path().to_str().unwrap());
    assert_eq!(output["jobId"], "codex-edit-1");
    assert_eq!(output["request"]["mediaId"], "media-1");
    assert_eq!(output["context"]["projectName"], "Codex Temporal");
    assert_eq!(output["context"]["mediaRelativePath"], "media/source.mp4");
    assert_eq!(output["context"]["mediaDurationSeconds"], 120.0);
    assert_eq!(output["context"]["mediaFps"], 24.0);
    assert!(output["context"]["timelineSummary"]
        .as_str()
        .expect("timeline summary")
        .contains("Use timeline item ids"));
    assert!(output["context"]["transcriptExcerpt"]
        .as_str()
        .expect("transcript excerpt")
        .contains("Hook"));
    assert!(output["context"]["projectFilesSummary"]
        .as_str()
        .expect("project files summary")
        .contains("context/project.json"));
}

#[test]
fn temporal_codex_edit_request_proposal_activity_sends_contextual_app_server_turn() {
    let project_root = tempfile::tempdir().expect("project root");
    write_minimal_project_skills(project_root.path());
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = temporal_codex_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let context_output = temporal_codex_edit_collect_project_context_activity_value(json!({
        "projectId": "project-codex",
        "projectRoot": project_root.path().to_str().expect("project root utf8"),
        "projectDir": project_dir.path().to_str().expect("project dir utf8"),
        "jobId": "codex-edit-1",
        "request": temporal_codex_request(),
    }))
    .expect("collect codex project context");
    let proposal = temporal_codex_valid_proposal();
    let mut transport = RecordingCodexTransport::with_responses(vec![
        json!({ "ok": true }),
        json!({ "thread": { "id": "thread-temporal-1" } }),
        json!({ "proposal": proposal }),
    ]);

    let output = temporal_codex_edit_request_proposal_activity_with_transport(
        &mut transport,
        context_output,
    )
    .expect("request codex proposal");

    assert_eq!(output["status"], "completed");
    assert_eq!(output["projectId"], "project-codex");
    assert_eq!(output["threadId"], "thread-temporal-1");
    assert_eq!(output["proposal"]["mediaId"], "media-1");
    assert_eq!(transport.requests.len(), 3);
    assert_eq!(transport.requests[0]["method"], "initialize");
    assert_eq!(transport.requests[1]["method"], "thread/start");
    assert_eq!(
        transport.requests[1]["params"]["cwd"],
        project_root.path().to_str().expect("project root utf8")
    );
    let developer_instructions = transport.requests[1]["params"]["developerInstructions"]
        .as_str()
        .expect("developer instructions");
    assert!(developer_instructions.contains("## Mandatory Loaded Skill Text"));
    assert!(developer_instructions.contains("video-creater-video-pipeline"));
    assert!(developer_instructions.contains("The app is EDL-first"));
    assert!(!developer_instructions.contains("Video Creater test pipeline skill"));
    assert_eq!(transport.requests[2]["method"], "turn/start");
    assert_eq!(
        transport.requests[2]["params"]["threadId"],
        "thread-temporal-1"
    );
    let prompt = transport.requests[2]["params"]["input"][0]["text"]
        .as_str()
        .expect("turn prompt");
    assert!(prompt.contains("Project files:"));
    assert!(prompt.contains("context/project.json"));
    assert!(prompt.contains("Project timeline:"));
    assert!(prompt.contains("Use timeline item ids"));
    assert!(!prompt.contains("providerCredentialEnvVar"));
}

#[test]
fn temporal_codex_edit_request_proposal_activity_rejects_returned_validation_issues() {
    let project_root = tempfile::tempdir().expect("project root");
    write_minimal_project_skills(project_root.path());
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = temporal_codex_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let context_output = temporal_codex_edit_collect_project_context_activity_value(json!({
        "projectId": "project-codex",
        "projectRoot": project_root.path().to_str().expect("project root utf8"),
        "projectDir": project_dir.path().to_str().expect("project dir utf8"),
        "jobId": "codex-edit-1",
        "request": temporal_codex_request(),
    }))
    .expect("collect codex project context");
    let mut proposal = temporal_codex_valid_proposal();
    proposal.clips = vec![CodexProposalClip {
        media_id: "media-1".to_string(),
        source_in: 0.0,
        source_out: 120.0,
        reason: "full source pass-through".to_string(),
    }];
    proposal.render_review.duration_seconds = 120.0;
    let mut transport = RecordingCodexTransport::with_responses(vec![
        json!({ "ok": true }),
        json!({ "thread": { "id": "thread-temporal-invalid" } }),
        json!({ "proposal": proposal }),
    ]);

    let error = temporal_codex_edit_request_proposal_activity_with_transport(
        &mut transport,
        context_output,
    )
    .expect_err("Temporal proposal activity must remain strict");

    assert!(matches!(
        error,
        TemporalWorkflowInputError::CodexProposal(message)
            if message.contains("clips") && message.contains("full source")
    ));
}

#[test]
fn temporal_codex_proposal_rejects_a_project_revision_changed_during_the_turn() {
    let project_root = tempfile::tempdir().expect("project root");
    write_minimal_project_skills(project_root.path());
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &temporal_codex_project()).unwrap();
    let input = temporal_codex_edit_collect_project_context_activity_value(json!({
        "projectId":"project-codex", "projectRoot":project_root.path(),
        "projectDir":project_dir.path(), "jobId":"codex-edit-1", "request":temporal_codex_request()
    }))
    .unwrap();
    let mut transport = RecordingCodexTransport::with_responses(vec![
        json!({"ok":true}),
        json!({"thread":{"id":"thread-revision"}}),
        json!({"proposal":temporal_codex_valid_proposal()}),
    ]);
    transport.mutate_project_on_turn = Some(project_dir.path().to_path_buf());

    let error = temporal_codex_edit_request_proposal_activity_with_transport(&mut transport, input)
        .unwrap_err();
    assert!(
        matches!(error, TemporalWorkflowInputError::CodexAppServer(message) if message.contains("revision conflict"))
    );
    assert_eq!(
        load_split_project(project_dir.path()).unwrap().name,
        "Concurrent edit"
    );
}

#[test]
fn temporal_codex_edit_validate_project_actions_checks_agent_proposal_without_mutating_project() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let project = temporal_codex_project();
    save_split_project(project_dir.path(), &project).expect("save split project");
    let request = temporal_codex_request();
    let proposal = temporal_codex_valid_proposal();

    let output = temporal_codex_edit_validate_project_actions_activity_value(json!({
        "projectDir": project_dir.path().to_str().expect("project dir utf8"),
        "request": request,
        "proposal": proposal,
    }))
    .expect("validate codex proposal activity");
    let reloaded = load_split_project(project_dir.path()).expect("reload split project");
    let caption_track = reloaded
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == "track-captions")
        .expect("caption track");

    assert_eq!(output["status"], "validated");
    assert_eq!(output["projectId"], "project-codex");
    assert_eq!(output["mediaId"], "media-1");
    assert_eq!(output["clipCount"], 1);
    assert_eq!(output["projectActionCount"], 3);
    assert_eq!(output["durationSeconds"], 45.0);
    assert!(caption_track.items.is_empty());
}

#[test]
fn temporal_codex_edit_persist_accepted_proposal_applies_actions_and_completes_job() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = temporal_codex_project();
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::CodexEdit,
        &project.id,
        "codex-edit-1",
        JobStatus::Running,
        "2026-06-26T12:00:00Z",
    ));
    save_split_project(project_dir.path(), &project).expect("save split project");
    let request = temporal_codex_request();
    let proposal = temporal_codex_valid_proposal();

    let output = temporal_codex_edit_persist_accepted_proposal_activity_value(json!({
        "projectDir": project_dir.path().to_str().expect("project dir utf8"),
        "request": request,
        "proposal": proposal,
        "jobId": "codex-edit-1",
        "updatedAt": "2026-06-26T12:02:00Z",
        "runId": "temporal-run-codex-1",
    }))
    .expect("persist accepted proposal");
    let reloaded = load_split_project(project_dir.path()).expect("reload split project");
    let caption_track = reloaded
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == "track-captions")
        .expect("caption track");
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "codex-edit-1")
        .expect("codex job");

    assert_eq!(output["status"], "persisted");
    assert_eq!(output["projectId"], "project-codex");
    assert_eq!(output["jobId"], "codex-edit-1");
    assert_eq!(output["projectActionCount"], 3);
    assert_eq!(caption_track.items.len(), 1);
    assert_eq!(caption_track.items[0].id, "caption-agent-hook");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.updated_at, "2026-06-26T12:02:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-codex-1")
    );
    assert!(output["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("written file")
            .ends_with("timeline.json")));
    assert!(output["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("written file")
            .ends_with("jobs/codex-edit-1/job.json")));
}

#[test]
fn temporal_codex_edit_failure_actions_mark_job_failed() {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::CodexEdit,
        "project-codex",
        "codex-edit-1",
        JobStatus::Running,
        "2026-06-26T12:00:00Z",
    );
    job.start_request = Some(temporal_codex_edit_start_request(
        "project-codex",
        "/tmp/video-creater/repo",
        "/tmp/video-creater/project-codex",
        "codex-edit-1",
        temporal_codex_request(),
    ));

    let actions = temporal_codex_edit_failure_actions(
        &job,
        Some("temporal-run-codex-1"),
        "2026-06-26T12:05:00Z",
    )
    .expect("codex failure actions");

    assert_eq!(
        actions,
        vec![ProjectAction::UpdateJobStatus {
            job_id: "codex-edit-1".to_string(),
            status: JobStatus::Failed,
            updated_at: "2026-06-26T12:05:00Z".to_string(),
            run_id: Some("temporal-run-codex-1".to_string()),
        }]
    );
}

#[test]
fn temporal_codex_edit_attach_failure_activity_marks_split_project_job_failed() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = temporal_codex_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::CodexEdit,
        &project.id,
        "codex-edit-1",
        JobStatus::Running,
        "2026-06-26T12:00:00Z",
    );
    let start_request = temporal_codex_edit_start_request(
        &project.id,
        "/tmp/video-creater/repo",
        project_dir.path().to_str().expect("project dir utf8"),
        "codex-edit-1",
        temporal_codex_request(),
    );
    job.start_request = Some(start_request.clone());
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");

    let output = temporal_codex_edit_attach_failure_activity_value(json!({
        "startRequest": start_request,
        "runId": "temporal-run-codex-1",
        "updatedAt": "2026-06-26T12:05:00Z",
        "error": "Codex proposal request failed",
    }))
    .expect("attach codex failure");
    let reloaded = load_split_project(project_dir.path()).expect("reload split project");
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "codex-edit-1")
        .expect("failed codex job");
    let caption_track = reloaded
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == "track-captions")
        .expect("caption track");

    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.updated_at, "2026-06-26T12:05:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-codex-1")
    );
    assert!(caption_track.items.is_empty());
    assert_eq!(output["status"], json!("failed"));
    assert_eq!(output["projectId"], json!("project-codex"));
    assert_eq!(output["jobId"], json!("codex-edit-1"));
    assert_eq!(output["runId"], json!("temporal-run-codex-1"));
    assert_eq!(output["error"], json!("Codex proposal request failed"));
    assert!(output["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("written file")
            .ends_with("jobs/codex-edit-1/job.json")));
}

#[test]
fn temporal_codex_edit_failure_activity_input_rebuilds_replayable_start_request() {
    let request = temporal_codex_request();
    let input = temporal_codex_edit_failure_activity_input_value(
        &json!({
            "projectId": "Project A",
            "projectRoot": "/tmp/video-creater/repo",
            "projectDir": "/tmp/video-creater/Project A",
            "jobId": "codex-edit-1",
            "request": request,
        }),
        "2026-06-26T12:05:00Z",
        Some("temporal-run-codex-1"),
        "RequestCodexProposal failed: app server unavailable",
    )
    .expect("codex failure input");

    assert_eq!(input["updatedAt"], json!("2026-06-26T12:05:00Z"));
    assert_eq!(input["runId"], json!("temporal-run-codex-1"));
    assert_eq!(
        input["error"],
        json!("RequestCodexProposal failed: app server unavailable")
    );
    assert_eq!(
        input["startRequest"]["workflowType"],
        json!("VideoCreaterCodexEditWorkflow")
    );
    assert_eq!(
        input["startRequest"]["input"]["jobId"],
        json!("codex-edit-1")
    );
    assert_eq!(
        input["startRequest"]["activityTypes"],
        json!([
            "CollectProjectContext",
            "RequestCodexProposal",
            "ValidateProjectActions",
            "PersistAcceptedProposal",
            "AttachCodexEditFailure"
        ])
    );

    let serialized = input.to_string().to_lowercase();
    assert!(!serialized.contains("fal_key"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("secret"));
    assert!(input.get("proposal").is_none());
    assert!(input["startRequest"]["input"].get("proposal").is_none());
}

#[test]
fn temporal_start_result_action_records_running_status_with_run_id() {
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

    let action = temporal_start_result_action(&job, "temporal-run-1", "2026-06-23T12:01:00Z")
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

#[test]
fn temporal_workflow_client_start_plan_validates_recorded_job_without_secrets() {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "Project A",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    ));

    let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");

    assert_eq!(
        plan.workflow_id,
        "video-creater/project-a/generate-media/generated-shot-1"
    );
    assert_eq!(plan.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(plan.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(plan.id_reuse_policy, "rejectDuplicate");
    assert_eq!(
        plan.input
            .get("startRequest")
            .and_then(|value| value.get("input"))
            .and_then(|value| value.get(concat!("providerCredential", "EnvVar"))),
        None
    );
    let serialized = serde_json::to_string(&plan).expect("serialize client start plan");
    assert!(!serialized.contains("unit-test-token"));
    assert!(!serialized.contains("Authorization"));
    assert!(!serialized.contains("Bearer"));
}

#[test]
fn temporal_workflow_client_start_plan_rejects_unsupported_reuse_policy() {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    let mut start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    );
    start_request.id_reuse_policy = "allowDuplicate".to_string();
    job.start_request = Some(start_request);

    let error = temporal_workflow_client_start_plan(&job)
        .expect_err("unsupported reuse policy should fail");

    assert_eq!(
        error,
        TemporalStartResultError::UnsupportedIdReusePolicy("allowDuplicate".to_string())
    );
}

#[cfg(feature = "temporal-worker")]
#[test]
fn temporal_workflow_start_options_map_validated_plan_to_temporal_sdk_options() {
    use temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy;

    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "Project A",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    ));
    let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");

    let options = video_creater_lib::workflows::temporal_workflow_start_options(&plan);

    assert_eq!(options.workflow_id, plan.workflow_id);
    assert_eq!(options.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(
        options.id_reuse_policy,
        WorkflowIdReusePolicy::RejectDuplicate
    );
}

#[cfg(feature = "temporal-worker")]
#[test]
fn temporal_workflow_start_dispatch_maps_plan_to_registered_workflow_target() {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "Project A",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    ));
    let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");

    let dispatch = video_creater_lib::workflows::temporal_workflow_start_dispatch(&plan)
        .expect("workflow start dispatch");

    assert_eq!(
        dispatch.target,
        video_creater_lib::workflows::TemporalWorkflowStartTarget::GenerateMedia
    );
    assert_eq!(dispatch.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(dispatch.workflow_id, plan.workflow_id);
    assert_eq!(dispatch.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(
        dispatch
            .input
            .get("startRequest")
            .and_then(|value| value.get("input"))
            .and_then(|value| value.get("assetId"))
            .and_then(|value| value.as_str()),
        Some("generated-shot-1")
    );
}

#[cfg(feature = "temporal-worker")]
#[test]
fn temporal_workflow_untyped_start_request_builds_raw_temporal_input() {
    use temporalio_common::{data_converters::PayloadConverter, WorkflowDefinition};

    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "Project A",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    ));
    let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");

    let request = video_creater_lib::workflows::temporal_workflow_untyped_start_request(&plan)
        .expect("untyped workflow start request");

    assert_eq!(request.workflow.name(), "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(request.options.workflow_id, plan.workflow_id);
    assert_eq!(
        request.options.task_queue,
        VIDEO_CREATER_TEMPORAL_TASK_QUEUE
    );
    assert_eq!(request.input.payloads.len(), 1);
    let decoded: serde_json::Value = request.input.clone().to_value(&PayloadConverter::default());
    assert_eq!(
        decoded
            .get("startRequest")
            .and_then(|value| value.get("input"))
            .and_then(|value| value.get("assetId"))
            .and_then(|value| value.as_str()),
        Some("generated-shot-1")
    );
    assert_eq!(
        decoded
            .get("startRequest")
            .and_then(|value| value.get("input"))
            .and_then(|value| value.get(concat!("providerCredential", "EnvVar"))),
        None
    );
}

#[test]
fn temporal_workflow_started_start_result_records_temporal_run_id() {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "Project A",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    ));
    let plan = temporal_workflow_client_start_plan(&job).expect("client start plan");

    let result =
        temporal_workflow_started_start_result(&plan, "temporal-run-1").expect("start result");

    assert_eq!(result.status, "started");
    assert_eq!(result.workflow_id, plan.workflow_id);
    assert_eq!(result.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(result.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(result.run_id.as_deref(), Some("temporal-run-1"));
    assert!(result.message.contains("Temporal workflow started"));
}

#[test]
fn temporal_start_result_action_rejects_missing_start_request() {
    let job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );

    let error = temporal_start_result_action(&job, "temporal-run-1", "2026-06-23T12:01:00Z")
        .expect_err("missing start request should fail");

    assert_eq!(
        error,
        TemporalStartResultError::MissingStartRequest("generated-shot-1".to_string())
    );
}

#[test]
fn temporal_start_result_action_rejects_mismatched_start_request() {
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
        "other-generated-shot",
        "other-generated-shot",
        true,
        None,
    ));

    let error = temporal_start_result_action(&job, "temporal-run-1", "2026-06-23T12:01:00Z")
        .expect_err("mismatched start request should fail");

    assert_eq!(
        error,
        TemporalStartResultError::MismatchedStartRequest("workflowId".to_string())
    );
}

#[test]
fn temporal_start_result_action_rejects_blank_run_id() {
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

    let error = temporal_start_result_action(&job, "  ", "2026-06-23T12:01:00Z")
        .expect_err("blank run id should fail");

    assert_eq!(error, TemporalStartResultError::BlankRunId);
}

#[test]
fn temporal_workflow_unavailable_start_result_validates_start_request_without_run_id() {
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

    let result =
        temporal_workflow_unavailable_start_result(&job).expect("unavailable start result");

    assert_eq!(result.status, "unavailable");
    assert_eq!(
        result.workflow_id,
        "video-creater/project-1/generate-media/generated-shot-1"
    );
    assert_eq!(result.workflow_type, "VideoCreaterGenerateMediaWorkflow");
    assert_eq!(result.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(result.run_id, None);
    assert!(result.message.contains("temporal-worker"));
    assert!(result.message.contains("temporal server start-dev"));
}

#[test]
fn temporal_generate_media_failure_actions_mark_job_and_asset_failed() {
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

    let actions = temporal_generate_media_failure_actions(
        &job,
        "generated-shot-1",
        Some("temporal-run-1"),
        "2026-06-23T12:05:00Z",
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
fn temporal_generate_media_failure_actions_reject_mismatched_asset_id() {
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

    let error = temporal_generate_media_failure_actions(
        &job,
        "other-generated-shot",
        Some("temporal-run-1"),
        "2026-06-23T12:05:00Z",
    )
    .expect_err("mismatched asset id should fail");

    assert_eq!(
        error,
        TemporalStartResultError::MismatchedStartRequest("input.assetId".to_string())
    );
}

#[test]
fn temporal_generate_media_failure_actions_reject_blank_updated_at() {
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

    let error = temporal_generate_media_failure_actions(&job, "generated-shot-1", None, "  ")
        .expect_err("blank timestamp should fail");

    assert_eq!(error, TemporalStartResultError::BlankUpdatedAt);
}

#[test]
fn temporal_generate_media_attach_failure_activity_marks_split_project_failed() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Failure".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        "generated-shot-1",
        JobStatus::Running,
        "2026-06-23T12:00:00Z",
    );
    let start_request = temporal_generate_media_start_request(
        &project.id,
        project_dir.path().to_str().expect("project dir utf8"),
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    );
    job.start_request = Some(start_request.clone());
    project.jobs.push(job);
    save_split_project(project_dir.path(), &project).expect("save split project");

    let result = temporal_generate_media_attach_failure_activity_value(json!({
        "startRequest": start_request,
        "runId": "temporal-run-1",
        "updatedAt": "2026-06-23T12:07:00Z",
    }))
    .expect("attach failure activity");
    let reloaded = load_split_project(project_dir.path()).expect("reload split project");
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "generated-shot-1")
        .expect("failed job");
    let generated_asset = reloaded
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("failed generated asset");

    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.updated_at, "2026-06-23T12:07:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-1")
    );
    assert_eq!(generated_asset.status, GeneratedAssetStatus::Failed);
    assert_eq!(result["jobId"], json!("generated-shot-1"));
    assert_eq!(result["assetId"], json!("generated-shot-1"));
    assert_eq!(result["status"], json!("failed"));
    assert_eq!(result["runId"], json!("temporal-run-1"));
    assert!(result["writtenFiles"]
        .as_array()
        .expect("written files")
        .iter()
        .any(|path| path
            .as_str()
            .expect("written file")
            .ends_with("generated/generated-shot-1/asset.json")));
    assert!(!serde_json::to_string(&result)
        .expect("serialize result")
        .contains("unit-test-token"));
}

#[test]
fn temporal_generate_media_mock_completion_actions_use_start_request_asset() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    let asset = generated_asset("generated-shot-1");
    project.generated_assets.push(asset);
    project.jobs.push({
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
        job
    });
    let start_request = project.jobs[0]
        .start_request
        .as_ref()
        .expect("start request");

    let actions = temporal_generate_media_mock_completion_actions(
        &project,
        start_request,
        "2026-06-23T12:05:00Z",
        Some("item-1"),
    )
    .expect("mock completion actions");

    assert_eq!(actions.len(), 3);
    assert_eq!(
        actions[0],
        ProjectAction::UpdateJobStatus {
            job_id: "generated-shot-1".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-06-23T12:05:00Z".to_string(),
            run_id: Some("mock-generated-shot-1".to_string()),
        }
    );
    assert!(matches!(
        &actions[1],
        ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            replacement,
            ..
        } if asset_id == "generated-shot-1"
            && outputs[0].relative_path == "generated/generated-shot-1/mock-output.mp4"
            && replacement.as_ref().is_some_and(|replacement| replacement.item_id == "item-1")
    ));
    assert_eq!(
        actions[2],
        ProjectAction::UpdateJobStatus {
            job_id: "generated-shot-1".to_string(),
            status: JobStatus::Completed,
            updated_at: "2026-06-23T12:05:00Z".to_string(),
            run_id: Some("mock-generated-shot-1".to_string()),
        }
    );
}

#[test]
fn temporal_generate_media_mock_completion_actions_use_placement_intent_replacement() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        true,
        Some(TemporalGenerateMediaBrief {
            name: None,
            target_folder_id: None,
            placement_intent: Some("replace:item-1".to_string()),
            prompt: None,
            model: None,
            references: None,
            settings: None,
        }),
    );

    let actions = temporal_generate_media_mock_completion_actions(
        &project,
        &start_request,
        "2026-06-23T12:05:00Z",
        None,
    )
    .expect("mock completion actions");

    assert!(matches!(
        &actions[1],
        ProjectAction::CompleteGeneratedAsset {
            replacement: Some(replacement),
            ..
        } if replacement.item_id == "item-1"
            && replacement.media_id == "generated-shot-1-mock-output"
    ));
}

#[test]
fn temporal_generate_media_mock_completion_actions_insert_timeline_audio_span() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    let mut asset = generated_asset("generated-audio-1");
    asset.kind = MediaKind::Audio;
    asset.placement_intent = Some("timeline".to_string());
    asset.model = GenerationModel {
        provider: "fal.ai".to_string(),
        id: "sonilo/v1.1/video-to-music".to_string(),
    };
    asset.references.provider_input_urls =
        vec!["https://v3.fal.media/files/provider-e2e/source.mp4".to_string()];
    asset.settings = GeneratedAssetSettings {
        duration_seconds: Some(3.5),
        fps: None,
        timeline_start_seconds: Some(8.25),
        video_source_start_seconds: Some(2.0),
        video_source_end_seconds: Some(5.5),
        ..GeneratedAssetSettings::default()
    };
    project.generated_assets.push(asset);
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-audio-1",
        "generated-audio-1",
        true,
        Some(TemporalGenerateMediaBrief {
            name: None,
            target_folder_id: None,
            placement_intent: Some("timeline".to_string()),
            prompt: None,
            model: None,
            references: None,
            settings: Some(json!({
                "durationSeconds": 3.5,
                "timelineStartSeconds": 8.25,
                "videoSourceStartSeconds": 2.0,
                "videoSourceEndSeconds": 5.5
            })),
        }),
    );

    let actions = temporal_generate_media_mock_completion_actions(
        &project,
        &start_request,
        "2026-06-23T12:05:00Z",
        None,
    )
    .expect("mock completion actions");

    assert_eq!(actions.len(), 4);
    assert!(matches!(
        &actions[1],
        ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            replacement: None,
            ..
        } if asset_id == "generated-audio-1"
            && outputs[0].media_id == "generated-audio-1-mock-output"
            && outputs[0].relative_path == "generated/generated-audio-1/mock-output.m4a"
            && outputs[0].duration_seconds == 3.5
    ));
    assert!(matches!(
        &actions[2],
        ProjectAction::InsertItems {
            target_track_id,
            insert_seconds,
            items,
        } if target_track_id == "track-audio"
            && *insert_seconds == 8.25
            && items.len() == 1
            && items[0].id == "generated-audio-1-timeline-audio"
            && items[0].kind == TimelineItemKind::AudioClip
            && items[0].start_seconds == 8.25
            && items[0].duration_seconds == 3.5
            && matches!(&items[0].source, TimelineSource::Media { media_id } if media_id == "generated-audio-1-mock-output")
            && items[0].properties.get("generatedAssetId") == Some(&json!("generated-audio-1"))
            && items[0].properties.get("generatedOutputMediaId") == Some(&json!("generated-audio-1-mock-output"))
            && items[0].properties.get("sourceIn") == Some(&json!(0.0))
            && items[0].properties.get("sourceOut") == Some(&json!(3.5))
    ));
}

#[test]
fn temporal_generate_media_mock_completion_actions_insert_timeline_visual_span() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    let mut asset = generated_asset("generated-video-1");
    asset.placement_intent = Some("timeline".to_string());
    asset.settings.timeline_start_seconds = Some(6.0);
    project.generated_assets.push(asset);
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-video-1",
        "generated-video-1",
        true,
        Some(TemporalGenerateMediaBrief {
            name: None,
            target_folder_id: None,
            placement_intent: Some("timeline".to_string()),
            prompt: None,
            model: None,
            references: None,
            settings: Some(json!({
                "timelineStartSeconds": 6.0
            })),
        }),
    );

    let actions = temporal_generate_media_mock_completion_actions(
        &project,
        &start_request,
        "2026-06-23T12:05:00Z",
        None,
    )
    .expect("mock completion actions");

    assert_eq!(actions.len(), 4);
    assert!(matches!(
        &actions[1],
        ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            replacement: None,
            ..
        } if asset_id == "generated-video-1"
            && outputs[0].media_id == "generated-video-1-mock-output"
            && outputs[0].relative_path == "generated/generated-video-1/mock-output.mp4"
            && outputs[0].duration_seconds == 5.0
    ));
    assert!(matches!(
        &actions[2],
        ProjectAction::InsertItems {
            target_track_id,
            insert_seconds,
            items,
        } if target_track_id == "track-video"
            && *insert_seconds == 6.0
            && items.len() == 1
            && items[0].id == "generated-video-1-timeline-visual"
            && items[0].kind == TimelineItemKind::VideoClip
            && items[0].start_seconds == 6.0
            && items[0].duration_seconds == 5.0
            && matches!(&items[0].source, TimelineSource::Media { media_id } if media_id == "generated-video-1-mock-output")
            && items[0].properties.get("generatedAssetId") == Some(&json!("generated-video-1"))
            && items[0].properties.get("generatedOutputMediaId") == Some(&json!("generated-video-1-mock-output"))
            && items[0].properties.get("sourceIn") == Some(&json!(0.0))
            && items[0].properties.get("sourceOut") == Some(&json!(5.0))
    ));
}

#[test]
fn temporal_generate_media_mock_completion_actions_reject_live_mode() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        false,
        None,
    );

    let error = temporal_generate_media_mock_completion_actions(
        &project,
        &start_request,
        "2026-06-23T12:05:00Z",
        None,
    )
    .expect_err("live mode should not use mock completion");

    assert_eq!(error, TemporalWorkflowInputError::MockModeRequired);
}

#[test]
fn temporal_generate_media_mock_completion_actions_reject_mismatched_job_id() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Temporal Mock".to_string(),
        "2026-06-23T12:00:00Z".to_string(),
    );
    project
        .generated_assets
        .push(generated_asset("generated-shot-1"));
    let start_request = temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "other-job-id",
        true,
        None,
    );

    let error = temporal_generate_media_mock_completion_actions(
        &project,
        &start_request,
        "2026-06-23T12:05:00Z",
        None,
    )
    .expect_err("mismatched job id should fail");

    assert_eq!(
        error,
        TemporalWorkflowInputError::MismatchedInputField("jobId".to_string())
    );
}

#[test]
fn temporal_workflow_spec_maps_nle_export_to_registered_types() {
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::ExportNleXml,
        "project-1",
        "nle-export-1",
    );

    assert_eq!(spec.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(spec.workflow_type, "VideoCreaterExportNleXmlWorkflow");
    assert_eq!(
        spec.workflow_id,
        "video-creater/project-1/export-nle-xml/nle-export-1"
    );
    assert_eq!(
        spec.activity_types,
        vec![
            "BuildNleXml",
            "ValidateNleXml",
            "WriteExportArtifact",
            "AttachExportReport"
        ]
    );
}

#[test]
fn temporal_workflow_spec_maps_mp4_export_to_registered_types() {
    let spec = temporal_workflow_spec(
        TemporalWorkflowKind::ExportMedia,
        "project-1",
        "mp4-export-1",
    );

    assert_eq!(spec.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(spec.workflow_type, "VideoCreaterExportMediaWorkflow");
    assert_eq!(
        spec.workflow_id,
        "video-creater/project-1/export-media/mp4-export-1"
    );
    assert_eq!(
        spec.activity_types,
        vec![
            "BuildRenderPlan",
            "ValidateExportProfile",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
            "WriteExportArtifact",
            "AttachExportReport"
        ]
    );
}

#[test]
fn temporal_workflow_kind_accepts_frontend_job_kind_names() {
    let generate_media: TemporalWorkflowKind =
        serde_json::from_value(json!("generate_media")).expect("deserialize generate kind");
    let render_draft: TemporalWorkflowKind =
        serde_json::from_value(json!("render_draft")).expect("deserialize render kind");
    let export_nle_xml: TemporalWorkflowKind =
        serde_json::from_value(json!("export_nle_xml")).expect("deserialize nle export kind");
    let export_media: TemporalWorkflowKind =
        serde_json::from_value(json!("export_media")).expect("deserialize media export kind");

    assert_eq!(generate_media, TemporalWorkflowKind::GenerateMedia);
    assert_eq!(render_draft, TemporalWorkflowKind::RenderDraft);
    assert_eq!(export_nle_xml, TemporalWorkflowKind::ExportNleXml);
    assert_eq!(export_media, TemporalWorkflowKind::ExportMedia);
}

#[test]
fn temporal_worker_manifest_lists_registered_workflows_and_activities() {
    let manifest = temporal_worker_manifest();

    assert_eq!(manifest.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(manifest.local_service_target, "http://localhost:7233");
    assert_eq!(manifest.local_web_ui_url, "http://localhost:8233");
    assert_eq!(manifest.local_dev_command, "temporal server start-dev");
    assert_eq!(manifest.feature_name, "temporal-worker");
    assert_eq!(
        manifest.worker_run_command,
        "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker"
    );
    assert_eq!(
        manifest.sdk_crates,
        vec![
            "temporalio-client",
            "temporalio-common",
            "temporalio-macros",
            "temporalio-sdk",
            "temporalio-sdk-core",
        ]
    );
    assert_eq!(manifest.required_tools, vec!["protoc", "temporal"]);
    assert_eq!(
        manifest.workflow_types,
        vec![
            "VideoCreaterGenerateMediaWorkflow",
            "VideoCreaterRenderDraftWorkflow",
            "VideoCreaterTranscribeMediaWorkflow",
            "VideoCreaterCodexEditWorkflow",
            "VideoCreaterExportMediaWorkflow",
            "VideoCreaterExportNleXmlWorkflow",
        ]
    );
    assert_eq!(
        manifest.activity_types,
        vec![
            "PrepareProviderInputs",
            "BuildFalGenerationRequest",
            "RunMediaProviderGeneration",
            "AttachGeneratedAssetFailure",
            "BuildRenderPlan",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
            "ProbeMedia",
            "RunTranscription",
            "StoreTranscript",
            "CollectProjectContext",
            "RequestCodexProposal",
            "ValidateProjectActions",
            "PersistAcceptedProposal",
            "AttachCodexEditFailure",
            "ValidateExportProfile",
            "WriteExportArtifact",
            "AttachExportReport",
            "BuildNleXml",
            "ValidateNleXml",
        ]
    );
}

#[test]
fn temporal_worker_registration_plan_maps_manifest_to_worker_registrations() {
    let plan = temporal_worker_registration_plan();
    let manifest = temporal_worker_manifest();

    assert_eq!(plan.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(plan.workflow_registrations, manifest.workflow_types);
    assert_eq!(plan.activity_registrations, manifest.activity_types);
    assert!(plan
        .workflow_registrations
        .iter()
        .all(|name| name.starts_with("VideoCreater")));
    assert!(plan
        .activity_registrations
        .iter()
        .any(|name| name == "RunMediaProviderGeneration"));
}

#[cfg(feature = "temporal-worker")]
#[test]
fn temporal_worker_options_register_manifest_workflow_and_activity_names() {
    let options = video_creater_lib::workflows::temporal_worker_options();
    let manifest = temporal_worker_manifest();
    let mut workflow_names: Vec<_> = options.workflows().workflow_types().collect();
    workflow_names.sort_unstable();
    let mut manifest_workflows: Vec<_> =
        manifest.workflow_types.iter().map(String::as_str).collect();
    manifest_workflows.sort_unstable();
    let activity_debug = format!("{:?}", options.activities());

    assert_eq!(options.task_queue, VIDEO_CREATER_TEMPORAL_TASK_QUEUE);
    assert_eq!(workflow_names, manifest_workflows);
    for activity_type in manifest.activity_types {
        assert!(
            activity_debug.contains(&activity_type),
            "worker options should register activity {activity_type}; got {activity_debug}"
        );
    }
    for removed_activity_type in [
        "RecordGenerationPrompt",
        "ImportGeneratedOutput",
        "AttachGeneratedAssetResult",
    ] {
        assert!(
            !activity_debug.contains(removed_activity_type),
            "worker options should not register obsolete generate-media activity {removed_activity_type}; got {activity_debug}"
        );
    }
}

#[test]
fn legacy_job_summaries_still_deserialize_without_workflow_metadata() {
    let summary: JobSummary = serde_json::from_value(json!({
        "id": "legacy-job",
        "kind": "render",
        "status": "completed",
        "updatedAt": "2026-06-20T00:00:00Z"
    }))
    .expect("deserialize legacy job summary");

    assert_eq!(summary.id, "legacy-job");
    assert_eq!(summary.workflow, None);
}

fn generated_asset(id: &str) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: id.to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Generated product push".to_string()),
        target_folder_id: None,
        placement_intent: Some("replace:item-1".to_string()),
        prompt: "floating product shot with crisp rim light".to_string(),
        model: GenerationModel {
            provider: "fal.ai".to_string(),
            id: "fal-ai/wan-25-preview/text-to-video".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..GeneratedAssetReferences::default()
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
    }
}

fn spawn_temporal_fal_run_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal run server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let responses = vec![
        temporal_fal_json_response(format!(
            r#"{{"request_id":"req-123","response_url":"{base_url}/fal-ai/wan-25-preview/text-to-video/requests/req-123/response","status_url":"{base_url}/fal-ai/wan-25-preview/text-to-video/requests/req-123/status","cancel_url":"{base_url}/fal-ai/wan-25-preview/text-to-video/requests/req-123/cancel","queue_position":0}}"#
        )),
        temporal_fal_json_response(format!(
            r#"{{"status":"COMPLETED","request_id":"req-123","response_url":"{base_url}/fal-ai/wan-25-preview/text-to-video/requests/req-123/response","logs":[{{"message":"Done.","timestamp":"2026-02-17T10:30:05.789Z"}}],"metrics":{{"inference_time":3.42}}}}"#
        )),
        temporal_fal_json_response(format!(
            r#"{{"video":{{"url":"{base_url}/generated/hero.mp4","width":1280,"height":720,"fps":24.0,"duration":5.0,"content_type":"video/mp4"}},"actual_prompt":"floating product shot with crisp rim light"}}"#
        )),
        ("application/octet-stream", b"result-video-bytes".to_vec()),
    ];

    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for (content_type, body) in responses {
            let (mut stream, _) = listener.accept().expect("accept fal request");
            let request = read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write fal response headers");
            stream.write_all(&body).expect("write fal response body");
            requests.push(request);
        }
        requests
    });

    Some((base_url, handle))
}

fn temporal_fal_json_response(body: String) -> (&'static str, Vec<u8>) {
    ("application/json", body.into_bytes())
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        let request = String::from_utf8_lossy(&buffer);
        if let Some((headers, _)) = request.split_once("\r\n\r\n") {
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let header_len = headers.len() + 4;
            if buffer.len() >= header_len + content_length {
                break;
            }
        }
    }

    String::from_utf8(buffer).expect("request utf8")
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}

fn temporal_codex_request() -> EditJobRequest {
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

fn write_minimal_project_skills(project_root: &Path) {
    fs::write(
        project_root.join("AGENTS.md"),
        "# Test AGENTS\n\nUse project-local skills.\n",
    )
    .expect("write AGENTS.md");
    let skills_root = project_root.join(".agents/skills");
    for (name, body) in [
        (
            "video-creater-video-pipeline",
            "# Video Creater test pipeline skill\n\nBuild a real EDL before visuals.\n",
        ),
        (
            "video-creater-graphics",
            "# Video Creater test graphics skill\n\nVisual layers need safe zones.\n",
        ),
        (
            "video-creater-visuals",
            "# Video Creater test visuals skill\n\nKeep editor UI dense.\n",
        ),
    ] {
        let skill_dir = skills_root.join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(skill_dir.join("SKILL.md"), body).expect("write skill");
    }
}

fn transcribe_store_input(mut input: Value, raw_artifact_path: &Path) -> Value {
    input
        .as_object_mut()
        .expect("store input should be a JSON object")
        .insert(
            "nativeOutput".to_string(),
            json!({
                "engine": FLUID_AUDIO_COREML_RUNTIME_ID,
                "rawArtifactPath": raw_artifact_path.display().to_string(),
                "words": [],
                "segments": []
            }),
        );
    input
}

fn temporal_codex_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-codex".to_string(),
        "Codex Temporal".to_string(),
        "2026-06-26T12:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/source.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Hook".to_string(),
            start_seconds: 1.0,
            end_seconds: 1.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });
    project
}

fn temporal_codex_valid_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: "media-1".to_string(),
        clips: vec![CodexProposalClip {
            media_id: "media-1".to_string(),
            source_in: 1.0,
            source_out: 46.0,
            reason: "strong hook and complete thought".to_string(),
        }],
        captions: Vec::new(),
        overlays: Vec::new(),
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: vec![ProjectAction::AddItems {
            target_track_id: "track-captions".to_string(),
            items: vec![TimelineItem {
                id: "caption-agent-hook".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source: TimelineSource::Text {
                    text: "Hook".to_string(),
                },
                label: "Hook caption".to_string(),
                properties: BTreeMap::new(),
            }],
        }],
        render_review: CodexRenderReview {
            duration_seconds: 45.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}

fn write_required_model_files(entry: &TranscriptionModelCatalogEntry, model_dir: &Path) {
    for file in &entry.required_files {
        let file_path = model_dir.join(&file.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("model file parent");
        }
        fs::write(file_path, b"present").expect("write model file");
    }
}

#[cfg(target_os = "macos")]
fn chmod(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set permissions");
}

#[test]
fn cargo_manifest_declares_temporal_worker_scaffold() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_manifest =
        std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");

    assert!(
        cargo_manifest.contains("temporal-worker = ["),
        "Cargo.toml should expose a temporal-worker feature"
    );

    for dependency in [
        "temporalio-client",
        "temporalio-common",
        "temporalio-macros",
        "temporalio-sdk",
        "temporalio-sdk-core",
    ] {
        assert!(
            cargo_manifest.contains(&format!("{dependency} = {{ version = \"0.4.0\"")),
            "Cargo.toml should pin optional dependency {dependency} to Temporal Rust SDK 0.4.0"
        );
        assert!(
            cargo_manifest.contains(&format!("\"dep:{dependency}\"")),
            "temporal-worker feature should enable {dependency}"
        );
    }

    assert!(
        manifest_dir
            .join("src/bin/video-creater-temporal-worker.rs")
            .is_file(),
        "Temporal worker binary should exist"
    );
}
