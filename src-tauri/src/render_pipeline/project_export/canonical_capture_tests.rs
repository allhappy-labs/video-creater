use super::*;
use crate::project::fixtures::sample_project;
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::split::{
    apply_project_actions_to_split_project_if_revision,
    apply_project_bookkeeping_actions_to_split_project_with_lease, load_split_project,
    save_split_project,
};
use image::GenericImageView;

fn fixture(disabled_video: bool) -> (tempfile::TempDir, VideoProject) {
    let root = tempfile::tempdir().unwrap();
    let mut project = sample_project();
    project.render_settings.width = 16;
    project.render_settings.height = 16;
    if disabled_video {
        // Keep a valid four-second timeline while the real sampler has no enabled
        // video to decode. No media fixture, subprocess, or render mock is needed.
        project.timeline.tracks[0].enabled = false;
    }
    let saved = save_split_project(root.path(), &project).unwrap().project;
    (root, saved)
}

#[test]
fn black_sampler_capture_preserves_revision_and_accepts_next_explicit_revision_edit() {
    let (root, before) = fixture(true);
    let result = super::super::render_prepared_preview_frame_to_split_project_folder(
        root.path(),
        0.5,
        "capture-revision-success",
        "2026-10-01",
    )
    .unwrap();
    assert_eq!(result.project.content_revision, before.content_revision);
    assert_eq!(result.project.timeline, before.timeline);
    assert_eq!(result.project.jobs[0].status, JobStatus::Completed);
    assert!(root.path().join(&result.preview_frame).is_file());
    assert_eq!(
        image::open(root.path().join(&result.preview_frame))
            .unwrap()
            .dimensions(),
        (16, 16)
    );
    let edited = apply_project_actions_to_split_project_if_revision(
        root.path(),
        vec![ProjectAction::UpdateProjectSettings {
            name: "Edit after canonical capture".into(),
            render_settings: before.render_settings,
        }],
        &before.id,
        before.content_revision,
    )
    .unwrap()
    .project;
    assert_eq!(edited.content_revision, before.content_revision + 1);
    assert_eq!(edited.jobs[0].status, JobStatus::Completed);
}

#[test]
fn failed_sampler_capture_preserves_editor_revision() {
    let (root, before) = fixture(false);
    // The fixture's active video points at absent media; failure is after job start.
    assert!(!root.path().join("media/input.mp4").exists());
    assert!(
        super::super::render_prepared_preview_frame_to_split_project_folder(
            root.path(),
            0.5,
            "capture-revision-failure",
            "2026-10-01",
        )
        .is_err()
    );
    let after = load_split_project(root.path()).unwrap();
    assert_eq!(after.jobs.len(), 1);
    assert_eq!(after.jobs[0].status, JobStatus::Failed);
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(after.timeline, before.timeline);
    assert!(after.render_reports.is_empty());
}

#[test]
fn capture_bookkeeping_cannot_admit_other_job_kinds() {
    let (root, before) = fixture(true);
    let job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &before.id,
        "ordinary-job",
        JobStatus::Queued,
        "2026-10-01",
    );
    let mutation = acquire_split_project_mutation_lease(root.path()).unwrap();
    assert!(
        apply_project_bookkeeping_actions_to_split_project_with_lease(
            root.path(),
            vec![ProjectAction::RecordJob { job: Box::new(job) }],
            &mutation,
        )
        .is_err()
    );
    assert_eq!(load_split_project(root.path()).unwrap(), before);
}

#[test]
fn sampler_failure_respects_cancellation_winner() {
    let (root, before) = fixture(true);
    let identity = CaptureIdentity::capture(root.path(), &before.id).unwrap();
    let job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &before.id,
        "capture-cancel-winner",
        JobStatus::Queued,
        "2026-10-01",
    );
    let mut job = job;
    job.kind = "captureCanonicalPreviewFrame".into();
    let attempt = register_project_render_attempt(root.path(), &before.id, &job.id, None).unwrap();
    let job_id = job.id.clone();
    identity
        .persist(
            root.path(),
            &job_id,
            &attempt.attempt_id,
            render_job_start_actions(&before, job, "2026-10-01", &attempt.attempt_id),
            false,
        )
        .unwrap();
    let cancellation = attempt.guard.token();
    assert_eq!(
        crate::render_pipeline::cancel::request_render_cancellation_by_locator(
            root.path(),
            &job_id,
            &attempt.attempt_id,
        ),
        crate::render_pipeline::cancel::RenderCancellationOutcome::Requested,
    );
    let capture = CaptureRequest {
        project_dir: root.path(),
        source_project: &before,
        playhead_seconds: 0.5,
        frame_end_seconds: 0.6,
        job_id: &job_id,
        attempt_id: &attempt.attempt_id,
        updated_at: "2026-10-01",
        cancellation: &cancellation,
        identity: &identity,
    };
    // Exercise the same failure handler used after a cancelled sampler operation.
    persist_capture_failure(&capture);
    let after = load_split_project(root.path()).unwrap();
    assert_eq!(after.jobs[0].status, JobStatus::Cancelled);
    assert_eq!(after.content_revision, before.content_revision);
    // A later failure notification must leave the terminal winner intact.
    persist_capture_failure(&capture);
    assert_eq!(load_split_project(root.path()).unwrap(), after);
}

fn capture_job(project: &VideoProject, job_id: &str) -> JobSummary {
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &project.id,
        job_id,
        JobStatus::Queued,
        "2026-10-01",
    );
    job.kind = "captureCanonicalPreviewFrame".into();
    job
}

#[test]
#[cfg(unix)]
fn capture_start_rejects_replaced_package_with_same_project_id() {
    let (root, before) = fixture(true);
    let identity = CaptureIdentity::capture(root.path(), &before.id).unwrap();
    let quarantine = tempfile::tempdir().unwrap();
    std::fs::rename(root.path(), quarantine.path().join("original-package")).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    let replacement = save_split_project(root.path(), &before).unwrap().project;
    assert_eq!(replacement.id, before.id);
    let job = capture_job(&before, "capture-replaced-package");
    let error = identity
        .persist(
            root.path(),
            &job.id,
            "old-attempt",
            render_job_start_actions(&before, job.clone(), "2026-10-01", "old-attempt"),
            false,
        )
        .unwrap_err();
    assert!(error.contains("package changed"), "{error}");
    assert_eq!(load_split_project(root.path()).unwrap(), replacement);
}

#[test]
fn capture_terminal_persistence_rejects_superseded_or_cancelled_attempt() {
    for cancelled in [false, true] {
        let (root, before) = fixture(true);
        let identity = CaptureIdentity::capture(root.path(), &before.id).unwrap();
        let job = capture_job(&before, "capture-stale-attempt");
        identity
            .persist(
                root.path(),
                &job.id,
                "old-attempt",
                render_job_start_actions(&before, job.clone(), "2026-10-01", "old-attempt"),
                false,
            )
            .unwrap();
        let mutation = acquire_split_project_mutation_lease(root.path()).unwrap();
        let current = apply_project_bookkeeping_actions_to_split_project_with_lease(
            root.path(),
            vec![ProjectAction::UpdateJobStatus {
                job_id: job.id.clone(),
                status: if cancelled {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Running
                },
                updated_at: "2026-10-02".into(),
                run_id: Some(
                    if cancelled {
                        "old-attempt"
                    } else {
                        "new-attempt"
                    }
                    .into(),
                ),
            }],
            &mutation,
        )
        .unwrap()
        .project;
        drop(mutation);
        let error = identity
            .persist(
                root.path(),
                &job.id,
                "old-attempt",
                vec![ProjectAction::UpdateJobStatus {
                    job_id: job.id.clone(),
                    status: JobStatus::Failed,
                    updated_at: "2026-10-03".into(),
                    run_id: Some("old-attempt".into()),
                }],
                true,
            )
            .unwrap_err();
        assert!(error.contains("terminal or superseded"), "{error}");
        assert_eq!(load_split_project(root.path()).unwrap(), current);
        assert_eq!(current.content_revision, before.content_revision);
    }
}
