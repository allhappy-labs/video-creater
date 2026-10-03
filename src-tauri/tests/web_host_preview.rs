#![cfg(feature = "web-host")]

use serde_json::json;
use video_creater_lib::project::model::VideoProject;
#[cfg(feature = "ges-render")]
use video_creater_lib::project::model::{MediaAsset, MediaKind};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::web_host::dispatcher::HostDispatcher;
use video_creater_lib::web_host::project_catalog::ProjectCatalog;
use video_creater_lib::web_host::rpc::{RpcDispatcher, RpcEnvelope};

fn source_request(project_id: String, media_id: &str) -> RpcEnvelope {
    RpcEnvelope {
        request_id: "source-preview".into(),
        operation: "prepare_project_preview".into(),
        project_id: Some(project_id),
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({"mediaId":media_id}),
    }
}

#[test]
fn source_preparation_rejects_media_outside_the_canonical_project() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preview.palmier");
    let project = VideoProject::new_empty(
        "saved".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    save_split_project(&path, &project).unwrap();
    let before = load_split_project(&path).unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let request = source_request(catalog.id_for_path(&path).unwrap(), "unknown");
    let dispatcher = HostDispatcher::with_project_catalog(catalog);
    assert!(dispatcher
        .dispatch(&request)
        .unwrap_err()
        .contains("not recorded"));
    assert_eq!(load_split_project(&path).unwrap(), before);
}

#[cfg(all(feature = "ges-render", unix))]
#[test]
fn legacy_source_metadata_probe_rejects_recorded_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preview.palmier");
    let mut project = VideoProject::new_empty(
        "saved".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    project.media.push(MediaAsset {
        id: "legacy".into(),
        name: None,
        relative_path: "media/legacy.json".into(),
        kind: MediaKind::Lottie,
        duration_seconds: 0.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    save_split_project(&path, &project).unwrap();
    let outside = root.path().join("private.json");
    std::fs::write(
        &outside,
        include_bytes!("fixtures/transitions/lottie-colour-steps.json"),
    )
    .unwrap();
    std::fs::create_dir_all(path.join("media")).unwrap();
    std::os::unix::fs::symlink(outside, path.join("media/legacy.json")).unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let request = source_request(catalog.id_for_path(&path).unwrap(), "legacy");
    let error = HostDispatcher::with_project_catalog(catalog)
        .dispatch(&request)
        .unwrap_err();
    assert!(error.contains("symlink"), "{error}");
}

#[cfg(feature = "ges-render")]
#[test]
fn remote_lottie_source_preparation_publishes_animated_scoped_frames_without_changing_the_project()
{
    use video_creater_lib::app_service::preview::recorded_preview_resource_paths;
    video_creater_lib::render_runtime::start_render_process_runtime().expect("curated runtime");
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preview.palmier");
    let mut project = VideoProject::new_empty(
        "saved".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    project.media.push(MediaAsset {
        id: "animation".into(),
        name: Some("Animation".into()),
        relative_path: "media/animation.json".into(),
        kind: MediaKind::Lottie,
        duration_seconds: 1.0,
        width: Some(128),
        height: Some(72),
        fps: Some(8.0),
        folder_id: None,
    });
    save_split_project(&path, &project).unwrap();
    std::fs::create_dir_all(path.join("media")).unwrap();
    std::fs::write(
        path.join("media/animation.json"),
        include_bytes!("fixtures/transitions/lottie-colour-steps.json"),
    )
    .unwrap();
    let before = load_split_project(&path).unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let request = source_request(catalog.id_for_path(&path).unwrap(), "animation");
    let dispatcher = HostDispatcher::with_project_catalog(catalog);
    let result = dispatcher
        .dispatch(&request)
        .expect("prepare remote Lottie source");
    assert_eq!(result["project"]["timeline"]["durationSeconds"], 1.0);
    assert_eq!(result["frameSequences"][0]["fps"], 8.0);
    let frames = result["frameSequences"][0]["framePaths"]
        .as_array()
        .unwrap();
    assert_eq!(frames.len(), 8);
    let first = image::open(path.join(frames[0].as_str().unwrap()))
        .unwrap()
        .to_rgba8();
    let later = image::open(path.join(frames[3].as_str().unwrap()))
        .unwrap()
        .to_rgba8();
    assert_eq!(first.dimensions(), (128, 72));
    assert_ne!(
        first.get_pixel(64, 36),
        later.get_pixel(64, 36),
        "source frames must animate"
    );
    let granted = recorded_preview_resource_paths(&path);
    for frame in frames {
        assert!(granted.contains(frame.as_str().unwrap()));
    }
    assert!(!granted.contains("media/animation.json"));
    assert!(!granted.contains("cache/precompose/private.png"));
    assert_eq!(load_split_project(&path).unwrap(), before);
}

#[cfg(feature = "ges-render")]
#[test]
fn legacy_dotlottie_source_metadata_is_probed_ephemerally_without_changing_canonical_media() {
    use std::io::Write;
    video_creater_lib::render_runtime::start_render_process_runtime().expect("curated runtime");
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preview.palmier");
    let mut project = VideoProject::new_empty(
        "saved".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    project.media.push(MediaAsset {
        id: "legacy".into(),
        name: Some("Legacy animation".into()),
        relative_path: "media/legacy.lottie".into(),
        kind: MediaKind::Lottie,
        duration_seconds: 0.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    save_split_project(&path, &project).unwrap();
    std::fs::create_dir_all(path.join("media")).unwrap();
    let mut archive =
        zip::ZipWriter::new(std::fs::File::create(path.join("media/legacy.lottie")).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("manifest.json", options).unwrap();
    archive
        .write_all(br#"{"version":"1.0","animations":[{"id":"colour"}]}"#)
        .unwrap();
    archive
        .start_file("animations/colour.json", options)
        .unwrap();
    archive
        .write_all(include_bytes!(
            "fixtures/transitions/lottie-colour-steps.json"
        ))
        .unwrap();
    archive.finish().unwrap();
    let before = load_split_project(&path).unwrap();
    let catalog = ProjectCatalog::new(vec![root.path().to_path_buf()]).unwrap();
    let request = source_request(catalog.id_for_path(&path).unwrap(), "legacy");
    let result = HostDispatcher::with_project_catalog(catalog)
        .dispatch(&request)
        .expect("prepare legacy source");
    assert_eq!(result["project"]["timeline"]["durationSeconds"], 4.0);
    assert_eq!(result["project"]["renderSettings"]["width"], 128);
    assert_eq!(result["project"]["renderSettings"]["height"], 72);
    assert_eq!(result["frameSequences"][0]["fps"], 24.0);
    assert_eq!(
        result["frameSequences"][0]["framePaths"]
            .as_array()
            .unwrap()
            .len(),
        96
    );
    assert_eq!(load_split_project(&path).unwrap(), before);
}
