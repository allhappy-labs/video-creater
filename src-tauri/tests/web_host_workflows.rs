use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{json, Value};
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::project::{
    fixtures::sample_project,
    split::{load_split_project, save_split_project},
};
use video_creater_lib::web_host::{
    dispatcher::HostDispatcher,
    project_catalog::ProjectCatalog,
    registry::RpcRegistry,
    rpc::{RpcEngine, RpcEnvelope},
};

fn execute(
    engine: &RpcEngine,
    id: &str,
    operation: &str,
    project_id: Option<&str>,
    revision: Option<u64>,
    payload: Value,
) -> Value {
    let response = engine.execute(
        "workflow-session",
        &BTreeSet::from([AuthorizationScope::HostAdmin]),
        &serde_json::to_vec(&RpcEnvelope {
            request_id: id.into(),
            operation: operation.into(),
            project_id: project_id.map(str::to_string),
            expected_revision: revision,
            editor_lease_token: project_id.map(|_| "test-editor-lease".into()),
            payload,
        })
        .unwrap(),
        100,
    );
    assert!(response.ok, "{operation}: {:?}", response.error);
    response.result.unwrap()
}

#[test]
fn remote_workflow_operations_are_registered_without_exposing_internal_builders() {
    let registry = RpcRegistry::from_inventory();
    for operation in [
        "search_project_media",
        "list_generation_model_catalog",
        "get_settings_health_snapshot",
        "export_nle_xml_to_split_project_folder",
        "export_palmier_project_package_to_split_project_folder",
        "run_generate_media_in_process",
        "run_transcribe_media_in_process",
        "remote_build_temporal_job_summary",
        "remote_build_temporal_transcribe_media_start_request",
        "remote_build_temporal_generate_media_start_request",
        "remote_build_temporal_start_result_action",
        "remote_start_temporal_workflow",
    ] {
        assert!(
            registry.operation(operation).is_some(),
            "missing {operation}"
        );
    }
    assert!(registry
        .operation("build_temporal_transcribe_media_start_request")
        .is_none());
    assert!(registry.operation("call_codex_local_tool").is_none());
}

#[test]
fn remote_search_and_workflow_builders_use_the_catalog_project() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let project = save_split_project(&path, &sample_project())
        .unwrap()
        .project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let result = execute(
        &engine,
        "search",
        "search_project_media",
        Some(&id),
        None,
        json!({"projectDir": "/outside", "query": "sample", "scope": "both", "limit": 20}),
    );
    assert!(result.is_object());
    let result = execute(
        &engine,
        "build",
        "remote_build_temporal_transcribe_media_start_request",
        Some(&id),
        None,
        json!({"projectId": "forged", "projectDir": "/outside", "mediaId": project.media[0].id, "jobId": "speech-job", "languageMode": "auto"}),
    );
    assert_eq!(result["input"]["projectId"], project.id);
    assert_eq!(result["input"]["projectDir"], id);
    assert_eq!(
        load_split_project(&path).unwrap().content_revision,
        project.content_revision
    );
}

#[test]
fn remote_generation_catalog_matches_the_native_catalog() {
    let engine = RpcEngine::new(Arc::new(HostDispatcher::default()));
    let result = execute(
        &engine,
        "catalog",
        "list_generation_model_catalog",
        None,
        None,
        json!({}),
    );
    assert!(result["models"]
        .as_array()
        .is_some_and(|models| !models.is_empty()));
}

#[test]
fn remote_xml_export_records_an_artifact_under_the_checked_project() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let project = save_split_project(&path, &sample_project())
        .unwrap()
        .project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let result = execute(
        &engine,
        "xml",
        "export_nle_xml_to_split_project_folder",
        Some(&id),
        Some(project.content_revision),
        json!({"projectDir": "/outside", "format": "premiereXmeml", "jobId": "xml-job", "updatedAt": "2026-10-03T00:00:00Z"}),
    );
    assert_eq!(result["job"]["status"], "completed");
    let saved = load_split_project(&path).unwrap();
    assert!(saved
        .export_artifacts
        .iter()
        .any(|artifact| artifact.job_id.as_deref() == Some("xml-job")));
    assert!(saved.content_revision > project.content_revision);
}

#[test]
fn remote_workflow_start_rejects_unrecognized_recorded_workflows_before_connecting() {
    use video_creater_lib::project::model::JobStatus;
    use video_creater_lib::workflows::{
        temporal_job_summary, temporal_transcribe_media_start_request, TemporalWorkflowKind,
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        &project.id,
        "speech-job",
        JobStatus::Queued,
        "2026-10-03T00:00:00Z",
    );
    let mut start = temporal_transcribe_media_start_request(
        &project.id,
        "outside",
        "media-1",
        "speech-job",
        "auto",
    );
    start.workflow_type = "UnrecognizedWorkflow".into();
    job.workflow.as_mut().unwrap().workflow_type = start.workflow_type.clone();
    job.start_request = Some(start);
    project.jobs.push(job.clone());
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response = engine.execute(
        "session",
        &BTreeSet::from([AuthorizationScope::HostAdmin]),
        &serde_json::to_vec(&RpcEnvelope {
            request_id: "unrecognized".into(),
            operation: "remote_start_temporal_workflow".into(),
            project_id: Some(id),
            expected_revision: Some(project.content_revision),
            editor_lease_token: Some("test-lease".into()),
            payload: json!({"job":job}),
        })
        .unwrap(),
        100,
    );
    assert_eq!(
        response.error.map(|error| error.code),
        Some(video_creater_lib::web_host::rpc::RpcErrorCode::InvalidRequest)
    );
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[test]
fn remote_xml_export_rejects_a_stale_revision_without_creating_output() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let project = save_split_project(&path, &sample_project())
        .unwrap()
        .project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response=engine.execute("session",&BTreeSet::from([AuthorizationScope::HostAdmin]),&serde_json::to_vec(&RpcEnvelope {
        request_id:"stale-xml".into(),operation:"export_nle_xml_to_split_project_folder".into(),project_id:Some(id),expected_revision:Some(project.content_revision.saturating_sub(1)),editor_lease_token:Some("test-lease".into()),payload:json!({"format":"premiereXmeml","jobId":"stale","updatedAt":"2026-10-03T00:00:00Z"}),
    }).unwrap(),100);
    assert_eq!(
        response.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::Conflict
    );
    assert_eq!(load_split_project(&path).unwrap(), project);
    assert!(!path.join("exports/project-test-premiere.xml").exists());
}

#[test]
fn invalid_xml_export_preserves_existing_output_and_project() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let project = save_split_project(&path, &sample_project())
        .unwrap()
        .project;
    let export = video_creater_lib::project::nle_export::export_project_timeline_to_nle_xml(
        &project,
        video_creater_lib::project::nle_export::NleXmlFormat::PremiereXmeml,
    )
    .unwrap();
    let output =
        video_creater_lib::project::nle_export::write_nle_xml_export(&path, &export).unwrap();
    std::fs::write(&output, b"existing user export").unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response=engine.execute("session",&BTreeSet::from([AuthorizationScope::HostAdmin]),&serde_json::to_vec(&RpcEnvelope {
        request_id:"invalid-xml".into(),operation:"export_nle_xml_to_split_project_folder".into(),project_id:Some(id),expected_revision:Some(project.content_revision),editor_lease_token:Some("test-lease".into()),payload:json!({"format":"premiereXmeml","jobId":"../invalid","updatedAt":"2026-10-03T00:00:00Z"}),
    }).unwrap(),100);
    assert!(!response.ok);
    assert_eq!(std::fs::read(&output).unwrap(), b"existing user export");
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[test]
fn remote_export_builder_rejects_absolute_destination_directory() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let project = save_split_project(&path, &sample_project())
        .unwrap()
        .project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response=engine.execute("session",&BTreeSet::from([AuthorizationScope::HostAdmin]),&serde_json::to_vec(&RpcEnvelope {
        request_id:"external-export".into(),operation:"remote_build_temporal_export_media_start_request".into(),project_id:Some(id),expected_revision:None,editor_lease_token:Some("test-lease".into()),payload:json!({"jobId":"export","profile":"mp4H264","quality":"final","width":1920,"height":1080,"outputPath":"exports/output.mp4","output":{"fileName":"output.mp4","directory":root.path()}}),
    }).unwrap(),100);
    assert_eq!(
        response.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::InvalidRequest
    );
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[test]
fn remote_workflow_rejects_finished_jobs_without_connecting() {
    use video_creater_lib::project::model::JobStatus;
    use video_creater_lib::workflows::{
        temporal_job_summary, temporal_transcribe_media_start_request, TemporalWorkflowKind,
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        &project.id,
        "finished",
        JobStatus::Completed,
        "2026-10-03T00:00:00Z",
    );
    job.start_request = Some(temporal_transcribe_media_start_request(
        &project.id,
        "opaque",
        &project.media[0].id,
        "finished",
        "auto",
    ));
    project.jobs.push(job.clone());
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response = engine.execute(
        "session",
        &BTreeSet::from([AuthorizationScope::HostAdmin]),
        &serde_json::to_vec(&RpcEnvelope {
            request_id: "finished-workflow".into(),
            operation: "remote_start_temporal_workflow".into(),
            project_id: Some(id),
            expected_revision: Some(project.content_revision),
            editor_lease_token: Some("test-lease".into()),
            payload: json!({"job":job}),
        })
        .unwrap(),
        100,
    );
    assert_eq!(
        response.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::InvalidRequest
    );
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[test]
fn remote_mock_generation_completes_the_saved_job_and_asset() {
    use video_creater_lib::project::model::*;
    use video_creater_lib::workflows::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("generation.palmier");
    let now = "2026-10-03T00:00:00Z";
    let mut project = VideoProject::new_empty("project-mock".into(), "Mock".into(), now.into());
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-mock".into(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Mock still".into()),
        target_folder_id: None,
        placement_intent: Some("library".into()),
        prompt: "offline mock still".into(),
        model: GenerationModel {
            provider: "fal.ai".into(),
            id: "fal-ai/flux/schnell".into(),
        },
        references: GeneratedAssetReferences::default(),
        settings: GeneratedAssetSettings::default(),
        outputs: vec![],
        created_at: now.into(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });
    let start = temporal_generate_media_start_request(
        &project.id,
        "opaque",
        "generated-mock",
        "generated-mock",
        true,
        None,
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        "generated-mock",
        JobStatus::Queued,
        now,
    );
    job.start_request = Some(start.clone());
    project.jobs.push(job);
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let result = execute(
        &engine,
        "generation",
        "run_generate_media_in_process",
        Some(&id),
        Some(project.content_revision),
        json!({"startRequest":start,"updatedAt":now}),
    );
    assert_eq!(result["jobs"][0]["status"], "completed");
    assert_eq!(result["generatedAssets"][0]["status"], "completed");
    assert_eq!(
        load_split_project(&path).unwrap().jobs[0].status,
        JobStatus::Completed
    );
}

#[test]
fn remote_transcription_runs_on_the_host_and_fails_the_saved_job_with_its_reason() {
    use video_creater_lib::project::model::*;
    use video_creater_lib::workflows::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("transcription.palmier");
    let now = "2026-10-04T00:00:00Z";
    let mut project = VideoProject::new_empty("project-speech".into(), "Speech".into(), now.into());
    // The media file does not exist, so the host's probe step fails before any model runs.
    project.media.push(MediaAsset {
        id: "media-1".into(),
        name: None,
        relative_path: "media/missing.mp4".into(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    let start = temporal_transcribe_media_start_request(
        &project.id,
        "opaque",
        "media-1",
        "transcribe-1",
        "en",
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        &project.id,
        "transcribe-1",
        JobStatus::Queued,
        now,
    );
    job.start_request = Some(start.clone());
    project.jobs.push(job);
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let request = |request_id: &str, start: &TemporalWorkflowStartRequest, revision: u64| {
        engine.execute(
            "workflow-session",
            &BTreeSet::from([AuthorizationScope::HostAdmin]),
            &serde_json::to_vec(&RpcEnvelope {
                request_id: request_id.into(),
                operation: "run_transcribe_media_in_process".into(),
                project_id: Some(id.clone()),
                expected_revision: Some(revision),
                editor_lease_token: Some("test-editor-lease".into()),
                payload: json!({"startRequest":start,"updatedAt":now}),
            })
            .unwrap(),
            100,
        )
    };

    let mut other = start.clone();
    other.input["languageMode"] = json!("de");
    let refused = request("transcription-other", &other, project.content_revision);
    assert!(!refused.ok);
    assert_eq!(
        load_split_project(&path).unwrap().jobs[0].status,
        JobStatus::Queued
    );

    let failed = request("transcription", &start, project.content_revision);
    assert!(!failed.ok, "a missing source cannot be transcribed");
    let saved = load_split_project(&path).unwrap();
    assert_eq!(saved.jobs[0].status, JobStatus::Failed);
    assert!(saved.jobs[0]
        .failure_reason
        .as_deref()
        .is_some_and(|reason| !reason.is_empty()));
    assert_eq!(
        saved.jobs[0]
            .workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("in-process/video-creater/project-speech/transcribe-media/transcribe-1")
    );
    assert!(saved.transcripts.is_empty());
}

#[test]
fn remote_project_bundle_export_completes_with_a_readable_project_package() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bundle.palmier");
    let project = video_creater_lib::project::model::VideoProject::new_empty(
        "project-bundle".into(),
        "Bundle".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let result = execute(
        &engine,
        "bundle",
        "export_palmier_project_package_to_split_project_folder",
        Some(&id),
        Some(project.content_revision),
        json!({"jobId":"bundle-job","outputPath":"exports/export.palmier","updatedAt":"2026-10-03T00:00:00Z"}),
    );
    assert_eq!(result["job"]["status"], "completed");
    assert_eq!(
        result["job"]["startRequest"]["input"]["profile"],
        "palmierProject"
    );
    assert_eq!(
        result["job"]["workflow"]["activityTypes"],
        result["job"]["startRequest"]["activityTypes"]
    );
    assert_eq!(
        load_split_project(&path.join("exports/export.palmier"))
            .unwrap()
            .id,
        project.id
    );
}

#[test]
fn remote_workflow_rejects_caller_authored_task_queues_before_connecting() {
    use video_creater_lib::project::model::JobStatus;
    use video_creater_lib::workflows::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workflows.palmier");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        &project.id,
        "queue-job",
        JobStatus::Queued,
        "2026-10-03T00:00:00Z",
    );
    let mut start = temporal_transcribe_media_start_request(
        &project.id,
        "opaque",
        &project.media[0].id,
        "queue-job",
        "auto",
    );
    start.task_queue = "unintended-worker".into();
    job.workflow.as_mut().unwrap().task_queue = start.task_queue.clone();
    job.start_request = Some(start);
    project.jobs.push(job.clone());
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let response = engine.execute(
        "session",
        &BTreeSet::from([AuthorizationScope::HostAdmin]),
        &serde_json::to_vec(&RpcEnvelope {
            request_id: "queue".into(),
            operation: "remote_start_temporal_workflow".into(),
            project_id: Some(id),
            expected_revision: Some(project.content_revision),
            editor_lease_token: Some("test-lease".into()),
            payload: json!({"job":job}),
        })
        .unwrap(),
        100,
    );
    assert_eq!(
        response.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::InvalidRequest
    );
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[cfg(not(feature = "temporal-worker"))]
#[test]
fn remote_workflow_admits_canonical_project_bundle_activity_plan() {
    use video_creater_lib::project::model::JobStatus;
    use video_creater_lib::workflows::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bundle.palmier");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "bundle-job",
        JobStatus::Queued,
        "2026-10-03T00:00:00Z",
    );
    let start = temporal_export_project_bundle_start_request(
        &project.id,
        "opaque",
        &job.id,
        "exports/export.palmier",
        true,
    );
    job.workflow.as_mut().unwrap().activity_types = start.activity_types.clone();
    job.start_request = Some(start);
    project.jobs.push(job.clone());
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(HostDispatcher::with_project_catalog(catalog)));
    let result = execute(
        &engine,
        "bundle-start",
        "remote_start_temporal_workflow",
        Some(&id),
        Some(project.content_revision),
        json!({"job":job}),
    );
    assert_eq!(result["status"], "unavailable");
    assert_eq!(load_split_project(&path).unwrap(), project);
}

#[cfg(not(feature = "temporal-worker"))]
#[test]
fn remote_reconciliation_reports_an_unavailable_runtime_without_changing_jobs() {
    use video_creater_lib::project::model::JobStatus;
    use video_creater_lib::workflows::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("reconcile.palmier");
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::TranscribeMedia,
        &project.id,
        "speech-job",
        JobStatus::Running,
        "2026-10-03T00:00:00Z",
    );
    job.start_request = Some(temporal_transcribe_media_start_request(
        &project.id,
        "opaque",
        &project.media[0].id,
        &job.id,
        "auto",
    ));
    job.workflow.as_mut().unwrap().run_id = Some("temporal-run".into());
    project.jobs.push(job);
    let project = save_split_project(&path, &project).unwrap().project;
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let id = catalog.id_for_path(&path).unwrap();
    let engine = RpcEngine::new(Arc::new(
        HostDispatcher::with_project_catalog(catalog)
            .with_settings_root(root.path().join("host-settings")),
    ));
    let result = execute(
        &engine,
        "reconcile",
        "reconcile_temporal_jobs_in_split_project_folder",
        Some(&id),
        Some(project.content_revision),
        json!({"updatedAt":"2026-10-03T01:00:00Z"}),
    );
    assert_eq!(result["serviceReachable"], false);
    assert!(result["project"].is_null());
    assert_eq!(load_split_project(&path).unwrap(), project);
}
