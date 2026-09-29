use super::cache::{
    cache_entry_dir, fingerprint, sha256_file, validate_cache_entry, CacheLock, CacheManifest,
    StagingCleanup,
};
use super::flatten_transitions::{
    retarget_split_transitions, widen_groups_over_transitions, FlattenTransitions, GroupSpan,
    ItemTransitions, TransitionSolid,
};
use super::{
    decode_video_frames_rgba, fps_rational, numeric_property, package_png_frames_as_mov,
    project_relative_path, seconds_to_micros, FrameDecodeSpec, PngSequenceSpec, PrecomposeReport,
};
use crate::frame_compositor::{
    apply_wipe_mask_rgba8, dissolve_rgba8_srgb, transform_rgba8_srgb, BlendBackendReport,
    BlendMode, CanvasTransform, FrameBlendCompositor, FrameProgram, KeyframeEasing, NumericCurve,
    NumericKeyframe, PreparedEffectStack, TransitionFrameState,
};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FlattenGroup {
    pub(super) start_seconds: f64,
    pub(super) end_seconds: f64,
    pub(super) top_track_index: usize,
    pub(super) top_item_id: String,
}

impl GroupSpan for FlattenGroup {
    fn span(&self) -> (f64, f64) {
        (self.start_seconds, self.end_seconds)
    }

    fn top_track_index(&self) -> usize {
        self.top_track_index
    }

    fn set_span(&mut self, start_seconds: f64, end_seconds: f64) {
        self.start_seconds = start_seconds;
        self.end_seconds = end_seconds;
    }

    fn absorb(&mut self, other: Self) {
        self.end_seconds = self.end_seconds.max(other.end_seconds);
        if other.top_track_index > self.top_track_index {
            self.top_track_index = other.top_track_index;
            self.top_item_id = other.top_item_id;
        }
    }
}

/// Composites clip transitions: factors dissolve in encoded sRGB, wipes mask
/// canvas columns, and dip solids draw in their track's slot.
const TRANSITION_CONTRACT: &str = "srgb-dissolve-canvas-wipe-track-solid-v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlattenFingerprint {
    schema_version: u32,
    kind: &'static str,
    start_micros: u64,
    end_micros: u64,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    frame_count: u32,
    top_track_index: usize,
    top_item_id: String,
    dependencies: Vec<FlattenDependencyFingerprint>,
    pixel_contract: String,
    blend_contract: String,
    sampling_policy: &'static str,
    compositor_revision: &'static str,
    compositor_backend: String,
    gpu_adapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transition_contract: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    transition_solids: Vec<TransitionSolid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlattenDependencyFingerprint {
    track_index: usize,
    item_index: usize,
    item_id: String,
    media_id: String,
    source_sha256: String,
    start_micros: u64,
    duration_micros: u64,
    source_in_micros: u64,
    source_out_micros: u64,
    playback_rate_micros: u32,
    frame_program: FrameProgram,
    effect_stack: PreparedEffectStack,
    blend_mode: &'static str,
    #[serde(skip_serializing_if = "ItemTransitions::is_empty")]
    transitions: ItemTransitions,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlattenStageManifest {
    schema_version: u32,
    fingerprint: String,
    frame_count: u32,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    start_micros: u64,
    end_micros: u64,
    dependency_item_ids: Vec<String>,
    blend_modes: Vec<String>,
    frame_rgba_sha256: Vec<String>,
    frame_png_sha256: Vec<String>,
    pixel_contract: String,
    blend_contract: String,
    blend_backend: BlendBackendReport,
}

pub(super) struct Dependency<'a> {
    track_index: usize,
    item_index: usize,
    item: &'a TimelineItem,
    media: &'a MediaAsset,
    source_path: std::path::PathBuf,
    source_sha256: String,
    source_in: f64,
    source_out: f64,
    speed: f64,
    frame_program: FrameProgram,
    effect_stack: PreparedEffectStack,
    blend_mode: BlendMode,
    blend_label: &'static str,
    transitions: ItemTransitions,
}

pub(super) fn prepare_flattened_composites(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let transitions = FlattenTransitions::plan(project);
    let groups = plan_groups(project, &transitions)?;
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let mut reports = Vec::with_capacity(groups.len());
    let mut synthetic = Vec::with_capacity(groups.len());
    for group in &groups {
        ensure_not_cancelled(cancellation)?;
        let (report, item, media) =
            render_group(project_dir, project, group, &transitions, cancellation)?;
        reports.push(report);
        synthetic.push((group.top_track_index, item, media));
    }
    rewrite_dependent_intervals(project, &groups)?;
    for (track_index, item, media) in synthetic {
        if !project.media.iter().any(|existing| existing.id == media.id) {
            project.media.push(media);
        }
        project.timeline.tracks[track_index].items.push(item);
        project.timeline.tracks[track_index]
            .items
            .sort_by(|left, right| left.start_seconds.total_cmp(&right.start_seconds));
    }
    project
        .timeline
        .tracks
        .retain(|track| track.kind != TrackKind::Video || !track.items.is_empty());
    Ok(reports)
}

fn plan_groups(
    project: &VideoProject,
    transitions: &FlattenTransitions,
) -> PipelineResult<Vec<FlattenGroup>> {
    let rich = rich_items(project)
        .into_iter()
        .map(|(track_index, item_index, item)| {
            let (start, end) = transitions
                .for_item(track_index, &item.id)
                .active_span(item);
            (track_index, item_index, item, start, end)
        })
        .collect::<Vec<_>>();
    let mut boundaries = rich
        .iter()
        .flat_map(|(track_index, _, item, start, end)| {
            [
                (*start, true, *track_index, item.id.as_str()),
                (*end, false, *track_index, item.id.as_str()),
            ]
        })
        .collect::<Vec<_>>();
    boundaries.sort_by(|left, right| left.0.total_cmp(&right.0));
    boundaries.dedup_by(|left, right| left.0 == right.0);
    let mut groups: Vec<FlattenGroup> = Vec::new();
    for window in boundaries.windows(2) {
        let start = window[0].0;
        let end = window[1].0;
        if !start.is_finite() || !end.is_finite() || end <= start {
            continue;
        }
        let Some((track_index, _, top, _, _)) = rich
            .iter()
            .filter(|(_, _, _, item_start, item_end)| *item_start <= start && *item_end > start)
            .max_by_key(|(track_index, _, _, _, _)| *track_index)
        else {
            continue;
        };
        if let Some(previous) = groups.last_mut() {
            if previous.top_track_index == *track_index
                && previous.top_item_id == top.id
                && (previous.end_seconds - start).abs() <= 1e-9
            {
                previous.end_seconds = end;
                continue;
            }
        }
        groups.push(FlattenGroup {
            start_seconds: start,
            end_seconds: end,
            top_track_index: *track_index,
            top_item_id: top.id.clone(),
        });
    }
    let groups = widen_groups_over_transitions(groups, transitions);
    for group in &groups {
        validate_group_track_overlaps(project, group)?;
    }
    Ok(groups)
}

fn rich_items(project: &VideoProject) -> Vec<(usize, usize, &TimelineItem)> {
    project
        .timeline
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.enabled && track.kind == TrackKind::Video)
        .flat_map(|(track_index, track)| {
            track
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| richer_blend(item).is_some() || has_prepared_effects(item))
                .map(move |(item_index, item)| (track_index, item_index, item))
        })
        .collect()
}

fn has_prepared_effects(item: &TimelineItem) -> bool {
    let stack = item
        .properties
        .get("effects")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|effects| {
            effects.iter().any(|effect| {
                effect.get("enabled").and_then(serde_json::Value::as_bool) != Some(false)
            })
        });
    let color_grade = item
        .properties
        .get("colorGrade")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|grade| grade.keys().any(|key| key != "lut"));
    stack || color_grade
}

fn richer_blend(item: &TimelineItem) -> Option<BlendMode> {
    match item
        .properties
        .get("blendMode")
        .and_then(serde_json::Value::as_str)
    {
        Some("source") => Some(BlendMode::Source),
        Some("add") => Some(BlendMode::Add),
        Some("darken") => Some(BlendMode::Darken),
        Some("multiply") => Some(BlendMode::Multiply),
        Some("colorBurn") => Some(BlendMode::ColorBurn),
        Some("lighten") => Some(BlendMode::Lighten),
        Some("screen") => Some(BlendMode::Screen),
        Some("colorDodge") => Some(BlendMode::ColorDodge),
        Some("overlay") => Some(BlendMode::Overlay),
        Some("softLight") => Some(BlendMode::SoftLight),
        Some("hardLight") => Some(BlendMode::HardLight),
        Some("difference") => Some(BlendMode::Difference),
        Some("exclusion") => Some(BlendMode::Exclusion),
        Some("hue") => Some(BlendMode::Hue),
        Some("saturation") => Some(BlendMode::Saturation),
        Some("color") => Some(BlendMode::Color),
        Some("luminosity") => Some(BlendMode::Luminosity),
        _ => None,
    }
}

fn validate_group_track_overlaps(
    project: &VideoProject,
    group: &FlattenGroup,
) -> PipelineResult<()> {
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if track_index > group.top_track_index || !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        let mut intervals = track
            .items
            .iter()
            .filter(|item| overlaps(item, group))
            .map(|item| {
                (
                    item.start_seconds,
                    item.start_seconds + item.duration_seconds,
                    &item.id,
                )
            })
            .collect::<Vec<_>>();
        intervals.sort_by(|left, right| left.0.total_cmp(&right.0));
        for pair in intervals.windows(2) {
            if pair[0].1 > pair[1].0 {
                return Err(vec![flatten_error(
                    "precompose.flatten.overlap",
                    "Flattened richer blends do not yet support simultaneous items on one video track.",
                )]);
            }
        }
    }
    Ok(())
}

fn render_group(
    project_dir: &Path,
    project: &VideoProject,
    group: &FlattenGroup,
    transitions: &FlattenTransitions,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<(PrecomposeReport, TimelineItem, MediaAsset)> {
    let width = project.render_settings.width;
    let height = project.render_settings.height;
    let fps = project.render_settings.fps;
    let (fps_numerator, fps_denominator) = fps_rational(fps).ok_or_else(|| {
        vec![flatten_error(
            "precompose.flatten.fps",
            "Flattened-composite frame rate is invalid.",
        )]
    })?;
    let duration = group.end_seconds - group.start_seconds;
    let frame_count = (duration * fps).ceil().max(1.0) as u32;
    let dependencies = collect_dependencies(project_dir, project, group, transitions)?;
    let solids = transitions.solids(
        group.top_track_index,
        group.start_seconds,
        group.end_seconds,
    );
    let mut compositor = FrameBlendCompositor::new(cfg!(feature = "gpu-render"));
    let dependency_fingerprints = dependencies
        .iter()
        .map(|dependency| dependency_fingerprint(dependency))
        .collect::<PipelineResult<Vec<_>>>()?;
    let start_micros = seconds_to_micros(group.start_seconds)?;
    let end_micros = seconds_to_micros(group.end_seconds)?;
    let (compositor_backend, gpu_adapter) = compositor.fingerprint_identity();
    let fingerprint = fingerprint(&FlattenFingerprint {
        schema_version: 1,
        kind: "flattenedComposite",
        start_micros,
        end_micros,
        width,
        height,
        fps_numerator,
        fps_denominator,
        frame_count,
        top_track_index: group.top_track_index,
        top_item_id: group.top_item_id.clone(),
        dependencies: dependency_fingerprints,
        pixel_contract: "rgba8-srgb-straight-v1".to_string(),
        blend_contract: "w3c-separable-linear-premultiplied-v1".to_string(),
        sampling_policy: "presentation-interval-hold-v1",
        compositor_revision: "shared-frame-program-effects-v3",
        compositor_backend: compositor_backend.to_string(),
        gpu_adapter: gpu_adapter.map(str::to_string),
        transition_contract: (!solids.is_empty()
            || dependencies
                .iter()
                .any(|dependency| !dependency.transitions.is_empty()))
        .then_some(TRANSITION_CONTRACT),
        transition_solids: solids.clone(),
    })?;
    let cache_dir = cache_entry_dir(project_dir, &fingerprint);
    fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir))
        .map_err(|error| vec![flatten_io_error("cache", error)])?;
    let _lock = CacheLock::acquire(project_dir, &fingerprint, Duration::from_secs(30))?;
    let mut cache_hit = true;
    let manifest = if let Some(manifest) = validate_cache_entry(&cache_dir, &fingerprint) {
        manifest
    } else {
        cache_hit = false;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
                .map_err(|error| vec![flatten_io_error("remove", error)])?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{}.staging-{}", fingerprint, Uuid::new_v4()));
        let mut staging_cleanup = StagingCleanup::new(staging.clone());
        let frames_dir = staging.join("frames");
        fs::create_dir_all(&frames_dir).map_err(|error| vec![flatten_io_error("frames", error)])?;
        let pixel_count = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                vec![flatten_error(
                    "precompose.flatten.dimensions",
                    "Flattened-composite dimensions overflowed.",
                )]
            })?;
        let mut canvases = vec![vec![0_u8; pixel_count * 4]; frame_count as usize];
        composite_group_layers(
            &dependencies,
            &solids,
            DependencyRenderContext {
                group,
                fps,
                fps_numerator,
                fps_denominator,
                width,
                height,
                canvases: &mut canvases,
                compositor: &mut compositor,
                cancellation,
            },
        )?;
        let mut frame_rgba_sha256 = Vec::with_capacity(frame_count as usize);
        let mut frame_png_sha256 = Vec::with_capacity(frame_count as usize);
        for (frame_index, canvas) in canvases.into_iter().enumerate() {
            ensure_not_cancelled(cancellation)?;
            frame_rgba_sha256.push(format!("{:x}", Sha256::digest(&canvas)));
            let frame_path = frames_dir.join(format!("frame-{frame_index:06}.png"));
            image::RgbaImage::from_raw(width, height, canvas)
                .ok_or_else(|| {
                    vec![flatten_error(
                        "precompose.flatten.frame",
                        "Flattened RGBA frame dimensions are invalid.",
                    )]
                })?
                .save(&frame_path)
                .map_err(|error| {
                    vec![flatten_error(
                        "precompose.flatten.png",
                        &format!("Flattened PNG could not be written: {error}"),
                    )]
                })?;
            frame_png_sha256.push(
                sha256_file(&frame_path)
                    .map_err(|error| vec![flatten_io_error("frameHash", error)])?,
            );
        }
        let intermediate = staging.join("intermediate.mov");
        package_png_frames_as_mov(
            &frames_dir,
            frame_count,
            PngSequenceSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
            },
            &intermediate,
            Duration::from_secs(60),
        )?;
        ensure_not_cancelled(cancellation)?;
        let stage_manifest = FlattenStageManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
            start_micros,
            end_micros,
            dependency_item_ids: dependencies
                .iter()
                .map(|value| value.item.id.clone())
                .collect(),
            blend_modes: dependencies
                .iter()
                .map(|value| value.blend_label.to_string())
                .collect(),
            frame_rgba_sha256,
            frame_png_sha256,
            pixel_contract: "rgba8-srgb-straight-v1".to_string(),
            blend_contract: "w3c-separable-linear-premultiplied-v1".to_string(),
            blend_backend: compositor.report().clone(),
        };
        fs::write(
            staging.join("manifest.json"),
            serde_json::to_vec_pretty(&stage_manifest).map_err(|error| {
                vec![flatten_error(
                    "precompose.flatten.manifest",
                    &error.to_string(),
                )]
            })?,
        )
        .map_err(|error| vec![flatten_io_error("manifest", error)])?;
        let intermediate_sha256 =
            sha256_file(&intermediate).map_err(|error| vec![flatten_io_error("hash", error)])?;
        let cache_manifest = CacheManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            worker_manifest: "manifest.json".to_string(),
            intermediate: "intermediate.mov".to_string(),
            intermediate_sha256,
            frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
        };
        fs::write(
            staging.join("cache-manifest.json"),
            serde_json::to_vec_pretty(&cache_manifest).map_err(|error| {
                vec![flatten_error(
                    "precompose.flatten.cacheManifest",
                    &error.to_string(),
                )]
            })?,
        )
        .map_err(|error| vec![flatten_io_error("cacheManifest", error)])?;
        ensure_not_cancelled(cancellation)?;
        fs::rename(&staging, &cache_dir)
            .map_err(|error| vec![flatten_io_error("publish", error)])?;
        staging_cleanup.disarm();
        cache_manifest
    };
    let stage_manifest: FlattenStageManifest = serde_json::from_slice(
        &fs::read(cache_dir.join(&manifest.worker_manifest))
            .map_err(|error| vec![flatten_io_error("readManifest", error)])?,
    )
    .map_err(|error| {
        vec![flatten_error(
            "precompose.flatten.manifest",
            &format!("Flattened stage manifest could not be decoded: {error}"),
        )]
    })?;
    let relative_intermediate =
        project_relative_path(project_dir, &cache_dir.join(&manifest.intermediate))?;
    let prepared_media_id = format!("precompose-{fingerprint}");
    let synthetic_item = TimelineItem {
        id: format!("flatten-{}-{}", group.top_item_id, &fingerprint[..12]),
        kind: TimelineItemKind::VideoClip,
        start_seconds: group.start_seconds,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: prepared_media_id.clone(),
        },
        label: format!("Flattened {}", group.top_item_id),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(duration)),
            ("blendMode".to_string(), json!("over")),
        ]),
    };
    let synthetic_media = MediaAsset {
        id: prepared_media_id.clone(),
        name: Some(format!("Flattened {}", group.top_item_id)),
        relative_path: relative_intermediate.clone(),
        kind: MediaKind::Video,
        duration_seconds: duration,
        width: Some(width),
        height: Some(height),
        fps: Some(fps),
        folder_id: None,
    };
    let report = PrecomposeReport {
        stage: "flattenedComposite".to_string(),
        item_id: group.top_item_id.clone(),
        media_id: dependencies
            .last()
            .map(|dependency| dependency.media.id.clone())
            .unwrap_or_default(),
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: Some(stage_manifest.blend_backend.backend),
        compositor_fallback: stage_manifest.blend_backend.fallback_reason,
        worker_manifest: project_relative_path(project_dir, &cache_dir.join("manifest.json"))?,
        intermediate: relative_intermediate,
    };
    Ok((report, synthetic_item, synthetic_media))
}

pub(super) fn collect_dependencies<'a>(
    project_dir: &Path,
    project: &'a VideoProject,
    group: &FlattenGroup,
    transitions: &FlattenTransitions,
) -> PipelineResult<Vec<Dependency<'a>>> {
    let mut dependencies = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if track_index > group.top_track_index || !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        for (item_index, item) in track.items.iter().enumerate() {
            let item_transitions = transitions.for_item(track_index, &item.id);
            let (active_start, active_end) = item_transitions.active_span(item);
            if active_start >= group.end_seconds || active_end <= group.start_seconds {
                continue;
            }
            let TimelineSource::Media { media_id } = &item.source else {
                return Err(vec![flatten_error(
                    "precompose.flatten.source",
                    "Richer-blend flattening requires media-backed visual items.",
                )]);
            };
            let media = project
                .media
                .iter()
                .find(|media| media.id == *media_id)
                .ok_or_else(|| {
                    vec![flatten_error(
                        "precompose.flatten.media",
                        "Flatten dependency media was not found.",
                    )]
                })?;
            if media.kind != MediaKind::Video {
                return Err(vec![flatten_error(
                    "precompose.flatten.mediaKind",
                    "Richer-blend flattening currently requires video or prepared Lottie/LUT sources.",
                )]);
            }
            let source_path = project_dir.join(&media.relative_path);
            let source_sha256 = format!(
                "{:x}",
                Sha256::digest(fs::read(&source_path).map_err(|error| {
                    vec![flatten_error(
                        "precompose.flatten.sourceRead",
                        &error.to_string(),
                    )]
                })?)
            );
            let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
            let source_out = numeric_property(&item.properties, "sourceOut")
                .unwrap_or(source_in + item.duration_seconds);
            let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
            if !source_in.is_finite()
                || !source_out.is_finite()
                || source_in < 0.0
                || source_out <= source_in
                || !speed.is_finite()
                || !(0.1..=8.0).contains(&speed)
            {
                return Err(vec![flatten_error(
                    "precompose.flatten.properties",
                    "Flatten dependency timing, speed, or opacity is invalid.",
                )]);
            }
            let frame_program = frame_program_for_item(item)?;
            let effect_stack = PreparedEffectStack::from_item_properties_for_duration(
                &item.properties,
                item.duration_seconds,
            )
            .map_err(|error| {
                vec![flatten_error(
                    "precompose.flatten.effects",
                    &format!("Effect stack preparation failed: {error}"),
                )]
            })?;
            let (blend_mode, blend_label) = blend_for_item(item)?;
            dependencies.push(Dependency {
                track_index,
                item_index,
                item,
                media,
                source_path,
                source_sha256,
                source_in,
                source_out,
                speed,
                frame_program,
                effect_stack,
                blend_mode,
                blend_label,
                transitions: item_transitions,
            });
        }
    }
    // Canonical clips on a track never overlap, so start order only matters for
    // a transition pair: the outgoing clip draws beneath the incoming clip.
    dependencies.sort_by(|left, right| {
        left.track_index
            .cmp(&right.track_index)
            .then_with(|| left.item.start_seconds.total_cmp(&right.item.start_seconds))
            .then_with(|| left.item_index.cmp(&right.item_index))
    });
    Ok(dependencies)
}

pub(super) struct DependencyRenderContext<'a> {
    pub(super) group: &'a FlattenGroup,
    pub(super) fps: f64,
    pub(super) fps_numerator: u32,
    pub(super) fps_denominator: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) canvases: &'a mut [Vec<u8>],
    pub(super) compositor: &'a mut FrameBlendCompositor,
    pub(super) cancellation: Option<&'a RenderCancellationToken>,
}

/// Composites every layer of a group in canonical order: tracks bottom to
/// top, and on each track the dip solids beneath the clips in start order.
pub(super) fn composite_group_layers(
    dependencies: &[Dependency<'_>],
    solids: &[TransitionSolid],
    context: DependencyRenderContext<'_>,
) -> PipelineResult<()> {
    enum Layer<'layer, 'project> {
        Solid(&'layer TransitionSolid),
        Clip(&'layer Dependency<'project>),
    }
    let mut layers = solids
        .iter()
        .map(|solid| (solid.track_index, 0, Layer::Solid(solid)))
        .chain(
            dependencies
                .iter()
                .map(|dependency| (dependency.track_index, 1, Layer::Clip(dependency))),
        )
        .collect::<Vec<_>>();
    layers.sort_by_key(|(track_index, rank, _)| (*track_index, *rank));
    let DependencyRenderContext {
        group,
        fps,
        fps_numerator,
        fps_denominator,
        width,
        height,
        canvases,
        compositor,
        cancellation,
    } = context;
    for (_, _, layer) in layers {
        ensure_not_cancelled(cancellation)?;
        match layer {
            Layer::Solid(solid) => {
                render_solid_frames(solid, group, fps, width, height, canvases, compositor)?
            }
            Layer::Clip(dependency) => render_dependency_frames(
                dependency,
                DependencyRenderContext {
                    group,
                    fps,
                    fps_numerator,
                    fps_denominator,
                    width,
                    height,
                    canvases: &mut *canvases,
                    compositor: &mut *compositor,
                    cancellation,
                },
            )?,
        }
    }
    Ok(())
}

fn render_solid_frames(
    solid: &TransitionSolid,
    group: &FlattenGroup,
    fps: f64,
    width: u32,
    height: u32,
    canvases: &mut [Vec<u8>],
    compositor: &mut FrameBlendCompositor,
) -> PipelineResult<()> {
    let mut frame = None;
    for (frame_index, canvas) in canvases.iter_mut().enumerate() {
        if !solid
            .window
            .contains(group.start_seconds + frame_index as f64 / fps)
        {
            continue;
        }
        let frame =
            frame.get_or_insert_with(|| solid.rgba.repeat(width as usize * height as usize));
        *canvas = compositor
            .composite_frame(canvas, frame, BlendMode::Over)
            .map_err(|error| vec![flatten_error("precompose.flatten.blendBackend", error)])?;
    }
    Ok(())
}

/// Canonical sampling of one visual clip at a timeline time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ClipFrameSample {
    /// Seconds from the clip's canonical start; negative in its head handle.
    pub(super) offset_seconds: f64,
    /// Source seconds: `sourceIn + offset * speed`, continuing into the handles.
    pub(super) source_seconds: f64,
    pub(super) transition: Option<TransitionFrameState>,
}

pub(super) fn sample_clip_at(
    item: &TimelineItem,
    transitions: &ItemTransitions,
    source_in: f64,
    speed: f64,
    seconds: f64,
) -> Option<ClipFrameSample> {
    if !transitions.is_active(item, seconds) {
        return None;
    }
    let offset_seconds = seconds - item.start_seconds;
    Some(ClipFrameSample {
        offset_seconds,
        source_seconds: source_in + offset_seconds * speed,
        transition: transitions.state_at(seconds),
    })
}

fn render_dependency_frames(
    dependency: &Dependency<'_>,
    context: DependencyRenderContext<'_>,
) -> PipelineResult<()> {
    let DependencyRenderContext {
        group,
        fps,
        fps_numerator,
        fps_denominator,
        width,
        height,
        canvases,
        compositor,
        cancellation,
    } = context;
    let sample_at = |frame_index: usize| {
        sample_clip_at(
            dependency.item,
            &dependency.transitions,
            dependency.source_in,
            dependency.speed,
            group.start_seconds + frame_index as f64 / fps,
        )
    };
    let active_indices = (0..canvases.len())
        .filter(|frame_index| sample_at(*frame_index).is_some())
        .collect::<Vec<_>>();
    let Some(first_index) = active_indices.first().copied() else {
        return Ok(());
    };
    let frame_count = active_indices.len() as u32;
    let source_start = sample_at(first_index)
        .map_or(dependency.source_in, |sample| sample.source_seconds)
        .max(0.0);
    let source_stop = source_start + frame_count as f64 / fps * dependency.speed;
    let tail_source_seconds = dependency.transitions.tail_seconds() * dependency.speed;
    if source_stop > dependency.source_out + tail_source_seconds + 1e-6
        || source_stop > dependency.media.duration_seconds + 1e-6
    {
        return Err(vec![flatten_error(
            "precompose.flatten.sourceRange",
            "Flatten dependency sampling extends beyond its source range.",
        )]);
    }
    let duration = dependency.item.duration_seconds;
    decode_video_frames_rgba(
        &dependency.source_path,
        FrameDecodeSpec {
            width,
            height,
            fps_numerator,
            fps_denominator,
            source_start_micros: seconds_to_micros(source_start)?,
            source_stop_micros: seconds_to_micros(source_stop)?,
            playback_rate_micros: (dependency.speed * 1_000_000.0).round() as u32,
            frame_count,
        },
        Duration::from_secs(120),
        |decoded_index, rgba| {
            ensure_not_cancelled(cancellation)?;
            let canvas_index = active_indices[decoded_index as usize];
            let canvas = &mut canvases[canvas_index];
            let Some(sample) = sample_at(canvas_index) else {
                return Ok(());
            };
            // Keyframes hold and fades clamp at the canonical clip bounds.
            let local_seconds = sample.offset_seconds.clamp(0.0, duration.max(0.0));
            let sampled_program =
                dependency
                    .frame_program
                    .sample(local_seconds)
                    .map_err(|error| {
                        vec![flatten_error(
                            "precompose.flatten.frameProgram",
                            &format!("Frame program sampling failed: {error}"),
                        )]
                    })?;
            let effected = dependency
                .effect_stack
                .apply_rgba8_srgb_at_seconds(
                    rgba,
                    width,
                    height,
                    canvas_index as u32,
                    local_seconds,
                )
                .map_err(|error| {
                    vec![flatten_error(
                        "precompose.flatten.effects",
                        &format!("Effect stack execution failed: {error}"),
                    )]
                })?;
            let mut source = transform_rgba8_srgb(&effected, width, height, &sampled_program)
                .map_err(|error| {
                    vec![flatten_error(
                        "precompose.flatten.frameProgram",
                        &format!("Frame program execution failed: {error}"),
                    )]
                })?;
            if let Some(transition) = sample.transition {
                if transition.wipe_inset_right.is_some() {
                    apply_wipe_mask_rgba8(&mut source, width, height, transition.progress);
                }
            }
            let composited = compositor
                .composite_frame(canvas, &source, dependency.blend_mode)
                .map_err(|error| vec![flatten_error("precompose.flatten.blendBackend", error)])?;
            *canvas = match sample.transition {
                Some(transition) if transition.opacity < 1.0 => {
                    dissolve_rgba8_srgb(canvas, &composited, transition.opacity)
                }
                _ => composited,
            };
            Ok(())
        },
    )
}

fn dependency_fingerprint(
    dependency: &Dependency<'_>,
) -> PipelineResult<FlattenDependencyFingerprint> {
    Ok(FlattenDependencyFingerprint {
        track_index: dependency.track_index,
        item_index: dependency.item_index,
        item_id: dependency.item.id.clone(),
        media_id: dependency.media.id.clone(),
        source_sha256: dependency.source_sha256.clone(),
        start_micros: seconds_to_micros(dependency.item.start_seconds)?,
        duration_micros: seconds_to_micros(dependency.item.duration_seconds)?,
        source_in_micros: seconds_to_micros(dependency.source_in)?,
        source_out_micros: seconds_to_micros(dependency.source_out)?,
        playback_rate_micros: (dependency.speed * 1_000_000.0).round() as u32,
        frame_program: dependency.frame_program.clone(),
        effect_stack: dependency.effect_stack.clone(),
        blend_mode: dependency.blend_label,
        transitions: dependency.transitions,
    })
}

pub(super) fn frame_program_for_item(item: &TimelineItem) -> PipelineResult<FrameProgram> {
    let mut program = FrameProgram::identity(item.duration_seconds);
    let keyframes = match item.properties.get("keyframes") {
        None => None,
        Some(serde_json::Value::Object(keyframes)) => Some(keyframes),
        Some(_) => {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                "Visual keyframes must be an object of numeric keyframe arrays.",
            )])
        }
    };
    if let Some(keyframes) = keyframes {
        const SUPPORTED: &[&str] = &[
            "opacity",
            "positionX",
            "positionY",
            "scale",
            "scaleX",
            "scaleY",
            "rotationDegrees",
            "cropTop",
            "cropRight",
            "cropBottom",
            "cropLeft",
            "volumeDb",
        ];
        if let Some(property) = keyframes
            .keys()
            .find(|property| !SUPPORTED.contains(&property.as_str()))
        {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                &format!("Frame program does not support keyframes for `{property}`."),
            )]);
        }
    }

    program.canvas_transform = canvas_transform_for_item(item)?;
    program.position_x = numeric_curve(item, "positionX", 0.0, -10_000.0, 10_000.0)?;
    program.position_y = numeric_curve(item, "positionY", 0.0, -10_000.0, 10_000.0)?;
    let uniform_scale = numeric_curve(item, "scale", 1.0, 0.01, 100.0)?;
    program.scale_x = if has_curve_or_base(item, "scaleX") {
        numeric_curve(item, "scaleX", 1.0, 0.01, 100.0)?
    } else {
        uniform_scale.clone()
    };
    program.scale_y = if has_curve_or_base(item, "scaleY") {
        numeric_curve(item, "scaleY", 1.0, 0.01, 100.0)?
    } else {
        uniform_scale
    };
    program.rotation_degrees = numeric_curve(item, "rotationDegrees", 0.0, -360.0, 360.0)?;
    program.opacity = numeric_curve(item, "opacity", 1.0, 0.0, 1.0)?;
    program.crop_top = numeric_curve(item, "cropTop", 0.0, 0.0, 1.0)?;
    program.crop_right = numeric_curve(item, "cropRight", 0.0, 0.0, 1.0)?;
    program.crop_bottom = numeric_curve(item, "cropBottom", 0.0, 0.0, 1.0)?;
    program.crop_left = numeric_curve(item, "cropLeft", 0.0, 0.0, 1.0)?;
    program.fade_in_seconds = optional_number_property(item, "fadeInSeconds")?.unwrap_or(0.0);
    program.fade_out_seconds = optional_number_property(item, "fadeOutSeconds")?.unwrap_or(0.0);
    if !program.fade_in_seconds.is_finite()
        || !program.fade_out_seconds.is_finite()
        || program.fade_in_seconds < 0.0
        || program.fade_out_seconds < 0.0
        || program.fade_in_seconds + program.fade_out_seconds > item.duration_seconds
    {
        return Err(vec![flatten_error(
            "precompose.flatten.fades",
            "Visual fades must be non-negative and fit inside the clip duration.",
        )]);
    }
    for seconds in [0.0, item.duration_seconds] {
        program.sample(seconds).map_err(|error| {
            vec![flatten_error(
                "precompose.flatten.frameProgram",
                &format!("Frame program is invalid: {error}"),
            )]
        })?;
    }
    Ok(program)
}

fn canvas_transform_for_item(item: &TimelineItem) -> PipelineResult<CanvasTransform> {
    let transform = match item.properties.get("transform") {
        None => None,
        Some(serde_json::Value::Object(transform)) => Some(transform),
        Some(_) => {
            return Err(vec![flatten_error(
                "precompose.flatten.transform",
                "Visual transform must be an object.",
            )])
        }
    };
    let value = |key: &str| {
        transform
            .and_then(|transform| transform.get(key))
            .or_else(|| item.properties.get(key))
    };
    let transform = CanvasTransform {
        center_x: typed_number(value("centerX"), "centerX")?.unwrap_or(0.5),
        center_y: typed_number(value("centerY"), "centerY")?.unwrap_or(0.5),
        width: typed_number(value("width"), "width")?.unwrap_or(1.0),
        height: typed_number(value("height"), "height")?.unwrap_or(1.0),
        flip_horizontal: typed_boolean(value("flipHorizontal"), "flipHorizontal")?.unwrap_or(false),
        flip_vertical: typed_boolean(value("flipVertical"), "flipVertical")?.unwrap_or(false),
    };
    if !transform.center_x.is_finite()
        || !transform.center_y.is_finite()
        || !(0.0..=1.0).contains(&transform.center_x)
        || !(0.0..=1.0).contains(&transform.center_y)
        || !transform.width.is_finite()
        || !transform.height.is_finite()
        || !(0.0..=1.0).contains(&transform.width)
        || !(0.0..=1.0).contains(&transform.height)
        || transform.width == 0.0
        || transform.height == 0.0
    {
        return Err(vec![flatten_error(
            "precompose.flatten.transform",
            "Canvas transform center and dimensions are outside supported normalized bounds.",
        )]);
    }
    Ok(transform)
}

fn has_curve_or_base(item: &TimelineItem, property: &str) -> bool {
    item.properties.contains_key(property)
        || item
            .properties
            .get("keyframes")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|keyframes| keyframes.contains_key(property))
}

fn numeric_curve(
    item: &TimelineItem,
    property: &str,
    fallback: f64,
    minimum: f64,
    maximum: f64,
) -> PipelineResult<NumericCurve> {
    let base = optional_number_property(item, property)?.unwrap_or(fallback);
    if !base.is_finite() || !(minimum..=maximum).contains(&base) {
        return Err(vec![flatten_error(
            "precompose.flatten.frameProgram",
            &format!("Frame program base `{property}` is outside supported bounds."),
        )]);
    }
    let Some(value) = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get(property))
    else {
        return Ok(NumericCurve::constant(base));
    };
    let Some(keyframes) = value.as_array() else {
        return Err(vec![flatten_error(
            "precompose.flatten.keyframes",
            &format!("Keyframes for `{property}` must be an array."),
        )]);
    };
    let mut parsed = Vec::with_capacity(keyframes.len());
    let mut previous = None;
    for (index, keyframe) in keyframes.iter().enumerate() {
        let Some(keyframe) = keyframe.as_object() else {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                &format!("Keyframe {index} for `{property}` must be an object."),
            )]);
        };
        let Some(at_seconds) = keyframe
            .get("atSeconds")
            .and_then(serde_json::Value::as_f64)
        else {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                &format!("Keyframe {index} for `{property}` is missing atSeconds."),
            )]);
        };
        let Some(value) = keyframe.get("value").and_then(serde_json::Value::as_f64) else {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                &format!("Keyframe {index} for `{property}` is missing value."),
            )]);
        };
        if !at_seconds.is_finite()
            || !(0.0..=item.duration_seconds).contains(&at_seconds)
            || previous.is_some_and(|previous| at_seconds <= previous)
            || !value.is_finite()
            || !(minimum..=maximum).contains(&value)
        {
            return Err(vec![flatten_error(
                "precompose.flatten.keyframes",
                &format!("Keyframe {index} for `{property}` is outside supported bounds."),
            )]);
        }
        let easing = match keyframe
            .get("easing")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("linear")
        {
            "linear" => KeyframeEasing::Linear,
            "hold" => KeyframeEasing::Hold,
            "easeIn" => KeyframeEasing::EaseIn,
            "easeOut" => KeyframeEasing::EaseOut,
            "easeInOut" | "smooth" => KeyframeEasing::EaseInOut,
            easing => {
                return Err(vec![flatten_error(
                    "precompose.flatten.keyframes",
                    &format!("Keyframe easing `{easing}` is not supported."),
                )])
            }
        };
        previous = Some(at_seconds);
        parsed.push(NumericKeyframe {
            at_seconds,
            value,
            easing,
        });
    }
    Ok(NumericCurve {
        base,
        keyframes: parsed,
    })
}

fn optional_number_property(item: &TimelineItem, property: &str) -> PipelineResult<Option<f64>> {
    typed_number(item.properties.get(property), property)
}

fn typed_number(value: Option<&serde_json::Value>, property: &str) -> PipelineResult<Option<f64>> {
    match value {
        None => Ok(None),
        Some(value) => value.as_f64().map(Some).ok_or_else(|| {
            vec![flatten_error(
                "precompose.flatten.frameProgram",
                &format!("Frame program property `{property}` must be numeric."),
            )]
        }),
    }
}

fn typed_boolean(
    value: Option<&serde_json::Value>,
    property: &str,
) -> PipelineResult<Option<bool>> {
    match value {
        None => Ok(None),
        Some(value) => value.as_bool().map(Some).ok_or_else(|| {
            vec![flatten_error(
                "precompose.flatten.frameProgram",
                &format!("Frame program property `{property}` must be boolean."),
            )]
        }),
    }
}

fn blend_for_item(item: &TimelineItem) -> PipelineResult<(BlendMode, &'static str)> {
    match item
        .properties
        .get("blendMode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("over")
    {
        "source" => Ok((BlendMode::Source, "source")),
        "over" | "normal" => Ok((BlendMode::Over, "over")),
        "add" => Ok((BlendMode::Add, "add")),
        "darken" => Ok((BlendMode::Darken, "darken")),
        "multiply" => Ok((BlendMode::Multiply, "multiply")),
        "colorBurn" => Ok((BlendMode::ColorBurn, "colorBurn")),
        "lighten" => Ok((BlendMode::Lighten, "lighten")),
        "screen" => Ok((BlendMode::Screen, "screen")),
        "colorDodge" => Ok((BlendMode::ColorDodge, "colorDodge")),
        "overlay" => Ok((BlendMode::Overlay, "overlay")),
        "softLight" => Ok((BlendMode::SoftLight, "softLight")),
        "hardLight" => Ok((BlendMode::HardLight, "hardLight")),
        "difference" => Ok((BlendMode::Difference, "difference")),
        "exclusion" => Ok((BlendMode::Exclusion, "exclusion")),
        "hue" => Ok((BlendMode::Hue, "hue")),
        "saturation" => Ok((BlendMode::Saturation, "saturation")),
        "color" => Ok((BlendMode::Color, "color")),
        "luminosity" => Ok((BlendMode::Luminosity, "luminosity")),
        mode => Err(vec![flatten_error(
            "precompose.flatten.blendMode",
            &format!("Richer blend mode `{mode}` is not implemented by the canonical CPU stage."),
        )]),
    }
}

fn overlaps(item: &TimelineItem, group: &FlattenGroup) -> bool {
    item.start_seconds < group.end_seconds
        && item.start_seconds + item.duration_seconds > group.start_seconds
}

fn rewrite_dependent_intervals(
    project: &mut VideoProject,
    groups: &[FlattenGroup],
) -> PipelineResult<()> {
    for (track_index, track) in project.timeline.tracks.iter_mut().enumerate() {
        if !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        let intervals = groups
            .iter()
            .filter(|group| track_index <= group.top_track_index)
            .map(|group| (group.start_seconds, group.end_seconds))
            .collect::<Vec<_>>();
        if intervals.is_empty() {
            continue;
        }
        let mut rewritten = Vec::new();
        let mut segment_edges = BTreeMap::new();
        for item in &track.items {
            let segments = split_item_outside_intervals(item, &intervals)?;
            if !(segments.len() == 1 && segments[0].id == item.id) {
                let item_end = item.start_seconds + item.duration_seconds;
                let kept_start = segments
                    .first()
                    .filter(|segment| (segment.start_seconds - item.start_seconds).abs() <= 1e-9)
                    .map(|segment| segment.id.clone());
                let kept_end = segments
                    .last()
                    .filter(|segment| {
                        (segment.start_seconds + segment.duration_seconds - item_end).abs() <= 1e-9
                    })
                    .map(|segment| segment.id.clone());
                segment_edges.insert(item.id.clone(), (kept_start, kept_end));
            }
            rewritten.extend(segments);
        }
        rewritten.sort_by(|left, right| left.start_seconds.total_cmp(&right.start_seconds));
        track.items = rewritten;
        retarget_split_transitions(track, &segment_edges);
    }
    Ok(())
}

fn split_item_outside_intervals(
    item: &TimelineItem,
    intervals: &[(f64, f64)],
) -> PipelineResult<Vec<TimelineItem>> {
    let item_start = item.start_seconds;
    let item_end = item.start_seconds + item.duration_seconds;
    let mut remaining = vec![(item_start, item_end)];
    for &(cut_start, cut_end) in intervals {
        let mut next = Vec::new();
        for (start, end) in remaining {
            if cut_end <= start || cut_start >= end {
                next.push((start, end));
                continue;
            }
            if cut_start > start {
                next.push((start, cut_start.min(end)));
            }
            if cut_end < end {
                next.push((cut_end.max(start), end));
            }
        }
        remaining = next;
    }
    if remaining.len() == 1
        && (remaining[0].0 - item_start).abs() <= 1e-9
        && (remaining[0].1 - item_end).abs() <= 1e-9
    {
        return Ok(vec![item.clone()]);
    }
    let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
    let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
    if !source_in.is_finite() || !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
        return Err(vec![flatten_error(
            "precompose.flatten.split",
            "A dependent clip could not be split because its source timing is invalid.",
        )]);
    }
    Ok(remaining
        .into_iter()
        .enumerate()
        .filter(|(_, (start, end))| end - start > 1e-9)
        .map(|(index, (start, end))| {
            let mut segment = item.clone();
            segment.id = format!("{}-precompose-segment-{}", item.id, index + 1);
            segment.start_seconds = start;
            segment.duration_seconds = end - start;
            let segment_source_in = source_in + (start - item_start) * speed;
            let segment_source_out = segment_source_in + segment.duration_seconds * speed;
            segment
                .properties
                .insert("sourceIn".to_string(), json!(segment_source_in));
            segment
                .properties
                .insert("sourceOut".to_string(), json!(segment_source_out));
            segment
        })
        .collect())
}

fn ensure_not_cancelled(cancellation: Option<&RenderCancellationToken>) -> PipelineResult<()> {
    if cancellation.is_some_and(RenderCancellationToken::is_cancelled) {
        return Err(vec![flatten_error(
            "precompose.flatten.cancelled",
            "Flattened-composite preparation was cancelled.",
        )]);
    }
    Ok(())
}

fn flatten_error(path: &str, message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        path,
        message,
        "Use Source/Over/Add natively, or keep richer-blend dependencies within the supported CPU frame contract.",
    )
}

fn flatten_io_error(path: &str, error: std::io::Error) -> PipelineError {
    flatten_error(
        &format!("precompose.flatten.{path}"),
        &format!("Flattened-composite cache operation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_compositor::{composite_rgba8_srgb, CubeLut};
    use crate::project::fixtures::sample_project;

    #[test]
    fn all_palmier_blend_modes_route_through_canonical_precomposition() {
        let mut item = sample_project().timeline.tracks[0].items[0].clone();
        for (label, expected) in [
            ("darken", BlendMode::Darken),
            ("multiply", BlendMode::Multiply),
            ("colorBurn", BlendMode::ColorBurn),
            ("lighten", BlendMode::Lighten),
            ("screen", BlendMode::Screen),
            ("colorDodge", BlendMode::ColorDodge),
            ("overlay", BlendMode::Overlay),
            ("softLight", BlendMode::SoftLight),
            ("hardLight", BlendMode::HardLight),
            ("difference", BlendMode::Difference),
            ("exclusion", BlendMode::Exclusion),
            ("hue", BlendMode::Hue),
            ("saturation", BlendMode::Saturation),
            ("color", BlendMode::Color),
            ("luminosity", BlendMode::Luminosity),
        ] {
            item.properties
                .insert("blendMode".to_string(), json!(label));
            assert_eq!(richer_blend(&item), Some(expected), "mode={label}");
            assert_eq!(
                blend_for_item(&item).expect("blend mapping"),
                (expected, label)
            );
        }
    }

    #[test]
    fn highest_rich_item_segments_and_merges_intervals() {
        let mut project = sample_project();
        let mut lower = project.timeline.tracks[0].clone();
        lower.items[0]
            .properties
            .insert("blendMode".to_string(), json!("multiply"));
        lower.items[0].start_seconds = 0.0;
        lower.items[0].duration_seconds = 4.0;
        let mut upper = lower.clone();
        upper.id = "upper".to_string();
        upper.items[0].id = "upper-rich".to_string();
        upper.items[0].start_seconds = 1.0;
        upper.items[0].duration_seconds = 2.0;
        upper.items[0]
            .properties
            .insert("blendMode".to_string(), json!("screen"));
        project.timeline.tracks = vec![lower, upper];

        let groups = plan_groups(&project, &FlattenTransitions::plan(&project)).expect("groups");
        assert_eq!(groups.len(), 3);
        assert_eq!((groups[0].start_seconds, groups[0].end_seconds), (0.0, 1.0));
        assert_eq!(groups[1].top_item_id, "upper-rich");
        assert_eq!((groups[2].start_seconds, groups[2].end_seconds), (3.0, 4.0));
    }

    fn transition_track(items: Vec<(&str, f64, f64, f64)>) -> crate::project::model::TimelineTrack {
        let mut track = sample_project().timeline.tracks[0].clone();
        track.items = items
            .into_iter()
            .map(|(id, start, duration, source_in)| TimelineItem {
                id: id.to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: start,
                duration_seconds: duration,
                source: TimelineSource::Media {
                    media_id: "media-1".to_string(),
                },
                label: id.to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(source_in)),
                    ("sourceOut".to_string(), json!(source_in + duration)),
                ]),
            })
            .collect();
        track
    }

    fn crossfade(left: &str, right: &str) -> crate::project::model::TimelineTransition {
        crate::project::model::TimelineTransition {
            id: format!("{left}-{right}"),
            left_item_id: left.to_string(),
            right_item_id: right.to_string(),
            kind: crate::project::model::TransitionKind::Crossfade,
            duration_seconds: 1.0,
        }
    }

    #[test]
    fn flattened_groups_widen_over_transition_windows_and_bake_them() {
        let mut project = sample_project();
        let mut track = transition_track(vec![("left", 0.0, 4.0, 2.0), ("right", 4.0, 4.0, 5.0)]);
        track.items[1]
            .properties
            .insert("blendMode".to_string(), json!("screen"));
        track.transitions = vec![crossfade("left", "right")];
        project.timeline.tracks = vec![track];

        let transitions = FlattenTransitions::plan(&project);
        let groups = plan_groups(&project, &transitions).expect("groups");
        assert_eq!(
            groups,
            [FlattenGroup {
                start_seconds: 3.5,
                end_seconds: 8.0,
                top_track_index: 0,
                top_item_id: "right".to_string(),
            }]
        );
        let left_transitions = transitions.for_item(0, "left");
        assert_eq!(
            left_transitions.active_span(&project.timeline.tracks[0].items[0]),
            (0.0, 4.5)
        );

        rewrite_dependent_intervals(&mut project, &groups).expect("rewrite");
        let track = &project.timeline.tracks[0];
        assert_eq!(track.items.len(), 1);
        assert_eq!(track.items[0].id, "left-precompose-segment-1");
        assert_eq!(
            (
                track.items[0].start_seconds,
                track.items[0].duration_seconds
            ),
            (0.0, 3.5)
        );
        assert!(
            track.transitions.is_empty(),
            "the baked transition is dropped"
        );
    }

    #[test]
    fn transitions_outside_flattened_groups_follow_the_split_segments() {
        let mut project = sample_project();
        let mut lower = transition_track(vec![("a", 0.0, 4.0, 0.0), ("b", 4.0, 4.0, 4.0)]);
        lower.transitions = vec![crossfade("a", "b")];
        let mut upper = transition_track(vec![("rich", 1.0, 1.0, 0.0)]);
        upper.id = "upper".to_string();
        upper.items[0]
            .properties
            .insert("blendMode".to_string(), json!("multiply"));
        project.timeline.tracks = vec![lower.clone(), upper.clone()];

        let transitions = FlattenTransitions::plan(&project);
        let groups = plan_groups(&project, &transitions).expect("groups");
        assert_eq!(
            groups.iter().map(|group| group.span()).collect::<Vec<_>>(),
            [(1.0, 2.0)]
        );
        rewrite_dependent_intervals(&mut project, &groups).expect("rewrite");
        let transition = &project.timeline.tracks[0].transitions[0];
        assert_eq!(
            (
                transition.left_item_id.as_str(),
                transition.right_item_id.as_str()
            ),
            ("a-precompose-segment-2", "b")
        );
        assert_eq!(
            crate::render_pipeline::transition_plan::plan_clip_transitions(&project).len(),
            1,
            "the retargeted transition still renders"
        );

        // A rich item touching the window widens its group over the whole window.
        upper.items[0].start_seconds = 3.0;
        upper.items[0].duration_seconds = 0.75;
        project.timeline.tracks = vec![lower, upper];
        let transitions = FlattenTransitions::plan(&project);
        let groups = plan_groups(&project, &transitions).expect("groups");
        assert_eq!(
            groups
                .iter()
                .map(|group| (group.span(), group.top_track_index))
                .collect::<Vec<_>>(),
            [((3.0, 4.5), 1)]
        );
        rewrite_dependent_intervals(&mut project, &groups).expect("rewrite");
        let track = &project.timeline.tracks[0];
        assert!(track.transitions.is_empty());
        assert_eq!(
            track
                .items
                .iter()
                .map(|item| (item.start_seconds, item.duration_seconds))
                .collect::<Vec<_>>(),
            [(0.0, 3.0), (4.5, 3.5)]
        );
    }

    #[test]
    fn groups_without_transitions_keep_the_rich_item_spans() {
        let mut project = sample_project();
        let mut track = transition_track(vec![("left", 0.0, 4.0, 2.0), ("right", 4.0, 4.0, 5.0)]);
        track.items[1]
            .properties
            .insert("blendMode".to_string(), json!("screen"));
        project.timeline.tracks = vec![track];
        let groups = plan_groups(&project, &FlattenTransitions::plan(&project)).expect("groups");
        assert_eq!(
            groups.iter().map(|group| group.span()).collect::<Vec<_>>(),
            [(4.0, 8.0)]
        );
    }

    #[test]
    fn malformed_frame_program_properties_fail_closed() {
        let mut project = sample_project();
        let item = &mut project.timeline.tracks[0].items[0];
        item.properties
            .insert("opacity".to_string(), json!("fully-visible"));
        let errors = frame_program_for_item(item).expect_err("string opacity must fail");
        assert!(errors[0].message.contains("`opacity` must be numeric"));

        item.properties.insert("opacity".to_string(), json!(1.0));
        item.properties
            .insert("transform".to_string(), json!({ "flipHorizontal": "yes" }));
        let errors = frame_program_for_item(item).expect_err("string flip must fail");
        assert!(errors[0]
            .message
            .contains("`flipHorizontal` must be boolean"));
    }

    #[test]
    fn effect_only_items_are_planned_for_cpu_precomposition() {
        let mut project = sample_project();
        project.timeline.tracks[0].items[0].properties.insert(
            "effects".to_string(),
            json!([{"effectType":"blur.gaussian","enabled":true,"params":{"radius":1.0}}]),
        );
        let groups =
            plan_groups(&project, &FlattenTransitions::plan(&project)).expect("effect groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].top_item_id,
            project.timeline.tracks[0].items[0].id
        );
    }

    #[test]
    fn precompose_effect_sampling_uses_clip_local_seconds_before_transform_and_blend() {
        let properties = BTreeMap::from([
            (
                "effects".to_string(),
                json!([{
                    "effectInstanceId":"exposure-a",
                    "effectType":"color.exposure",
                    "enabled":true,
                    "params":{"ev":0.0}
                }]),
            ),
            (
                "effectParameterKeyframes".to_string(),
                json!({"exposure-a":{"ev":[
                    {"atSeconds":0.0,"value":0.0,"easing":"linear"},
                    {"atSeconds":2.0,"value":2.0,"easing":"linear"}
                ]}}),
            ),
        ]);
        let stack = PreparedEffectStack::from_item_properties_for_duration(&properties, 2.0)
            .expect("animated exposure");
        let source = [40, 80, 120, 255];
        let at_start = stack
            .apply_rgba8_srgb_at_seconds(&source, 1, 1, 0, 0.0)
            .expect("start sample");
        let at_middle = stack
            .apply_rgba8_srgb_at_seconds(&source, 1, 1, 1, 1.0)
            .expect("middle sample");
        let at_end = stack
            .apply_rgba8_srgb_at_seconds(&source, 1, 1, 2, 2.0)
            .expect("end sample");

        assert_eq!(at_start, source);
        assert!(at_middle[0] > at_start[0]);
        assert!(at_end[0] > at_middle[0]);
        assert_eq!(at_end[3], 255);
    }

    #[test]
    fn canonical_pipeline_orders_color_then_lut_then_effect_then_keyframe_then_blend() {
        let invert = CubeLut::parse(
            "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n",
        )
        .expect("invert LUT");
        let source = [40, 80, 120, 255];
        let color_properties =
            BTreeMap::from([("colorGrade".to_string(), json!({"exposure":1.0}))]);
        let pre_lut = PreparedEffectStack::from_item_properties(&color_properties)
            .expect("color stack")
            .before_lut();
        let colored = pre_lut
            .apply_rgba8_srgb(&source, 1, 1, 0)
            .expect("pre-LUT color");
        let lutted =
            invert.apply_rgba8_srgb_strength(colored.as_slice().try_into().expect("pixel"), 1.0);
        let stack = PreparedEffectStack::from_value(Some(&json!([{
            "effectType":"stylize.glow",
            "enabled":true,
            "params":{"intensity":0.75,"radius":0.0,"threshold":0.0,"warmth":0.5}
        }])))
        .expect("effect stack");
        let effected = stack.apply_rgba8_srgb(&lutted, 1, 1, 0).expect("effect");
        let mut program = FrameProgram::identity(1.0);
        program.opacity = NumericCurve::constant(0.5);
        let transformed =
            transform_rgba8_srgb(&effected, 1, 1, &program.sample(0.5).expect("sample"))
                .expect("transform");
        let output = composite_rgba8_srgb(
            [10, 20, 30, 255],
            transformed.as_slice().try_into().expect("pixel"),
            BlendMode::Screen,
        );

        let wrongly_lutted_first = invert.apply_rgba8_srgb_strength(source, 1.0);
        let reordered = pre_lut
            .apply_rgba8_srgb(&wrongly_lutted_first, 1, 1, 0)
            .expect("wrongly reordered color/LUT");
        assert_ne!(
            output.as_slice(),
            reordered.as_slice(),
            "pipeline stages must not commute"
        );
        assert_eq!(transformed[3], 128, "keyframed opacity follows effects");
    }
}
