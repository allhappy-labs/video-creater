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
