use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;

use tempfile::tempdir;
use video_creater_lib::app_service::context::{ClientKind, RequestContext};
use video_creater_lib::app_service::error::ServiceErrorCode;
use video_creater_lib::app_service::events::FakeEventSink;
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::app_service::projects::ProjectService;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::split::{
    load_split_project, save_split_project, split_project_manifest_path,
};

fn context(project_id: &str, revision: u64) -> RequestContext {
    RequestContext::new(
        ClientKind::Test,
        "project-service-test",
        Some(project_id.to_string()),
        Some(revision),
        BTreeSet::from([
            AuthorizationScope::ProjectRead,
            AuthorizationScope::ProjectWrite,
        ]),
        Some("lease-test".into()),
    )
    .expect("valid context")
}

#[test]
fn direct_service_save_matches_the_canonical_split_writer() {
    let direct_dir = tempdir().expect("direct tempdir");
    let compatibility_dir = tempdir().expect("compatibility tempdir");
    let project = sample_project();
    let events = Arc::new(FakeEventSink::default());
    let service = ProjectService::new(events.clone());

    let direct = service
        .save(&context(&project.id, 0), direct_dir.path(), project.clone())
        .expect("direct service save");
    let compatibility =
        save_split_project(compatibility_dir.path(), &project).expect("existing split writer save");

    assert_eq!(direct.project, compatibility.project);
    assert_eq!(
        fs::read(split_project_manifest_path(direct_dir.path())).expect("direct manifest"),
        fs::read(split_project_manifest_path(compatibility_dir.path())).expect("compat manifest")
    );
    assert_eq!(
        load_split_project(direct_dir.path()).unwrap(),
        direct.project
    );
    assert_eq!(events.recorded().len(), 1);
}

#[test]
fn stale_revision_has_no_partial_write_and_emits_no_success_event() {
    let project_dir = tempdir().expect("project tempdir");
    let initial = save_split_project(project_dir.path(), &sample_project())
        .expect("seed split project")
        .project;
    let events = Arc::new(FakeEventSink::default());
    let service = ProjectService::new(events.clone());
    let before =
        fs::read(split_project_manifest_path(project_dir.path())).expect("manifest before");

    let error = service
        .apply_action(
            &context(&initial.id, initial.content_revision.saturating_sub(1)),
            project_dir.path(),
            ProjectAction::UpdateProjectSettings {
                name: "Must not persist".into(),
                render_settings: initial.render_settings.clone(),
            },
        )
        .expect_err("stale revision must fail");

    assert_eq!(error.code(), ServiceErrorCode::RevisionConflict);
    assert_eq!(
        fs::read(split_project_manifest_path(project_dir.path())).expect("manifest after"),
        before
    );
    assert!(events.recorded().is_empty());
}

#[test]
fn progress_is_prompt_under_a_held_mutation_lease_without_parsing_sidecars() {
    use std::sync::mpsc;
    use std::time::Duration;
    use video_creater_lib::project::job_progress::JobProgressReporter;
    use video_creater_lib::project::mutation::acquire_split_project_mutation_lease;
    let dir = tempdir().unwrap();
    let project = save_split_project(dir.path(), &sample_project())
        .unwrap()
        .project;
    assert!(JobProgressReporter::new(dir.path(), "render-live").report(0.4));
    // Progress bookkeeping must not depend on canonical timeline validity/recovery.
    fs::write(dir.path().join("timeline.json"), b"invalid JSON").unwrap();
    let storage_lease =
        video_creater_lib::settings::storage::acquire_storage_mutation_lease().unwrap();
    let lease = acquire_split_project_mutation_lease(dir.path()).unwrap();
    let path = dir.path().to_path_buf();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let service = ProjectService::new(Arc::new(FakeEventSink::default()));
        tx.send(service.job_progress(&context(&project.id, project.content_revision), &path))
            .unwrap();
    });
    let observed = rx.recv_timeout(Duration::from_millis(500));
    drop(lease);
    drop(storage_lease);
    worker.join().unwrap();
    let snapshots = observed
        .expect("progress must not wait for the mutation lease")
        .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].progress, 0.4);
}

#[test]
fn progress_rejects_missing_wrong_identity_and_manifest_symlinks() {
    let dir = tempdir().unwrap();
    let project = save_split_project(dir.path(), &sample_project())
        .unwrap()
        .project;
    let service = ProjectService::new(Arc::new(FakeEventSink::default()));
    assert_eq!(
        service
            .job_progress(&context("other-project", 1), dir.path())
            .unwrap_err()
            .code(),
        ServiceErrorCode::Forbidden
    );
    fs::remove_file(split_project_manifest_path(dir.path())).unwrap();
    assert_eq!(
        service
            .job_progress(&context(&project.id, 1), dir.path())
            .unwrap_err()
            .code(),
        ServiceErrorCode::NotFound
    );
    #[cfg(unix)]
    {
        let external = tempdir().unwrap();
        save_split_project(external.path(), &project).unwrap();
        std::os::unix::fs::symlink(
            split_project_manifest_path(external.path()),
            split_project_manifest_path(dir.path()),
        )
        .unwrap();
        assert_eq!(
            service
                .job_progress(&context(&project.id, 1), dir.path())
                .unwrap_err()
                .code(),
            ServiceErrorCode::Forbidden
        );
    }
}

#[test]
fn progress_skips_partial_snapshots_and_keeps_valid_progress() {
    use video_creater_lib::project::job_progress::JobProgressReporter;
    let dir = tempdir().unwrap();
    let project = save_split_project(dir.path(), &sample_project())
        .unwrap()
        .project;
    assert!(JobProgressReporter::new(dir.path(), "valid-job").report(0.5));
    fs::write(
        dir.path().join("logs/job-progress/partial-job.json"),
        b"{\"jobId\":",
    )
    .unwrap();
    let service = ProjectService::new(Arc::new(FakeEventSink::default()));
    let progress = service
        .job_progress(&context(&project.id, 1), dir.path())
        .unwrap();
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].job_id, "valid-job");
}

#[test]
fn action_preconditions_precede_validation_and_identity_is_checked_under_the_write_lease() {
    let dir = tempdir().unwrap();
    let initial = save_split_project(dir.path(), &sample_project())
        .unwrap()
        .project;
    let events = Arc::new(FakeEventSink::default());
    let service = ProjectService::new(events.clone());
    let invalid = ProjectAction::UpdateProjectSettings {
        name: "".into(),
        render_settings: initial.render_settings.clone(),
    };
    let stale = service
        .apply_action(
            &context(&initial.id, initial.content_revision - 1),
            dir.path(),
            invalid.clone(),
        )
        .unwrap_err();
    assert_eq!(stale.code(), ServiceErrorCode::RevisionConflict);
    let unauthorized = service
        .apply_action(
            &context("other-project", initial.content_revision),
            dir.path(),
            invalid,
        )
        .unwrap_err();
    assert_eq!(unauthorized.code(), ServiceErrorCode::Forbidden);
    assert_eq!(load_split_project(dir.path()).unwrap(), initial);
    assert!(events.recorded().is_empty());
}

#[test]
fn a_failed_action_batch_does_not_persist_earlier_actions() {
    let dir = tempdir().unwrap();
    let initial = save_split_project(dir.path(), &sample_project())
        .unwrap()
        .project;
    let events = Arc::new(FakeEventSink::default());
    let service = ProjectService::new(events.clone());
    let result = service.apply_actions(
        &context(&initial.id, initial.content_revision),
        dir.path(),
        vec![
            ProjectAction::UpdateProjectSettings {
                name: "Valid first action".into(),
                render_settings: initial.render_settings.clone(),
            },
            ProjectAction::UpdateProjectSettings {
                name: "".into(),
                render_settings: initial.render_settings.clone(),
            },
        ],
    );
    assert_eq!(result.unwrap_err().code(), ServiceErrorCode::InvalidInput);
    assert_eq!(load_split_project(dir.path()).unwrap(), initial);
    assert!(events.recorded().is_empty());
}

#[cfg(feature = "web-host")]
#[test]
fn desktop_service_and_host_share_action_and_progress_contracts() {
    use serde_json::json;
    use video_creater_lib::project::job_progress::{JobProgressReporter, JobProgressSnapshot};
    use video_creater_lib::project::split::ProjectActionWriteResult;
    use video_creater_lib::web_host::dispatcher::HostDispatcher;
    use video_creater_lib::web_host::project_catalog::ProjectCatalog;
    use video_creater_lib::web_host::rpc::{RpcDispatcher, RpcEnvelope};

    let root = tempdir().unwrap();
    let desktop_path = root.path().join("desktop");
    let host_path = root.path().join("host");
    let initial = save_split_project(&desktop_path, &sample_project())
        .unwrap()
        .project;
    save_split_project(&host_path, &sample_project()).unwrap();
    let desktop_context = RequestContext::new(
        ClientKind::Desktop,
        "desktop-contract",
        Some(initial.id.clone()),
        Some(initial.content_revision),
        BTreeSet::from([
            AuthorizationScope::ProjectRead,
            AuthorizationScope::ProjectWrite,
        ]),
        None,
    )
    .unwrap();
    let service = ProjectService::new(Arc::new(FakeEventSink::default()));
    let action = ProjectAction::UpdateProjectSettings {
        name: "contract edit".into(),
        render_settings: initial.render_settings,
    };
    let desktop = service
        .apply_action(&desktop_context, &desktop_path, action.clone())
        .unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let mut request = RpcEnvelope {
        request_id: "host-contract".into(),
        operation: "apply_project_action_to_split_project_folder".into(),
        project_id: Some(catalog.id_for_path(&host_path).unwrap()),
        expected_revision: Some(initial.content_revision),
        editor_lease_token: Some("lease".into()),
        payload: json!({"action": action}),
    };
    let dispatcher = HostDispatcher::with_project_catalog(catalog);
    let host: ProjectActionWriteResult =
        serde_json::from_value(dispatcher.dispatch_typed(&request).unwrap()).unwrap();
    assert_eq!(host.project, desktop.project);

    assert!(JobProgressReporter::new(&desktop_path, "render-contract").report(0.25));
    assert!(JobProgressReporter::new(&host_path, "render-contract").report(0.25));
    request.operation = "load_job_progress_from_split_project_folder".into();
    request.expected_revision = None;
    request.editor_lease_token = None;
    request.payload = json!({});
    let host_progress: Vec<JobProgressSnapshot> =
        serde_json::from_value(dispatcher.dispatch_typed(&request).unwrap()).unwrap();
    let desktop_progress = service
        .job_progress(&desktop_context, &desktop_path)
        .unwrap();
    assert_eq!(host_progress.len(), desktop_progress.len());
    assert_eq!(host_progress[0].job_id, desktop_progress[0].job_id);
    assert_eq!(host_progress[0].progress, desktop_progress[0].progress);
}

#[test]
fn save_rejects_a_different_project_with_the_same_revision_after_authorization() {
    let root = tempdir().unwrap();
    let project_dir = root.path().join("project");
    let authorized = save_split_project(&project_dir, &sample_project())
        .unwrap()
        .project;
    let admitted_context = context(&authorized.id, authorized.content_revision);
    let mut replacement = sample_project();
    replacement.id = "replacement-project".into();
    replacement.name = "Replacement package".into();
    let replacement_dir = root.path().join("replacement");
    let replacement = save_split_project(&replacement_dir, &replacement)
        .unwrap()
        .project;
    assert_eq!(authorized.content_revision, replacement.content_revision);
    // Deterministic competing writer between authorization and the save primitive's lease.
    fs::rename(&project_dir, root.path().join("previous")).unwrap();
    fs::rename(&replacement_dir, &project_dir).unwrap();
    let manifest_before = fs::read(split_project_manifest_path(&project_dir)).unwrap();
    let events = Arc::new(FakeEventSink::default());
    let service = ProjectService::new(events.clone());
    let error = service
        .save(&admitted_context, &project_dir, authorized)
        .expect_err("same revision cannot authorize overwriting a different project");
    assert_eq!(error.code(), ServiceErrorCode::Forbidden);
    assert_eq!(load_split_project(&project_dir).unwrap(), replacement);
    assert_eq!(
        fs::read(split_project_manifest_path(&project_dir)).unwrap(),
        manifest_before
    );
    assert!(events.recorded().is_empty());
}
