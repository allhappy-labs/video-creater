use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, ProjectExportArtifact, ProjectExportArtifactKind, VideoProject,
};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::web_host::dispatcher::HostDispatcher;
use video_creater_lib::web_host::http::{host_router, HostHttpConfig, PairResponse};
use video_creater_lib::web_host::project_catalog::ProjectCatalog;

#[tokio::test]
async fn media_and_artifact_tickets_stream_scoped_ranges_without_paths_in_urls() {
    let root = tempdir().unwrap();
    let projects = root.path().join("projects");
    let project_dir = projects.join("demo.palmier");
    std::fs::create_dir_all(&projects).unwrap();
    let mut project = VideoProject::new_empty(
        "project-a".into(),
        "Demo".into(),
        "2026-09-24T00:00:00Z".into(),
    );
    project.media.push(MediaAsset {
        id: "media-a".into(),
        name: Some("clip.mp4".into()),
        relative_path: "media/clip.mp4".into(),
        kind: MediaKind::Video,
        duration_seconds: 1.0,
        width: Some(16),
        height: Some(9),
        fps: Some(30.0),
        folder_id: None,
    });
    project.export_artifacts.push(ProjectExportArtifact {
        schema_version: 1,
        id: "export-a".into(),
        kind: ProjectExportArtifactKind::Mp4,
        format: "mp4".into(),
        path: "exports/final.mp4".into(),
        mime_type: "video/mp4".into(),
        job_id: None,
        created_at: "2026-09-24T00:00:00Z".into(),
    });
    save_split_project(&project_dir, &project).unwrap();
    std::fs::create_dir_all(project_dir.join("media")).unwrap();
    std::fs::create_dir_all(project_dir.join("exports")).unwrap();
    std::fs::create_dir_all(project_dir.join("renders/preview/frames")).unwrap();
    std::fs::write(project_dir.join("media/clip.mp4"), b"0123456789abcdef").unwrap();
    std::fs::write(project_dir.join("exports/final.mp4"), b"artifact-bytes").unwrap();
    std::fs::write(
        project_dir.join("renders/preview/frames/frame.png"),
        b"preview-bytes",
    )
    .unwrap();
    std::fs::write(
        project_dir.join("renders/index.json"),
        serde_json::to_vec(&json!({
            "schemaVersion":1,
            "reports":[{
                "reportId":"preview",
                "path":"renders/preview/report.json",
                "status":"completed",
                "outputPath":"renders/preview/frames/frame.png"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let catalog = ProjectCatalog::new(vec![projects]).unwrap();
    let project_id = catalog.id_for_path(&project_dir).unwrap();
    let app = host_router(
        HostHttpConfig {
            assets_dir: root.path().to_path_buf(),
            session_file: root.path().join("sessions.json"),
            public_origin: "http://host.test".into(),
            host_label: "Host".into(),
            pairing_code: "123456".into(),
            reusable_pairing_code: false,
            issued_at: 100,
            now_override: Some(101),
            uploads_dir: root.path().join("uploads"),
            upload_max_bytes: 1_024,
            upload_reserve_bytes: 0,
            project_catalog: catalog.clone(),
            allow_direct_test_identity: true,
        },
        Arc::new(HostDispatcher::with_project_catalog(catalog)),
    )
    .unwrap();
    let (cookie, csrf) = pair(&app).await;

    let prepared_path = "cache/precompose/v1/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/frames/frame-000000.png";
    std::fs::create_dir_all(project_dir.join(prepared_path).parent().unwrap()).unwrap();
    std::fs::write(project_dir.join(prepared_path), b"prepared-frame").unwrap();
    std::fs::write(project_dir.join("media/private.txt"), b"private").unwrap();
    let captured_path = "renders/capture/preview-qa/preview-frames/preview-0001.png";
    std::fs::create_dir_all(project_dir.join(captured_path).parent().unwrap()).unwrap();
    std::fs::write(project_dir.join(captured_path), b"capture").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        project_dir.join("media/private.txt"),
        project_dir.join("cache/precompose/link.png"),
    )
    .unwrap();
    std::fs::write(
        project_dir.join("cache/preview-resources.json"),
        serde_json::to_vec(&json!({"schemaVersion":1,"paths":[prepared_path,captured_path,"media/private.txt","cache/precompose/../private.txt","cache/precompose/link.png"]})).unwrap(),
    ).unwrap();
    let prepared_ticket = post_json(
        &app,
        "/api/v1/resource-tickets/media",
        &cookie,
        &csrf,
        json!({"projectId":project_id,"relativePaths":[prepared_path,captured_path]}),
    )
    .await;
    assert_eq!(prepared_ticket.0, StatusCode::OK);
    let prepared_url = prepared_ticket.1["urls"][format!("{project_id}/{prepared_path}")]
        .as_str()
        .unwrap();
    let prepared_frame = app
        .clone()
        .oneshot(
            Request::get(prepared_url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prepared_frame.status(), StatusCode::OK);
    assert_eq!(
        &to_bytes(prepared_frame.into_body(), 64).await.unwrap()[..],
        b"prepared-frame"
    );
    for unrecorded in [
        "cache/precompose/unrecorded.png",
        "media/private.txt",
        "cache/precompose/../private.txt",
        "cache/precompose/link.png",
    ] {
        let denied = post_json(
            &app,
            "/api/v1/resource-tickets/media",
            &cookie,
            &csrf,
            json!({"projectId":project_id,"relativePaths":[unrecorded]}),
        )
        .await;
        assert_eq!(denied.0, StatusCode::FORBIDDEN, "{unrecorded}");
    }

    let media_ticket = post_json(
        &app,
        "/api/v1/resource-tickets/media",
        &cookie,
        &csrf,
        json!({"projectId":project_id,"relativePaths":["media/clip.mp4"]}),
    )
    .await;
    assert_eq!(media_ticket.0, StatusCode::OK);
    let media_url = media_ticket.1["urls"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_str()
        .unwrap();
    assert!(media_url.starts_with("/api/v1/media/"));
    assert!(!media_url.contains("clip"));
    assert!(!media_url.contains("media%2F"));

    let ranged = app
        .clone()
        .oneshot(
            Request::get(media_url)
                .header(header::COOKIE, &cookie)
                .header(header::RANGE, "bytes=4-7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.headers()[header::CONTENT_RANGE], "bytes 4-7/16");
    assert_eq!(ranged.headers()[header::CONTENT_LENGTH], "4");
    assert_eq!(ranged.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(ranged.headers()[header::CACHE_CONTROL], "private, no-store");
    assert_eq!(
        &to_bytes(ranged.into_body(), 16).await.unwrap()[..],
        b"4567"
    );

    let head = app
        .clone()
        .oneshot(
            Request::head(media_url)
                .header(header::COOKIE, &cookie)
                .header(header::RANGE, "bytes=8-11")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(head.headers()[header::CONTENT_RANGE], "bytes 8-11/16");
    assert_eq!(head.headers()[header::CONTENT_LENGTH], "4");
    assert!(to_bytes(head.into_body(), 16).await.unwrap().is_empty());

    let preview_ticket = post_json(
        &app,
        "/api/v1/resource-tickets/media",
        &cookie,
        &csrf,
        json!({"projectId":project_id,"relativePaths":["renders/preview/frames/frame.png"]}),
    )
    .await;
    assert_eq!(preview_ticket.0, StatusCode::OK);
    assert!(preview_ticket.1["urls"]
        .as_object()
        .and_then(|urls| urls.values().next())
        .and_then(Value::as_str)
        .is_some_and(|url| url.starts_with("/api/v1/media/")));

    let invalid_range = app
        .clone()
        .oneshot(
            Request::get(media_url)
                .header(header::COOKIE, &cookie)
                .header(header::RANGE, "bytes=99-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_range.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(invalid_range.headers()[header::CONTENT_RANGE], "bytes */16");

    let artifact_ticket = post_json(
        &app,
        "/api/v1/resource-tickets/artifact",
        &cookie,
        &csrf,
        json!({"projectId":project_id,"artifactId":"export-a"}),
    )
    .await;
    assert_eq!(artifact_ticket.0, StatusCode::OK);
    let artifact_url = artifact_ticket.1["url"].as_str().unwrap();
    let artifact = app
        .clone()
        .oneshot(
            Request::get(artifact_url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(artifact.status(), StatusCode::OK);
    assert_eq!(
        artifact.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"final.mp4\""
    );

    let unauthenticated = app
        .oneshot(Request::get(media_url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
}

async fn pair(app: &axum::Router) -> (String, String) {
    let paired = app
        .clone()
        .oneshot(
            Request::post("/api/v1/pair")
                .header(header::ORIGIN, "http://host.test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"code":"123456","displayName":"Phone"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = paired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let pair: PairResponse =
        serde_json::from_slice(&to_bytes(paired.into_body(), 16_384).await.unwrap()).unwrap();
    (cookie, pair.csrf_token)
}

async fn post_json(
    app: &axum::Router,
    path: &str,
    cookie: &str,
    csrf: &str,
    value: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&value).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
        .unwrap_or(Value::Null);
    (status, body)
}
