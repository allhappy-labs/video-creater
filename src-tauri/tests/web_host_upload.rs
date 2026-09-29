use tempfile::tempdir;
use video_creater_lib::web_host::upload::{UploadError, UploadStore};

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
