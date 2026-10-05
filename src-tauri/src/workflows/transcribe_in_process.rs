//! Runs a recorded `transcribe_media` job in this process. It sequences the same probe, run and
//! store activities as `VideoCreaterTranscribeMediaWorkflow`, for builds and settings without a
//! Temporal worker. The job is marked running under an `in-process/` run id, completed by the store
//! activity, and failed with its reason when a step fails or an earlier session left it unfinished.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use super::{
    temporal_start_result_action, temporal_transcribe_probe_media_activity_value,
    temporal_transcribe_run_activity_value, temporal_transcribe_store_activity_value,
    temporal_transcribe_workflow_activity_plan_value, TemporalWorkflowInputError,
    TemporalWorkflowKind,
};
use crate::project::action::ProjectAction;
use crate::project::model::{JobStatus, JobSummary, TemporalWorkflowStartRequest, VideoProject};
use crate::project::split::{apply_project_actions_to_split_project, load_split_project};
use crate::transcription::job::TemporalTranscribeMediaWorkflowInput;

/// The run id prefix of a transcription this process runs itself.
pub const IN_PROCESS_TRANSCRIPTION_RUN_ID_PREFIX: &str = "in-process/";
/// The failure reason of a transcription an earlier app session left running.
pub const TRANSCRIPTION_INTERRUPTED_REASON: &str =
    "Transcription stopped when the app closed. Start it again.";

type ActivityResult = Result<Value, TemporalWorkflowInputError>;

fn active_transcriptions() -> &'static Mutex<HashSet<(String, String)>> {
    static ACTIVE: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Holds one job's place in the process-wide registry until the run ends, however it ends.
struct ActiveTranscription {
    key: (String, String),
}

impl ActiveTranscription {
    fn register(project_id: &str, job_id: &str) -> Option<Self> {
        let key = (project_id.to_string(), job_id.to_string());
        let inserted = active_transcriptions()
            .lock()
            .expect("active transcription registry mutex")
            .insert(key.clone());
        // Built only for the run that owns the entry: dropping a guard removes it.
        inserted.then(|| Self { key })
    }
}

impl Drop for ActiveTranscription {
    fn drop(&mut self) {
        if let Ok(mut active) = active_transcriptions().lock() {
            active.remove(&self.key);
        }
    }
}

/// Whether this process is running the job right now.
pub fn is_transcription_job_active(project_id: &str, job_id: &str) -> bool {
    active_transcriptions()
        .lock()
        .expect("active transcription registry mutex")
        .contains(&(project_id.to_string(), job_id.to_string()))
}

/// Runs the recorded transcription job to its end and returns the saved project. The project folder
/// is the one the request names.
pub fn run_transcribe_media_in_process(
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
) -> Result<VideoProject, TemporalWorkflowInputError> {
    let input = workflow_input(start_request)?;
    run_transcribe_media_in_process_at(Path::new(&input.project_dir), start_request, updated_at)
}

/// [`run_transcribe_media_in_process`] for a caller that has resolved the project folder itself,
/// such as the remote host, whose recorded requests name a catalog locator instead of a path.
pub fn run_transcribe_media_in_process_at(
    project_dir: &Path,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
) -> Result<VideoProject, TemporalWorkflowInputError> {
    run_transcribe_media_in_process_with_activities(
        project_dir,
        start_request,
        updated_at,
        temporal_transcribe_probe_media_activity_value,
        temporal_transcribe_run_activity_value,
    )
}

fn workflow_input(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<TemporalTranscribeMediaWorkflowInput, TemporalWorkflowInputError> {
    serde_json::from_value(start_request.input.clone())
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}

/// [`run_transcribe_media_in_process_at`] with the probe and run activities supplied by the caller.
pub fn run_transcribe_media_in_process_with_activities(
    project_dir: &Path,
    start_request: &TemporalWorkflowStartRequest,
    updated_at: &str,
    probe: impl FnOnce(Value) -> ActivityResult,
    run: impl FnOnce(Value) -> ActivityResult,
) -> Result<VideoProject, TemporalWorkflowInputError> {
    if start_request.workflow_type != TemporalWorkflowKind::TranscribeMedia.workflow_type() {
        return Err(mismatch("workflowType"));
    }
    let input = workflow_input(start_request)?;
    let project = load_project(project_dir)?;
    if project.id != input.project_id {
        return Err(mismatch("projectId"));
    }
    let job = recorded_job(&project, &input.job_id, start_request)?;
    let Some(_active) = ActiveTranscription::register(&project.id, &job.id) else {
        return Err(TemporalWorkflowInputError::Transcription(
            "this transcription is already running".to_string(),
        ));
    };
    if job.status != JobStatus::Queued {
        return Err(TemporalWorkflowInputError::Transcription(format!(
            "the transcription job is {}, not queued",
            job_status_name(&job.status)
        )));
    }

    let run_id = format!(
        "{IN_PROCESS_TRANSCRIPTION_RUN_ID_PREFIX}{}",
        start_request.workflow_id
    );
    let start_action = temporal_start_result_action(job, &run_id, updated_at)
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))?;
    apply_actions(project_dir, vec![start_action])?;

    let outcome = (|| {
        // The activities read and write the resolved folder, whatever locator the request recorded.
        let mut activity_input = start_request.input.clone();
        activity_input["projectDir"] = json!(project_dir.display().to_string());
        let plan = temporal_transcribe_workflow_activity_plan_value(
            activity_input,
            updated_at,
            Some(&run_id),
        )?;
        let mut store_input = run(probe(plan["probeMediaInput"].clone())?)?;
        let Some(store_object) = store_input.as_object_mut() else {
            return Err(TemporalWorkflowInputError::Transcription(
                "RunTranscription output must be a JSON object".to_string(),
            ));
        };
        store_object.insert(
            "updatedAt".to_string(),
            json!(chrono::Utc::now().to_rfc3339()),
        );
        store_object.insert("runId".to_string(), json!(run_id));
        temporal_transcribe_store_activity_value(store_input)
    })();

    if let Err(error) = outcome {
        // The job must not stay running; the step's own error is the one worth reporting.
        let _ = apply_actions(
            project_dir,
            vec![ProjectAction::RecordJobFailure {
                job_id: input.job_id.clone(),
                reason: error.to_string(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                run_id: Some(run_id),
            }],
        );
        return Err(error);
    }
    load_project(project_dir)
}

/// Fails transcription jobs that an earlier app session left running in process. A job this
/// process is running, or one a Temporal worker owns, is left alone. Returns the failed job ids.
pub fn fail_interrupted_in_process_transcriptions(
    project_dir: &Path,
    updated_at: &str,
) -> Result<Vec<String>, TemporalWorkflowInputError> {
    let project = load_project(project_dir)?;
    let interrupted: Vec<&JobSummary> = project
        .jobs
        .iter()
        .filter(|job| {
            job.kind == TemporalWorkflowKind::TranscribeMedia.job_kind()
                && matches!(job.status, JobStatus::Running | JobStatus::Progress)
                && job
                    .workflow
                    .as_ref()
                    .and_then(|workflow| workflow.run_id.as_deref())
                    .is_some_and(|run_id| {
                        run_id.starts_with(IN_PROCESS_TRANSCRIPTION_RUN_ID_PREFIX)
                    })
                && !is_transcription_job_active(&project.id, &job.id)
        })
        .collect();
    if interrupted.is_empty() {
        return Ok(Vec::new());
    }
    let actions = interrupted
        .iter()
        .map(|job| ProjectAction::RecordJobFailure {
            job_id: job.id.clone(),
            reason: TRANSCRIPTION_INTERRUPTED_REASON.to_string(),
            updated_at: updated_at.to_string(),
            run_id: None,
        })
        .collect();
    let failed = interrupted.iter().map(|job| job.id.clone()).collect();
    apply_actions(project_dir, actions)?;
    Ok(failed)
}

fn recorded_job<'project>(
    project: &'project VideoProject,
    job_id: &str,
    start_request: &TemporalWorkflowStartRequest,
) -> Result<&'project JobSummary, TemporalWorkflowInputError> {
    let job = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| mismatch("jobId"))?;
    if job.kind != TemporalWorkflowKind::TranscribeMedia.job_kind() {
        return Err(mismatch("kind"));
    }
    // The run uses the request the editor recorded, never a different one supplied later.
    if job.start_request.as_ref() != Some(start_request) {
        return Err(mismatch("startRequest"));
    }
    Ok(job)
}

fn job_status_name(status: &JobStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "in an unknown state".to_string())
}

fn mismatch(field: &str) -> TemporalWorkflowInputError {
    TemporalWorkflowInputError::MismatchedInputField(field.to_string())
}

fn load_project(project_dir: &Path) -> Result<VideoProject, TemporalWorkflowInputError> {
    load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))
}

fn apply_actions(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
) -> Result<(), TemporalWorkflowInputError> {
    apply_project_actions_to_split_project(project_dir, actions)
        .map(|_| ())
        .map_err(|error| TemporalWorkflowInputError::ApplyProjectActions(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::super::{
        temporal_job_summary, temporal_transcribe_media_start_request,
        temporal_transcribe_run_activity_value_with_backend, transcribe_run_artifact_dir,
    };
    use super::*;
    use crate::project::model::{MediaAsset, MediaKind};
    use crate::project::split::save_split_project;
    use crate::transcription::runtime::{
        TranscriptToken, TranscriptionBackend, TranscriptionRuntimeError, TranscriptionRuntimeJob,
        TranscriptionRuntimeOutput,
    };

    const MODEL_ID: &str = "nvidia/parakeet-tdt-0.6b-v3";
    const RUNTIME_ID: &str = "sherpa_onnx";
    const UPDATED_AT: &str = "2026-10-04T10:00:00Z";

    struct OneWordBackend;

    impl TranscriptionBackend for OneWordBackend {
        fn transcribe(
            &self,
            _job: &TranscriptionRuntimeJob,
            model_id: &str,
        ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
            Ok(TranscriptionRuntimeOutput {
                runtime_id: RUNTIME_ID.to_string(),
                model_id: model_id.to_string(),
                tokens: vec![TranscriptToken {
                    token: "Hello".to_string(),
                    start: 0.1,
                    end: 0.4,
                    confidence: None,
                }],
            })
        }
    }

    /// A saved project with one media asset and a queued transcription job for it.
    fn queued_project(
        project_id: &str,
        job_id: &str,
    ) -> (tempfile::TempDir, TemporalWorkflowStartRequest) {
        let root = tempfile::tempdir().expect("project dir");
        let mut project = VideoProject::new_empty(
            project_id.to_string(),
            "Transcribe".to_string(),
            UPDATED_AT.to_string(),
        );
        project.media.push(MediaAsset {
            id: "media-1".to_string(),
            name: None,
            relative_path: "media/source.mp4".to_string(),
            kind: MediaKind::Video,
            duration_seconds: 12.0,
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            folder_id: None,
        });
        let start_request = temporal_transcribe_media_start_request(
            project_id,
            &root.path().display().to_string(),
            "media-1",
            job_id,
            "en",
        );
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::TranscribeMedia,
            project_id,
            job_id,
            JobStatus::Queued,
            UPDATED_AT,
        );
        job.start_request = Some(start_request.clone());
        project.jobs.push(job);
        save_split_project(root.path(), &project).expect("save project");
        (root, start_request)
    }

    fn ready_probe(input: Value) -> ActivityResult {
        let project_dir = input["projectDir"]
            .as_str()
            .expect("project dir")
            .to_string();
        let job_id = input["jobId"].as_str().expect("job id").to_string();
        Ok(json!({
            "status": "ready",
            "projectId": input["projectId"],
            "projectDir": project_dir,
            "mediaId": input["mediaId"],
            "jobId": job_id,
            "languageMode": input["languageMode"],
            "sourcePath": format!("{project_dir}/media/source.mp4"),
            "artifactPath": format!("{project_dir}/transcripts/{job_id}-transcript.json"),
            "mediaKind": "video",
            "mediaRelativePath": "media/source.mp4",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": 1920,
            "mediaHeight": 1080,
            "mediaFps": 30.0,
            "modelId": MODEL_ID,
            "modelPath": "/tmp/model",
            "runtimeId": RUNTIME_ID
        }))
    }

    fn one_word_run(input: Value) -> ActivityResult {
        let artifact_dir = transcribe_run_artifact_dir(
            Path::new(input["projectDir"].as_str().expect("project dir")),
            input["jobId"].as_str().expect("job id"),
        )?;
        temporal_transcribe_run_activity_value_with_backend(input, &OneWordBackend, &artifact_dir)
    }

    fn job<'project>(project: &'project VideoProject, job_id: &str) -> &'project JobSummary {
        project
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .expect("job")
    }

    #[test]
    fn a_queued_transcription_runs_to_a_stored_transcript_and_a_completed_job() {
        let (root, start_request) = queued_project("in-process-ok", "transcribe-ok");

        let project = run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            ready_probe,
            one_word_run,
        )
        .expect("transcription");

        let completed = job(&project, "transcribe-ok");
        assert_eq!(completed.status, JobStatus::Completed);
        assert_eq!(
            completed
                .workflow
                .as_ref()
                .and_then(|workflow| workflow.run_id.as_deref()),
            Some("in-process/video-creater/in-process-ok/transcribe-media/transcribe-ok")
        );
        assert_eq!(project.transcripts.len(), 1);
        assert_eq!(project.transcripts[0].media_id, "media-1");
        assert_eq!(project.transcripts[0].words.len(), 1);
        assert_eq!(
            project,
            load_split_project(root.path()).expect("saved project")
        );
        assert!(!is_transcription_job_active(
            "in-process-ok",
            "transcribe-ok"
        ));
    }

    #[test]
    fn a_request_recorded_with_a_catalog_locator_runs_in_the_resolved_folder() {
        let (root, _) = queued_project("in-process-locator", "transcribe-locator");
        let mut project = load_split_project(root.path()).expect("project");
        let mut start_request = project.jobs[0]
            .start_request
            .clone()
            .expect("start request");
        start_request.input["projectDir"] = json!("catalog-locator");
        project.jobs[0].start_request = Some(start_request.clone());
        save_split_project(root.path(), &project).expect("save project");

        let project = run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            |input| {
                assert_eq!(
                    input["projectDir"],
                    json!(root.path().display().to_string())
                );
                ready_probe(input)
            },
            one_word_run,
        )
        .expect("transcription");

        assert_eq!(
            job(&project, "transcribe-locator").status,
            JobStatus::Completed
        );
        assert_eq!(project.transcripts.len(), 1);
    }

    #[test]
    fn a_failed_step_fails_the_job_with_its_reason_and_stores_no_transcript() {
        let (root, start_request) = queued_project("in-process-fail", "transcribe-fail");

        let error = run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            ready_probe,
            |_| {
                Err(TemporalWorkflowInputError::Transcription(
                    "the model is not installed".to_string(),
                ))
            },
        )
        .expect_err("run failure");

        assert_eq!(
            error.to_string(),
            "transcribe media workflow failed: the model is not installed"
        );
        let project = load_split_project(root.path()).expect("saved project");
        let failed = job(&project, "transcribe-fail");
        assert_eq!(failed.status, JobStatus::Failed);
        assert_eq!(
            failed.failure_reason.as_deref(),
            Some("transcribe media workflow failed: the model is not installed")
        );
        assert!(project.transcripts.is_empty());
        assert!(!is_transcription_job_active(
            "in-process-fail",
            "transcribe-fail"
        ));
    }

    #[test]
    fn a_job_already_running_in_this_process_is_not_started_twice() {
        let (root, start_request) = queued_project("in-process-twice", "transcribe-twice");
        let mut nested = None;

        run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            |input| {
                nested = Some(run_transcribe_media_in_process_with_activities(
                    root.path(),
                    &start_request,
                    UPDATED_AT,
                    ready_probe,
                    one_word_run,
                ));
                ready_probe(input)
            },
            one_word_run,
        )
        .expect("first run");

        let error = nested.expect("nested attempt").expect_err("second start");
        assert_eq!(
            error.to_string(),
            "transcribe media workflow failed: this transcription is already running"
        );
        let project = load_split_project(root.path()).expect("saved project");
        assert_eq!(
            job(&project, "transcribe-twice").status,
            JobStatus::Completed
        );
        assert_eq!(project.transcripts.len(), 1);
    }

    #[test]
    fn a_finished_job_and_a_different_request_are_refused_without_changing_the_project() {
        let (root, start_request) = queued_project("in-process-refuse", "transcribe-refuse");
        let mut other = start_request.clone();
        other.input["languageMode"] = json!("de");
        let before = load_split_project(root.path()).expect("project");

        let error = run_transcribe_media_in_process_with_activities(
            root.path(),
            &other,
            UPDATED_AT,
            ready_probe,
            one_word_run,
        )
        .expect_err("different request");
        assert!(
            matches!(error, TemporalWorkflowInputError::MismatchedInputField(field) if field == "startRequest")
        );
        assert_eq!(
            before,
            load_split_project(root.path()).expect("unchanged project")
        );

        run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            ready_probe,
            one_word_run,
        )
        .expect("first run");
        let error = run_transcribe_media_in_process_with_activities(
            root.path(),
            &start_request,
            UPDATED_AT,
            ready_probe,
            one_word_run,
        )
        .expect_err("finished job");
        assert_eq!(
            error.to_string(),
            "transcribe media workflow failed: the transcription job is completed, not queued"
        );
        let project = load_split_project(root.path()).expect("saved project");
        assert_eq!(
            job(&project, "transcribe-refuse").status,
            JobStatus::Completed
        );
    }

    #[test]
    fn a_transcription_left_running_by_an_earlier_session_is_failed_on_open() {
        let (root, start_request) = queued_project("in-process-stale", "transcribe-stale");
        let project = load_split_project(root.path()).expect("project");
        let run_id = format!("in-process/{}", start_request.workflow_id);
        let started =
            temporal_start_result_action(job(&project, "transcribe-stale"), &run_id, UPDATED_AT)
                .expect("start action");
        apply_actions(root.path(), vec![started]).expect("mark running");

        // A run that is live in this process is not interrupted work.
        let live = ActiveTranscription::register("in-process-stale", "transcribe-stale")
            .expect("register");
        assert!(
            fail_interrupted_in_process_transcriptions(root.path(), UPDATED_AT)
                .expect("live check")
                .is_empty()
        );
        drop(live);

        let failed =
            fail_interrupted_in_process_transcriptions(root.path(), "2026-10-04T11:00:00Z")
                .expect("recovery");
        assert_eq!(failed, vec!["transcribe-stale".to_string()]);
        let project = load_split_project(root.path()).expect("saved project");
        let stale = job(&project, "transcribe-stale");
        assert_eq!(stale.status, JobStatus::Failed);
        assert_eq!(
            stale.failure_reason.as_deref(),
            Some(TRANSCRIPTION_INTERRUPTED_REASON)
        );
        assert!(
            fail_interrupted_in_process_transcriptions(root.path(), UPDATED_AT)
                .expect("second pass")
                .is_empty()
        );
    }

    #[test]
    fn queued_jobs_and_temporal_runs_are_not_treated_as_interrupted() {
        let (root, start_request) = queued_project("in-process-keep", "transcribe-keep");
        assert!(
            fail_interrupted_in_process_transcriptions(root.path(), UPDATED_AT)
                .expect("queued job")
                .is_empty()
        );

        let project = load_split_project(root.path()).expect("project");
        let started = temporal_start_result_action(
            job(&project, "transcribe-keep"),
            "temporal-run-1",
            UPDATED_AT,
        )
        .expect("start action");
        apply_actions(root.path(), vec![started]).expect("mark running");
        assert!(
            fail_interrupted_in_process_transcriptions(root.path(), UPDATED_AT)
                .expect("temporal run")
                .is_empty()
        );
        let project = load_split_project(root.path()).expect("saved project");
        assert_eq!(job(&project, "transcribe-keep").status, JobStatus::Running);
        let _ = start_request;
    }
}
