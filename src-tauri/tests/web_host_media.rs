use std::fs;

use tempfile::tempdir;
use video_creater_lib::web_host::media::{
    parse_single_range, resolve_project_resource, ByteRange, RangeError,
};
use video_creater_lib::web_host::project_catalog::ProjectCatalog;

#[test]
fn parses_one_rfc_byte_range_and_rejects_invalid_or_multiple_ranges() {
    assert_eq!(parse_single_range(None, 100), Ok(None));
    assert_eq!(
        parse_single_range(Some("bytes=10-19"), 100),
        Ok(Some(ByteRange { start: 10, end: 19 }))
    );
    assert_eq!(
        parse_single_range(Some("bytes=90-"), 100),
        Ok(Some(ByteRange { start: 90, end: 99 }))
    );
    assert_eq!(
        parse_single_range(Some("bytes=-8"), 100),
        Ok(Some(ByteRange { start: 92, end: 99 }))
    );
    assert_eq!(
        parse_single_range(Some("bytes=100-101"), 100),
        Err(RangeError::Unsatisfiable)
    );
    assert_eq!(
        parse_single_range(Some("bytes=0-1,4-5"), 100),
        Err(RangeError::Multiple)
    );
}

#[test]
fn bounded_range_matrix_never_returns_out_of_bounds_bytes() {
    for total in 0..=128_u64 {
        for start in 0..=160_u64 {
            for width in [0_u64, 1, 2, 7, 31, 127, 255] {
                let header = format!("bytes={start}-{}", start.saturating_add(width));
                if let Ok(Some(range)) = parse_single_range(Some(&header), total) {
                    assert!(total > 0);
                    assert!(range.start <= range.end);
                    assert!(range.end < total);
                    assert_eq!(range.length(), range.end - range.start + 1);
                }
            }
        }
    }
    for hostile in [
        "",
        "bytes=",
        "bytes=--",
        "bytes=0-1,2-3",
        "items=0-1",
        "bytes=١-٢",
        "bytes=%2e%2e-9",
        "bytes=18446744073709551616-",
        "bytes=-0",
    ] {
        assert!(parse_single_range(Some(hostile), 128).is_err());
    }
}

#[cfg(unix)]
#[test]
fn canonical_resource_resolution_rejects_traversal_and_changed_symlinks() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let projects = sandbox.path().join("projects");
    let project = projects.join("demo.palmier");
    fs::create_dir_all(project.join("media")).unwrap();
    fs::write(
        project.join("video-creater.project.json"),
        r#"{"id":"p","name":"Demo","createdAt":"now"}"#,
    )
    .unwrap();
    fs::write(project.join("media/clip.mp4"), b"video").unwrap();
    let outside = sandbox.path().join("secret.mp4");
    fs::write(&outside, b"secret").unwrap();
    let catalog = ProjectCatalog::new(vec![projects]).unwrap();
    let project_id = catalog.id_for_path(&project).unwrap();

    assert_eq!(
        resolve_project_resource(&catalog, &project_id, "media/clip.mp4").unwrap(),
        fs::canonicalize(project.join("media/clip.mp4")).unwrap()
    );
    assert!(resolve_project_resource(&catalog, &project_id, "../secret.mp4").is_err());
    symlink(&outside, project.join("media/escape.mp4")).unwrap();
    assert!(resolve_project_resource(&catalog, &project_id, "media/escape.mp4").is_err());
}
