//! Job bookkeeping written after an agent apply (frame capture, job status,
//! render reports, export artifacts) neither blocks Undo nor is erased by it.

use super::support::{
    agent_history_json, conversation_fixture_project, folder_snapshot, saved_split_project,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    CodexConversationApplyRequest, CodexConversationApplyResult, CodexConversationEditProposal,
};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{
    JobStatus, JobSummary, ProjectExportArtifact, ProjectExportArtifactKind, ProjectRenderReport,
    RenderReportCheckStatus, RenderReportStatus, RenderReportStreams, VideoProject,
};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
    undo_latest_agent_project_batch, ProjectAgentUndoOutcome,
};

fn apply(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    review_approved: bool,
) -> CodexConversationApplyResult {
    let project = load_split_project(project_dir).expect("load project");
    let proposal = CodexConversationEditProposal {
        summary: "Lowered the interview.".to_string(),
        edl: Vec::new(),
        project_actions: actions,
        render_review: None,
    };
    let prepared =
        prepare_codex_conversation_proposal(&project, &proposal).expect("proposal prepares");
    apply_codex_conversation_proposal(
        project_dir,
        CodexConversationApplyRequest {
            proposal,
            action_ids: prepared.action_ids,
            review_approved,
            session_id: None,
        },
    )
    .expect("apply")
}

fn volume_action() -> ProjectAction {
    ProjectAction::UpdateAudioVolume {
        item_id: "audio-1".to_string(),
        volume_db: Some(-2.0),
    }
}

fn job(id: &str) -> JobSummary {
    JobSummary {
        id: id.to_string(),
        kind: "captureCanonicalPreviewFrame".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-09-15T10:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

fn render_report(id: &str) -> ProjectRenderReport {
    let output_path = format!("renders/{id}/output.mov");
    let checks = [
        "duration",
        "streams",
        "captionAlignment",
        "overlayTiming",
        "visualFrameEvidence",
        "artifactPaths",
        "logPath",
    ]
    .into_iter()
    .map(|check| (check.to_string(), RenderReportCheckStatus::Passed))
    .collect::<BTreeMap<_, _>>();
    ProjectRenderReport {
        schema_version: 1,
        id: id.to_string(),
        status: RenderReportStatus::Completed,
        output_path: output_path.clone(),
        duration_seconds: 0.04,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: false,
        },
        checks,
        artifacts: vec![output_path],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: format!("logs/{id}.log"),
        created_at: "2026-09-15T10:00:01Z".to_string(),
    }
}

fn export_artifact(id: &str) -> ProjectExportArtifact {
    ProjectExportArtifact {
        schema_version: 1,
        id: id.to_string(),
        kind: ProjectExportArtifactKind::NleXml,
        format: "premiereXmeml".to_string(),
        path: format!("exports/{id}.xml"),
        mime_type: "application/xml".to_string(),
        job_id: Some(id.to_string()),
        created_at: "2026-09-15T10:00:02Z".to_string(),
    }
}

/// What the result-frame capture writes right after an apply: a job, its
/// completion, and a render report.
fn simulate_frame_capture(project_dir: &Path, id: &str) {
    apply_project_actions_to_split_project(
        project_dir,
        vec![
            ProjectAction::RecordJob {
                job: Box::new(job(id)),
            },
            ProjectAction::UpdateJobStatus {
                job_id: id.to_string(),
                status: JobStatus::Completed,
                updated_at: "2026-09-15T10:00:01Z".to_string(),
                run_id: None,
            },
            ProjectAction::AttachRenderReport {
                report: render_report(id),
            },
        ],
    )
    .expect("frame capture bookkeeping");
}

fn record_export(project_dir: &Path, id: &str) {
    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::RecordExportArtifact {
            artifact: export_artifact(id),
        }],
    )
    .expect("export artifact bookkeeping");
}

fn ids<T>(records: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
    records
        .iter()
        .map(|record| id(record).to_string())
        .collect()
}

fn assert_undone(outcome: ProjectAgentUndoOutcome, project_dir: &Path) -> VideoProject {
    let ProjectAgentUndoOutcome::Undone { project, .. } = outcome else {
        panic!("expected undone outcome, got {outcome:?}");
    };
    let persisted = load_split_project(project_dir).expect("load undone project");
    assert_eq!(*project, persisted);
    persisted
}

/// The content hash rule shipped by 54f8a19d, kept here to pin old entries.
fn old_rule_hash(project: &VideoProject) -> String {
    let mut content = project.clone();
    content.content_revision = 0;
    content.codex_thread_id = None;
    content.updated_at = String::new();
    let digest = Sha256::digest(serde_json::to_vec(&content).expect("project json"));
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn rewrite_history(project_dir: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let mut history = agent_history_json(project_dir);
    edit(&mut history);
    std::fs::write(
        project_dir.join("context").join("agent-history.json"),
        serde_json::to_vec_pretty(&history).expect("history json"),
    )
    .expect("write history");
}

#[test]
fn undo_succeeds_after_frame_capture_and_export_bookkeeping_and_keeps_those_records() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let applied = apply(&project_dir, vec![volume_action()], false);
    simulate_frame_capture(&project_dir, "capture-frame-1");
    record_export(&project_dir, "export-1");

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo");

    let restored = assert_undone(outcome, &project_dir);
    assert_eq!(restored.timeline, before.timeline);
    assert_eq!(restored.timelines, before.timelines);
    assert_eq!(ids(&restored.jobs, |job| &job.id), vec!["capture-frame-1"]);
    assert_eq!(restored.jobs[0].status, JobStatus::Completed);
    assert_eq!(
        ids(&restored.render_reports, |report| &report.id),
        vec!["capture-frame-1"]
    );
    assert_eq!(
        ids(&restored.export_artifacts, |artifact| &artifact.id),
        vec!["export-1"]
    );
    assert_eq!(agent_history_json(&project_dir)["entries"], json!([]));
}

#[test]
fn undo_still_conflicts_after_a_content_edit_alongside_bookkeeping() {
    let (_dir, project_dir, _before) = saved_split_project(conversation_fixture_project());
    let applied = apply(&project_dir, vec![volume_action()], false);
    simulate_frame_capture(&project_dir, "capture-frame-1");
    let mut edited = load_split_project(&project_dir).expect("load");
    edited.timeline.tracks[0].items[0].label = "Manual edit".to_string();
    save_split_project(&project_dir, &edited).expect("user edit");
    let snapshot = folder_snapshot(&project_dir);

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo outcome");

    let ProjectAgentUndoOutcome::Conflict { entry_id, message } = outcome else {
        panic!("expected conflict, got {outcome:?}");
    };
    assert_eq!(entry_id, applied.history_entry_id);
    assert!(message.contains("changed after this edit was applied"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn undo_removes_exactly_the_bookkeeping_the_batch_itself_recorded() {
    let (_dir, project_dir, _before) = saved_split_project(conversation_fixture_project());
    simulate_frame_capture(&project_dir, "capture-frame-0");
    let before = load_split_project(&project_dir).expect("load");
    let applied = apply(
        &project_dir,
        vec![
            volume_action(),
            ProjectAction::RecordJob {
                job: Box::new(job("agent-job-1")),
            },
            ProjectAction::AttachRenderReport {
                report: render_report("agent-report-1"),
            },
            ProjectAction::RecordExportArtifact {
                artifact: export_artifact("agent-export-1"),
            },
        ],
        true,
    );
    simulate_frame_capture(&project_dir, "capture-frame-1");

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo");

    let restored = assert_undone(outcome, &project_dir);
    assert_eq!(restored.timeline, before.timeline);
    assert_eq!(
        ids(&restored.jobs, |job| &job.id),
        vec!["capture-frame-0", "capture-frame-1"]
    );
    assert_eq!(
        ids(&restored.render_reports, |report| &report.id),
        vec!["capture-frame-0", "capture-frame-1"]
    );
    assert!(restored.export_artifacts.is_empty());
}

#[test]
fn old_rule_hash_entries_without_a_rule_version_still_undo() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let applied = apply(
        &project_dir,
        vec![
            volume_action(),
            ProjectAction::RecordJob {
                job: Box::new(job("agent-job-1")),
            },
        ],
        true,
    );
    let after = load_split_project(&project_dir).expect("load applied");
    rewrite_history(&project_dir, |history| {
        let entry = history["entries"][0].as_object_mut().expect("entry");
        let keys = entry.keys().cloned().collect::<Vec<_>>();
        assert!(keys.contains(&"afterContentHashVersion".to_string()));
        entry.retain(|key, _| {
            !matches!(
                key.as_str(),
                "afterContentHashVersion" | "addedBookkeepingIds"
            )
        });
        entry.insert("afterContentHash".to_string(), json!(old_rule_hash(&after)));
    });

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo");

    let restored = assert_undone(outcome, &project_dir);
    assert_eq!(restored.timeline, before.timeline);
    assert!(restored.jobs.is_empty(), "the batch's own job is removed");
}

#[test]
fn legacy_full_snapshot_entries_tolerate_later_bookkeeping() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let mut changed = before.clone();
    changed.name = "Agent edit".to_string();
    let after = save_split_project(&project_dir, &changed)
        .expect("save after")
        .project;
    rewrite_history(&project_dir, |history| {
        *history = json!({
            "schemaVersion": 1,
            "entries": [{ "id": "agent-edit-1", "actionCount": 1, "before": before, "after": after }],
        });
    });
    simulate_frame_capture(&project_dir, "capture-frame-1");

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");

    let restored = assert_undone(outcome, &project_dir);
    assert_eq!(restored.name, before.name);
    assert_eq!(ids(&restored.jobs, |job| &job.id), vec!["capture-frame-1"]);
    assert_eq!(
        ids(&restored.render_reports, |report| &report.id),
        vec!["capture-frame-1"]
    );
}
