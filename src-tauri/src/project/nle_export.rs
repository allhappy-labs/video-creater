use super::model::{
    GeneratedAsset, MediaAsset, MediaKind, ProjectExportArtifact, ProjectExportArtifactKind,
    Timeline, TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TrackKind,
    VideoProject,
};
use super::nested_opacity::{
    apply_nested_opacity_curves, fade_opacity_curve_for_item, opacity_curve_for_item,
    validate_nested_wrapper_opacity_keyframes, AbsoluteOpacityCurve,
};
use super::reverse::truncated_tail_source_property;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

mod fcpxml_spine;
mod fcpxml_storylines;
mod fcpxml_titles;
mod fcpxml_transitions;
mod transitions;

use transitions::NleTransitionSpan;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NleXmlFormat {
    PremiereXmeml,
    DavinciFcpxml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NleXmlExport {
    pub filename: String,
    pub mime_type: String,
    pub xml: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum NleXmlExportError {
    #[error("render settings fps must be finite and greater than zero")]
    InvalidFrameRate,
    #[error("timeline item `{item_id}` references missing media `{media_id}`")]
    MissingMedia { item_id: String, media_id: String },
    #[error("timeline item `{item_id}` has an unsupported source for NLE export")]
    UnsupportedSource { item_id: String },
    #[error("timeline item `{item_id}` has an invalid source range")]
    InvalidSourceRange { item_id: String },
    #[error("nested timeline `{timeline_id}` was not found")]
    MissingTimeline { timeline_id: String },
    #[error("nested timeline cycle includes `{timeline_id}`")]
    NestedTimelineCycle { timeline_id: String },
    #[error("nested timeline wrapper `{item_id}` has unsupported properties")]
    UnsupportedNestedWrapper { item_id: String },
    #[error("could not write NLE XML export `{path}`: {message}")]
    Io { path: String, message: String },
}

#[derive(Debug, Clone)]
struct NleClip<'a> {
    item: &'a TimelineItem,
    media: &'a MediaAsset,
    generated_asset: Option<&'a GeneratedAsset>,
    track_kind: TrackKind,
    track_lane: i64,
    track_enabled: bool,
    timeline_start_frames: i64,
    timeline_end_frames: i64,
    duration_frames: i64,
    source_in_frames: i64,
    source_out_frames: i64,
    speed: f64,
    sequence_width: u32,
    sequence_height: u32,
    incoming_transition: Option<NleTransitionSpan>,
    outgoing_transition: Option<NleTransitionSpan>,
}

#[derive(Debug, Clone)]
struct NleCaption<'a> {
    item: &'a TimelineItem,
    text: &'a str,
    timeline_start_frames: i64,
    duration_frames: i64,
    timeline_end_frames: i64,
}

#[derive(Debug, Clone)]
struct NleTextOverlay<'a> {
    item: &'a TimelineItem,
    text: &'a str,
    timeline_start_frames: i64,
    duration_frames: i64,
    timeline_end_frames: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NleScalarKeyframe {
    when_frames: i64,
    value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NlePointKeyframe {
    when_frames: i64,
    x: f64,
    y: f64,
}

pub fn export_project_timeline_to_nle_xml(
    project: &VideoProject,
    format: NleXmlFormat,
) -> Result<NleXmlExport, NleXmlExportError> {
    let expanded_project = project_with_expanded_nested_timelines(project)?;
    let project = &expanded_project;
    let fps = normalized_fps(project.render_settings.fps)?;
    let mut clips = collect_nle_clips(project, fps)?;
    transitions::attach_nle_transitions(project, &mut clips, fps);
    let captions = collect_nle_captions(project, fps)?;
    let text_overlays = collect_nle_text_overlays(project, fps)?;
    let xml = match format {
        NleXmlFormat::PremiereXmeml => {
            render_premiere_xmeml(project, &clips, &captions, &text_overlays, fps)
        }
        NleXmlFormat::DavinciFcpxml => {
            fcpxml_spine::render_davinci_fcpxml(project, &clips, &captions, &text_overlays, fps)
        }
    };
    let filename = match format {
        NleXmlFormat::PremiereXmeml => format!("{}-premiere.xml", safe_filename(&project.id)),
        NleXmlFormat::DavinciFcpxml => format!("{}-davinci.fcpxml", safe_filename(&project.id)),
    };

    Ok(NleXmlExport {
        filename,
        mime_type: "application/xml".to_string(),
        xml,
    })
}

/// Flatten sequence sources before interchange export. This makes the NLE receive
/// the same direct source clips, timing, opacity, and normalized canvas transform
/// semantics as the native render plan.
fn project_with_expanded_nested_timelines(
    project: &VideoProject,
) -> Result<VideoProject, NleXmlExportError> {
    let active_timeline_id = project.active_timeline_id.as_deref().unwrap_or("main");
    let mut timelines_by_id = project
        .timelines
        .iter()
        .map(|entry| (entry.id.as_str(), &entry.timeline))
        .collect::<BTreeMap<_, _>>();
    timelines_by_id.insert(active_timeline_id, &project.timeline);

    let mut expanded = project.clone();
    expanded.timeline = expand_timeline_for_nle(
        &project.timeline,
        NleExpansionContext {
            timelines_by_id: &timelines_by_id,
            ancestors: vec![active_timeline_id],
            offset_seconds: 0.0,
            end_seconds: f64::INFINITY,
            parent_enabled: true,
            parent_opacity_curves: Vec::new(),
            parent_volume_db: 0.0,
            parent_transform: NestedCanvasTransform::identity(),
            fps: project.render_settings.fps,
            namespace: "root".to_string(),
        },
    )?;
    Ok(expanded)
}

struct NleExpansionContext<'a> {
    timelines_by_id: &'a BTreeMap<&'a str, &'a Timeline>,
    ancestors: Vec<&'a str>,
    offset_seconds: f64,
    end_seconds: f64,
    parent_enabled: bool,
    parent_opacity_curves: Vec<AbsoluteOpacityCurve>,
    parent_volume_db: f64,
    parent_transform: NestedCanvasTransform,
    fps: f64,
    namespace: String,
}

fn expand_timeline_for_nle<'a>(
    timeline: &Timeline,
    context: NleExpansionContext<'a>,
) -> Result<Timeline, NleXmlExportError> {
    let NleExpansionContext {
        timelines_by_id,
        ancestors,
        offset_seconds,
        end_seconds,
        parent_enabled,
        parent_opacity_curves,
        parent_volume_db,
        parent_transform,
        fps,
        namespace,
    } = context;
    let mut tracks = Vec::new();
    let mut all_nested_tracks = Vec::new();
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        let effective_enabled = parent_enabled && track.enabled;
        let mut own_items = Vec::new();
        let mut own_source_item_ids = Vec::new();
        let mut nested_tracks = Vec::new();
        for item in &track.items {
            let item_start = offset_seconds + item.start_seconds;
            let item_end = (item_start + item.duration_seconds).min(end_seconds);
            if item_end <= item_start {
                continue;
            }
            let mut expanded_item = item.clone();
            if namespace != "root" {
                expanded_item.id = format!("{namespace}:{}", item.id);
            }
            expanded_item.start_seconds = item_start;
            expanded_item.duration_seconds = item_end - item_start;

            let TimelineSource::Timeline { timeline_id } = &item.source else {
                apply_nested_opacity_curves(
                    &mut expanded_item,
                    &parent_opacity_curves,
                    item_start,
                    item_end,
                    fps,
                )
                .map_err(|_| NleXmlExportError::UnsupportedNestedWrapper {
                    item_id: item.id.clone(),
                })?;
                compose_nested_transform(&mut expanded_item, parent_transform)?;
                compose_nested_volume_db(&mut expanded_item, parent_volume_db)?;
                if item_end < item_start + item.duration_seconds {
                    // Same trim as render: reversed clips keep the end of their window.
                    let speed = timeline_property_positive_number(item, "speed").unwrap_or(1.0);
                    if let Some((key, seconds)) = truncated_tail_source_property(
                        item,
                        speed,
                        item.duration_seconds,
                        expanded_item.duration_seconds,
                    ) {
                        expanded_item
                            .properties
                            .insert(key.to_string(), serde_json::json!(seconds));
                    }
                }
                own_items.push(expanded_item);
                own_source_item_ids.push(item.id.as_str());
                continue;
            };

            if ancestors.contains(&timeline_id.as_str()) {
                return Err(NleXmlExportError::NestedTimelineCycle {
                    timeline_id: timeline_id.clone(),
                });
            }
            let nested_timeline = timelines_by_id.get(timeline_id.as_str()).ok_or_else(|| {
                NleXmlExportError::MissingTimeline {
                    timeline_id: timeline_id.clone(),
                }
            })?;
            let wrapper = nested_timeline_wrapper_properties(item)?;
            let wrapper_opacity_curve =
                opacity_curve_for_item(item, item_start, item.duration_seconds).map_err(|_| {
                    NleXmlExportError::UnsupportedNestedWrapper {
                        item_id: item.id.clone(),
                    }
                })?;
            let mut nested_opacity_curves = parent_opacity_curves.clone();
            nested_opacity_curves.push(wrapper_opacity_curve);
            if let Some(fade_curve) =
                fade_opacity_curve_for_item(item, item_start, item.duration_seconds).map_err(
                    |_| NleXmlExportError::UnsupportedNestedWrapper {
                        item_id: item.id.clone(),
                    },
                )?
            {
                nested_opacity_curves.push(fade_curve);
            }
            let mut nested_ancestors = ancestors.clone();
            nested_ancestors.push(timeline_id);
            nested_tracks.extend(
                expand_timeline_for_nle(
                    nested_timeline,
                    NleExpansionContext {
                        timelines_by_id,
                        ancestors: nested_ancestors,
                        offset_seconds: item_start,
                        end_seconds: item_end,
                        parent_enabled: effective_enabled,
                        parent_opacity_curves: nested_opacity_curves,
                        parent_volume_db: parent_volume_db + wrapper.volume_db,
                        parent_transform: parent_transform.compose(wrapper.transform),
                        fps,
                        namespace: format!("{namespace}:{track_index}:{}", item.id),
                    },
                )?
                .tracks,
            );
        }
        tracks.push(TimelineTrack {
            transitions: transitions::expanded_track_transitions(
                track,
                &own_source_item_ids,
                &namespace,
            ),
            id: format!("{namespace}:{}:{track_index}", track.id),
            name: track.name.clone(),
            kind: track.kind.clone(),
            locked: track.locked,
            sync_locked: track.sync_locked,
            enabled: effective_enabled,
            items: own_items,
        });
        all_nested_tracks.extend(nested_tracks);
    }
    tracks.extend(all_nested_tracks);
    Ok(Timeline {
        duration_seconds: (timeline.duration_seconds + offset_seconds).min(end_seconds),
        tracks,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NestedCanvasTransform {
    center_x: f64,
    center_y: f64,
    width: f64,
    height: f64,
    flip_horizontal: bool,
    flip_vertical: bool,
}

impl NestedCanvasTransform {
    fn identity() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }

    fn compose(self, child: Self) -> Self {
        let child_center_x = if self.flip_horizontal {
            1.0 - child.center_x
        } else {
            child.center_x
        };
        let child_center_y = if self.flip_vertical {
            1.0 - child.center_y
        } else {
            child.center_y
        };
        Self {
            center_x: self.center_x + (child_center_x - 0.5) * self.width,
            center_y: self.center_y + (child_center_y - 0.5) * self.height,
            width: self.width * child.width,
            height: self.height * child.height,
            flip_horizontal: self.flip_horizontal != child.flip_horizontal,
            flip_vertical: self.flip_vertical != child.flip_vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NestedTimelineWrapperProperties {
    volume_db: f64,
    transform: NestedCanvasTransform,
}

fn nested_timeline_wrapper_properties(
    item: &TimelineItem,
) -> Result<NestedTimelineWrapperProperties, NleXmlExportError> {
    if !item.properties.keys().all(|key| {
        matches!(
            key.as_str(),
            "opacity" | "keyframes" | "transform" | "volumeDb" | "fadeInSeconds" | "fadeOutSeconds"
        )
    }) {
        return Err(NleXmlExportError::UnsupportedNestedWrapper {
            item_id: item.id.clone(),
        });
    }
    validate_nested_wrapper_opacity_keyframes(item).map_err(|_| {
        NleXmlExportError::UnsupportedNestedWrapper {
            item_id: item.id.clone(),
        }
    })?;
    let volume_db = item
        .properties
        .get("volumeDb")
        .map(|value| {
            value
                .as_f64()
                .filter(|volume_db| volume_db.is_finite() && (-60.0..=24.0).contains(volume_db))
                .ok_or_else(|| NleXmlExportError::UnsupportedNestedWrapper {
                    item_id: item.id.clone(),
                })
        })
        .transpose()?
        .unwrap_or(0.0);
    Ok(NestedTimelineWrapperProperties {
        volume_db,
        transform: nested_canvas_transform(item)?,
    })
}

fn nested_canvas_transform(
    item: &TimelineItem,
) -> Result<NestedCanvasTransform, NleXmlExportError> {
    let Some(transform_value) = item.properties.get("transform") else {
        return Ok(NestedCanvasTransform::identity());
    };
    let Some(transform) = transform_value.as_object() else {
        return Err(NleXmlExportError::UnsupportedNestedWrapper {
            item_id: item.id.clone(),
        });
    };
    let number = |key: &str, fallback: f64, minimum: f64| {
        let value = transform
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(fallback);
        if !value.is_finite() || !(minimum..=1.0).contains(&value) {
            return Err(NleXmlExportError::UnsupportedNestedWrapper {
                item_id: item.id.clone(),
            });
        }
        Ok(value)
    };
    Ok(NestedCanvasTransform {
        center_x: number("centerX", 0.5, 0.0)?,
        center_y: number("centerY", 0.5, 0.0)?,
        width: number("width", 1.0, f64::MIN_POSITIVE)?,
        height: number("height", 1.0, f64::MIN_POSITIVE)?,
        flip_horizontal: transform
            .get("flipHorizontal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        flip_vertical: transform
            .get("flipVertical")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn compose_nested_volume_db(
    item: &mut TimelineItem,
    parent_volume_db: f64,
) -> Result<(), NleXmlExportError> {
    if parent_volume_db.abs() <= f64::EPSILON || !matches!(item.kind, TimelineItemKind::AudioClip) {
        return Ok(());
    }
    let child_volume_db = item
        .properties
        .get("volumeDb")
        .map(|value| {
            value
                .as_f64()
                .filter(|volume_db| volume_db.is_finite() && (-60.0..=24.0).contains(volume_db))
                .ok_or_else(|| NleXmlExportError::UnsupportedNestedWrapper {
                    item_id: item.id.clone(),
                })
        })
        .transpose()?
        .unwrap_or(0.0);
    let composed_volume_db = child_volume_db + parent_volume_db;
    if !(-60.0..=24.0).contains(&composed_volume_db) {
        return Err(NleXmlExportError::UnsupportedNestedWrapper {
            item_id: item.id.clone(),
        });
    }
    item.properties.insert(
        "volumeDb".to_string(),
        serde_json::json!(composed_volume_db),
    );
    Ok(())
}

fn compose_nested_transform(
    item: &mut TimelineItem,
    parent_transform: NestedCanvasTransform,
) -> Result<(), NleXmlExportError> {
    if parent_transform == NestedCanvasTransform::identity()
        || matches!(item.kind, TimelineItemKind::AudioClip)
    {
        return Ok(());
    }
    let composed = parent_transform.compose(nested_canvas_transform(item)?);
    item.properties.insert(
        "transform".to_string(),
        serde_json::json!({
            "centerX": composed.center_x,
            "centerY": composed.center_y,
            "width": composed.width,
            "height": composed.height,
            "flipHorizontal": composed.flip_horizontal,
            "flipVertical": composed.flip_vertical,
        }),
    );
    Ok(())
}

pub fn nle_xml_export_artifact(
    export: &NleXmlExport,
    format: NleXmlFormat,
    job_id: &str,
    created_at: &str,
) -> ProjectExportArtifact {
    ProjectExportArtifact {
        schema_version: 1,
        id: job_id.to_string(),
        kind: ProjectExportArtifactKind::NleXml,
        format: format.artifact_format().to_string(),
        path: format!("exports/{}", export.filename),
        mime_type: export.mime_type.clone(),
        job_id: Some(job_id.to_string()),
        created_at: created_at.to_string(),
    }
}

pub fn write_nle_xml_export(
    project_dir: &Path,
    export: &NleXmlExport,
) -> Result<PathBuf, NleXmlExportError> {
    let exports_dir = project_dir.join("exports");
    std::fs::create_dir_all(&exports_dir).map_err(|error| NleXmlExportError::Io {
        path: exports_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let export_path = exports_dir.join(&export.filename);
    let temporary_path = exports_dir.join(format!(".{}.tmp", export.filename));
    std::fs::write(&temporary_path, &export.xml).map_err(|error| NleXmlExportError::Io {
        path: temporary_path.display().to_string(),
        message: error.to_string(),
    })?;
    std::fs::rename(&temporary_path, &export_path).map_err(|error| NleXmlExportError::Io {
        path: export_path.display().to_string(),
        message: error.to_string(),
    })?;

    Ok(export_path)
}

impl NleXmlFormat {
    fn artifact_format(self) -> &'static str {
        match self {
            NleXmlFormat::PremiereXmeml => "premiereXmeml",
            NleXmlFormat::DavinciFcpxml => "davinciFcpxml",
        }
    }
}

fn collect_nle_clips<'a>(
    project: &'a VideoProject,
    fps: i64,
) -> Result<Vec<NleClip<'a>>, NleXmlExportError> {
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media))
        .collect::<BTreeMap<_, _>>();
    let generated_asset_by_output_media_id = project
        .generated_assets
        .iter()
        .flat_map(|asset| {
            asset
                .outputs
                .iter()
                .map(move |output| (output.media_id.as_str(), asset))
        })
        .collect::<BTreeMap<_, _>>();
    let mut clips = Vec::new();
    let visual_track_count = project
        .timeline
        .tracks
        .iter()
        .filter(|track| matches!(track.kind, TrackKind::Video))
        .count() as i64;
    let mut visual_ordinal = 0_i64;
    let mut audio_ordinal = 0_i64;

    for track in &project.timeline.tracks {
        if !matches!(track.kind, TrackKind::Video | TrackKind::Audio) {
            continue;
        }
        let track_lane = match track.kind {
            TrackKind::Video => {
                // FCPXML's primary storyline lives directly in the spine and
                // therefore has no `lane` attribute. Higher video tracks are
                // connected clips in positive lanes above that storyline.
                let lane = visual_track_count - visual_ordinal - 1;
                visual_ordinal += 1;
                lane
            }
            TrackKind::Audio => {
                let lane = -(audio_ordinal + 1);
                audio_ordinal += 1;
                lane
            }
            _ => 0,
        };

        for item in &track.items {
            if !matches!(
                item.kind,
                TimelineItemKind::VideoClip
                    | TimelineItemKind::ImageClip
                    | TimelineItemKind::GeneratedClip
                    | TimelineItemKind::AudioClip
            ) {
                continue;
            }

            let media_id = match &item.source {
                TimelineSource::Media { media_id } => media_id,
                _ => {
                    return Err(NleXmlExportError::UnsupportedSource {
                        item_id: item.id.clone(),
                    });
                }
            };
            let media = media_by_id.get(media_id.as_str()).ok_or_else(|| {
                NleXmlExportError::MissingMedia {
                    item_id: item.id.clone(),
                    media_id: media_id.clone(),
                }
            })?;
            if matches!(media.kind, MediaKind::Lottie) {
                continue;
            }
            let source_in_seconds = timeline_property_seconds(item, "sourceIn").unwrap_or(0.0);
            let speed = if item.properties.contains_key("speed") {
                timeline_property_positive_number(item, "speed").unwrap_or(f64::NAN)
            } else {
                1.0
            };
            let source_out_seconds = timeline_property_seconds(item, "sourceOut")
                .unwrap_or(source_in_seconds + item.duration_seconds * speed);

            if !item.start_seconds.is_finite()
                || !item.duration_seconds.is_finite()
                || item.start_seconds < 0.0
                || item.duration_seconds <= 0.0
                || !speed.is_finite()
                || speed <= 0.0
                || !source_in_seconds.is_finite()
                || !source_out_seconds.is_finite()
                || source_in_seconds < 0.0
                || source_out_seconds <= source_in_seconds
                || (media.duration_seconds.is_finite()
                    && media.duration_seconds > 0.0
                    && source_out_seconds > media.duration_seconds)
            {
                return Err(NleXmlExportError::InvalidSourceRange {
                    item_id: item.id.clone(),
                });
            }

            let timeline_start_frames = seconds_to_frames(item.start_seconds, fps);
            let duration_frames = seconds_to_frames(item.duration_seconds, fps);
            let source_in_frames = seconds_to_frames(source_in_seconds, fps);
            let source_out_frames = seconds_to_frames(source_out_seconds, fps);

            clips.push(NleClip {
                item,
                media,
                generated_asset: generated_asset_by_output_media_id
                    .get(media.id.as_str())
                    .copied(),
                track_kind: track.kind.clone(),
                track_lane,
                track_enabled: track.enabled,
                timeline_start_frames,
                timeline_end_frames: timeline_start_frames + duration_frames,
                duration_frames,
                source_in_frames,
                source_out_frames,
                speed,
                sequence_width: project.render_settings.width,
                sequence_height: project.render_settings.height,
                incoming_transition: None,
                outgoing_transition: None,
            });
        }
    }

    Ok(clips)
}

fn collect_nle_captions<'a>(
    project: &'a VideoProject,
    fps: i64,
) -> Result<Vec<NleCaption<'a>>, NleXmlExportError> {
    let mut captions = Vec::new();
    for track in &project.timeline.tracks {
        if !matches!(track.kind, TrackKind::Caption) {
            continue;
        }

        for item in &track.items {
            if !matches!(item.kind, TimelineItemKind::Caption) {
                continue;
            }

            let TimelineSource::Text { text } = &item.source else {
                return Err(NleXmlExportError::UnsupportedSource {
                    item_id: item.id.clone(),
                });
            };

            if !item.start_seconds.is_finite()
                || !item.duration_seconds.is_finite()
                || item.start_seconds < 0.0
                || item.duration_seconds <= 0.0
            {
                return Err(NleXmlExportError::InvalidSourceRange {
                    item_id: item.id.clone(),
                });
            }

            let timeline_start_frames = seconds_to_frames(item.start_seconds, fps);
            let duration_frames = seconds_to_frames(item.duration_seconds, fps);
            captions.push(NleCaption {
                item,
                text,
                timeline_start_frames,
                duration_frames,
                timeline_end_frames: timeline_start_frames + duration_frames,
            });
        }
    }

    Ok(captions)
}

fn collect_nle_text_overlays<'a>(
    project: &'a VideoProject,
    fps: i64,
) -> Result<Vec<NleTextOverlay<'a>>, NleXmlExportError> {
    let mut overlays = Vec::new();
    for track in &project.timeline.tracks {
        if !matches!(track.kind, TrackKind::Overlay) {
            continue;
        }

        for item in &track.items {
            if !matches!(item.kind, TimelineItemKind::Overlay) {
                continue;
            }

            let TimelineSource::Text { text } = &item.source else {
                continue;
            };

            if !item.start_seconds.is_finite()
                || !item.duration_seconds.is_finite()
                || item.start_seconds < 0.0
                || item.duration_seconds <= 0.0
            {
                return Err(NleXmlExportError::InvalidSourceRange {
                    item_id: item.id.clone(),
                });
            }

            let timeline_start_frames = seconds_to_frames(item.start_seconds, fps);
            let duration_frames = seconds_to_frames(item.duration_seconds, fps);
            overlays.push(NleTextOverlay {
                item,
                text,
                timeline_start_frames,
                duration_frames,
                timeline_end_frames: timeline_start_frames + duration_frames,
            });
        }
    }

    Ok(overlays)
}

fn render_premiere_xmeml(
    project: &VideoProject,
    clips: &[NleClip<'_>],
    captions: &[NleCaption<'_>],
    text_overlays: &[NleTextOverlay<'_>],
    fps: i64,
) -> String {
    let duration_frames = seconds_to_frames(project.timeline.duration_seconds, fps);
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<xmeml version=\"5\">\n");
    xml.push_str("  <sequence>\n");
    push_text_element(&mut xml, 4, "name", &project.name);
    push_number_element(&mut xml, 4, "duration", duration_frames);
    xml.push_str("    <rate>\n");
    push_number_element(&mut xml, 6, "timebase", fps);
    xml.push_str("      <ntsc>FALSE</ntsc>\n");
    xml.push_str("    </rate>\n");
    xml.push_str("    <media>\n");
    xml.push_str("      <video>\n");
    xml.push_str("        <format>\n");
    xml.push_str("          <samplecharacteristics>\n");
    push_number_element(&mut xml, 12, "width", project.render_settings.width);
    push_number_element(&mut xml, 12, "height", project.render_settings.height);
    xml.push_str("          </samplecharacteristics>\n");
    xml.push_str("        </format>\n");
    xml.push_str("        <track>\n");
    for clip in clips
        .iter()
        .filter(|clip| matches!(clip.track_kind, TrackKind::Video))
    {
        push_xmeml_clip(&mut xml, clip, fps);
    }
    xml.push_str("        </track>\n");
    if !captions.is_empty() {
        xml.push_str("        <track>\n");
        for caption in captions {
            push_xmeml_caption(&mut xml, caption);
        }
        xml.push_str("        </track>\n");
    }
    if !text_overlays.is_empty() {
        xml.push_str("        <track>\n");
        for overlay in text_overlays {
            push_xmeml_text_overlay(&mut xml, overlay);
        }
        xml.push_str("        </track>\n");
    }
    xml.push_str("      </video>\n");
    xml.push_str("      <audio>\n");
    xml.push_str("        <track>\n");
    for clip in clips
        .iter()
        .filter(|clip| matches!(clip.track_kind, TrackKind::Audio))
    {
        push_xmeml_clip(&mut xml, clip, fps);
    }
    for (index, clip) in clips.iter().enumerate() {
        if xmeml_has_source_audio(clip) {
            push_xmeml_video_source_audio_clip(&mut xml, clips, index, fps);
        }
    }
    xml.push_str("        </track>\n");
    xml.push_str("      </audio>\n");
    xml.push_str("    </media>\n");
    xml.push_str("  </sequence>\n");
    xml.push_str("</xmeml>\n");
    xml
}

fn push_xmeml_text_overlay(xml: &mut String, overlay: &NleTextOverlay<'_>) {
    push_xmeml_text_generator(
        xml,
        &overlay.item.id,
        &overlay.item.label,
        overlay.timeline_start_frames,
        overlay.timeline_end_frames,
        overlay.text,
    );
}

fn push_xmeml_caption(xml: &mut String, caption: &NleCaption<'_>) {
    push_xmeml_text_generator(
        xml,
        &caption.item.id,
        &caption.item.label,
        caption.timeline_start_frames,
        caption.timeline_end_frames,
        caption.text,
    );
}

fn push_xmeml_text_generator(
    xml: &mut String,
    item_id: &str,
    label: &str,
    start_frames: i64,
    end_frames: i64,
    text: &str,
) {
    xml.push_str(&format!(
        "          <generatoritem id=\"{}\">\n",
        xml_escape(item_id)
    ));
    push_text_element(xml, 12, "name", label);
    push_number_element(xml, 12, "start", start_frames);
    push_number_element(xml, 12, "end", end_frames);
    xml.push_str("            <effect>\n");
    push_text_element(xml, 14, "name", "Text");
    push_text_element(xml, 14, "effectid", "Text");
    xml.push_str("              <parameter>\n");
    push_text_element(xml, 16, "name", "Text");
    push_text_element(xml, 16, "value", text);
    xml.push_str("              </parameter>\n");
    xml.push_str("            </effect>\n");
    xml.push_str("          </generatoritem>\n");
}

fn push_xmeml_clip(xml: &mut String, clip: &NleClip<'_>, fps: i64) {
    let file_duration_frames = seconds_to_frames(clip.media.duration_seconds, fps);
    let has_source_audio = xmeml_has_source_audio(clip);
    let (incoming, outgoing) = transitions::xmeml_clip_transitions(clip);
    xml.push_str(&format!(
        "          <clipitem id=\"{}\">\n",
        xml_escape(&clip.item.id)
    ));
    push_text_element(xml, 12, "name", &clip.item.label);
    if has_source_audio {
        push_text_element(xml, 12, "masterclipid", &xmeml_masterclip_id(&clip.item.id));
    }
    transitions::push_xmeml_clip_bounds(xml, clip, incoming, outgoing);
    let note = clip_export_note(
        clip.generated_asset,
        nle_clip_limitation_note(clip, "premiereXmeml"),
    );
    if let Some(note) = note {
        xml.push_str("            <logginginfo>\n");
        push_text_element(xml, 14, "description", &note);
        xml.push_str("            </logginginfo>\n");
    }
    xml.push_str("            <file>\n");
    push_text_element(xml, 14, "name", &file_name(&clip.media.relative_path));
    push_text_element(
        xml,
        14,
        "pathurl",
        &format!("file://{}", clip.media.relative_path),
    );
    push_number_element(xml, 14, "duration", file_duration_frames);
    xml.push_str("            </file>\n");
    if xmeml_is_visual_clip(clip) {
        push_xmeml_transform_filter(xml, clip, fps);
        push_xmeml_crop_filter(xml, clip.item, fps);
        push_xmeml_opacity_filter(xml, clip.item, fps);
    }
    if matches!(clip.track_kind, TrackKind::Audio) {
        push_xmeml_audio_volume_filter(xml, clip, fps);
    }
    if has_source_audio {
        push_xmeml_link_nodes(xml, &clip.item.id, true);
    }
    xml.push_str("          </clipitem>\n");
    if let Some(span) = outgoing {
        let media_type = if matches!(clip.track_kind, TrackKind::Audio) {
            transitions::TransitionMedia::Audio
        } else {
            transitions::TransitionMedia::Video
        };
        transitions::push_xmeml_transition(xml, span, fps, media_type);
    }
}

fn push_xmeml_video_source_audio_clip(
    xml: &mut String,
    clips: &[NleClip<'_>],
    index: usize,
    fps: i64,
) {
    let clip = &clips[index];
    let file_duration_frames = seconds_to_frames(clip.media.duration_seconds, fps);
    let audio_item_id = xmeml_source_audio_clip_id(&clip.item.id);
    let (incoming, outgoing) = transitions::xmeml_source_audio_transitions(clips, index);
    xml.push_str(&format!(
        "          <clipitem id=\"{}\">\n",
        xml_escape(&audio_item_id)
    ));
    push_text_element(xml, 12, "name", &clip.item.label);
    push_text_element(xml, 12, "masterclipid", &xmeml_masterclip_id(&clip.item.id));
    transitions::push_xmeml_clip_bounds(xml, clip, incoming, outgoing);
    xml.push_str("            <file>\n");
    push_text_element(xml, 14, "name", &file_name(&clip.media.relative_path));
    push_text_element(
        xml,
        14,
        "pathurl",
        &format!("file://{}", clip.media.relative_path),
    );
    push_number_element(xml, 14, "duration", file_duration_frames);
    xml.push_str("            </file>\n");
    push_xmeml_audio_volume_filter(xml, clip, fps);
    push_xmeml_link_nodes(xml, &clip.item.id, false);
    xml.push_str("          </clipitem>\n");
    if let Some(span) = outgoing {
        transitions::push_xmeml_transition(xml, span, fps, transitions::TransitionMedia::Audio);
    }
}

fn push_xmeml_link_nodes(xml: &mut String, video_item_id: &str, is_video_clip: bool) {
    let audio_item_id = xmeml_source_audio_clip_id(video_item_id);
    let this_id = if is_video_clip {
        video_item_id.to_string()
    } else {
        audio_item_id.clone()
    };
    let other_id = if is_video_clip {
        audio_item_id
    } else {
        video_item_id.to_string()
    };

    xml.push_str("            <link>\n");
    push_text_element(xml, 14, "linkclipref", &this_id);
    push_text_element(
        xml,
        14,
        "mediatype",
        if is_video_clip { "video" } else { "audio" },
    );
    push_number_element(xml, 14, "trackindex", 1);
    push_number_element(xml, 14, "clipindex", 1);
    xml.push_str("            </link>\n");
    xml.push_str("            <link>\n");
    push_text_element(xml, 14, "linkclipref", &other_id);
    push_text_element(
        xml,
        14,
        "mediatype",
        if is_video_clip { "audio" } else { "video" },
    );
    push_number_element(xml, 14, "trackindex", 1);
    push_number_element(xml, 14, "clipindex", 1);
    xml.push_str("            </link>\n");
}

/// Whether a video clip's own sound is written as a linked XMEML audio
/// clipitem. Detached sound is its own audio clip instead.
fn xmeml_has_source_audio(clip: &NleClip<'_>) -> bool {
    matches!(clip.track_kind, TrackKind::Video)
        && !audio_is_detached(clip.item)
        && matches!(
            clip.item.kind,
            TimelineItemKind::VideoClip | TimelineItemKind::GeneratedClip
        )
        && (static_volume_db(clip.item).is_some()
            || scalar_keyframe_track(clip.item, "volumeDb", 1, -60.0, 24.0)
                .flatten()
                .is_some())
}

fn audio_is_detached(item: &TimelineItem) -> bool {
    item.properties
        .get("audioDetached")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn xmeml_source_audio_clip_id(video_item_id: &str) -> String {
    format!("{video_item_id}-audio")
}

fn xmeml_masterclip_id(video_item_id: &str) -> String {
    format!("masterclip-{video_item_id}")
}

fn xmeml_is_visual_clip(clip: &NleClip<'_>) -> bool {
    matches!(clip.track_kind, TrackKind::Video)
        && matches!(
            clip.item.kind,
            TimelineItemKind::VideoClip
                | TimelineItemKind::ImageClip
                | TimelineItemKind::GeneratedClip
        )
}

fn push_xmeml_transform_filter(xml: &mut String, clip: &NleClip<'_>, fps: i64) {
    let transform = clip
        .item
        .properties
        .get("transform")
        .and_then(|value| value.as_object());
    let center_x = transform
        .and_then(|transform| transform_number(transform, "centerX"))
        .unwrap_or(0.5);
    let center_y = transform
        .and_then(|transform| transform_number(transform, "centerY"))
        .unwrap_or(0.5);
    let width = transform
        .and_then(|transform| transform_number(transform, "width"))
        .unwrap_or(1.0);
    let rotation = -transform
        .and_then(|transform| transform_number(transform, "rotation"))
        .unwrap_or(0.0);
    let rotation_keyframes =
        scalar_keyframe_track(clip.item, "rotationDegrees", fps, -360.0, 360.0)
            .flatten()
            .map(|keyframes| {
                keyframes
                    .into_iter()
                    .map(|keyframe| NleScalarKeyframe {
                        when_frames: keyframe.when_frames,
                        value: -keyframe.value,
                    })
                    .collect::<Vec<_>>()
            });
    let scale_keyframes = scalar_keyframe_track(clip.item, "scale", fps, 0.01, 100.0)
        .flatten()
        .map(|keyframes| {
            keyframes
                .into_iter()
                .map(|keyframe| NleScalarKeyframe {
                    when_frames: keyframe.when_frames,
                    value: (keyframe.value * 100.0).clamp(0.0, 1000.0),
                })
                .collect::<Vec<_>>()
        });
    let scale = (width * 100.0).clamp(0.0, 1000.0);
    let center_h = center_x - 0.5;
    let center_v = center_y - 0.5;
    let center_keyframes =
        xmeml_center_keyframes(clip.item, fps, center_x, center_y).unwrap_or_default();
    let needs_scale = (scale - 100.0).abs() > 0.1 || scale_keyframes.is_some();
    let needs_rotation = rotation.abs() > 0.05 || rotation_keyframes.is_some();
    let needs_center =
        center_h.abs() > 0.001 || center_v.abs() > 0.001 || !center_keyframes.is_empty();
    if !needs_scale && !needs_rotation && !needs_center {
        return;
    }

    xml.push_str("            <filter>\n");
    xml.push_str("              <effect>\n");
    push_text_element(xml, 16, "name", "Basic Motion");
    push_text_element(xml, 16, "effectid", "basic");
    if needs_scale {
        let scale_base = scale_keyframes
            .as_ref()
            .and_then(|keyframes| keyframes.first())
            .map(|keyframe| keyframe.value)
            .unwrap_or(scale);
        push_xmeml_scalar_parameter_with_keyframes(
            xml,
            "scale",
            "Scale",
            "0",
            "1000",
            &format!("{scale_base:.2}"),
            scale_keyframes.as_deref(),
        );
    }
    if needs_rotation {
        let rotation_base = rotation_keyframes
            .as_ref()
            .and_then(|keyframes| keyframes.first())
            .map(|keyframe| keyframe.value)
            .unwrap_or(rotation);
        push_xmeml_scalar_parameter_with_keyframes(
            xml,
            "rotation",
            "Rotation",
            "-100000",
            "100000",
            &format!("{rotation_base:.2}"),
            rotation_keyframes.as_deref(),
        );
    }
    if needs_center {
        xml.push_str("                <parameter>\n");
        push_text_element(xml, 18, "parameterid", "center");
        push_text_element(xml, 18, "name", "Center");
        xml.push_str("                  <value>\n");
        let center_base = center_keyframes
            .first()
            .copied()
            .unwrap_or(NlePointKeyframe {
                when_frames: 0,
                x: center_h,
                y: center_v,
            });
        push_text_element(xml, 20, "horiz", &format!("{:.5}", center_base.x));
        push_text_element(xml, 20, "vert", &format!("{:.5}", center_base.y));
        xml.push_str("                  </value>\n");
        for keyframe in center_keyframes {
            xml.push_str("                  <keyframe>\n");
            push_number_element(xml, 20, "when", keyframe.when_frames);
            xml.push_str("                    <value>\n");
            push_text_element(xml, 22, "horiz", &format!("{:.5}", keyframe.x));
            push_text_element(xml, 22, "vert", &format!("{:.5}", keyframe.y));
            xml.push_str("                    </value>\n");
            xml.push_str("                  </keyframe>\n");
        }
        xml.push_str("                </parameter>\n");
    }
    xml.push_str("              </effect>\n");
    xml.push_str("            </filter>\n");
}

fn push_xmeml_crop_filter(xml: &mut String, item: &TimelineItem, fps: i64) {
    let crop_keyframes = [
        ("left", "cropLeft"),
        ("right", "cropRight"),
        ("top", "cropTop"),
        ("bottom", "cropBottom"),
    ]
    .map(|(id, key)| {
        (
            id,
            scalar_keyframe_track(item, key, fps, 0.0, 1.0)
                .flatten()
                .map(scale_fraction_keyframes_to_percent),
        )
    });
    let has_keyframes = crop_keyframes
        .iter()
        .any(|(_, keyframes)| keyframes.is_some());

    if has_keyframes {
        let has_nonzero_keyframe = crop_keyframes
            .iter()
            .filter_map(|(_, keyframes)| keyframes.as_ref())
            .flatten()
            .any(|keyframe| keyframe.value.abs() > 0.0005);
        if !has_nonzero_keyframe {
            return;
        }

        xml.push_str("            <filter>\n");
        xml.push_str("              <effect>\n");
        push_text_element(xml, 16, "name", "Crop");
        push_text_element(xml, 16, "effectid", "crop");
        for (id, keyframes) in crop_keyframes {
            let base = keyframes
                .as_ref()
                .and_then(|keyframes| keyframes.first())
                .map(|keyframe| keyframe.value)
                .unwrap_or(0.0);
            push_xmeml_scalar_parameter_with_keyframes(
                xml,
                id,
                id,
                "0",
                "100",
                &format!("{base:.2}"),
                keyframes.as_deref(),
            );
        }
        xml.push_str("              </effect>\n");
        xml.push_str("            </filter>\n");
        return;
    }

    let Some((top, right, bottom, left)) = static_crop_values(item) else {
        return;
    };
    if [top, right, bottom, left]
        .iter()
        .all(|value| value.is_finite() && value.abs() <= 0.0005)
    {
        return;
    }

    xml.push_str("            <filter>\n");
    xml.push_str("              <effect>\n");
    push_text_element(xml, 16, "name", "Crop");
    push_text_element(xml, 16, "effectid", "crop");
    push_xmeml_scalar_parameter(
        xml,
        "left",
        "left",
        "0",
        "100",
        &format!("{:.2}", left * 100.0),
    );
    push_xmeml_scalar_parameter(
        xml,
        "right",
        "right",
        "0",
        "100",
        &format!("{:.2}", right * 100.0),
    );
    push_xmeml_scalar_parameter(
        xml,
        "top",
        "top",
        "0",
        "100",
        &format!("{:.2}", top * 100.0),
    );
    push_xmeml_scalar_parameter(
        xml,
        "bottom",
        "bottom",
        "0",
        "100",
        &format!("{:.2}", bottom * 100.0),
    );
    xml.push_str("              </effect>\n");
    xml.push_str("            </filter>\n");
}

fn push_xmeml_opacity_filter(xml: &mut String, item: &TimelineItem, fps: i64) {
    let opacity_keyframes = scalar_keyframe_track(item, "opacity", fps, 0.0, 1.0)
        .flatten()
        .map(|keyframes| {
            keyframes
                .into_iter()
                .map(|keyframe| NleScalarKeyframe {
                    when_frames: keyframe.when_frames,
                    value: (keyframe.value * 100.0).clamp(0.0, 100.0),
                })
                .collect::<Vec<_>>()
        });
    let opacity_percent = if let Some(keyframes) = &opacity_keyframes {
        keyframes
            .first()
            .map(|keyframe| keyframe.value)
            .unwrap_or(100.0)
    } else {
        let Some(opacity) = static_opacity(item) else {
            return;
        };
        if (opacity - 1.0).abs() <= 0.0005 {
            return;
        }
        (opacity.clamp(0.0, 1.0) * 100.0).clamp(0.0, 100.0)
    };

    xml.push_str("            <filter>\n");
    xml.push_str("              <effect>\n");
    push_text_element(xml, 16, "name", "Opacity");
    push_text_element(xml, 16, "effectid", "opacity");
    xml.push_str("                <parameter>\n");
    push_text_element(xml, 18, "parameterid", "opacity");
    push_text_element(xml, 18, "name", "Opacity");
    push_text_element(xml, 18, "valuemin", "0");
    push_text_element(xml, 18, "valuemax", "100");
    push_text_element(xml, 18, "value", &format!("{opacity_percent:.1}"));
    if let Some(keyframes) = opacity_keyframes {
        for keyframe in keyframes {
            xml.push_str("                  <keyframe>\n");
            push_number_element(xml, 20, "when", keyframe.when_frames);
            push_text_element(xml, 20, "value", &format!("{:.1}", keyframe.value));
            xml.push_str("                  </keyframe>\n");
        }
    }
    xml.push_str("                </parameter>\n");
    xml.push_str("              </effect>\n");
    xml.push_str("            </filter>\n");
}

fn push_xmeml_scalar_parameter(
    xml: &mut String,
    id: &str,
    name: &str,
    min: &str,
    max: &str,
    value: &str,
) {
    xml.push_str("                <parameter>\n");
    push_text_element(xml, 18, "parameterid", id);
    push_text_element(xml, 18, "name", name);
    push_text_element(xml, 18, "valuemin", min);
    push_text_element(xml, 18, "valuemax", max);
    push_text_element(xml, 18, "value", value);
    xml.push_str("                </parameter>\n");
}

fn push_xmeml_scalar_parameter_with_keyframes(
    xml: &mut String,
    id: &str,
    name: &str,
    min: &str,
    max: &str,
    value: &str,
    keyframes: Option<&[NleScalarKeyframe]>,
) {
    xml.push_str("                <parameter>\n");
    push_text_element(xml, 18, "parameterid", id);
    push_text_element(xml, 18, "name", name);
    push_text_element(xml, 18, "valuemin", min);
    push_text_element(xml, 18, "valuemax", max);
    push_text_element(xml, 18, "value", value);
    if let Some(keyframes) = keyframes {
        for keyframe in keyframes {
            xml.push_str("                  <keyframe>\n");
            push_number_element(xml, 20, "when", keyframe.when_frames);
            push_text_element(xml, 20, "value", &format!("{:.2}", keyframe.value));
            xml.push_str("                  </keyframe>\n");
        }
    }
    xml.push_str("                </parameter>\n");
}

fn push_xmeml_audio_volume_filter(xml: &mut String, clip: &NleClip<'_>, fps: i64) {
    let volume_keyframes = scalar_keyframe_track(clip.item, "volumeDb", fps, -60.0, 24.0)
        .flatten()
        .map(|keyframes| {
            keyframes
                .into_iter()
                .map(|keyframe| NleScalarKeyframe {
                    when_frames: keyframe.when_frames,
                    value: db_to_linear_volume(keyframe.value).clamp(0.0, 3.98107),
                })
                .collect::<Vec<_>>()
        });
    let linear_level = if let Some(keyframes) = &volume_keyframes {
        keyframes
            .first()
            .map(|keyframe| keyframe.value)
            .unwrap_or(1.0)
    } else {
        let Some(volume_db) = static_volume_db(clip.item) else {
            return;
        };
        if volume_db.abs() <= 0.0005 {
            return;
        }
        db_to_linear_volume(volume_db).clamp(0.0, 3.98107)
    };

    xml.push_str("            <filter>\n");
    xml.push_str("              <effect>\n");
    push_text_element(xml, 16, "name", "Audio Levels");
    push_text_element(xml, 16, "effectid", "audiolevels");
    xml.push_str("                <parameter>\n");
    push_text_element(xml, 18, "parameterid", "level");
    push_text_element(xml, 18, "name", "Level");
    push_text_element(xml, 18, "valuemin", "0");
    push_text_element(xml, 18, "valuemax", "3.98107");
    push_text_element(xml, 18, "value", &format_xmeml_decimal(linear_level));
    if let Some(keyframes) = volume_keyframes {
        for keyframe in keyframes {
            xml.push_str("                  <keyframe>\n");
            push_number_element(xml, 20, "when", keyframe.when_frames);
            push_text_element(xml, 20, "value", &format_xmeml_decimal(keyframe.value));
            xml.push_str("                  </keyframe>\n");
        }
    }
    xml.push_str("                </parameter>\n");
    xml.push_str("              </effect>\n");
    xml.push_str("            </filter>\n");
}

fn fcpxml_clip_start(clip: &NleClip<'_>, fps: i64) -> String {
    if !clip_is_retimed(clip) {
        return format!("{}/{}s", clip.source_in_frames, fps);
    }
    let (speed_num, speed_den) = rational_speed(clip.speed);
    format_fcpxml_rational_time(clip.source_in_frames * speed_den, fps * speed_num)
}

fn fcpxml_time_map_node(clip: &NleClip<'_>, fps: i64) -> Option<String> {
    if !clip_is_retimed(clip) {
        return None;
    }
    let media_frames = seconds_to_frames(clip.media.duration_seconds, fps);
    if media_frames <= 0 {
        return None;
    }
    let (speed_num, speed_den) = rational_speed(clip.speed);
    Some(format!(
        "<timeMap frameSampling=\"floor\">\n                <timept time=\"0s\" value=\"0s\" interp=\"linear\"/>\n                <timept time=\"{}\" value=\"{}\" interp=\"linear\"/>\n              </timeMap>",
        format_fcpxml_rational_time(media_frames * speed_den, fps * speed_num),
        format_fcpxml_rational_time(media_frames, fps)
    ))
}

fn clip_is_retimed(clip: &NleClip<'_>) -> bool {
    (clip.speed - 1.0).abs() > 0.001
}

fn rational_speed(speed: f64) -> (i64, i64) {
    let mut best_num = 1_i64;
    let mut best_den = 1_i64;
    let mut best_error = f64::INFINITY;
    for denominator in 1_i64..=1000 {
        let numerator = (speed * denominator as f64).round() as i64;
        if numerator <= 0 {
            continue;
        }
        let error = (speed - numerator as f64 / denominator as f64).abs();
        if error < best_error {
            best_num = numerator;
            best_den = denominator;
            best_error = error;
            if error <= f64::EPSILON {
                break;
            }
        }
    }
    (best_num, best_den)
}

fn format_fcpxml_rational_time(numerator: i64, denominator: i64) -> String {
    if numerator == 0 {
        return "0s".to_string();
    }
    let divisor = gcd_i64(numerator.abs(), denominator.abs());
    let numerator = numerator / divisor;
    let denominator = denominator / divisor;
    if denominator == 1 {
        format!("{numerator}s")
    } else {
        format!("{numerator}/{denominator}s")
    }
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.max(1)
}

fn fcpxml_static_audio_volume_node(item: &TimelineItem) -> Option<String> {
    let volume_db = static_volume_db(item)?;
    if volume_db.abs() <= 0.0005 {
        return None;
    }
    Some(format!(
        "<adjust-volume amount=\"{}\"/>",
        format_fcpxml_number(volume_db)
    ))
}

fn fcpxml_transform_node(clip: &NleClip<'_>, fps: i64) -> Option<String> {
    let transform = clip
        .item
        .properties
        .get("transform")
        .and_then(|value| value.as_object());
    let center_x = transform
        .and_then(|transform| transform_number(transform, "centerX"))
        .unwrap_or(0.5);
    let center_y = transform
        .and_then(|transform| transform_number(transform, "centerY"))
        .unwrap_or(0.5);
    let width = transform
        .and_then(|transform| transform_number(transform, "width"))
        .unwrap_or(1.0);
    let height = transform
        .and_then(|transform| transform_number(transform, "height"))
        .unwrap_or(1.0);
    let rotation = -transform
        .and_then(|transform| transform_number(transform, "rotation"))
        .unwrap_or(0.0);
    let flip_horizontal =
        transform.is_some_and(|transform| transform_bool(transform, "flipHorizontal"));
    let flip_vertical =
        transform.is_some_and(|transform| transform_bool(transform, "flipVertical"));
    let scale_keyframes = scalar_keyframe_track(clip.item, "scale", fps, 0.01, 100.0).flatten();
    let position_keyframes = fcpxml_position_keyframes(clip, fps, center_x, center_y);
    let rotation_keyframes =
        scalar_keyframe_track(clip.item, "rotationDegrees", fps, -360.0, 360.0)
            .flatten()
            .map(|keyframes| {
                keyframes
                    .into_iter()
                    .map(|keyframe| NleScalarKeyframe {
                        when_frames: keyframe.when_frames,
                        value: -keyframe.value,
                    })
                    .collect::<Vec<_>>()
            });
    let scale_base = scale_keyframes
        .as_ref()
        .and_then(|keyframes| keyframes.first())
        .map(|keyframe| keyframe.value)
        .unwrap_or(width);
    let height_base = scale_keyframes
        .as_ref()
        .and_then(|keyframes| keyframes.first())
        .map(|keyframe| keyframe.value)
        .unwrap_or(height);
    let (scale_x, scale_y) = fcpxml_scale_values(
        clip,
        scale_base,
        height_base,
        flip_horizontal,
        flip_vertical,
    );
    let (position_x, position_y) = position_keyframes
        .as_ref()
        .and_then(|keyframes| keyframes.first())
        .map(|keyframe| (keyframe.x, keyframe.y))
        .unwrap_or_else(|| fcpxml_position_values(clip, center_x, center_y));
    let rotation_base = rotation_keyframes
        .as_ref()
        .and_then(|keyframes| keyframes.first())
        .map(|keyframe| keyframe.value)
        .unwrap_or(rotation);
    let has_scale_keyframes = scale_keyframes.is_some();
    let has_position_keyframes = position_keyframes
        .as_ref()
        .is_some_and(|keyframes| !keyframes.is_empty());
    let has_rotation_keyframes = rotation_keyframes.is_some();
    let moved = (center_x - 0.5).abs() > 0.0005
        || (center_y - 0.5).abs() > 0.0005
        || has_position_keyframes;
    let rotated = rotation_base.abs() > 0.005 || has_rotation_keyframes;
    let scaled = (scale_x - 1.0).abs() > 0.0005 || (scale_y - 1.0).abs() > 0.0005;
    if !moved && !rotated && !scaled && !has_scale_keyframes {
        return None;
    }

    let rotation_attr = if rotated {
        format!(" rotation=\"{}\"", format_fcpxml_number(rotation_base))
    } else {
        String::new()
    };
    let mut xml = format!(
        "<adjust-transform scale=\"{} {}\"{} anchor=\"0 0\" position=\"{} {}\"",
        format_fcpxml_number(scale_x),
        format_fcpxml_number(scale_y),
        rotation_attr,
        format_fcpxml_number(position_x),
        format_fcpxml_number(position_y)
    );
    if !has_scale_keyframes && !has_position_keyframes && !has_rotation_keyframes {
        xml.push_str("/>");
        return Some(xml);
    }

    xml.push_str(">\n");
    if let Some(keyframes) = scale_keyframes {
        xml.push_str(&fcpxml_transform_param(
            "scale",
            &format!(
                "{} {}",
                format_fcpxml_number(scale_x),
                format_fcpxml_number(scale_y)
            ),
            fps,
            keyframes.iter().map(|keyframe| {
                let (scale_x, scale_y) = fcpxml_scale_values(
                    clip,
                    keyframe.value,
                    keyframe.value,
                    flip_horizontal,
                    flip_vertical,
                );
                (
                    keyframe.when_frames,
                    format!(
                        "{} {}",
                        format_fcpxml_number(scale_x),
                        format_fcpxml_number(scale_y)
                    ),
                )
            }),
        ));
    }
    if let Some(keyframes) = position_keyframes {
        if !keyframes.is_empty() {
            xml.push_str(&fcpxml_transform_param(
                "position",
                &format!(
                    "{} {}",
                    format_fcpxml_number(position_x),
                    format_fcpxml_number(position_y)
                ),
                fps,
                keyframes.iter().map(|keyframe| {
                    (
                        keyframe.when_frames,
                        format!(
                            "{} {}",
                            format_fcpxml_number(keyframe.x),
                            format_fcpxml_number(keyframe.y)
                        ),
                    )
                }),
            ));
        }
    }
    if let Some(keyframes) = rotation_keyframes {
        xml.push_str(&fcpxml_transform_param(
            "rotation",
            &format_fcpxml_number(rotation_base),
            fps,
            keyframes
                .iter()
                .map(|keyframe| (keyframe.when_frames, format_fcpxml_number(keyframe.value))),
        ));
    }
    xml.push_str("              </adjust-transform>");
    Some(xml)
}

fn fcpxml_scale_values(
    clip: &NleClip<'_>,
    width: f64,
    height: f64,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> (f64, f64) {
    let (fit_width, fit_height) = clip_fit_fractions(clip);
    let mut scale_x = width / fit_width;
    let mut scale_y = height / fit_height;
    if flip_horizontal {
        scale_x = -scale_x;
    }
    if flip_vertical {
        scale_y = -scale_y;
    }
    (scale_x, scale_y)
}

fn fcpxml_position_values(clip: &NleClip<'_>, center_x: f64, center_y: f64) -> (f64, f64) {
    let unit = clip_project_height_unit(clip);
    let position_x = (center_x - 0.5) * clip_project_width(clip) / unit;
    let position_y = (0.5 - center_y) * 100.0;
    (position_x, position_y)
}

fn fcpxml_position_keyframes(
    clip: &NleClip<'_>,
    fps: i64,
    center_x: f64,
    center_y: f64,
) -> Option<Vec<NlePointKeyframe>> {
    let x_keyframes = scalar_keyframe_track(clip.item, "positionX", fps, -10000.0, 10000.0)?;
    let y_keyframes = scalar_keyframe_track(clip.item, "positionY", fps, -10000.0, 10000.0)?;
    if x_keyframes.is_none() && y_keyframes.is_none() {
        return Some(Vec::new());
    }

    let x_keyframes = x_keyframes.unwrap_or_default();
    let y_keyframes = y_keyframes.unwrap_or_default();
    let mut frames = x_keyframes
        .iter()
        .chain(y_keyframes.iter())
        .map(|keyframe| keyframe.when_frames)
        .collect::<Vec<_>>();
    frames.sort_unstable();
    frames.dedup();

    Some(
        frames
            .into_iter()
            .map(|when_frames| {
                let center_x = scalar_value_at_frame(&x_keyframes, when_frames, center_x);
                let center_y = scalar_value_at_frame(&y_keyframes, when_frames, center_y);
                let (x, y) = fcpxml_position_values(clip, center_x, center_y);
                NlePointKeyframe { when_frames, x, y }
            })
            .collect(),
    )
}

fn fcpxml_transform_param<I>(name: &str, base: &str, fps: i64, keyframes: I) -> String
where
    I: IntoIterator<Item = (i64, String)>,
{
    let mut xml = format!("                <param name=\"{name}\" value=\"{base}\">\n");
    xml.push_str("                  <keyframeAnimation>\n");
    for (when_frames, value) in keyframes {
        xml.push_str(&format!(
            "                    <keyframe time=\"{when_frames}/{fps}s\" value=\"{value}\"/>\n"
        ));
    }
    xml.push_str("                  </keyframeAnimation>\n");
    xml.push_str("                </param>\n");
    xml
}

fn fcpxml_static_crop_node(clip: &NleClip<'_>) -> Option<String> {
    let crop = static_crop_values(clip.item)?;
    let (top, right, bottom, left) = crop;
    if [top, right, bottom, left]
        .iter()
        .all(|value| value.is_finite() && value.abs() <= 0.0005)
    {
        return None;
    }

    let (lr_scale, tb_scale) = crop_scales(clip);
    Some(format!(
        "<adjust-crop mode=\"trim\"><trim-rect top=\"{}\" right=\"{}\" bottom=\"{}\" left=\"{}\"/></adjust-crop>",
        format_fcpxml_number(top * tb_scale),
        format_fcpxml_number(right * lr_scale),
        format_fcpxml_number(bottom * tb_scale),
        format_fcpxml_number(left * lr_scale)
    ))
}

fn fcpxml_opacity_node(item: &TimelineItem, fps: i64) -> Option<String> {
    let opacity_keyframes = scalar_keyframe_track(item, "opacity", fps, 0.0, 1.0).flatten();
    if let Some(keyframes) = opacity_keyframes {
        let opacity = keyframes
            .first()
            .map(|keyframe| keyframe.value)
            .unwrap_or(1.0);
        let opacity = opacity.clamp(0.0, 1.0);
        let mut xml = format!(
            "<adjust-blend amount=\"{}\">\n",
            format_fcpxml_number(opacity)
        );
        xml.push_str(&format!(
            "                <param name=\"amount\" value=\"{}\">\n",
            format_fcpxml_number(opacity)
        ));
        xml.push_str("                  <keyframeAnimation>\n");
        for keyframe in keyframes {
            xml.push_str(&format!(
                "                    <keyframe time=\"{}/{}s\" value=\"{}\"/>\n",
                keyframe.when_frames,
                fps,
                format_fcpxml_number(keyframe.value)
            ));
        }
        xml.push_str("                  </keyframeAnimation>\n");
        xml.push_str("                </param>\n");
        xml.push_str("              </adjust-blend>");
        return Some(xml);
    }

    let opacity = static_opacity(item)?;
    if (opacity - 1.0).abs() <= 0.0005 {
        return None;
    }
    Some(format!(
        "<adjust-blend amount=\"{}\"/>",
        format_fcpxml_number(opacity.clamp(0.0, 1.0))
    ))
}

fn transform_number(
    transform: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<f64> {
    transform
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

fn transform_bool(transform: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    transform
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn clip_fit_fractions(clip: &NleClip<'_>) -> (f64, f64) {
    let sequence_width = clip_project_width(clip);
    let sequence_height = clip_project_height(clip);
    let media_width = clip.media.width.unwrap_or(clip.sequence_width) as f64;
    let media_height = clip.media.height.unwrap_or(clip.sequence_height) as f64;
    if media_width <= 0.0 || media_height <= 0.0 {
        return (1.0, 1.0);
    }
    let fit = (sequence_width / media_width).min(sequence_height / media_height);
    (
        (media_width * fit / sequence_width).max(f64::EPSILON),
        (media_height * fit / sequence_height).max(f64::EPSILON),
    )
}

fn crop_scales(clip: &NleClip<'_>) -> (f64, f64) {
    let sequence_width = clip_project_width(clip);
    let sequence_height = clip_project_height(clip);
    let media_width = clip.media.width.unwrap_or(clip.sequence_width) as f64;
    let media_height = clip.media.height.unwrap_or(clip.sequence_height) as f64;
    if media_width <= 0.0 || media_height <= 0.0 {
        return (100.0, 100.0);
    }

    let fit = (sequence_width / media_width).min(sequence_height / media_height);
    if fit <= 0.0 || !fit.is_finite() {
        return (100.0, 100.0);
    }

    (media_width * 100.0 / sequence_height, 100.0 / fit)
}

fn clip_project_width(clip: &NleClip<'_>) -> f64 {
    f64::from(clip.sequence_width.max(1))
}

fn clip_project_height(clip: &NleClip<'_>) -> f64 {
    f64::from(clip.sequence_height.max(1))
}

fn clip_project_height_unit(clip: &NleClip<'_>) -> f64 {
    clip_project_height(clip) / 100.0
}

fn format_fcpxml_number(value: f64) -> String {
    let value = if value.abs() < 0.00005 { 0.0 } else { value };
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn format_xmeml_decimal(value: f64) -> String {
    format!("{value:.4}")
}

fn static_volume_db(item: &TimelineItem) -> Option<f64> {
    item.properties
        .get("volumeDb")
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

fn static_opacity(item: &TimelineItem) -> Option<f64> {
    item.properties
        .get("opacity")
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

fn db_to_linear_volume(volume_db: f64) -> f64 {
    10_f64.powf(volume_db / 20.0)
}

fn scale_fraction_keyframes_to_percent(
    keyframes: Vec<NleScalarKeyframe>,
) -> Vec<NleScalarKeyframe> {
    keyframes
        .into_iter()
        .map(|keyframe| NleScalarKeyframe {
            when_frames: keyframe.when_frames,
            value: (keyframe.value * 100.0).clamp(0.0, 100.0),
        })
        .collect()
}

fn xmeml_center_keyframes(
    item: &TimelineItem,
    fps: i64,
    center_x: f64,
    center_y: f64,
) -> Option<Vec<NlePointKeyframe>> {
    let x_keyframes = scalar_keyframe_track(item, "positionX", fps, -10000.0, 10000.0)?;
    let y_keyframes = scalar_keyframe_track(item, "positionY", fps, -10000.0, 10000.0)?;
    if x_keyframes.is_none() && y_keyframes.is_none() {
        return Some(Vec::new());
    }

    let x_keyframes = x_keyframes.unwrap_or_default();
    let y_keyframes = y_keyframes.unwrap_or_default();
    let mut frames = x_keyframes
        .iter()
        .chain(y_keyframes.iter())
        .map(|keyframe| keyframe.when_frames)
        .collect::<Vec<_>>();
    frames.sort_unstable();
    frames.dedup();

    Some(
        frames
            .into_iter()
            .map(|when_frames| NlePointKeyframe {
                when_frames,
                x: scalar_value_at_frame(&x_keyframes, when_frames, center_x) - 0.5,
                y: scalar_value_at_frame(&y_keyframes, when_frames, center_y) - 0.5,
            })
            .collect(),
    )
}

fn scalar_value_at_frame(keyframes: &[NleScalarKeyframe], when_frames: i64, fallback: f64) -> f64 {
    keyframes
        .iter()
        .find(|keyframe| keyframe.when_frames == when_frames)
        .or_else(|| {
            keyframes
                .iter()
                .rev()
                .find(|keyframe| keyframe.when_frames <= when_frames)
        })
        .map(|keyframe| keyframe.value)
        .unwrap_or(fallback)
}

fn clip_export_note(
    generated_asset: Option<&GeneratedAsset>,
    limitation: Option<String>,
) -> Option<String> {
    match (generated_asset.map(generated_clip_provenance), limitation) {
        (Some(provenance), Some(limitation)) => Some(format!("{provenance}; {limitation}")),
        (Some(provenance), None) => Some(provenance),
        (None, Some(limitation)) => Some(limitation),
        (None, None) => None,
    }
}

fn nle_clip_limitation_note(clip: &NleClip<'_>, format: &str) -> Option<String> {
    let mut limitations = Vec::new();
    if non_empty_property(clip.item, "effects") {
        limitations.push("effect stack");
    }
    if has_unsupported_keyframes(clip.item, format) {
        limitations.push("keyframes");
    }
    if has_explicit_keyframe_easing(clip.item) {
        limitations.push("keyframe easing");
    }
    if non_empty_property(clip.item, "colorGrade") {
        limitations.push("color grade");
    }
    if has_nonzero_or_malformed_number_property(clip.item, "fadeInSeconds")
        || has_nonzero_or_malformed_number_property(clip.item, "fadeOutSeconds")
    {
        limitations.push("clip fades");
    }
    if has_unsupported_blend_mode(clip.item) {
        limitations.push("blend mode");
    }
    if has_static_crop(clip.item) {
        limitations.push("static crop");
    }
    if format == "premiereXmeml" && has_flip_transform(clip.item) {
        limitations.push("flip transform");
    }
    if format != "davinciFcpxml"
        && non_empty_property(clip.item, "opacity")
        && static_opacity(clip.item).is_none()
    {
        limitations.push("opacity");
    }
    if non_empty_property(clip.item, "volumeDb") && static_volume_db(clip.item).is_none() {
        limitations.push("audio volume");
    }
    // FCPXML carries speed as a timeMap. Apple's FCP7 XML reference describes
    // XMEML Time Remap only through graphdict speed keyframes, without the
    // effect and parameter ids, so XMEML notes speed instead of writing it.
    if format == "premiereXmeml" && clip_is_retimed(clip) {
        limitations.push("speed");
    }
    if super::reverse::is_reversed(clip.item) {
        limitations.push("reverse");
    }
    limitations.extend(transitions::transition_limitations(clip, format));

    if limitations.is_empty() {
        return None;
    }

    Some(format!(
        "Video Creater NLE export limitation: {} are not round-tripped into {format}; render a final video for baked visual/audio look.",
        join_natural_list(&limitations)
    ))
}

fn non_empty_property(item: &TimelineItem, key: &str) -> bool {
    item.properties.get(key).is_some_and(|value| {
        !value.is_null() && value.as_array().is_none_or(|array| !array.is_empty())
    })
}

fn has_nonzero_or_malformed_number_property(item: &TimelineItem, key: &str) -> bool {
    let Some(value) = item.properties.get(key) else {
        return false;
    };
    value
        .as_f64()
        .is_none_or(|number| !number.is_finite() || number.abs() > 0.0005)
}

fn has_unsupported_blend_mode(item: &TimelineItem) -> bool {
    let Some(value) = item.properties.get("blendMode") else {
        return false;
    };
    !matches!(value.as_str(), Some("normal" | "over"))
}

fn has_static_crop(item: &TimelineItem) -> bool {
    ["cropTop", "cropRight", "cropBottom", "cropLeft"]
        .iter()
        .any(|key| has_nonzero_or_malformed_number_property(item, key))
}

fn has_flip_transform(item: &TimelineItem) -> bool {
    item.properties
        .get("transform")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|transform| {
            transform_bool(transform, "flipHorizontal") || transform_bool(transform, "flipVertical")
        })
}

fn has_explicit_keyframe_easing(item: &TimelineItem) -> bool {
    item.properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|tracks| {
            tracks.values().any(|track| {
                track.as_array().is_some_and(|keyframes| {
                    keyframes.iter().any(|keyframe| {
                        keyframe
                            .get("easing")
                            .is_some_and(|easing| !easing.is_null())
                    })
                })
            })
        })
}

fn has_unsupported_keyframes(item: &TimelineItem, format: &str) -> bool {
    let Some(keyframes) = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };

    keyframes.iter().any(|(key, value)| {
        if value.as_array().is_none_or(|array| array.is_empty()) {
            return false;
        }
        if matches!(
            key.as_str(),
            "cropTop" | "cropRight" | "cropBottom" | "cropLeft"
        ) {
            if format == "premiereXmeml" {
                return scalar_keyframe_track(item, key, 1, 0.0, 1.0).is_none();
            }
            return static_crop_keyframe_value(value).is_none();
        }
        if key == "opacity" && matches!(format, "premiereXmeml" | "davinciFcpxml") {
            return scalar_keyframe_track(item, "opacity", 1, 0.0, 1.0).is_none();
        }
        if key == "volumeDb" && format == "premiereXmeml" {
            return scalar_keyframe_track(item, "volumeDb", 1, -60.0, 24.0).is_none();
        }
        if key == "rotationDegrees" && matches!(format, "premiereXmeml" | "davinciFcpxml") {
            return scalar_keyframe_track(item, "rotationDegrees", 1, -360.0, 360.0).is_none();
        }
        if key == "scale" && matches!(format, "premiereXmeml" | "davinciFcpxml") {
            return scalar_keyframe_track(item, "scale", 1, 0.01, 100.0).is_none();
        }
        if matches!(key.as_str(), "positionX" | "positionY")
            && matches!(format, "premiereXmeml" | "davinciFcpxml")
        {
            return scalar_keyframe_track(item, key, 1, -10000.0, 10000.0).is_none();
        }
        true
    })
}

fn scalar_keyframe_track(
    item: &TimelineItem,
    key: &str,
    fps: i64,
    min: f64,
    max: f64,
) -> Option<Option<Vec<NleScalarKeyframe>>> {
    let keyframes = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)?;
    let Some(value) = keyframes.get(key) else {
        return Some(None);
    };
    let array = value.as_array()?;
    if array.is_empty() {
        return Some(None);
    }

    let mut parsed = Vec::with_capacity(array.len());
    for keyframe in array {
        let at_seconds = keyframe_time_seconds(keyframe)?;
        if at_seconds < 0.0 || at_seconds > item.duration_seconds + 0.0005 {
            return None;
        }
        let value = keyframe
            .get("value")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())?
            .clamp(min, max);
        parsed.push(NleScalarKeyframe {
            when_frames: seconds_to_frames(at_seconds, fps),
            value,
        });
    }
    parsed.sort_by_key(|keyframe| keyframe.when_frames);
    Some(Some(parsed))
}

fn keyframe_time_seconds(keyframe: &serde_json::Value) -> Option<f64> {
    keyframe
        .get("atSeconds")
        .or_else(|| keyframe.get("timeSeconds"))
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
}

fn static_crop_values(item: &TimelineItem) -> Option<(f64, f64, f64, f64)> {
    let keyframes = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)?;

    let top = static_crop_track_value(keyframes, "cropTop")?;
    let right = static_crop_track_value(keyframes, "cropRight")?;
    let bottom = static_crop_track_value(keyframes, "cropBottom")?;
    let left = static_crop_track_value(keyframes, "cropLeft")?;
    if [top, right, bottom, left]
        .iter()
        .all(|value| value.is_none())
    {
        return None;
    }

    Some((
        top.unwrap_or(0.0),
        right.unwrap_or(0.0),
        bottom.unwrap_or(0.0),
        left.unwrap_or(0.0),
    ))
}

fn static_crop_track_value(
    keyframes: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<Option<f64>> {
    let Some(value) = keyframes.get(key) else {
        return Some(None);
    };
    static_crop_keyframe_value(value)
}

fn static_crop_keyframe_value(value: &serde_json::Value) -> Option<Option<f64>> {
    let array = value.as_array()?;
    match array.as_slice() {
        [] => Some(None),
        [keyframe] => {
            let at_seconds = keyframe
                .get("atSeconds")
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite())?;
            if at_seconds.abs() > 0.0005 {
                return None;
            }
            let value = keyframe
                .get("value")
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite())?;
            Some(Some(value.clamp(0.0, 1.0)))
        }
        _ => None,
    }
}

fn join_natural_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [only] => (*only).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let last = items.last().expect("non-empty");
            format!("{}, and {last}", items[..items.len() - 1].join(", "))
        }
    }
}

fn generated_clip_provenance(asset: &GeneratedAsset) -> String {
    let display_name = asset.name.as_deref().unwrap_or(asset.id.as_str());
    let references = generated_reference_summary(asset);
    if references.is_empty() {
        format!(
            "Generated asset: {}; Model: {}/{}; Prompt: {}",
            display_name, asset.model.provider, asset.model.id, asset.prompt
        )
    } else {
        format!(
            "Generated asset: {}; Model: {}/{}; Prompt: {}; References: {}",
            display_name, asset.model.provider, asset.model.id, asset.prompt, references
        )
    }
}

fn generated_reference_summary(asset: &GeneratedAsset) -> String {
    let mut references = Vec::new();
    if let Some(first_frame_media_id) = &asset.references.first_frame_media_id {
        references.push(first_frame_media_id.as_str());
    }
    if let Some(last_frame_media_id) = &asset.references.last_frame_media_id {
        references.push(last_frame_media_id.as_str());
    }
    for media_id in &asset.references.media_ids {
        if !references.contains(&media_id.as_str()) {
            references.push(media_id.as_str());
        }
    }
    references.join(", ")
}

fn push_text_element(xml: &mut String, indent: usize, name: &str, value: &str) {
    xml.push_str(&format!(
        "{}<{}>{}</{}>\n",
        " ".repeat(indent),
        name,
        xml_escape(value),
        name
    ));
}

fn push_number_element(xml: &mut String, indent: usize, name: &str, value: impl std::fmt::Display) {
    xml.push_str(&format!(
        "{}<{}>{}</{}>\n",
        " ".repeat(indent),
        name,
        value,
        name
    ));
}

fn timeline_property_seconds(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(serde_json::Value::as_f64)
}

fn timeline_property_positive_number(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn normalized_fps(fps: f64) -> Result<i64, NleXmlExportError> {
    if !fps.is_finite() || fps <= 0.0 {
        return Err(NleXmlExportError::InvalidFrameRate);
    }

    Ok(fps.round() as i64)
}

fn seconds_to_frames(seconds: f64, fps: i64) -> i64 {
    (seconds * fps as f64).round() as i64
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn safe_filename(value: &str) -> String {
    let slug = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
