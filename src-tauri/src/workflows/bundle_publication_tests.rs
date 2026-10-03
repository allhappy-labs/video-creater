use super::*;
use crate::project::nle_export::write_nle_xml_export;

#[test]
fn failed_bundle_preparation_preserves_previous_export() {
    let root = tempfile::tempdir().unwrap();
    let exports = root.path().join("exports");
    fs::create_dir(&exports).unwrap();
    let output = exports.join("previous.palmier");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("previous.txt"), b"previous user export").unwrap();
    // The compatibility-file stage must reject an invalid source project.
    fs::write(root.path().join("project.json"), b"invalid project").unwrap();
    assert!(write_project_bundle_package(root.path(), &output, "replacement", true).is_err());
    assert_eq!(
        fs::read(output.join("previous.txt")).unwrap(),
        b"previous user export"
    );
    assert_eq!(fs::read_dir(&exports).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn nle_export_rejects_symlinked_exports_without_touching_external_files() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("review.xml"), b"external user file").unwrap();
    std::os::unix::fs::symlink(&outside, project.join("exports")).unwrap();
    let export = NleXmlExport {
        filename: "review.xml".into(),
        mime_type: "application/xml".into(),
        xml: "replacement".into(),
    };
    assert!(write_nle_xml_export(&project, &export).is_err());
    assert_eq!(
        fs::read(outside.join("review.xml")).unwrap(),
        b"external user file"
    );
}

#[cfg(unix)]
#[test]
fn nle_export_never_follows_a_predictable_stage_symlink() {
    let root = tempfile::tempdir().unwrap();
    let exports = root.path().join("exports");
    fs::create_dir(&exports).unwrap();
    let outside = root.path().join("private.txt");
    fs::write(&outside, b"private user file").unwrap();
    std::os::unix::fs::symlink(&outside, exports.join(".review.xml.tmp")).unwrap();
    let export = NleXmlExport {
        filename: "review.xml".into(),
        mime_type: "application/xml".into(),
        xml: "replacement".into(),
    };
    write_nle_xml_export(root.path(), &export).unwrap();
    assert_eq!(fs::read(&outside).unwrap(), b"private user file");
    assert!(!fs::symlink_metadata(exports.join("review.xml"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn successful_bundle_replacement_publishes_a_complete_project_and_removes_stage() {
    let root = tempfile::tempdir().unwrap();
    let project_dir = root.path().join("project.palmier");
    let project = crate::project::model::VideoProject::new_empty(
        "bundle-test".into(),
        "Bundle".into(),
        "2026-10-03T00:00:00Z".into(),
    );
    crate::project::split::save_split_project(&project_dir, &project).unwrap();
    let exports = project_dir.join("exports");
    fs::create_dir_all(&exports).unwrap();
    let output = exports.join("previous.palmier");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("previous.txt"), b"previous user export").unwrap();
    write_project_bundle_package(&project_dir, &output, "replacement", true).unwrap();
    assert_eq!(
        crate::project::split::load_split_project(&output)
            .unwrap()
            .id,
        project.id
    );
    assert!(!output.join("previous.txt").exists());
    assert!(fs::read_dir(&exports).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".replacement-project-bundle-")));
}

#[test]
fn failed_bundle_publication_and_no_overwrite_preserve_the_previous_export() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("previous.palmier");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("previous.txt"), b"previous user export").unwrap();
    assert!(publish_project_bundle(&root.path().join("missing-stage"), &output, true).is_err());
    let staging = root.path().join("stage");
    fs::create_dir(&staging).unwrap();
    assert!(publish_project_bundle(&staging, &output, false).is_err());
    assert_eq!(
        fs::read(output.join("previous.txt")).unwrap(),
        b"previous user export"
    );
}

#[test]
fn failed_nle_publication_and_no_overwrite_preserve_the_previous_export() {
    let root = tempfile::tempdir().unwrap();
    let exports = root.path().join("exports");
    fs::create_dir(&exports).unwrap();
    let output = exports.join("review.xml");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("previous.txt"), b"previous user export").unwrap();
    let export = NleXmlExport {
        filename: "review.xml".into(),
        mime_type: "application/xml".into(),
        xml: "replacement".into(),
    };
    assert!(write_nle_xml_export(root.path(), &export).is_err());
    assert_eq!(
        fs::read(output.join("previous.txt")).unwrap(),
        b"previous user export"
    );
    fs::remove_dir_all(&output).unwrap();
    fs::write(&output, b"previous user export").unwrap();
    assert!(
        crate::project::nle_export::write_nle_xml_export_with_overwrite(
            root.path(),
            &export,
            false
        )
        .is_err()
    );
    assert_eq!(fs::read(&output).unwrap(), b"previous user export");
    assert_eq!(fs::read_dir(&exports).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn temporal_nle_writer_rejects_symlinked_exports_without_external_mutation() {
    let root = tempfile::tempdir().unwrap();
    let project_dir = root.path().join("project.palmier");
    let project = crate::project::fixtures::sample_project();
    crate::project::split::save_split_project(&project_dir, &project).unwrap();
    let export = export_project_timeline_to_nle_xml(&project, NleXmlFormat::PremiereXmeml).unwrap();
    let start = temporal_export_nle_xml_start_request(
        &project.id,
        project_dir.to_str().unwrap(),
        "nle-job",
        NleXmlFormat::PremiereXmeml,
        &format!("exports/{}", export.filename),
    );
    let build =
        temporal_export_nle_xml_build_activity_value(json!({"startRequest":start})).unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join(&export.filename), b"external user export").unwrap();
    let exports = project_dir.join("exports");
    if exports.exists() {
        fs::remove_dir_all(&exports).unwrap();
    }
    std::os::unix::fs::symlink(&outside, &exports).unwrap();
    assert!(temporal_export_nle_xml_write_artifact_activity_value(build).is_err());
    assert_eq!(
        fs::read(outside.join(&export.filename)).unwrap(),
        b"external user export"
    );
}

#[test]
fn repeated_bundle_export_excludes_recorded_bundles_and_preserves_authored_directories() {
    let root = tempfile::tempdir().unwrap();
    let project_dir = root.path().join("project.palmier");
    let now = "2026-10-03T00:00:00Z";
    let mut project = VideoProject::new_empty("repeat-bundle".into(), "Bundle".into(), now.into());
    crate::project::split::save_split_project(&project_dir, &project).unwrap();
    let exports = project_dir.join("exports");
    let first = exports.join("first.palmier");
    write_project_bundle_package(&project_dir, &first, "first-job", true).unwrap();
    project.export_artifacts.push(ProjectExportArtifact {
        schema_version: 1,
        id: "first-job".into(),
        kind: ProjectExportArtifactKind::ProjectBundle,
        format: "palmierProject".into(),
        path: "exports/first.palmier".into(),
        mime_type: "application/vnd.video-creater.project".into(),
        job_id: None,
        created_at: now.into(),
    });
    crate::project::split::save_split_project(&project_dir, &project).unwrap();
    let authored = exports.join("authored.palmier");
    fs::create_dir(&authored).unwrap();
    fs::write(authored.join("keep.txt"), b"authored content").unwrap();
    fs::write(exports.join("loose.xml"), b"loose export").unwrap();
    let second = exports.join("second.palmier");
    write_project_bundle_package(&project_dir, &second, "second-job", true).unwrap();
    assert!(!second.join("exports/first.palmier").exists());
    assert_eq!(
        fs::read(second.join("exports/authored.palmier/keep.txt")).unwrap(),
        b"authored content"
    );
    assert_eq!(
        fs::read(second.join("exports/loose.xml")).unwrap(),
        b"loose export"
    );
    assert!(first.join("project.json").exists());
}
