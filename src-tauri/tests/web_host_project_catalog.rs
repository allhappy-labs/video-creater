#[cfg(unix)]
mod unix_tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use tempfile::tempdir;
    use video_creater_lib::project::model::{MediaAsset, MediaKind, VideoProject};
    use video_creater_lib::web_host::dispatcher::HostDispatcher;
    use video_creater_lib::web_host::project_catalog::ProjectCatalog;
    use video_creater_lib::web_host::rpc::{RpcDispatcher, RpcEnvelope};

    #[test]
    fn catalog_returns_opaque_metadata_and_rejects_projects_outside_allowed_roots() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path().join("allowed");
        let outside = sandbox.path().join("outside.palmier");
        write_project(&root.join("inside.palmier"), "inside-id", "Inside project");
        write_project(&outside, "outside-id", "Outside project");
        symlink(&outside, root.join("escape.palmier")).unwrap();

        let catalog = ProjectCatalog::new(vec![root.clone()]).unwrap();
        let projects = catalog.list().unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Inside project");
        assert!(!projects[0].project_id.contains("inside"));
        let encoded = serde_json::to_string(&projects).unwrap();
        assert!(!encoded.contains(root.to_str().unwrap()));
        assert_eq!(
            catalog.resolve(&projects[0].project_id).unwrap(),
            fs::canonicalize(root.join("inside.palmier")).unwrap()
        );
    }

    #[test]
    fn catalog_rechecks_symlink_targets_on_every_open() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path().join("allowed");
        let first = root.join("nested/first.palmier");
        let outside = sandbox.path().join("outside.palmier");
        write_project(&first, "first-id", "First project");
        write_project(&outside, "outside-id", "Outside project");
        let alias = root.join("alias.palmier");
        symlink(&first, &alias).unwrap();
        let catalog = ProjectCatalog::new(vec![root]).unwrap();
        let alias_id = catalog.id_for_path(&alias).unwrap();

        fs::remove_file(&alias).unwrap();
        symlink(&outside, &alias).unwrap();

        assert!(catalog.resolve(&alias_id).is_err());
    }

    #[test]
    fn catalog_never_advertises_hidden_transaction_snapshots() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path().join("allowed");
        let project = root.join("visible.palmier");
        let staging = root.join(".video-creater-settings-stage.fixture");
        let backup = root.join(".video-creater-settings-backup.fixture");
        write_project(&project, "visible-id", "Visible project");
        write_project(&staging, "visible-id", "Visible project");
        write_project(&backup, "visible-id", "Visible project");

        let catalog = ProjectCatalog::new(vec![root]).unwrap();
        let projects = catalog.list().unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Visible project");
        assert_eq!(catalog.resolve(&projects[0].project_id).unwrap(), project);
        assert!(catalog.id_for_path(&staging).is_err());
        assert!(catalog.id_for_path(&backup).is_err());
    }

    #[test]
    fn host_dispatcher_lists_creates_and_opens_projects_by_catalog_id() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path().join("projects");
        let dispatcher =
            HostDispatcher::with_project_catalog(ProjectCatalog::new(vec![root]).unwrap());
        let mut project = VideoProject::new_empty(
            "project-one".into(),
            "Browser project".into(),
            "2026-09-24T00:00:00Z".into(),
        );
        project.media.push(MediaAsset {
            id: "media-zero-duration".into(),
            name: Some("unprobed.wav".into()),
            relative_path: "media/unprobed.wav".into(),
            kind: MediaKind::Audio,
            duration_seconds: 0.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "media-browser-fixture".into(),
            name: Some("fixture.wav".into()),
            relative_path: "media/fixture.wav".into(),
            kind: MediaKind::Audio,
            duration_seconds: 1.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "media-video-fixture".into(),
            name: Some("fixture.webm".into()),
            relative_path: "media/fixture.webm".into(),
            kind: MediaKind::Video,
            duration_seconds: 1.0,
            width: Some(320),
            height: Some(180),
            fps: Some(24.0),
            folder_id: None,
        });
        let created = dispatcher
            .dispatch(&request(
                "remote_create_project",
                None,
                serde_json::json!({"project":project}),
            ))
            .unwrap();
        let catalog_id = created["catalogProjectId"].as_str().unwrap();

        let listed = dispatcher
            .dispatch(&request(
                "remote_list_projects",
                None,
                serde_json::json!({}),
            ))
            .unwrap();
        assert_eq!(listed[0]["projectId"], catalog_id);
        assert_eq!(listed[0]["name"], "Browser project");
        assert!(!listed
            .to_string()
            .contains(sandbox.path().to_str().unwrap()));

        let opened = dispatcher
            .dispatch(&request(
                "load_split_project_from_folder",
                Some(catalog_id),
                serde_json::json!({"projectDir":catalog_id}),
            ))
            .unwrap();
        assert_eq!(opened["id"], created["project"]["id"]);
        assert_ne!(opened["id"], "project-one");

        let revision = opened["contentRevision"].as_u64().unwrap();
        let mut action_request = request(
            "apply_project_action_to_split_project_folder",
            Some(catalog_id),
            serde_json::json!({
                "projectDir": catalog_id,
                "action": {
                    "type": "createMediaFolder",
                    "folder": {"id":"browser-folder","name":"From browser","parentId":null}
                }
            }),
        );
        action_request.expected_revision = Some(revision);
        let updated = dispatcher.dispatch(&action_request).unwrap();
        assert_eq!(
            updated["project"]["mediaFolders"][0]["name"],
            "From browser"
        );
        assert!(updated["project"]["contentRevision"].as_u64().unwrap() > revision);

        let reloaded = dispatcher
            .dispatch(&request(
                "load_split_project_from_folder",
                Some(catalog_id),
                serde_json::json!({"projectDir":catalog_id}),
            ))
            .unwrap();
        assert_eq!(reloaded["mediaFolders"][0]["id"], "browser-folder");

        let mut reconcile_request = request(
            "reconcile_temporal_jobs_in_split_project_folder",
            Some(catalog_id),
            serde_json::json!({
                "projectDir": catalog_id,
                "updatedAt": "2026-09-24T00:00:30Z"
            }),
        );
        reconcile_request.expected_revision = reloaded["contentRevision"].as_u64();
        let reconciled = dispatcher.dispatch(&reconcile_request).unwrap();
        assert_eq!(
            reconciled["serviceReachable"],
            cfg!(feature = "temporal-worker")
        );
        if cfg!(feature = "temporal-worker") {
            assert_eq!(reconciled["project"]["id"], reloaded["id"]);
        } else {
            assert!(reconciled["project"].is_null());
        }
        assert_eq!(reconciled["failedJobIds"], serde_json::json!([]));

        let fixture_dispatcher = HostDispatcher::with_project_catalog_and_agent_fixture(
            ProjectCatalog::new(vec![sandbox.path().join("projects")]).unwrap(),
            true,
        );
        let revision = reloaded["contentRevision"].as_u64().unwrap();
        let mut turn_request = request(
            "start_codex_conversation_edit_for_project",
            Some(catalog_id),
            serde_json::json!({
                "projectDir": catalog_id,
                "project": reloaded,
                "request": {
                    "prompt":"Build a concise EDL rough cut",
                    "focus":{"mediaIds":[],"timelineItemIds":[]},
                    "createdAt":"2026-09-24T00:01:00Z"
                }
            }),
        );
        turn_request.expected_revision = Some(revision);
        let turn = fixture_dispatcher.dispatch(&turn_request).unwrap();
        assert_eq!(turn["proposal"]["edl"][0]["mediaId"], "media-video-fixture");
        assert_eq!(turn["proposal"]["edl"][0]["sourceIn"], 0.1);
        assert_eq!(turn["proposal"]["edl"][0]["sourceOut"], 0.9);
        assert_eq!(
            turn["threadResponse"]["requiredSkills"][0],
            "video-creater-video-pipeline"
        );
        assert_eq!(turn["preparedProposal"]["actions"][0]["type"], "addItems");

        let mut apply_request = request(
            "apply_codex_conversation_proposal",
            Some(catalog_id),
            serde_json::json!({
                "projectDir":catalog_id,
                "proposal":turn["proposal"],
                "actionIds":turn["preparedProposal"]["actionIds"],
                "reviewApproved":false,
                "sessionId":null
            }),
        );
        apply_request.expected_revision = Some(revision);
        let applied = fixture_dispatcher.dispatch(&apply_request).unwrap();
        assert_eq!(
            applied["project"]["timeline"]["tracks"][0]["items"][0]["properties"]["sourceIn"],
            0.1
        );
        assert_eq!(
            applied["project"]["timeline"]["tracks"][0]["items"][0]["properties"]["sourceOut"],
            0.9
        );

        let filmstrip_error = fixture_dispatcher
            .dispatch(&request(
                "cache_timeline_filmstrip_in_split_project_folder",
                Some(catalog_id),
                serde_json::json!({
                    "projectDir":catalog_id,
                    "mediaId":"missing-media",
                    "sourceIn":0.0,
                    "sourceOut":0.5,
                    "speed":1.0,
                    "zoomBucket":100,
                    "heightBucket":68,
                    "clipPixelWidth":240.0
                }),
            ))
            .unwrap_err();
        assert!(filmstrip_error.contains("media was not found"));

        let preview_error = fixture_dispatcher
            .dispatch(&request(
                "capture_canonical_preview_frame_in_split_project_folder",
                Some(catalog_id),
                serde_json::json!({
                    "projectDir":catalog_id,
                    "playheadSeconds":2.0,
                    "jobId":"out-of-range-preview",
                    "updatedAt":"2026-09-24T00:02:00Z"
                }),
            ))
            .unwrap_err();
        assert!(preview_error.contains("before the end"));
    }

    fn request(
        operation: &str,
        project_id: Option<&str>,
        payload: serde_json::Value,
    ) -> RpcEnvelope {
        RpcEnvelope {
            request_id: format!("request-{operation}"),
            operation: operation.into(),
            project_id: project_id.map(str::to_owned),
            expected_revision: None,
            editor_lease_token: None,
            payload,
        }
    }

    fn write_project(path: &std::path::Path, id: &str, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("video-creater.project.json"),
            serde_json::json!({"id":id,"name":name,"createdAt":"2026-09-24T00:00:00Z"}).to_string(),
        )
        .unwrap();
    }
}
