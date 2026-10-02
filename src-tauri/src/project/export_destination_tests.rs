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
fn copy_materialization_preserves_an_unrelated_partial_file() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let unrelated = outside.path().join(".Name.export-mp4H264-1.partial");
    fs::write(&unrelated, b"keep another producer's staging file").unwrap();
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let result = materialize(&destination, &source, LinkPolicy::CopyOnly).unwrap();
    assert_eq!(fs::read(result.absolute_path).unwrap(), b"rendered bytes");
    assert_eq!(
        fs::read(unrelated).expect("unrelated staging must remain"),
        b"keep another producer's staging file"
    );
}

#[test]
fn prepared_copy_is_hidden_and_drop_removes_only_its_owned_stage() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    assert!(!outside.path().join("Name.mp4").exists());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    drop(prepared);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn prepared_drop_cleans_the_original_directory_after_parent_replacement() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let chosen = outside.path().join("chosen");
    let moved = outside.path().join("moved");
    fs::create_dir(&chosen).unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(&chosen)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    fs::rename(&chosen, &moved).unwrap();
    fs::create_dir(&chosen).unwrap();
    fs::write(chosen.join("user.mp4"), b"keep replacement").unwrap();
    drop(prepared);
    assert_eq!(
        fs::read(chosen.join("user.mp4")).unwrap(),
        b"keep replacement"
    );
    assert_eq!(
        fs::read_dir(&moved).unwrap().count(),
        0,
        "owned stages in a moved parent must be cleaned through the retained directory descriptor"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "child helper invoked by interrupted_stage_is_recovered_on_the_next_prepare"]
fn interrupted_stage_child() {
    let Some(root) = std::env::var_os("VIDEO_CREATER_INTERRUPTED_EXPORT_TEST") else {
        return;
    };
    let root = PathBuf::from(root);
    let project = root.join("project");
    let outside = root.join("outside");
    let source = rendered_source(&project, b"interrupted render bytes");
    let destination =
        resolve_export_destination(&project, &output("Name", Some(&outside)), "mp4").unwrap();
    let _prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "interrupted-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    if std::env::var_os("VIDEO_CREATER_HOLD_EXPORT_TEST").is_some() {
        fs::write(root.join("ready"), b"ready").unwrap();
        for _ in 0..1000 {
            if root.join("release").exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            root.join("release").exists(),
            "parent did not release child stage"
        );
    }
    // A normal destructor would hide the restart case. Exit releases OS locks but
    // deliberately skips Rust cleanup, just as an interrupted worker does.
    std::process::exit(0);
}

#[cfg(unix)]
#[test]
fn interrupted_stage_is_recovered_on_the_next_prepare() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&outside).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "project::export_destination::tests::interrupted_stage_child",
            "--ignored",
        ])
        .env("VIDEO_CREATER_INTERRUPTED_EXPORT_TEST", root.path())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    let source = rendered_source(&project, b"next render bytes");
    let destination =
        resolve_export_destination(&project, &output("Name", Some(&outside)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "next-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        1,
        "a dead worker's owned stage must be recovered, leaving only the live stage"
    );
    drop(prepared);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn interrupted_stage_recovery_preserves_a_replacement_payload() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&outside).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "project::export_destination::tests::interrupted_stage_child",
            "--ignored",
        ])
        .env("VIDEO_CREATER_INTERRUPTED_EXPORT_TEST", root.path())
        .status()
        .unwrap();
    assert!(status.success());
    let stage = fs::read_dir(&outside)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let replacement = root.path().join("replacement");
    fs::write(&replacement, b"keep replacement bytes").unwrap();
    fs::rename(&replacement, stage.join("output")).unwrap();
    let source = rendered_source(&project, b"next render bytes");
    let destination =
        resolve_export_destination(&project, &output("Name", Some(&outside)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "next-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    drop(prepared);
    assert_eq!(
        fs::read(stage.join("output")).unwrap(),
        b"keep replacement bytes"
    );
    assert!(
        stage.join("owner.json").exists(),
        "mismatching ownership must remain forensic evidence"
    );
}

#[cfg(unix)]
#[test]
fn recovery_excludes_a_stage_locked_by_another_process() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    let outside = root.path().join("outside");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&outside).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "project::export_destination::tests::interrupted_stage_child",
            "--ignored",
        ])
        .env("VIDEO_CREATER_INTERRUPTED_EXPORT_TEST", root.path())
        .env("VIDEO_CREATER_HOLD_EXPORT_TEST", "1")
        .spawn()
        .unwrap();
    for _ in 0..500 {
        if root.path().join("ready").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        root.path().join("ready").exists(),
        "child did not initialize stage"
    );
    let source = rendered_source(&project, b"next render bytes");
    let destination =
        resolve_export_destination(&project, &output("Name", Some(&outside)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "next-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        2,
        "active child stage must survive cleanup"
    );
    drop(prepared);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    fs::write(root.path().join("release"), b"release").unwrap();
    assert!(child.wait().unwrap().success());
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "after-child",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        1,
        "dead child stage should now recover"
    );
    drop(prepared);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn publication_owner_rolls_back_in_the_original_directory_after_a_parent_swap() {
    let project = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let chosen = root.path().join("chosen");
    let moved = root.path().join("moved");
    fs::create_dir(&chosen).unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(&chosen)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let owner = prepared.publication_owner().unwrap();
    let published = prepared.publish().unwrap();
    fs::rename(&chosen, &moved).unwrap();
    fs::create_dir(&chosen).unwrap();
    fs::write(chosen.join("Name.mp4"), b"keep replacement bytes").unwrap();
    owner.rollback(&published.absolute_path).unwrap();
    assert!(!moved.join("Name.mp4").exists());
    assert_eq!(
        fs::read(chosen.join("Name.mp4")).unwrap(),
        b"keep replacement bytes"
    );
}

#[cfg(unix)]
#[test]
fn publication_owner_rejects_another_name_even_when_it_links_the_same_inode() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let owner = prepared.publication_owner().unwrap();
    let published = prepared.publish().unwrap();
    let user_alias = outside.path().join("Name (2).mp4");
    fs::hard_link(&published.absolute_path, &user_alias).unwrap();
    assert!(owner.rollback(&user_alias).is_err());
    assert_eq!(fs::read(&user_alias).unwrap(), b"rendered bytes");
    assert!(published.absolute_path.exists());
}

#[cfg(unix)]
#[test]
fn publication_boundary_parent_swap_cannot_redirect_output_or_rollback() {
    let project = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let chosen = root.path().join("chosen");
    let moved = root.path().join("moved");
    fs::create_dir(&chosen).unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(&chosen)), "mp4").unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let result = prepared.publish_with_boundary_hook(&mut |path| {
        fs::rename(&chosen, &moved).unwrap();
        fs::create_dir(&chosen).unwrap();
        fs::write(path, b"keep replacement bytes").unwrap();
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(chosen.join("Name.mp4")).unwrap(),
        b"keep replacement bytes"
    );
    assert_eq!(
        fs::read_dir(&moved).unwrap().count(),
        0,
        "publication and cleanup must use the original directory descriptor"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn publication_boundary_stage_swap_links_the_held_inode_and_preserves_replacement() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let stage = fs::read_dir(outside.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let published = prepared
        .publish_with_boundary_hook(&mut |_| {
            let replacement = outside.path().join("replacement");
            fs::write(&replacement, b"keep replacement bytes").unwrap();
            fs::rename(replacement, stage.join("output")).unwrap();
        })
        .unwrap();
    assert_eq!(
        fs::read(published.absolute_path).unwrap(),
        b"rendered bytes"
    );
    assert_eq!(
        fs::read(stage.join("output")).unwrap(),
        b"keep replacement bytes"
    );
}

#[test]
fn copy_can_be_cancelled_midway_without_publishing_or_leaving_a_stage() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), &vec![7; 1024 * 1024]);
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let mut calls = 0;
    let result = prepare_export_output_cancellable(
        ExportMaterialization {
            destination: &destination,
            source: &source,
            job_id: "same-job",
            link_policy: LinkPolicy::CopyOnly,
        },
        &mut || {
            calls += 1;
            calls < 4
        },
    );
    assert!(matches!(result, Err(ExportDestinationError::Cancelled)));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn concurrent_prepared_copies_use_distinct_stages_and_publish_numbered_names() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let prepare = || {
        prepare_export_output(ExportMaterialization {
            destination: &destination,
            source: &source,
            job_id: "same-job",
            link_policy: LinkPolicy::CopyOnly,
        })
        .unwrap()
    };
    let first = prepare();
    let second = prepare();
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 2);
    assert_eq!(
        first.publish().unwrap().absolute_path,
        outside.path().join("Name.mp4")
    );
    assert_eq!(
        second.publish().unwrap().absolute_path,
        outside.path().join("Name (2).mp4")
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn prepared_publication_rejects_a_replaced_parent_without_deleting_replacement_bytes() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let destination_dir = outside.path().join("chosen");
    fs::create_dir(&destination_dir).unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination = resolve_export_destination(
        project.path(),
        &output("Name", Some(&destination_dir)),
        "mp4",
    )
    .unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let stage_name = fs::read_dir(&destination_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    fs::rename(&destination_dir, outside.path().join("original")).unwrap();
    fs::create_dir(&destination_dir).unwrap();
    let replacement = destination_dir.join(stage_name);
    fs::write(&replacement, b"keep replacement bytes").unwrap();
    assert!(prepared.publish().is_err());
    assert_eq!(fs::read(replacement).unwrap(), b"keep replacement bytes");
    assert!(!destination_dir.join("Name.mp4").exists());
}

#[cfg(unix)]
#[test]
fn prepared_publication_rejects_a_replaced_stage_without_deleting_replacement_bytes() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = rendered_source(project.path(), b"rendered bytes");
    let destination =
        resolve_export_destination(project.path(), &output("Name", Some(outside.path())), "mp4")
            .unwrap();
    let prepared = prepare_export_output(ExportMaterialization {
        destination: &destination,
        source: &source,
        job_id: "same-job",
        link_policy: LinkPolicy::CopyOnly,
    })
    .unwrap();
    let stage = fs::read_dir(outside.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("output");
    let replacement = outside.path().join("replacement");
    fs::write(&replacement, b"keep replacement bytes").unwrap();
    fs::rename(replacement, &stage).unwrap();
    assert!(prepared.publish().is_err());
    assert_eq!(fs::read(stage).unwrap(), b"keep replacement bytes");
    assert!(!outside.path().join("Name.mp4").exists());
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
