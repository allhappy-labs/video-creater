use tempfile::tempdir;
use video_creater_lib::web_host::upload::{UploadError, UploadStore};

fn dotlottie_bytes(ids: &[&str]) -> Vec<u8> {
    use std::io::Write;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("manifest.json", options).unwrap();
    archive.write_all(&serde_json::to_vec(&serde_json::json!({"version":"1.0","animations": ids.iter().map(|id| serde_json::json!({"id":id})).collect::<Vec<_>>()})).unwrap()).unwrap();
    for id in ids {
        archive
            .start_file(format!("animations/{id}.json"), options)
            .unwrap();
        archive
            .write_all(include_bytes!(
                "fixtures/transitions/lottie-colour-steps.json"
            ))
            .unwrap();
    }
    archive.finish().unwrap().into_inner()
}

#[test]
fn single_animation_dotlottie_uploads_import_with_native_metadata() {
    use video_creater_lib::project::{
        import::import_media_files,
        model::{MediaKind, VideoProject},
    };
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().join("uploads"), 16_384, 0).unwrap();
    let bytes = dotlottie_bytes(&["colour"]);
    let completed = store
        .stage("animation.lottie", bytes.len() as u64, [bytes])
        .expect("dotLottie upload");
    assert_eq!(completed.media_type, "application/vnd.lottie");
    let source = store.completed_path(&completed.upload_id).unwrap();
    let project = VideoProject::new_empty(
        "animation".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    let result =
        import_media_files(&root.path().join("project.palmier"), project, &[source]).unwrap();
    assert_eq!(result.imported[0].kind, MediaKind::Lottie);
    assert_eq!(result.imported[0].duration_seconds, 4.0);
    assert_eq!(result.imported[0].width, Some(128));
    assert_eq!(result.imported[0].height, Some(72));
    assert_eq!(result.imported[0].fps, Some(24.0));
}

#[test]
fn ambiguous_dotlottie_upload_requires_animation_selection() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 16_384, 0).unwrap();
    let bytes = dotlottie_bytes(&["one", "two"]);
    let error = store
        .stage("multiple.lottie", bytes.len() as u64, [bytes])
        .unwrap_err();
    assert!(
        format!("{error:?}").contains("AmbiguousAnimation"),
        "{error:?}"
    );
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn native_dotlottie_import_rejects_multiple_animations_with_a_precise_error() {
    use video_creater_lib::project::{import::import_media_files, model::VideoProject};
    let root = tempdir().unwrap();
    let source = root.path().join("multiple.lottie");
    std::fs::write(&source, dotlottie_bytes(&["one", "two"])).unwrap();
    let project = VideoProject::new_empty(
        "animation".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    let error =
        import_media_files(&root.path().join("project.palmier"), project, &[source]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("multiple animations require an explicit animationId"),
        "{error}"
    );
}

#[test]
fn dotlottie_uploads_reject_unsafe_archives_using_renderer_validation() {
    use std::io::Write;
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 16_384, 0).unwrap();
    for name in ["../private.json", "animations/anim.json"] {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("manifest.json", options).unwrap();
        archive
            .write_all(br#"{"animations":[{"id":"anim"}]}"#)
            .unwrap();
        archive.start_file(name, options).unwrap();
        archive.write_all(br#"{"fr":24,"op":96,"w":128,"h":72,"layers":[],"assets":[{"p":"https://private.invalid/image.png"}]}"#).unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert_eq!(
            store.stage("unsafe.lottie", bytes.len() as u64, [bytes]),
            Err(UploadError::UnsupportedMedia)
        );
        assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    }
}

#[test]
fn concurrent_uploads_are_reserved_and_bounded() {
    let root = tempfile::tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 1_024, 0).unwrap();
    let first = store.begin("one.mp4", 16).unwrap();
    let second = store.begin("two.mp4", 16).unwrap();
    let third = store.begin("three.mp4", 16).unwrap();
    let fourth = store.begin("four.mp4", 16).unwrap();
    assert!(matches!(
        store.begin("five.mp4", 16),
        Err(UploadError::Busy)
    ));
    drop(first);
    assert!(store.begin("replacement.mp4", 16).is_ok());
    drop((second, third, fourth));
}

#[test]
fn upload_uses_host_ids_validates_bytes_and_persists_hash_metadata() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 1_024, 0).unwrap();
    let bytes = mp4_bytes();

    let completed = store
        .stage("clip.mp4", bytes.len() as u64, [&bytes[..8], &bytes[8..]])
        .unwrap();

    assert_eq!(completed.display_name, "clip.mp4");
    assert_eq!(completed.media_type, "video/mp4");
    assert_eq!(completed.size_bytes, bytes.len() as u64);
    assert_eq!(completed.sha256.len(), 64);
    assert!(!completed.upload_id.contains("clip"));
    assert!(store
        .completed_path(&completed.upload_id)
        .unwrap()
        .is_file());
}

#[test]
fn valid_lottie_json_uploads_import_with_animation_metadata() {
    use std::collections::BTreeMap;
    use video_creater_lib::project::{
        import::import_media_files_with_names,
        model::{MediaKind, VideoProject},
    };
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().join("uploads"), 16_384, 0).unwrap();
    let bytes = include_bytes!("fixtures/transitions/lottie-colour-steps.json");
    let completed = store
        .stage("animation.json", bytes.len() as u64, [bytes.as_slice()])
        .expect("valid Lottie upload");
    assert_eq!(completed.media_type, "application/vnd.lottie+json");
    let source = store.completed_path(&completed.upload_id).unwrap();
    let project = VideoProject::new_empty(
        "animation".into(),
        "Preview".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    let imported = import_media_files_with_names(
        &root.path().join("project.palmier"),
        project,
        std::slice::from_ref(&source),
        &BTreeMap::from([(source.clone(), completed.display_name)]),
    )
    .unwrap();
    assert_eq!(imported.imported[0].kind, MediaKind::Lottie);
    assert_eq!(imported.imported[0].duration_seconds, 4.0);
    assert_eq!(imported.imported[0].width, Some(128));
    assert_eq!(imported.imported[0].height, Some(72));
    assert_eq!(imported.imported[0].fps, Some(24.0));
}

#[test]
fn arbitrary_and_invalid_lottie_json_uploads_are_rejected_without_partial_files() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 16_384, 0).unwrap();
    for bytes in [
        br#"{"private":"notes"}"#.as_slice(),
        br#"{"fr":24,"op":96,"layers":[]}"#.as_slice(),
        br#"{"fr":0,"op":96,"w":128,"h":72,"layers":[]}"#.as_slice(),
    ] {
        assert_eq!(
            store.stage("animation.json", bytes.len() as u64, [bytes]),
            Err(UploadError::UnsupportedMedia)
        );
        assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    }
}

#[test]
fn upload_rejects_traversal_size_mismatch_limits_and_unknown_media_without_partials() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 24, 0).unwrap();

    assert_eq!(
        store.stage("../clip.mp4", 16, [mp4_bytes()]),
        Err(UploadError::InvalidName)
    );
    assert_eq!(
        store.stage("clip.mp4", 3, [mp4_bytes()]),
        Err(UploadError::SizeMismatch)
    );
    assert_eq!(
        store.stage("clip.mp4", 100, [mp4_bytes()]),
        Err(UploadError::TooLarge)
    );
    assert_eq!(
        store.stage("notes.txt", 5, [b"hello".as_slice()]),
        Err(UploadError::UnsupportedMedia)
    );
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn upload_fails_before_writing_when_reserved_free_space_is_unavailable() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 1_024, u64::MAX).unwrap();
    assert_eq!(
        store.stage("clip.mp4", 16, [mp4_bytes()]),
        Err(UploadError::InsufficientSpace)
    );
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn expired_completed_uploads_are_reported_and_removed_without_touching_recent_uploads() {
    let root = tempdir().unwrap();
    let store = UploadStore::new(root.path().to_path_buf(), 1_024, 0).unwrap();
    let expired = store
        .stage("expired.mp4", mp4_bytes().len() as u64, [mp4_bytes()])
        .unwrap();
    let removed = store
        .cleanup_expired(expired.created_at_unix_seconds + 86_401, 86_400)
        .unwrap();

    assert_eq!(removed, vec![expired.upload_id.clone()]);
    assert_eq!(
        store.completed_path(&expired.upload_id),
        Err(UploadError::InvalidId)
    );
    let recent = store
        .stage("recent.mp4", mp4_bytes().len() as u64, [mp4_bytes()])
        .unwrap();
    assert!(store
        .cleanup_expired(recent.created_at_unix_seconds + 60, 86_400)
        .unwrap()
        .is_empty());
    assert!(store.completed_path(&recent.upload_id).is_ok());
}

fn mp4_bytes() -> Vec<u8> {
    [
        0_u8, 0, 0, 16, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm', 0, 0, 0, 0,
    ]
    .to_vec()
}
