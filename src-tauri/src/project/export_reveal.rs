//! The restricted "Show in folder" check: the editor may reveal only files the project itself
//! records as export output, never an arbitrary path. That includes an export saved to a chosen
//! folder outside the project, by its exact recorded absolute path.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::model::VideoProject;

pub const UNRECORDED_EXPORT_MESSAGE: &str = "This file isn't a recorded export of this project.";
const MISSING_EXPORT_MESSAGE: &str = "This file is no longer in the project folder.";
const MISSING_OUTSIDE_EXPORT_MESSAGE: &str =
    "This file is no longer in the folder it was exported to.";
const REVEAL_FAILED_MESSAGE: &str = "The file manager couldn't show this file.";

/// Paths the project records as export output: every `exportArtifacts[].path`, plus each render
/// report's `outputPath` and `logPath` (in-process renders record no export artifact, only a
/// report under `renders/<jobId>/`). Job summaries carry no output or log path of their own.
/// An export saved to a folder outside the project records an absolute artifact path; render
/// report paths are always project-relative.
pub fn recorded_export_paths(project: &VideoProject) -> Vec<&str> {
    project
        .export_artifacts
        .iter()
        .map(|artifact| artifact.path.as_str())
        .chain(
            project
                .render_reports
                .iter()
                .flat_map(|report| [report.output_path.as_str(), report.log_path.as_str()]),
        )
        .collect()
}

/// A path's project-relative form: absolute paths must sit under `project_dirs` (as given or
/// canonical); `..`, root and prefix components are refused.
fn project_relative(path: &str, project_dirs: &[&Path]) -> Option<PathBuf> {
    let path = Path::new(path.trim());
    let relative = if path.is_absolute() {
        project_dirs
            .iter()
            .find_map(|dir| path.strip_prefix(dir).ok())?
    } else {
        path
    };
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

/// Reveals `artifact_path` (project-relative, or absolute under `project_dir`) with `reveal` when
/// it names a recorded export path that resolves inside the project folder after following
/// symlinks. `reveal` receives the canonical absolute path; its error detail is dropped for a plain
/// message, so callers log the detail themselves.
pub fn reveal_recorded_export_with<F>(
    project_dir: &Path,
    project: &VideoProject,
    artifact_path: &str,
    reveal: F,
) -> Result<(), String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let canonical_dir = fs::canonicalize(project_dir)
        .map_err(|error| format!("The project folder is unavailable: {error}"))?;
    let dirs = [project_dir, canonical_dir.as_path()];
    let Some(requested) = project_relative(artifact_path, &dirs) else {
        return reveal_recorded_outside_export(project, artifact_path, reveal);
    };
    if !recorded_export_paths(project)
        .into_iter()
        .any(|recorded| project_relative(recorded, &dirs).as_ref() == Some(&requested))
    {
        return Err(UNRECORDED_EXPORT_MESSAGE.to_string());
    }
    let resolved = fs::canonicalize(canonical_dir.join(&requested))
        .map_err(|_| MISSING_EXPORT_MESSAGE.to_string())?;
    if !resolved.starts_with(&canonical_dir) {
        return Err(UNRECORDED_EXPORT_MESSAGE.to_string());
    }
    reveal(&resolved).map_err(|_| REVEAL_FAILED_MESSAGE.to_string())
}

/// Reveals an absolute path outside the project only when it equals a recorded absolute
/// `exportArtifacts[].path`, is not a symlink and still exists.
fn reveal_recorded_outside_export<F>(
    project: &VideoProject,
    artifact_path: &str,
    reveal: F,
) -> Result<(), String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let requested = Path::new(artifact_path.trim());
    let lexically_clean = requested.is_absolute()
        && requested.components().all(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::Normal(_)
            )
        });
    let recorded = lexically_clean
        && project.export_artifacts.iter().any(|artifact| {
            let recorded = Path::new(&artifact.path);
            recorded.is_absolute() && recorded == requested
        });
    if !recorded {
        return Err(UNRECORDED_EXPORT_MESSAGE.to_string());
    }
    let metadata =
        fs::symlink_metadata(requested).map_err(|_| MISSING_OUTSIDE_EXPORT_MESSAGE.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err(UNRECORDED_EXPORT_MESSAGE.to_string());
    }
    let resolved =
        fs::canonicalize(requested).map_err(|_| MISSING_OUTSIDE_EXPORT_MESSAGE.to_string())?;
    reveal(&resolved).map_err(|_| REVEAL_FAILED_MESSAGE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::{
        ProjectExportArtifact, ProjectExportArtifactKind, ProjectRenderReport, RenderReportStatus,
        RenderReportStreams,
    };
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    fn project() -> VideoProject {
        let mut project = VideoProject::new_empty(
            "project-1".to_string(),
            "Demo".to_string(),
            "2026-09-15T00:00:00Z".to_string(),
        );
        project.export_artifacts.push(ProjectExportArtifact {
            schema_version: 1,
            id: "nle-1".to_string(),
            kind: ProjectExportArtifactKind::NleXml,
            format: "premiereXmeml".to_string(),
            path: "exports/demo.xml".to_string(),
            mime_type: "application/xml".to_string(),
            job_id: Some("nle-1".to_string()),
            created_at: "2026-09-15T00:00:00Z".to_string(),
        });
        project.render_reports.push(ProjectRenderReport {
            schema_version: 1,
            id: "export-mp4-1".to_string(),
            status: RenderReportStatus::Completed,
            output_path: "renders/export-mp4-1/output.mp4".to_string(),
            duration_seconds: 6.0,
            quality: None,
            requested_width: None,
            requested_height: None,
            actual_width: None,
            actual_height: None,
            streams: RenderReportStreams {
                video: true,
                audio: true,
            },
            checks: BTreeMap::new(),
            artifacts: Vec::new(),
            preview_comparison_request: None,
            preview_comparison: None,
            log_path: "renders/export-mp4-1/render.log".to_string(),
            created_at: "2026-09-15T00:00:00Z".to_string(),
        });
        project
    }

    fn project_folder() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("project folder");
        for file in [
            "exports/demo.xml",
            "renders/export-mp4-1/output.mp4",
            "renders/export-mp4-1/render.log",
            "media/private.mp4",
        ] {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
            fs::write(&path, b"x").expect("write file");
        }
        root
    }

    fn revealed(project_dir: &Path, project: &VideoProject, path: &str) -> Result<PathBuf, String> {
        let revealed = Mutex::new(None);
        reveal_recorded_export_with(project_dir, project, path, |resolved| {
            *revealed.lock().expect("revealed") = Some(resolved.to_path_buf());
            Ok(())
        })?;
        Ok(revealed
            .into_inner()
            .expect("revealed")
            .expect("reveal was called"))
    }

    #[test]
    fn reveal_export_artifact_reveals_a_recorded_artifact_by_its_absolute_path() {
        let folder = project_folder();
        let canonical = fs::canonicalize(folder.path()).expect("canonical folder");
        let project = project();

        assert_eq!(
            revealed(folder.path(), &project, "exports/demo.xml"),
            Ok(canonical.join("exports/demo.xml"))
        );
        let absolute = folder.path().join("exports/demo.xml");
        assert_eq!(
            revealed(folder.path(), &project, &absolute.display().to_string()),
            Ok(canonical.join("exports/demo.xml"))
        );
        assert_eq!(
            revealed(folder.path(), &project, "./exports/demo.xml"),
            Ok(canonical.join("exports/demo.xml"))
        );
    }

    #[test]
    fn reveal_export_artifact_reveals_a_recorded_render_output_and_log() {
        let folder = project_folder();
        let canonical = fs::canonicalize(folder.path()).expect("canonical folder");
        let project = project();

        assert_eq!(
            revealed(folder.path(), &project, "renders/export-mp4-1/output.mp4"),
            Ok(canonical.join("renders/export-mp4-1/output.mp4"))
        );
        assert_eq!(
            revealed(folder.path(), &project, "renders/export-mp4-1/render.log"),
            Ok(canonical.join("renders/export-mp4-1/render.log"))
        );
    }

    #[test]
    fn reveal_export_artifact_refuses_an_unrecorded_path() {
        let folder = project_folder();
        let error =
            reveal_recorded_export_with(folder.path(), &project(), "media/private.mp4", |_| {
                panic!("an unrecorded path must not be revealed")
            })
            .expect_err("unrecorded path is refused");
        assert_eq!(error, "This file isn't a recorded export of this project.");
    }

    #[test]
    fn reveal_export_artifact_refuses_parent_traversal_and_paths_outside_the_project() {
        let folder = project_folder();
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        let mut project = project();
        let escaping = "renders/export-mp4-1/../../../outside.mp4";
        project.export_artifacts[0].path = escaping.to_string();

        for path in [
            escaping.to_string(),
            "../exports/demo.xml".to_string(),
            outside.path().display().to_string(),
            "/etc/passwd".to_string(),
            String::new(),
        ] {
            let error = reveal_recorded_export_with(folder.path(), &project, &path, |_| {
                panic!("{path} must not be revealed")
            })
            .expect_err("escaping path is refused");
            assert_eq!(error, UNRECORDED_EXPORT_MESSAGE, "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn reveal_export_artifact_refuses_a_recorded_symlink_that_escapes_the_project() {
        let folder = project_folder();
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        let link = folder.path().join("exports/demo.xml");
        fs::remove_file(&link).expect("remove recorded file");
        std::os::unix::fs::symlink(outside.path(), &link).expect("symlink");

        let error =
            reveal_recorded_export_with(folder.path(), &project(), "exports/demo.xml", |_| {
                panic!("a symlink escape must not be revealed")
            })
            .expect_err("symlink escape is refused");
        assert_eq!(error, UNRECORDED_EXPORT_MESSAGE);
    }

    #[test]
    fn reveal_export_artifact_reports_a_missing_recorded_file_plainly() {
        let folder = project_folder();
        fs::remove_file(folder.path().join("renders/export-mp4-1/output.mp4")).expect("remove");

        let error = reveal_recorded_export_with(
            folder.path(),
            &project(),
            "renders/export-mp4-1/output.mp4",
            |_| panic!("a missing file must not be revealed"),
        )
        .expect_err("missing file is reported");
        assert_eq!(error, "This file is no longer in the project folder.");
    }

    fn project_with_outside_artifact(path: &Path) -> VideoProject {
        let mut project = project();
        project.export_artifacts.push(ProjectExportArtifact {
            schema_version: 1,
            id: "export-mp4H264-1".to_string(),
            kind: ProjectExportArtifactKind::Mp4,
            format: "mp4H264".to_string(),
            path: path.display().to_string(),
            mime_type: "video/mp4".to_string(),
            job_id: Some("export-mp4H264-1".to_string()),
            created_at: "2026-09-17T00:00:00Z".to_string(),
        });
        project
    }

    #[test]
    fn reveal_export_artifact_reveals_a_recorded_absolute_artifact_outside_the_project() {
        let folder = project_folder();
        let outside = tempfile::tempdir().expect("outside folder");
        let exported = outside.path().join("Edison intro (2).mp4");
        fs::write(&exported, b"x").expect("exported file");
        let project = project_with_outside_artifact(&exported);

        assert_eq!(
            revealed(folder.path(), &project, &exported.display().to_string()),
            Ok(fs::canonicalize(&exported).expect("canonical export"))
        );
    }

    #[test]
    fn reveal_export_artifact_refuses_an_unrecorded_absolute_path_outside_the_project() {
        let folder = project_folder();
        let outside = tempfile::tempdir().expect("outside folder");
        let exported = outside.path().join("Edison intro.mp4");
        let sibling = outside.path().join("private.mp4");
        fs::write(&exported, b"x").expect("exported file");
        fs::write(&sibling, b"x").expect("sibling file");
        let project = project_with_outside_artifact(&exported);

        let error = reveal_recorded_export_with(
            folder.path(),
            &project,
            &sibling.display().to_string(),
            |_| panic!("an unrecorded sibling must not be revealed"),
        )
        .expect_err("unrecorded sibling is refused");
        assert_eq!(error, UNRECORDED_EXPORT_MESSAGE);
    }

    #[cfg(unix)]
    #[test]
    fn reveal_export_artifact_refuses_a_recorded_absolute_symlink() {
        let folder = project_folder();
        let outside = tempfile::tempdir().expect("outside folder");
        let target = tempfile::NamedTempFile::new().expect("symlink target");
        let link = outside.path().join("Edison intro.mp4");
        std::os::unix::fs::symlink(target.path(), &link).expect("symlink");
        let project = project_with_outside_artifact(&link);

        let error = reveal_recorded_export_with(
            folder.path(),
            &project,
            &link.display().to_string(),
            |_| panic!("a recorded symlink must not be revealed"),
        )
        .expect_err("recorded symlink is refused");
        assert_eq!(error, UNRECORDED_EXPORT_MESSAGE);
    }

    #[test]
    fn reveal_export_artifact_reports_a_missing_absolute_artifact_plainly() {
        let folder = project_folder();
        let outside = tempfile::tempdir().expect("outside folder");
        let exported = outside.path().join("Edison intro.mp4");
        let project = project_with_outside_artifact(&exported);

        let error = reveal_recorded_export_with(
            folder.path(),
            &project,
            &exported.display().to_string(),
            |_| panic!("a missing file must not be revealed"),
        )
        .expect_err("missing file is reported");
        assert_eq!(
            error,
            "This file is no longer in the folder it was exported to."
        );
    }

    #[test]
    fn reveal_export_artifact_reports_a_file_manager_failure_plainly() {
        let folder = project_folder();
        let error =
            reveal_recorded_export_with(folder.path(), &project(), "exports/demo.xml", |_| {
                Err("no file manager".to_string())
            })
            .expect_err("reveal failure is returned");
        assert_eq!(error, "The file manager couldn't show this file.");
    }
}
