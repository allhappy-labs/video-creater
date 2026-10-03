//! Canonical preview preparation shared by desktop and remote viewers.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path};

use crate::precompose::{
    prepare_project_for_render, prepared_media_timeline_span, PrecomposeReport,
};
use crate::project::model::{
    MediaKind, Timeline, TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind,
    VideoProject,
};
use crate::project::mutation::{
    acquire_split_project_artifact_lease, acquire_split_project_mutation_lease,
};
use crate::project::split::{
    load_split_project, resolve_project_relative_path, validate_split_project_write_path,
};
use crate::project::storage;
use crate::render_pipeline::project_export::{
    expand_project_nested_timelines_for_render, PreparedPreviewFrameResult,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPreviewCommandResult {
    pub project: VideoProject,
    pub reports: Vec<PrecomposeReport>,
    pub frame_sequences: Vec<PreparedPreviewFrameSequence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPreviewFrameSequence {
    pub item_id: String,
    pub prepared_media_id: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub fps: f64,
    pub frame_paths: Vec<String>,
}

pub fn prepare_project_preview(
    project_dir: &Path,
    fallback_project: Option<VideoProject>,
    media_id: Option<&str>,
) -> Result<PreparedPreviewCommandResult, String> {
    // Preparation owns the project's derived files for its whole run, so it cannot race a
    // render's precompose publication; the project lease covers its read alone, so a
    // render-length preparation never keeps an editor write waiting.
    let _artifacts = acquire_split_project_artifact_lease(project_dir)?;
    let project = {
        let _project_lease = acquire_split_project_mutation_lease(project_dir)?;
        if storage::project_file_path(project_dir).exists() {
            load_split_project(project_dir).map_err(|error| error.to_string())?
        } else {
            fallback_project
                .ok_or_else(|| "saved project is required for preview preparation".to_string())?
        }
    };
    let project = match media_id {
        Some(media_id) => source_preview_project(project, media_id, project_dir)?,
        None => project,
    };
    let expanded =
        expand_project_nested_timelines_for_render(&project).map_err(pipeline_error_message)?;
    let prepared =
        prepare_project_for_render(project_dir, &expanded).map_err(pipeline_error_message)?;
    let mut frame_sequences = Vec::new();
    for track in &prepared.project.timeline.tracks {
        for item in &track.items {
            let TimelineSource::Media { media_id } = &item.source else {
                continue;
            };
            let Some(report) = prepared
                .reports
                .iter()
                .find(|report| report.prepared_media_id == *media_id)
            else {
                continue;
            };
            if !report.has_frame_sequence() {
                continue;
            }
            let frames_dir = Path::new(&report.intermediate)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join("frames");
            let absolute_frames_dir = project_dir.join(&frames_dir);
            let mut frame_paths = fs::read_dir(&absolute_frames_dir)
                .map_err(|error| format!("prepared preview frames could not be listed: {error}"))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
                .collect::<Vec<_>>();
            frame_paths.sort();
            let frame_paths = frame_paths
                .into_iter()
                .map(|path| {
                    path.strip_prefix(project_dir)
                        .map(|relative| relative.to_string_lossy().to_string())
                        .map_err(|_| {
                            "prepared preview frame escaped the project folder".to_string()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if frame_paths.is_empty() {
                return Err(format!(
                    "prepared preview sequence for {} contains no PNG frames",
                    item.id
                ));
            }
            let fps = prepared
                .project
                .media
                .iter()
                .find(|media| media.id == *media_id)
                .and_then(|media| media.fps)
                .unwrap_or(prepared.project.render_settings.fps);
            // Prepared frames can begin with transition handles before the clip.
            let (start_seconds, duration_seconds) =
                prepared_media_timeline_span(&prepared.project, item);
            frame_sequences.push(PreparedPreviewFrameSequence {
                item_id: item.id.clone(),
                prepared_media_id: media_id.clone(),
                start_seconds,
                duration_seconds,
                fps,
                frame_paths,
            });
        }
    }
    let result = PreparedPreviewCommandResult {
        project: prepared.project,
        reports: prepared.reports,
        frame_sequences,
    };
    record_prepared_resources(project_dir, &result, media_id)?;
    Ok(result)
}

fn source_preview_project(
    mut project: VideoProject,
    media_id: &str,
    project_dir: &Path,
) -> Result<VideoProject, String> {
    let media_index = project
        .media
        .iter()
        .position(|media| media.id == media_id)
        .ok_or_else(|| "preview media is not recorded by this project".to_string())?;
    let mut media = project.media[media_index].clone();
    if media.kind != MediaKind::Lottie {
        return Err("source preview requires Lottie media".into());
    }
    let missing_duration = !media.duration_seconds.is_finite() || media.duration_seconds <= 0.0;
    let missing_width = media.width.is_none_or(|value| value == 0);
    let missing_height = media.height.is_none_or(|value| value == 0);
    let missing_fps = media
        .fps
        .is_none_or(|value| !value.is_finite() || value <= 0.0);
    if missing_duration || missing_width || missing_height || missing_fps {
        let source = resolve_project_relative_path(project_dir, &media.relative_path)
            .map_err(|error| error.to_string())?;
        validate_split_project_write_path(project_dir, &source)
            .map_err(|error| error.to_string())?;
        let metadata = crate::project::import::inspect_lottie_source_metadata(&source)?;
        if missing_duration {
            media.duration_seconds = metadata.duration_seconds;
        }
        if missing_width {
            media.width = Some(metadata.width);
        }
        if missing_height {
            media.height = Some(metadata.height);
        }
        if missing_fps {
            media.fps = Some(metadata.fps);
        }
        // Repair only this preparation's clone; the saved project remains authoritative.
        project.media[media_index] = media.clone();
    }
    project.render_settings.width = media
        .width
        .filter(|value| *value > 0)
        .unwrap_or(project.render_settings.width);
    project.render_settings.height = media
        .height
        .filter(|value| *value > 0)
        .unwrap_or(project.render_settings.height);
    project.render_settings.fps = media
        .fps
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(project.render_settings.fps);
    project.timeline = Timeline {
        duration_seconds: media.duration_seconds,
        tracks: vec![TimelineTrack {
            id: "source-preview".into(),
            name: "Source preview".into(),
            kind: TrackKind::Video,
            locked: false,
            sync_locked: false,
            enabled: true,
            transitions: vec![],
            items: vec![TimelineItem {
                id: format!("source-preview-{media_id}"),
                kind: TimelineItemKind::LottieClip,
                start_seconds: 0.0,
                duration_seconds: media.duration_seconds,
                source: TimelineSource::Media {
                    media_id: media_id.to_owned(),
                },
                label: media.name.clone().unwrap_or_else(|| media_id.to_owned()),
                properties: BTreeMap::from([
                    ("sourceIn".into(), json!(0.0)),
                    ("sourceOut".into(), json!(media.duration_seconds)),
                    ("looping".into(), json!(true)),
                ]),
            }],
        }],
    };
    project.timelines.clear();
    project.active_timeline_id = None;
    Ok(project)
}

const PREVIEW_RESOURCE_MANIFEST: &str = "cache/preview-resources.json";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_PATHS: usize = 65_536;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResourceManifest {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    paths: BTreeSet<String>,
    #[serde(default)]
    groups: BTreeMap<String, PreviewResourceGroup>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PreviewResourceGroup {
    generation: u64,
    paths: BTreeSet<String>,
}

enum PreviewResourceScope<'a> {
    Timeline,
    Source(&'a str),
    Capture(&'a str),
}

impl PreviewResourceScope<'_> {
    fn key(&self) -> String {
        match self {
            Self::Timeline => "timeline-current".into(),
            Self::Source(id) => format!("source:{:x}", Sha256::digest(id.as_bytes())),
            Self::Capture(path) => format!("capture:{:x}", Sha256::digest(path.as_bytes())),
        }
    }
}

fn is_preview_resource(path: &str) -> bool {
    let components = path.split('/').collect::<Vec<_>>();
    if path.len() > 1024
        || components.iter().any(|component| {
            component.is_empty()
                || *component == "."
                || *component == ".."
                || component.contains('\\')
        })
    {
        return false;
    }
    if !Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return false;
    }
    [
        "cache/precompose/",
        "cache/audio-reverse/",
        "cache/audio-denoise/",
        "cache/audio-retime/",
        ".video-creater/cache/compatibility/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
        || (path.starts_with("renders/")
            && path.ends_with("/preview-qa/preview-frames/preview-0001.png"))
}

/// Only exact files recorded by successful Rust preparation or capture are grantable.
/// Canonical project content and arbitrary files under cache/renders are never added here.
pub fn recorded_preview_resource_paths(project_dir: &Path) -> BTreeSet<String> {
    read_resource_manifest(project_dir)
        .map(|manifest| {
            manifest
                .groups
                .into_values()
                .flat_map(|group| group.paths)
                .collect()
        })
        .unwrap_or_default()
}

fn read_resource_manifest(project_dir: &Path) -> Option<PreviewResourceManifest> {
    let path = resolve_project_relative_path(project_dir, PREVIEW_RESOURCE_MANIFEST).ok()?;
    if validate_split_project_write_path(project_dir, &path).is_err()
        || !fs::metadata(&path).is_ok_and(|metadata| metadata.len() <= MAX_MANIFEST_BYTES)
    {
        return None;
    }
    let mut manifest =
        serde_json::from_slice::<PreviewResourceManifest>(&fs::read(path).ok()?).ok()?;
    let total = manifest.paths.len()
        + manifest
            .groups
            .values()
            .map(|group| group.paths.len())
            .sum::<usize>();
    if manifest.schema_version != 1 || total > MAX_MANIFEST_PATHS || manifest.groups.len() > 119 {
        return None;
    }
    // Existing schema-1 flat manifests remain readable and migrate on the next publication.
    if !manifest.paths.is_empty() {
        manifest.groups.insert(
            "legacy".into(),
            PreviewResourceGroup {
                generation: 0,
                paths: std::mem::take(&mut manifest.paths),
            },
        );
    }
    for group in manifest.groups.values_mut() {
        group.paths.retain(|relative| is_preview_resource(relative));
    }
    manifest.groups.retain(|_, group| !group.paths.is_empty());
    Some(manifest)
}

fn record_resources(
    project_dir: &Path,
    new_paths: BTreeSet<String>,
    scope: PreviewResourceScope<'_>,
) -> Result<(), String> {
    let _artifacts = acquire_split_project_artifact_lease(project_dir)?;
    let mut groups = read_resource_manifest(project_dir)
        .map(|manifest| manifest.groups)
        .unwrap_or_default();
    let mut paths = BTreeSet::new();
    for relative in new_paths {
        let path = resolve_project_relative_path(project_dir, &relative)
            .map_err(|error| error.to_string())?;
        validate_split_project_write_path(project_dir, &path).map_err(|error| error.to_string())?;
        if !is_preview_resource(&relative) || !path.is_file() {
            return Err(
                "prepared preview resource is missing or outside the derived preview folders"
                    .into(),
            );
        }
        paths.insert(relative);
    }
    if paths.len() > MAX_MANIFEST_PATHS {
        return Err("prepared preview resource manifest exceeds its limit".into());
    }
    let key = scope.key();
    let generation = groups
        .values()
        .map(|group| group.generation)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    if matches!(scope, PreviewResourceScope::Timeline) {
        groups.remove("timeline-previous");
        if let Some(current) = groups.remove("timeline-current") {
            groups.insert("timeline-previous".into(), current);
        }
    }
    groups.insert(key.clone(), PreviewResourceGroup { generation, paths });
    for (prefix, limit) in [("source:", 16), ("capture:", 100)] {
        while groups.keys().filter(|key| key.starts_with(prefix)).count() > limit {
            let oldest = groups
                .iter()
                .filter(|(key, _)| key.starts_with(prefix))
                .min_by_key(|(_, group)| group.generation)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                groups.remove(&oldest);
            }
        }
    }
    // Keep the current timeline and the resource set being published; retire older source/capture
    // sets before a cumulative budget could reject a perfectly valid new preparation.
    while groups
        .values()
        .map(|group| group.paths.len())
        .sum::<usize>()
        > MAX_MANIFEST_PATHS
    {
        let oldest = groups
            .iter()
            .filter(|(candidate, _)| **candidate != key && candidate.as_str() != "timeline-current")
            .min_by_key(|(_, group)| group.generation)
            .map(|(key, _)| key.clone());
        if let Some(oldest) = oldest {
            groups.remove(&oldest);
        } else {
            return Err("active preview resource sets exceed the manifest limit".into());
        }
    }
    let path = project_dir.join(PREVIEW_RESOURCE_MANIFEST);
    validate_split_project_write_path(project_dir, &path).map_err(|error| error.to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "preview manifest parent is missing".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&PreviewResourceManifest {
        schema_version: 1,
        paths: BTreeSet::new(),
        groups,
    })
    .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("prepared preview resource manifest exceeds its byte limit".into());
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(&path)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn record_prepared_resources(
    project_dir: &Path,
    result: &PreparedPreviewCommandResult,
    media_id: Option<&str>,
) -> Result<(), String> {
    let paths = result
        .reports
        .iter()
        .map(|report| report.intermediate.clone())
        .chain(
            result
                .frame_sequences
                .iter()
                .flat_map(|sequence| sequence.frame_paths.iter().cloned()),
        )
        .collect();
    record_resources(
        project_dir,
        paths,
        media_id
            .map(PreviewResourceScope::Source)
            .unwrap_or(PreviewResourceScope::Timeline),
    )
}

pub fn record_captured_preview_resource(
    project_dir: &Path,
    result: &PreparedPreviewFrameResult,
) -> Result<(), String> {
    record_resources(
        project_dir,
        BTreeSet::from([result.preview_frame.clone()]),
        PreviewResourceScope::Capture(&result.preview_frame),
    )
}

fn pipeline_error_message(errors: Vec<crate::render_pipeline::error::PipelineError>) -> String {
    errors
        .into_iter()
        .map(|error| {
            let details = if error.details.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    error
                        .details
                        .into_iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                "{}: {} Fix: {}{}",
                error.path, error.message, error.fix, details
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_frame(root: &Path, name: &str) -> String {
        let relative = format!("cache/precompose/{name}/frames/frame-000000.png");
        fs::create_dir_all(root.join(&relative).parent().unwrap()).unwrap();
        fs::write(root.join(&relative), b"frame").unwrap();
        relative
    }

    #[test]
    fn source_resource_sets_are_replaced_and_bounded_independently_of_timeline() {
        let root = tempfile::tempdir().unwrap();
        let timeline = write_frame(root.path(), "timeline");
        record_resources(
            root.path(),
            BTreeSet::from([timeline.clone()]),
            PreviewResourceScope::Timeline,
        )
        .unwrap();
        for revision in 0..20 {
            let source = write_frame(root.path(), &format!("source-{revision}"));
            record_resources(
                root.path(),
                BTreeSet::from([source]),
                PreviewResourceScope::Source(&format!("media-{revision}")),
            )
            .unwrap();
        }
        let replacement = write_frame(root.path(), "source-19-new");
        record_resources(
            root.path(),
            BTreeSet::from([replacement.clone()]),
            PreviewResourceScope::Source("media-19"),
        )
        .unwrap();
        let paths = recorded_preview_resource_paths(root.path());
        assert_eq!(paths.len(), 17);
        assert!(paths.contains(&timeline));
        assert!(paths.contains(&replacement));
        assert!(!paths.contains("cache/precompose/source-19/frames/frame-000000.png"));
        assert!(!paths.contains("cache/precompose/source-0/frames/frame-000000.png"));
    }

    #[test]
    fn legacy_flat_manifest_migrates_without_authorizing_arbitrary_paths() {
        let root = tempfile::tempdir().unwrap();
        let legacy = write_frame(root.path(), "legacy");
        fs::write(
            root.path().join(PREVIEW_RESOURCE_MANIFEST),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1,
                "paths": [legacy, "private.txt", "cache/precompose/../private.txt"]
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            recorded_preview_resource_paths(root.path()),
            BTreeSet::from([legacy.clone()])
        );
        let fresh = write_frame(root.path(), "fresh");
        record_resources(
            root.path(),
            BTreeSet::from([fresh.clone()]),
            PreviewResourceScope::Timeline,
        )
        .unwrap();
        assert_eq!(
            recorded_preview_resource_paths(root.path()),
            BTreeSet::from([legacy, fresh])
        );
        let manifest = read_resource_manifest(root.path()).unwrap();
        assert!(manifest.paths.is_empty());
        assert!(manifest.groups.contains_key("legacy"));
    }

    #[test]
    fn capture_resource_sets_retire_old_captures_without_removing_timeline() {
        let root = tempfile::tempdir().unwrap();
        let timeline = write_frame(root.path(), "timeline");
        record_resources(
            root.path(),
            BTreeSet::from([timeline.clone()]),
            PreviewResourceScope::Timeline,
        )
        .unwrap();
        for revision in 0..102 {
            let relative =
                format!("renders/capture-{revision}/preview-qa/preview-frames/preview-0001.png");
            fs::create_dir_all(root.path().join(&relative).parent().unwrap()).unwrap();
            fs::write(root.path().join(&relative), b"frame").unwrap();
            record_resources(
                root.path(),
                BTreeSet::from([relative.clone()]),
                PreviewResourceScope::Capture(&relative),
            )
            .unwrap();
        }
        let paths = recorded_preview_resource_paths(root.path());
        assert_eq!(paths.len(), 101);
        assert!(paths.contains(&timeline));
        assert!(!paths.contains("renders/capture-0/preview-qa/preview-frames/preview-0001.png"));
        assert!(paths.contains("renders/capture-101/preview-qa/preview-frames/preview-0001.png"));
    }

    #[test]
    fn resource_membership_does_not_probe_unrequested_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("cache")).unwrap();
        let missing = "cache/precompose/not-yet-requested/frame.png";
        fs::write(
            root.path().join(PREVIEW_RESOURCE_MANIFEST),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1, "paths": [missing]
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            recorded_preview_resource_paths(root.path()),
            BTreeSet::from([missing.into()])
        );
    }

    #[test]
    fn repeated_preview_revisions_keep_only_the_latest_two_resource_sets() {
        let root = tempfile::tempdir().unwrap();
        for revision in 0..20 {
            let relative = format!("cache/precompose/revision-{revision}/frames/frame-000000.png");
            fs::create_dir_all(root.path().join(&relative).parent().unwrap()).unwrap();
            fs::write(root.path().join(&relative), b"frame").unwrap();
            record_resources(
                root.path(),
                BTreeSet::from([relative]),
                PreviewResourceScope::Timeline,
            )
            .unwrap();
        }
        let paths = recorded_preview_resource_paths(root.path());
        assert_eq!(paths.len(), 2);
        assert!(paths.contains("cache/precompose/revision-19/frames/frame-000000.png"));
        assert!(paths.contains("cache/precompose/revision-18/frames/frame-000000.png"));
        assert!(!paths.contains("cache/precompose/revision-0/frames/frame-000000.png"));
    }
}
