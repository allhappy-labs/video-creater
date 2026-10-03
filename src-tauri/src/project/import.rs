use super::model::{MediaAsset, MediaKind, VideoProject};
use super::mutation::acquire_split_project_mutation_lease;
use super::source_probe::probe_source_metadata;
use super::{
    split::{load_split_project, save_split_project},
    storage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkippedFile {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaResult {
    pub project: VideoProject,
    pub imported: Vec<MediaAsset>,
    pub skipped: Vec<ImportSkippedFile>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ImportMediaError {
    #[error("no source files were selected")]
    EmptySelection,
    #[error("no selected files can be imported")]
    NoImportableFiles,
    #[error("{0}")]
    InvalidLottie(String),
    #[error("failed to save project after import: {0}")]
    SaveProject(String),
    #[error("failed to copy media from {source_path} to {destination}: {message}")]
    Copy {
        source_path: String,
        destination: String,
        message: String,
    },
}

const MAX_IMPORTED_MEDIA_NAME_CHARS: usize = 120;

pub fn import_media_files(
    project_dir: &Path,
    project: VideoProject,
    source_paths: &[PathBuf],
) -> Result<ImportMediaResult, ImportMediaError> {
    import_media_files_with_names(project_dir, project, source_paths, &BTreeMap::new())
}

/// Imports media, naming each source from `names` when it has a non-blank
/// entry (for example a saved timeline range), else from its file stem.
pub fn import_media_files_with_names(
    project_dir: &Path,
    mut project: VideoProject,
    source_paths: &[PathBuf],
    names: &BTreeMap<PathBuf, String>,
) -> Result<ImportMediaResult, ImportMediaError> {
    let _project_lease =
        acquire_split_project_mutation_lease(project_dir).map_err(ImportMediaError::SaveProject)?;
    if source_paths.is_empty() {
        return Err(ImportMediaError::EmptySelection);
    }
    let split_layout = project_uses_split_layout(project_dir)?;
    if split_layout {
        project = load_split_project(project_dir)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
    }

    let mut imported = Vec::new();
    let mut skipped = Vec::new();

    for source_path in source_paths {
        if source_path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("lottie"))
        {
            if let Err(message) = dotlottie_metadata(source_path) {
                if message.contains("multiple animations") {
                    return Err(ImportMediaError::InvalidLottie(message));
                }
            }
        }
        match importable_media_kind(source_path) {
            Some(kind) if source_path.is_file() => {
                let display_name = names.get(source_path).map(String::as_str);
                let asset = copy_media_asset(project_dir, source_path, kind, display_name)?;
                project.media.push(asset.clone());
                imported.push(asset);
            }
            Some(_) => skipped.push(ImportSkippedFile {
                source_path: source_path.display().to_string(),
                reason: "source path is not a file".to_string(),
            }),
            None => skipped.push(ImportSkippedFile {
                source_path: source_path.display().to_string(),
                reason: "unsupported media extension".to_string(),
            }),
        }
    }

    if imported.is_empty() {
        return Err(ImportMediaError::NoImportableFiles);
    }

    if split_layout {
        project = save_split_project(project_dir, &project)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?
            .project;
    } else {
        storage::save_project(project_dir, &project)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
    }

    Ok(ImportMediaResult {
        project,
        imported,
        skipped,
    })
}

pub(crate) fn persist_imported_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<(), ImportMediaError> {
    if project_uses_split_layout(project_dir)? {
        save_split_project(project_dir, project)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
    } else {
        storage::save_project(project_dir, project)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn project_uses_split_layout(project_dir: &Path) -> Result<bool, ImportMediaError> {
    let manifest_path = storage::project_file_path(project_dir);
    if manifest_path.exists() {
        let manifest = std::fs::read_to_string(&manifest_path)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
        let value: Value = serde_json::from_str(&manifest)
            .map_err(|error| ImportMediaError::SaveProject(error.to_string()))?;
        Ok(value.get("layout").and_then(Value::as_str) == Some("split"))
    } else {
        Ok(false)
    }
}

fn copy_media_asset(
    project_dir: &Path,
    source_path: &Path,
    kind: MediaKind,
    display_name: Option<&str>,
) -> Result<MediaAsset, ImportMediaError> {
    let metadata = import_metadata_for(source_path, &kind);
    let display_name = display_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.chars()
                .take(MAX_IMPORTED_MEDIA_NAME_CHARS)
                .collect::<String>()
        });
    let asset_id = format!("media-{}", uuid::Uuid::new_v4());
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let source_stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_file_stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "media".to_string());
    let stem = display_name
        .as_deref()
        .map(sanitize_file_stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| source_stem.clone());
    let relative_path = format!("media/{asset_id}-{stem}.{extension}");
    let destination = project_dir.join(&relative_path);

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| ImportMediaError::Copy {
            source_path: source_path.display().to_string(),
            destination: destination.display().to_string(),
            message: error.to_string(),
        })?;
    }

    std::fs::copy(source_path, &destination).map_err(|error| ImportMediaError::Copy {
        source_path: source_path.display().to_string(),
        destination: destination.display().to_string(),
        message: error.to_string(),
    })?;

    Ok(MediaAsset {
        id: asset_id,
        name: Some(display_name.unwrap_or(source_stem)),
        relative_path,
        kind,
        duration_seconds: metadata.duration_seconds.unwrap_or(0.0),
        width: metadata.width,
        height: metadata.height,
        fps: metadata.fps,
        folder_id: None,
    })
}

#[derive(Debug, Clone, Default)]
struct ImportedMediaMetadata {
    duration_seconds: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
}

fn sanitize_file_stem(stem: &str) -> String {
    stem.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn importable_media_kind(path: &Path) -> Option<MediaKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi" | "flv" | "ogv" => Some(MediaKind::Video),
        "wav" | "mp3" | "m4a" | "aac" | "aiff" | "aifc" | "flac" | "ogg" => Some(MediaKind::Audio),
        "png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "heic" | "heif" => {
            Some(MediaKind::Image)
        }
        "lottie" | "json" if path_is_lottie_media(path) => Some(MediaKind::Lottie),
        _ => None,
    }
}

pub(crate) fn path_is_lottie_media(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => lottie_json_metadata(path).is_some(),
        Some("lottie") => dotlottie_metadata(path).is_ok(),
        _ => false,
    }
}

fn import_metadata_for(path: &Path, kind: &MediaKind) -> ImportedMediaMetadata {
    match kind {
        MediaKind::Lottie => lottie_json_metadata(path)
            .or_else(|| dotlottie_metadata(path).ok())
            .unwrap_or_default(),
        MediaKind::Video | MediaKind::Audio | MediaKind::Image => wav_metadata(path)
            .or_else(|| {
                probe_source_metadata(path)
                    .ok()
                    .map(|metadata| ImportedMediaMetadata {
                        duration_seconds: metadata.duration_seconds,
                        width: metadata.width,
                        height: metadata.height,
                        fps: metadata.fps,
                    })
                    .or_else(|| {
                        crate::precompose::compatibility::probe_compatibility_source(path).map(
                            |probe| ImportedMediaMetadata {
                                duration_seconds: (probe.duration_seconds > 0.0)
                                    .then_some(probe.duration_seconds),
                                width: probe.video.as_ref().map(|video| video.width),
                                height: probe.video.as_ref().map(|video| video.height),
                                fps: probe.video.as_ref().map(|video| video.fps),
                            },
                        )
                    })
            })
            .filter(|metadata| match kind {
                MediaKind::Video => metadata.width.is_some(),
                MediaKind::Audio => metadata.width.is_none() && metadata.duration_seconds.is_some(),
                MediaKind::Image => metadata.width.is_some(),
                _ => false,
            })
            .unwrap_or_default(),
        _ => ImportedMediaMetadata::default(),
    }
}

fn wav_metadata(path: &Path) -> Option<ImportedMediaMetadata> {
    if !path.extension()?.to_str()?.eq_ignore_ascii_case("wav") {
        return None;
    }
    let reader = hound::WavReader::open(path).ok()?;
    let sample_rate = reader.spec().sample_rate;
    if sample_rate == 0 || reader.duration() == 0 {
        return None;
    }
    Some(ImportedMediaMetadata {
        duration_seconds: Some(f64::from(reader.duration()) / f64::from(sample_rate)),
        width: None,
        height: None,
        fps: None,
    })
}

fn lottie_json_metadata(path: &Path) -> Option<ImportedMediaMetadata> {
    if !path.extension()?.to_str()?.eq_ignore_ascii_case("json") {
        return None;
    }

    lottie_json_metadata_from_bytes(&read_lottie_source_bytes(path).ok()?)
}

#[cfg(feature = "web-host")]
pub(crate) fn valid_lottie_json_bytes(bytes: &[u8]) -> bool {
    lottie_json_metadata_from_bytes(bytes).is_some()
}

fn lottie_json_metadata_from_bytes(bytes: &[u8]) -> Option<ImportedMediaMetadata> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object()?;
    if !object.get("layers").is_some_and(Value::is_array) {
        return None;
    }

    let fps = object.get("fr").and_then(Value::as_f64)?;
    let in_point = object.get("ip").and_then(Value::as_f64).unwrap_or(0.0);
    let out_point = object.get("op").and_then(Value::as_f64)?;
    if !fps.is_finite()
        || fps <= 0.0
        || !in_point.is_finite()
        || !out_point.is_finite()
        || out_point <= in_point
    {
        return None;
    }

    let width = object
        .get("w")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)?;
    let height = object
        .get("h")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)?;
    let duration_seconds = (out_point - in_point) / fps;
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return None;
    }

    Some(ImportedMediaMetadata {
        duration_seconds: Some(duration_seconds),
        width: Some(width),
        height: Some(height),
        fps: Some(fps),
    })
}

pub(crate) struct LottieSourceMetadata {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

pub(crate) fn inspect_lottie_source_metadata(path: &Path) -> Result<LottieSourceMetadata, String> {
    let metadata = if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("json"))
    {
        lottie_json_metadata(path).ok_or_else(|| "Lottie source metadata is invalid".to_string())?
    } else {
        dotlottie_metadata(path)?
    };
    Ok(LottieSourceMetadata {
        duration_seconds: metadata
            .duration_seconds
            .ok_or_else(|| "Lottie duration is missing".to_string())?,
        width: metadata
            .width
            .ok_or_else(|| "Lottie width is missing".to_string())?,
        height: metadata
            .height
            .ok_or_else(|| "Lottie height is missing".to_string())?,
        fps: metadata
            .fps
            .ok_or_else(|| "Lottie frame rate is missing".to_string())?,
    })
}

const MAX_LOTTIE_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn read_lottie_source_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_LOTTIE_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_LOTTIE_IMPORT_BYTES {
        return Err("dotLottie source exceeds import byte limit".into());
    }
    Ok(bytes)
}

fn dotlottie_metadata(path: &Path) -> Result<ImportedMediaMetadata, String> {
    dotlottie_metadata_from_bytes(&read_lottie_source_bytes(path)?)
}

#[cfg(feature = "web-host")]
pub(crate) fn valid_dotlottie_bytes(bytes: &[u8]) -> Result<(), String> {
    dotlottie_metadata_from_bytes(bytes).map(|_| ())
}

fn dotlottie_metadata_from_bytes(bytes: &[u8]) -> Result<ImportedMediaMetadata, String> {
    use std::io::{Cursor, Read};
    use video_creater_precompose_protocol::{archive::validate_dotlottie_archive, WorkerBudgets};
    let budgets = WorkerBudgets {
        max_source_bytes: MAX_LOTTIE_IMPORT_BYTES,
        max_archive_entries: 512,
        max_expanded_archive_bytes: 256 * 1024 * 1024,
        max_compression_ratio: 200,
        max_frames: 1,
        max_pixels_per_frame: 1,
        max_wall_time_ms: 1,
        max_memory_bytes: 1,
        max_output_bytes: 1,
    };
    let ids =
        validate_dotlottie_archive(bytes, None, &budgets).map_err(|error| error.to_string())?;
    if ids.len() != 1 {
        return Err("dotLottie source must contain exactly one animation".into());
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let old_name = format!("animations/{}.json", ids[0]);
    let name = if archive.file_names().any(|name| name == old_name) {
        old_name
    } else {
        format!("a/{}.json", ids[0])
    };
    let entry = archive
        .by_name(&name)
        .map_err(|_| "dotLottie manifest animation is missing".to_string())?;
    if entry.size() > MAX_LOTTIE_IMPORT_BYTES {
        return Err("dotLottie animation exceeds import byte limit".into());
    }
    let mut json = Vec::new();
    entry
        .take(MAX_LOTTIE_IMPORT_BYTES + 1)
        .read_to_end(&mut json)
        .map_err(|error| error.to_string())?;
    if json.len() as u64 > MAX_LOTTIE_IMPORT_BYTES {
        return Err("dotLottie animation exceeds import byte limit".into());
    }
    lottie_json_metadata_from_bytes(&json)
        .ok_or_else(|| "dotLottie animation metadata is invalid".into())
}

#[cfg(test)]
mod mutation_tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::split::{load_split_project, save_split_project};

    #[test]
    fn import_media_reloads_canonical_split_project_before_mutating() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let stale_project = sample_project();
        save_split_project(&project_dir, &stale_project).unwrap();
        let mut canonical_project = stale_project.clone();
        canonical_project.name = "Settings-promoted name".to_string();
        save_split_project(&project_dir, &canonical_project).unwrap();
        let source = temp_dir.path().join("source.mp4");
        std::fs::write(&source, b"test media").unwrap();

        let result = import_media_files(&project_dir, stale_project, &[source]).unwrap();

        let persisted = load_split_project(&project_dir).unwrap();
        assert_eq!(result.project, persisted);
        assert_eq!(persisted.name, "Settings-promoted name");
        assert_eq!(persisted.media.len(), canonical_project.media.len() + 1);
        crate::project::split::replace_split_project_if_revision(
            &project_dir,
            result.project.clone(),
            result.project.content_revision,
        )
        .expect("subsequent CAS save");
    }
}

#[cfg(test)]
mod named_import_tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::split::save_split_project;
    use std::collections::BTreeMap;

    fn save_range_source(root: &Path) -> PathBuf {
        let source = root.join("renders/save-range-1/output.webm");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"test media").unwrap();
        source
    }

    fn imported_with(
        names: &BTreeMap<PathBuf, String>,
        sources: &[PathBuf],
        project_dir: &Path,
    ) -> Vec<MediaAsset> {
        import_media_files_with_names(project_dir, sample_project(), sources, names)
            .unwrap()
            .imported
    }

    #[test]
    fn named_import_uses_the_display_name_for_the_media_and_its_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        save_split_project(&project_dir, &sample_project()).unwrap();
        let source = save_range_source(&project_dir);
        let names = BTreeMap::from([(
            source.clone(),
            "Edison Restoration Demo 00:04–00:09".to_string(),
        )]);

        let imported = imported_with(&names, &[source], &project_dir);

        assert_eq!(
            imported[0].name.as_deref(),
            Some("Edison Restoration Demo 00:04–00:09")
        );
        assert_eq!(
            imported[0].relative_path,
            format!(
                "media/{}-edison-restoration-demo-00-04-00-09.webm",
                imported[0].id
            )
        );
    }

    #[test]
    fn named_import_falls_back_to_the_file_stem() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        save_split_project(&project_dir, &sample_project()).unwrap();
        let source = save_range_source(&project_dir);
        let other = temp_dir.path().join("Other Clip.mp4");
        std::fs::write(&other, b"test media").unwrap();
        let names = BTreeMap::from([(source.clone(), "   ".to_string())]);

        let imported = imported_with(&names, &[source, other], &project_dir);

        assert_eq!(imported[0].name.as_deref(), Some("output"));
        assert!(imported[0].relative_path.ends_with("-output.webm"));
        assert_eq!(imported[1].name.as_deref(), Some("other-clip"));
    }

    #[test]
    fn named_import_truncates_long_names_on_a_char_boundary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        save_split_project(&project_dir, &sample_project()).unwrap();
        let source = save_range_source(&project_dir);
        let long_name = format!("{}{}", "a".repeat(119), "é".repeat(10));
        let names = BTreeMap::from([(source.clone(), long_name)]);

        let imported = imported_with(&names, &[source], &project_dir);

        let name = imported[0].name.clone().unwrap();
        assert_eq!(name.chars().count(), 120);
        assert_eq!(name, format!("{}é", "a".repeat(119)));
    }
}
