use super::*;
use crate::render_pipeline::output_profile::gstreamer_output_profile_target;

fn output(file_name: &str, directory: Option<&Path>) -> ExportOutputRequest {
    ExportOutputRequest {
        file_name: file_name.to_string(),
        directory: directory.map(|path| path.display().to_string()),
    }
}

fn rendered_source(dir: &Path, bytes: &[u8]) -> PathBuf {
    let render_dir = dir.join("renders/export-1");
    fs::create_dir_all(&render_dir).expect("render dir");
    let source = render_dir.join("output.mp4");
    fs::write(&source, bytes).expect("rendered output");
    source
}

fn materialize(
    destination: &ExportDestination,
    source: &Path,
    link_policy: LinkPolicy,
) -> Result<MaterializedExport, ExportDestinationError> {
    materialize_export_output(ExportMaterialization {
        destination,
        source,
        job_id: "export-mp4H264-1",
        link_policy,
    })
}

#[test]
fn export_file_names_keep_a_plain_stem() {
    assert_eq!(
        validate_export_file_name("Edison intro", "mp4").as_deref(),
        Ok("Edison intro")
    );
    assert_eq!(
        validate_export_file_name("Edison intro.MP4", "mp4").as_deref(),
        Ok("Edison intro")
    );
    assert_eq!(
        validate_export_file_name(" Edison intro ", "mp4").as_deref(),
        Ok("Edison intro")
    );
    assert_eq!(
        validate_export_file_name(&"a".repeat(180), "mp4").map(|stem| stem.len()),
        Ok(180)
    );
}

#[test]
fn export_file_names_refuse_unsafe_names_in_plain_words() {
    use ExportDestinationError::*;
    let cases: [(&str, ExportDestinationError); 10] = [
        ("", BlankName),
        ("  ", BlankName),
        ("a/b", NameHasSlash),
        ("a\\b", NameHasSlash),
        (".", NameStartsWithDot),
        ("..", NameStartsWithDot),
        (".hidden", NameStartsWithDot),
        ("a\u{0}b", NameHasControlCharacter),
        ("a\nb", NameHasControlCharacter),
        (&"a".repeat(181), NameTooLong),
    ];
    for (name, expected) in cases {
        assert_eq!(
            validate_export_file_name(name, "mp4"),
            Err(expected),
            "{name:?}"
        );
    }
    assert_eq!(
        NameHasSlash.to_string(),
        "Export names can't contain slashes."
    );
    assert_eq!(BlankName.to_string(), "Give the export a name.");
    assert_eq!(
        NameStartsWithDot.to_string(),
        "Export names can't start with a dot."
    );
    assert_eq!(
        NameHasControlCharacter.to_string(),
        "Export names can't contain control characters."
    );
    assert_eq!(
        NameTooLong.to_string(),
        "Export names can't be longer than 180 bytes."
    );
}

#[test]
fn destinations_default_to_the_project_exports_folder() {
    let project = tempfile::tempdir().expect("project");
    let destination =
        resolve_export_destination(project.path(), &output("Edison intro", None), "mp4")
            .expect("default destination");
    assert_eq!(destination.directory, project.path().join("exports"));
    assert!(destination.inside_project_exports);
    assert_eq!(destination.stem, "Edison intro");
}

#[test]
fn destinations_accept_absolute_folders_outside_the_project_and_inside_exports() {
    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    let destination =
        resolve_export_destination(project.path(), &output("a", Some(outside.path())), "mp4")
            .expect("outside folder");
    assert!(!destination.inside_project_exports);
    assert_eq!(destination.directory, outside.path());

    let sub = project.path().join("exports/sub");
    fs::create_dir_all(&sub).expect("exports sub");
    let destination = resolve_export_destination(project.path(), &output("a", Some(&sub)), "mp4")
        .expect("exports subfolder");
    assert!(destination.inside_project_exports);
    assert_eq!(destination.recorded_path("a.mp4"), "exports/sub/a.mp4");
}

#[cfg(unix)]
#[test]
fn a_symlinked_folder_inside_the_project_is_recorded_where_it_resolves_and_can_be_revealed() {
    use crate::project::export_reveal::reveal_recorded_export_with;
    use crate::project::model::{ProjectExportArtifact, ProjectExportArtifactKind, VideoProject};

    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    fs::create_dir_all(project.path().join("exports")).expect("exports");
    let link = project.path().join("exports/shared");
    std::os::unix::fs::symlink(outside.path(), &link).expect("symlink folder");

    let destination = resolve_export_destination(project.path(), &output("a", Some(&link)), "mp4")
        .expect("symlinked folder");
    let canonical_outside = fs::canonicalize(outside.path()).expect("canonical outside");
    assert!(!destination.inside_project_exports);
    assert_eq!(destination.directory, canonical_outside);
    let recorded = destination.recorded_path("a.mp4");
    assert_eq!(
        recorded,
        canonical_outside.join("a.mp4").display().to_string()
    );

    fs::write(outside.path().join("a.mp4"), b"x").expect("exported file");
    let mut recorded_project = VideoProject::new_empty(
        "project-1".to_string(),
        "Demo".to_string(),
        "2026-09-17T00:00:00Z".to_string(),
    );
    recorded_project
        .export_artifacts
        .push(ProjectExportArtifact {
            schema_version: 1,
            id: "export-mp4H264-1".to_string(),
            kind: ProjectExportArtifactKind::Mp4,
            format: "mp4H264".to_string(),
            path: recorded.clone(),
            mime_type: "video/mp4".to_string(),
            job_id: Some("export-mp4H264-1".to_string()),
            created_at: "2026-09-17T00:00:00Z".to_string(),
        });
    let mut revealed = None;
    reveal_recorded_export_with(project.path(), &recorded_project, &recorded, |path| {
        revealed = Some(path.to_path_buf());
        Ok(())
    })
    .expect("the recorded export can be revealed");
    assert_eq!(revealed, Some(canonical_outside.join("a.mp4")));
}

#[test]
fn destinations_refuse_relative_missing_and_project_folders() {
    let project = tempfile::tempdir().expect("project");
    let relative = ExportOutputRequest {
        file_name: "a".to_string(),
        directory: Some("exports".to_string()),
    };
    assert_eq!(
        resolve_export_destination(project.path(), &relative, "mp4"),
        Err(ExportDestinationError::DirectoryNotAbsolute)
    );
    let missing = project.path().join("gone");
    let error = resolve_export_destination(project.path(), &output("a", Some(&missing)), "mp4")
        .expect_err("missing folder");
    assert_eq!(error.to_string(), "The export folder no longer exists.");

    let media = project.path().join("media");
    fs::create_dir_all(&media).expect("media");
    for directory in [media.as_path(), project.path()] {
        let error =
            resolve_export_destination(project.path(), &output("a", Some(directory)), "mp4")
                .expect_err("inside project");
        assert_eq!(
            error.to_string(),
            "Choose a folder outside the project, or the project's exports folder."
        );
    }
}

#[test]
fn free_path_takes_the_next_numbered_name() {
    let outside = tempfile::tempdir().expect("outside");
    let destination = resolve_export_destination(
        Path::new("/nonexistent-project"),
        &output("Name", Some(outside.path())),
        "mp4",
    )
    .expect("destination");
    assert_eq!(destination.free_path(), Ok(outside.path().join("Name.mp4")));
    fs::write(outside.path().join("Name.mp4"), b"x").expect("existing");
    assert_eq!(
        destination.free_path(),
        Ok(outside.path().join("Name (2).mp4"))
    );
    fs::write(outside.path().join("Name (2).mp4"), b"x").expect("existing 2");
    assert_eq!(
        destination.free_path(),
        Ok(outside.path().join("Name (3).mp4"))
    );
    for index in 3..=999 {
        fs::write(outside.path().join(format!("Name ({index}).mp4")), b"x").expect("fill");
    }
    assert_eq!(
        destination.free_path(),
        Err(ExportDestinationError::NoFreeName)
    );
}

#[cfg(unix)]
#[test]
fn materialize_links_the_render_into_the_project_exports_folder() {
    use std::os::unix::fs::MetadataExt;
    let project = tempfile::tempdir().expect("project");
    let source = rendered_source(project.path(), b"rendered");
    let destination =
        resolve_export_destination(project.path(), &output("Name", None), "mp4").expect("dest");
    let result = materialize(&destination, &source, LinkPolicy::Auto).expect("materialized");
    assert_eq!(result.recorded_path, "exports/Name.mp4");
    assert_eq!(
        result.absolute_path,
        project.path().join("exports/Name.mp4")
    );
    assert_eq!(
        fs::metadata(&source).expect("source").ino(),
        fs::metadata(&result.absolute_path).expect("export").ino()
    );
}

#[test]
fn materialize_records_an_absolute_path_outside_the_project() {
    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    let source = rendered_source(project.path(), b"rendered");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .expect("dest");
    let result = materialize(&destination, &source, LinkPolicy::Auto).expect("materialized");
    let expected = outside.path().join("Name.mp4");
    assert_eq!(result.recorded_path, expected.display().to_string());
    assert!(Path::new(&result.recorded_path).is_absolute());
    assert_eq!(fs::read(expected).expect("export"), b"rendered");
}

#[test]
fn materialize_copy_fallback_is_byte_identical_and_leaves_no_staging_file() {
    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    let source = rendered_source(project.path(), b"rendered bytes");
    fs::write(outside.path().join("Name.mp4"), b"keep").expect("existing");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .expect("dest");
    let result = materialize(&destination, &source, LinkPolicy::CopyOnly).expect("copied");
    assert_eq!(result.absolute_path, outside.path().join("Name (2).mp4"));
    assert_eq!(
        fs::read(&result.absolute_path).expect("copy"),
        b"rendered bytes"
    );
    assert_eq!(
        fs::read(outside.path().join("Name.mp4")).expect("kept"),
        b"keep"
    );
    let leftovers = fs::read_dir(outside.path())
        .expect("list")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".partial"))
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "staging files left: {leftovers:?}");
}

#[test]
fn materialize_never_overwrites_a_file_that_appears_before_the_link() {
    for policy in [LinkPolicy::Auto, LinkPolicy::CopyOnly] {
        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::tempdir().expect("outside");
        let source = rendered_source(project.path(), b"rendered");
        let destination = resolve_export_destination(
            project.path(),
            &output("Name", Some(outside.path())),
            "mp4",
        )
        .expect("dest");
        let raced = outside.path().join("Name.mp4");
        let mut created = false;
        let result = materialize_export_output_with_hook(
            ExportMaterialization {
                destination: &destination,
                source: &source,
                job_id: "export-mp4H264-1",
                link_policy: policy,
            },
            &mut |candidate| {
                if !created && candidate == raced {
                    fs::write(candidate, b"keep").expect("racing file");
                    created = true;
                }
            },
        )
        .expect("materialized");
        assert_eq!(
            result.absolute_path,
            outside.path().join("Name (2).mp4"),
            "{policy:?}"
        );
        assert_eq!(fs::read(&raced).expect("raced"), b"keep");
    }
}

#[test]
fn materialize_refuses_an_empty_render() {
    let project = tempfile::tempdir().expect("project");
    let source = rendered_source(project.path(), b"");
    let destination =
        resolve_export_destination(project.path(), &output("Name", None), "mp4").expect("dest");
    assert_eq!(
        materialize(&destination, &source, LinkPolicy::Auto),
        Err(ExportDestinationError::EmptySource)
    );
    assert!(!project.path().join("exports/Name.mp4").exists());
}

#[test]
fn artifact_contracts_match_the_gstreamer_output_profiles() {
    assert_eq!(
        export_artifact_contract(ExportProfile::Mp4H264),
        Some((ProjectExportArtifactKind::Mp4, "mp4", "video/mp4"))
    );
    assert_eq!(
        export_artifact_contract(ExportProfile::ProResMov),
        Some((ProjectExportArtifactKind::Mov, "mov", "video/quicktime"))
    );
    assert_eq!(
        export_artifact_contract(ExportProfile::Webm),
        Some((ProjectExportArtifactKind::Webm, "webm", "video/webm"))
    );
    assert_eq!(
        export_artifact_contract(ExportProfile::PalmierProject),
        None
    );
    for profile in [
        ExportProfile::Webm,
        ExportProfile::Mp4H264,
        ExportProfile::Mp4H265,
        ExportProfile::ProResMov,
    ] {
        let (_, extension, _) = export_artifact_contract(profile).expect("video contract");
        if let Some(target) = gstreamer_output_profile_target(profile) {
            assert_eq!(extension, target.extension, "{profile:?}");
        }
    }
}

#[test]
fn artifact_records_match_the_temporal_writer() {
    let artifact = export_artifact_for(
        "export-mp4H264-1",
        ExportProfile::Mp4H264,
        "exports/Name.mp4",
        "2026-09-17T00:00:00Z",
    )
    .expect("artifact");
    assert_eq!(
        artifact,
        ProjectExportArtifact {
            schema_version: 1,
            id: "export-mp4H264-1".to_string(),
            kind: ProjectExportArtifactKind::Mp4,
            format: "mp4H264".to_string(),
            path: "exports/Name.mp4".to_string(),
            mime_type: "video/mp4".to_string(),
            job_id: Some("export-mp4H264-1".to_string()),
            created_at: "2026-09-17T00:00:00Z".to_string(),
        }
    );
    assert_eq!(
        export_artifact_for("j", ExportProfile::PalmierProject, "exports/a.palmier", "t"),
        Err(ExportDestinationError::UnsupportedProfile)
    );
}
