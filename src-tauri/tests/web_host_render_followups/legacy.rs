use super::helpers::*;
use axum::http::StatusCode;
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{JobStatus, VideoProject};
use video_creater_lib::project::mutation::acquire_split_project_artifact_lease;
use video_creater_lib::project::split::load_split_project;
use video_creater_lib::render_pipeline::cancel::{
    request_render_cancellation_by_locator, RenderCancellationOutcome,
};
use video_creater_lib::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    MediaRenderAdmission, MediaRenderAttempt,
};
use video_creater_lib::settings::storage::acquire_storage_mutation_lease;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_host_pending_worker_allows_authenticated_operations_beyond_editor_ttl() {
    let host = Host::new("host-pending-ttl");
    let first = host.pair("First editor").await.unwrap();
    let second = host.pair("Second editor").await.unwrap();
    let lease = host.lease(&first, false).await.unwrap();
    let token = lease["editorLeaseToken"].as_str().unwrap();
    let initial_expiry = lease["expiresAt"].as_u64().unwrap();
    let job = "host-pending-job";
    let admission_request = host.admission(
        "pending-admission",
        job,
        token,
        host.project.content_revision,
    );
    let bytes = serde_json::to_value(&admission_request).unwrap();
    assert_eq!(
        host.request("/api/v1/rpc", None, bytes.clone())
            .await
            .unwrap()
            .0,
        StatusCode::UNAUTHORIZED
    );
    let invalid_csrf = Browser {
        csrf: "invalid".into(),
        ..first.clone()
    };
    assert_eq!(
        host.request("/api/v1/rpc", Some(&invalid_csrf), bytes)
            .await
            .unwrap()
            .0,
        StatusCode::FORBIDDEN
    );
    let mut wrong_lease = admission_request.clone();
    wrong_lease.editor_lease_token = Some("wrong-token".into());
    assert_eq!(
        host.rpc(&first, &wrong_lease).await.unwrap().0,
        StatusCode::CONFLICT
    );
    assert!(load_split_project(&host.path).unwrap().jobs.is_empty());

    let storage = acquire_storage_mutation_lease().unwrap();
    let started = Instant::now();
    // Collect failures, release storage, then await the production worker before asserting.
    // Timeout cannot abort spawn_blocking handlers; retaining the lock during panic/runtime
    // shutdown would deadlock a genuinely regressed admission handler.
    let phase: Check<()> = async {
        let admission: MediaRenderAdmission = decode(host.success(&first, &admission_request).await?)?;
        require(admission.admission_protocol == 1, "wrong admission protocol")?;
        require(admission.source_revision == host.project.content_revision, "wrong admitted revision")?;
        let recovery: MediaRenderAttempt = decode(host.success(&first, &host.envelope("recover-live-attempt", "recover_render_attempt_in_split_project_folder", Some(token), None, json!({"jobId": job, "attemptId": format!("render-attempt/{job}")}))).await?)?;
        require(recovery == MediaRenderAttempt::Pending, "recovery must not interrupt or restart a live pinned worker")?;
        let mut cycle = 0;
        while started.elapsed() < Duration::from_secs(31) {
            require(host.attempt(&first, job, cycle).await? == MediaRenderAttempt::Pending, "worker must remain pending behind storage ownership")?;
            let renewed = host.lease(&first, false).await?;
            require(renewed["editorLeaseToken"] == lease["editorLeaseToken"], "renewal replaced editor ownership")?;
            for (label, project_id) in [("active", &host.project_id), ("unrelated", &host.other_id)] {
                for operation in ["read_project_snapshot_from_split_project_folder", "load_job_progress_from_split_project_folder"] {
                    let mut request = host.envelope(&format!("{label}-{cycle}-{operation}"), operation, None, None, json!({}));
                    request.project_id = Some(project_id.clone());
                    let value = host.success(&first, &request).await?;
                    if operation == "read_project_snapshot_from_split_project_folder" {
                        let snapshot: VideoProject = decode(value)?;
                        if label == "active" {
                            require(snapshot.id == host.project.id, "snapshot resolved the wrong project")?;
                            require(snapshot.jobs.iter().any(|candidate| candidate.id == job && candidate.status == JobStatus::Queued), "pending job missing from HTTP snapshot")?;
                        } else {
                            require(snapshot.id == format!("{}-other", host.project.id), "unrelated snapshot identity changed")?;
                            require(snapshot.jobs.is_empty(), "unrelated project inherited render bookkeeping")?;
                        }
                    } else {
                        require(value.as_array().is_some_and(Vec::is_empty), "pre-encode progress should be empty")?;
                    }
                }
            }
            cycle += 1;
            tokio::time::sleep(Duration::from_secs(5).min(Duration::from_secs(31).saturating_sub(started.elapsed()))).await;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
        require(now > initial_expiry, "actual initial thirty-second editor TTL did not elapse")?;
        let action = ProjectAction::UpdateProjectSettings { name: "Edited through HTTP after initial TTL".into(), render_settings: host.project.render_settings.clone() };
        let edited = host.success(&first, &host.envelope("edit-after-ttl", "apply_project_action_to_split_project_folder", Some(token), Some(admission.project.content_revision), json!({"action": action}))).await?;
        require(edited["project"]["name"] == "Edited through HTTP after initial TTL", "edit did not commit")?;
        let replay: MediaRenderAdmission = decode(host.success(&first, &admission_request).await?)?;
        require(replay == admission, "same HTTP request must replay its exact admission ACK")?;
        let revision = edited["project"]["contentRevision"].as_u64().ok_or("edit revision absent")?;
        host.success(&first, &host.envelope("cancel-pending", "cancel_render_job_in_split_project_folder", Some(token), Some(revision), json!({"jobId": job, "attemptId": format!("render-attempt/{job}"), "updatedAt": UPDATED_AT}))).await?;
        let replacement = host.lease(&second, true).await?;
        let replacement_token = replacement["editorLeaseToken"].as_str().ok_or("replacement token absent")?;
        let denied = host.envelope("stale-owner-after-takeover", "apply_project_action_to_split_project_folder", Some(token), Some(revision), json!({"action": ProjectAction::UpdateProjectSettings { name: "Stale edit".into(), render_settings: host.project.render_settings.clone() }}));
        require(host.rpc(&first, &denied).await?.0 == StatusCode::CONFLICT, "takeover did not revoke old editor token")?;
        // Explicit user cancellation is a canonical edit. The replacement owner obtains
        // its current revision through the authenticated read path before writing.
        let current: VideoProject = decode(host.success(&second, &host.envelope("snapshot-after-takeover", "read_project_snapshot_from_split_project_folder", None, None, json!({}))).await?)?;
        require(current.name == "Edited through HTTP after initial TTL", "takeover lost the committed editor name")?;
        require(current.jobs.iter().any(|candidate| candidate.id == job && candidate.status == JobStatus::Cancelled), "HTTP cancellation did not persist the cancelled job")?;
        let mut accepted = denied;
        accepted.request_id = "replacement-owner-edit".into();
        accepted.editor_lease_token = Some(replacement_token.into());
        accepted.expected_revision = Some(current.content_revision);
        accepted.payload["action"] = serde_json::to_value(ProjectAction::UpdateProjectSettings { name: "Replacement editor".into(), render_settings: host.project.render_settings.clone() }).map_err(|e| e.to_string())?;
        host.success(&second, &accepted).await?;
        require(host.attempt(&second, job, 99).await? == MediaRenderAttempt::Pending, "cancelled worker should remain pending until storage is released")?;
        Ok(())
    }.await;
    let pending_elapsed = started.elapsed();
    drop(storage);
    if phase.is_err() {
        host.settle_after_failure(job).await;
    }
    // This read also waits for worker completion on a failed phase before reporting its failure.
    let terminal = host.terminal(&second, job).await;
    if terminal.is_err() {
        host.settle_after_failure(job).await;
    }
    phase.unwrap();
    assert!(matches!(
        terminal.unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: false,
            ..
        }
    ));
    let latest = load_split_project(&host.path).unwrap();
    assert_eq!(latest.name, "Replacement editor");
    assert_eq!(
        latest
            .jobs
            .iter()
            .find(|candidate| candidate.id == job)
            .unwrap()
            .status,
        JobStatus::Cancelled
    );
    assert!(latest.render_reports.is_empty());
    eprintln!("actual host worker pending for {pending_elapsed:?}, authenticated HTTP operations passed; pending before encoding, not >30 seconds of encoding");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_host_render_uses_immutable_admission_and_durable_replay() {
    let host = Host::new("host-immutable-render");
    let browser = host.pair("Editor").await.unwrap();
    let lease = host.lease(&browser, false).await.unwrap();
    let token = lease["editorLeaseToken"].as_str().unwrap();
    let job = "host-immutable-job";
    let request = host.admission(
        "immutable-admission",
        job,
        token,
        host.project.content_revision,
    );
    let artifact = acquire_split_project_artifact_lease(&host.path).unwrap();
    let phase: Check<MediaRenderAdmission> = async {
        let admission: MediaRenderAdmission = decode(host.success(&browser, &request).await?)?;
        let action = ProjectAction::RemoveItems {
            item_ids: vec![host.project.timeline.tracks[0].items[0].id.clone()],
        };
        let edited = host
            .success(
                &browser,
                &host.envelope(
                    "edit-admitted-snapshot",
                    "apply_project_action_to_split_project_folder",
                    Some(token),
                    Some(admission.project.content_revision),
                    json!({"action": action}),
                ),
            )
            .await?;
        require(
            edited["project"]["timeline"]["tracks"][0]["items"]
                .as_array()
                .is_some_and(Vec::is_empty),
            "HTTP edit did not remove admitted timeline clip",
        )?;
        require(
            host.attempt(&browser, job, 0).await? == MediaRenderAttempt::Pending,
            "artifact owner must prevent encoding",
        )?;
        Ok(admission)
    }
    .await;
    drop(artifact);
    if phase.is_err() {
        host.settle_after_failure(job).await;
    }
    let terminal = host.terminal(&browser, job).await;
    if terminal.is_err() {
        host.settle_after_failure(job).await;
    }
    let admission = phase.unwrap();
    let MediaRenderAttempt::Completed { result } = terminal.unwrap() else {
        panic!("actual HTTP-admitted worker failed to render");
    };
    assert!(result.project.timeline.tracks[0].items.is_empty());
    let output = host.path.join(&result.output_path);
    let (probe, _) =
        probe_media_with_gstreamer(&output, Duration::from_secs(30), "hostImmutableSnapshot")
            .unwrap();
    assert!((probe.duration_seconds.unwrap() - 6.0).abs() < 0.5);
    assert!(probe.video.is_some() && probe.audio.is_some());
    let modified = std::fs::metadata(&output).unwrap().modified().unwrap();
    let exact: MediaRenderAdmission =
        decode(host.success(&browser, &request).await.unwrap()).unwrap();
    assert_eq!(exact, admission);
    let mut conflicting_request = request.clone();
    conflicting_request.payload["width"] = json!(640);
    let (_, conflict) = host.rpc(&browser, &conflicting_request).await.unwrap();
    assert!(!conflict.ok);
    assert_eq!(
        conflict.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::Conflict
    );
    let mut durable_request = request;
    durable_request.request_id = "durable-replay-new-http-id".into();
    let durable: MediaRenderAdmission =
        decode(host.success(&browser, &durable_request).await.unwrap()).unwrap();
    assert_eq!(durable.source_revision, admission.source_revision);
    assert_eq!(durable.attempt_id, admission.attempt_id);
    assert!(durable.project.timeline.tracks[0].items.is_empty());
    let recovered: MediaRenderAttempt = decode(
        host.success(
            &browser,
            &host.envelope(
                "recover-completed-attempt",
                "recover_render_attempt_in_split_project_folder",
                Some(token),
                None,
                json!({"jobId": job, "attemptId": admission.attempt_id}),
            ),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(recovered, MediaRenderAttempt::Completed { .. }));
    assert_eq!(
        request_render_cancellation_by_locator(&host.path, job, &admission.attempt_id),
        RenderCancellationOutcome::NotFound
    );
    assert_eq!(
        std::fs::metadata(&output).unwrap().modified().unwrap(),
        modified
    );
    let latest = load_split_project(&host.path).unwrap();
    assert_eq!(
        latest
            .jobs
            .iter()
            .filter(|candidate| candidate.id == job)
            .count(),
        1
    );
    assert_eq!(
        latest
            .render_reports
            .iter()
            .filter(|candidate| candidate.id == job)
            .count(),
        1
    );
    assert!(latest.timeline.tracks[0].items.is_empty());
}
