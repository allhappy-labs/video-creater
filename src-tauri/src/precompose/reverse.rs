//! Reversed video intermediates.
//!
//! A reversed video clip (`properties.reverse`) reads source
//! `sourceOut - t * speed` at clip-local second `t` (`project::reverse`).
//! Render preparation decodes the source span the clip and its transition
//! handles read forwards, at the output frame rate and clip speed, and
//! encodes the frames with `pngenc` in reverse order into a PNG-in-QuickTime
//! intermediate (the spike's Strategy B,
//! `docs/research/2026-09-16-reverse-intermediate-spike.md`).
//!
//! The intermediate covers `head + duration + tail` timeline seconds and bakes
//! the speed, like LUT and Lottie intermediates: the prepared clip is
//! reverse-free and plays it at speed 1 from `sourceIn = head`. Output frame
//! `k` shows the source frame at `source_seconds_at((k + 1) / fps - head)`, so
//! the frames are exactly those a forward clip over the same span shows, in
//! reverse order. When the span's first frame would start before source 0,
//! the earliest decoded frame is held for those frames.
//!
//! Later preparation (LUT, flattening) runs on the reversed intermediate.

use super::cache::{
    cache_entry_dir, fingerprint, sha256_file, validate_cache_entry, CacheLock, CacheManifest,
    StagingCleanup,
};
use super::{
    ensure_not_cancelled, fps_rational, numeric_property, package_png_frames_as_mov,
    precompose_item_error, project_relative_path, seconds_to_micros, FrameDecodeSpec,
    PngSequenceSpec, PrecomposeReport, PreparationTask, PreparedSourceHandles,
};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use crate::project::reverse::{is_reversed, REVERSE_PROPERTY};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::PipelineResult;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 1;
const PIXEL_CONTRACT: &str = "rgba8-srgb-straight-v1";
const SAMPLING_POLICY: &str = "forward-decode-reversed-output-order-v1";
const ENCODER: &str = "gstreamer-pngenc-compression-6-v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReverseFingerprint<'a> {
    schema_version: u32,
    source_sha256: &'a str,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    source_start_micros: u64,
    source_stop_micros: u64,
    playback_rate_micros: u32,
    frame_count: u32,
    held_frame_count: u32,
    pixel_contract: &'static str,
    sampling_policy: &'static str,
    encoder: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReverseStageManifest<'a> {
    schema_version: u32,
    fingerprint: &'a str,
    frame_count: u32,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    source_sha256: &'a str,
    source_start_seconds: f64,
    source_stop_seconds: f64,
    speed: f64,
    held_frame_count: u32,
    frame_rgba_sha256: Vec<String>,
    frame_png_sha256: Vec<String>,
    pixel_contract: &'static str,
    sampling_policy: &'static str,
    encoder: &'static str,
}

/// The forward decode that fills a reversed intermediate of `frame_count`
/// frames: decoded frame `j` becomes output frame
/// `frame_count - 1 - held_frames - j`, and the last `held_frames` output
/// frames repeat the earliest decoded one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ReversedDecodePlan {
    pub(super) frame_count: u32,
    pub(super) held_frames: u32,
    pub(super) decode_start_seconds: f64,
    pub(super) decode_stop_seconds: f64,
}

/// Plans the decode for a clip reading source window `[_, source_out]` at
/// `speed`, with `head` handle seconds and a span of `span_seconds` at `fps`.
pub(super) fn reversed_decode_plan(
    source_out: f64,
    speed: f64,
    head_seconds: f64,
    span_seconds: f64,
    fps: f64,
) -> Option<ReversedDecodePlan> {
    let frame_count = (span_seconds * fps).ceil().max(1.0) as u32;
    let step = speed / fps;
    let decode_stop_seconds = source_out + head_seconds * speed;
    let first_frame_start = decode_stop_seconds - f64::from(frame_count) * step;
    let held_frames = if first_frame_start < -1e-9 {
        (-first_frame_start / step - 1e-9).ceil() as u32
    } else {
        0
    };
    (held_frames < frame_count).then(|| ReversedDecodePlan {
        frame_count,
        held_frames,
        decode_start_seconds: (first_frame_start + f64::from(held_frames) * step).max(0.0),
        decode_stop_seconds,
    })
}

/// Rejects reversed clips on enabled tracks that preparation cannot reverse:
/// only video clips of video media on video tracks and media-backed audio
/// clips play in reverse.
pub(super) fn reject_unreversible_items(project: &VideoProject) -> PipelineResult<()> {
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }
        for (item_index, item) in track.items.iter().enumerate() {
            if !is_reversed(item) {
                continue;
            }
            let media_kind = match &item.source {
                TimelineSource::Media { media_id } => project
                    .media
                    .iter()
                    .find(|media| media.id == *media_id)
                    .map(|media| &media.kind),
                _ => None,
            };
            let reversible = match item.kind {
                TimelineItemKind::AudioClip => media_kind.is_some(),
                TimelineItemKind::VideoClip => {
                    track.kind == TrackKind::Video && media_kind == Some(&MediaKind::Video)
                }
                _ => false,
            };
            if !reversible {
                return Err(vec![
                    crate::render_pipeline::reverse_guard::unprepared_reversed_item_error(
                        track_index,
                        item_index,
                        item,
                    ),
                ]);
            }
        }
    }
    Ok(())
}

/// Replaces a reversed video clip with a reverse-free clip of a cached,
/// reversed intermediate that covers its transition handles.
pub(super) fn prepare_reversed_video(
    project_dir: &Path,
    project: &mut VideoProject,
    task: &PreparationTask,
    handles: PreparedSourceHandles,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let item = project
        .timeline
        .tracks
        .get(task.track_index)
        .and_then(|track| track.items.get(task.item_index))
        .cloned()
        .ok_or_else(|| vec![precompose_item_error(task, "Reversed clip was not found.")])?;
    let TimelineSource::Media { media_id } = &item.source else {
        return Err(vec![precompose_item_error(
            task,
            "Reversed clips need video media.",
        )]);
    };
    let media = project
        .media
        .iter()
        .find(|media| media.id == *media_id && media.kind == MediaKind::Video)
        .cloned()
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "Reversed video media was not found.",
            )]
        })?;
    let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
    let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
    let source_out = numeric_property(&item.properties, "sourceOut")
        .unwrap_or(source_in + item.duration_seconds * speed);
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || !speed.is_finite()
        || !(0.1..=8.0).contains(&speed)
    {
        return Err(vec![precompose_item_error(
            task,
            "Reversed clip source range or playback speed is invalid.",
        )]);
    }
    let width = media.width.unwrap_or(project.render_settings.width);
    let height = media.height.unwrap_or(project.render_settings.height);
    let fps = project.render_settings.fps;
    let (fps_numerator, fps_denominator) = fps_rational(fps).ok_or_else(|| {
        vec![precompose_item_error(
            task,
            "Reversed clip output frame rate is invalid.",
        )]
    })?;
    let span_seconds = handles.span_seconds(item.duration_seconds);
    let plan = reversed_decode_plan(source_out, speed, handles.head_seconds, span_seconds, fps)
        .ok_or_else(|| {
            vec![precompose_item_error(
                task,
                "Reversed clip handles read before the start of the media.",
            )]
        })?;
    let source_start_micros = seconds_to_micros(plan.decode_start_seconds)?;
    let source_stop_micros = seconds_to_micros(plan.decode_stop_seconds)?;
    if media.duration_seconds > 0.0
        && source_stop_micros > seconds_to_micros(media.duration_seconds)?.saturating_add(1)
    {
        return Err(vec![precompose_item_error(
            task,
            "Reversed clip handles read beyond the end of the media.",
        )]);
    }
    let playback_rate_micros = (speed * 1_000_000.0).round() as u32;
    let source_path = project_dir.join(&media.relative_path);
    let source_sha256 = super::cache::sha256_file_cancellable(&source_path, || {
        cancellation.is_some_and(RenderCancellationToken::is_cancelled)
    })
    .map_err(|error| {
        vec![precompose_item_error(
            task,
            &format!("Reversed clip source could not be read: {error}"),
        )]
    })?;
    let fingerprint = fingerprint(&ReverseFingerprint {
        schema_version: SCHEMA_VERSION,
        source_sha256: &source_sha256,
        width,
        height,
        fps_numerator,
        fps_denominator,
        source_start_micros,
        source_stop_micros,
        playback_rate_micros,
        frame_count: plan.frame_count,
        held_frame_count: plan.held_frames,
        pixel_contract: PIXEL_CONTRACT,
        sampling_policy: SAMPLING_POLICY,
        encoder: ENCODER,
    })?;
    let cache_dir = cache_entry_dir(project_dir, &fingerprint);
    let io_error = |what: &str, error: std::io::Error| {
        vec![precompose_item_error(
            task,
            &format!("Reversed clip {what} failed: {error}"),
        )]
    };
    fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir))
        .map_err(|error| io_error("cache directory creation", error))?;
    // Scales with the frame count: 1080p decode plus PNG encoding measured
    // about 50 ms per frame in the spike.
    let timeout = Duration::from_millis(u64::from(plan.frame_count).saturating_mul(500))
        .max(Duration::from_secs(120));
    let _lock = CacheLock::acquire(project_dir, &fingerprint, Duration::from_secs(30))?;
    let cache_hit = validate_cache_entry(&cache_dir, &fingerprint).is_some();
    if !cache_hit {
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
                .map_err(|error| io_error("stale cache removal", error))?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{fingerprint}.staging-{}", Uuid::new_v4()));
        let mut staging_cleanup = StagingCleanup::new(staging.clone());
        let frames_dir = staging.join("frames");
        fs::create_dir_all(&frames_dir).map_err(|error| io_error("staging", error))?;
        let decoded = plan.frame_count - plan.held_frames;
        let encoded = encode_frames(
            &source_path,
            FrameDecodeSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
                source_start_micros,
                source_stop_micros,
                playback_rate_micros,
                frame_count: decoded,
            },
            &frames_dir,
            decoded - 1,
            timeout,
            cancellation,
        )?;
        // Output order: decoded frame j is output frame `decoded - 1 - j`.
        let mut frame_rgba_sha256 = encoded.rgba_sha256;
        let mut frame_png_sha256 = encoded.png_sha256;
        frame_rgba_sha256.reverse();
        frame_png_sha256.reverse();
        let held_path = frames_dir.join(format!("frame-{:06}.png", decoded - 1));
        for index in decoded..plan.frame_count {
            fs::copy(&held_path, frames_dir.join(format!("frame-{index:06}.png")))
                .map_err(|error| io_error("held frame copy", error))?;
            frame_rgba_sha256.push(frame_rgba_sha256[decoded as usize - 1].clone());
            frame_png_sha256.push(frame_png_sha256[decoded as usize - 1].clone());
        }
        ensure_not_cancelled(cancellation)?;
        let intermediate = staging.join("intermediate.mov");
        package_png_frames_as_mov(
            &frames_dir,
            plan.frame_count,
            PngSequenceSpec {
                width,
                height,
                fps_numerator,
                fps_denominator,
            },
            &intermediate,
            timeout,
        )?;
        ensure_not_cancelled(cancellation)?;
        let stage_manifest = ReverseStageManifest {
            schema_version: 1,
            fingerprint: &fingerprint,
            frame_count: plan.frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
            source_sha256: &source_sha256,
            source_start_seconds: plan.decode_start_seconds,
            source_stop_seconds: plan.decode_stop_seconds,
            speed,
            held_frame_count: plan.held_frames,
            frame_rgba_sha256,
            frame_png_sha256,
            pixel_contract: PIXEL_CONTRACT,
            sampling_policy: SAMPLING_POLICY,
            encoder: ENCODER,
        };
        let cache_manifest = CacheManifest {
            schema_version: 1,
            fingerprint: fingerprint.clone(),
            worker_manifest: "manifest.json".to_string(),
            intermediate: "intermediate.mov".to_string(),
            intermediate_sha256: sha256_file(&intermediate)
                .map_err(|error| io_error("intermediate hashing", error))?,
            frame_count: plan.frame_count,
            width,
            height,
            fps_numerator,
            fps_denominator,
        };
        for (file, value) in [
            ("manifest.json", serde_json::to_vec_pretty(&stage_manifest)),
            (
                "cache-manifest.json",
                serde_json::to_vec_pretty(&cache_manifest),
            ),
        ] {
            let bytes = value.map_err(|error| {
                vec![precompose_item_error(
                    task,
                    &format!("Reversed clip {file} could not be serialized: {error}"),
                )]
            })?;
            fs::write(staging.join(file), bytes).map_err(|error| io_error(file, error))?;
        }
        fs::rename(&staging, &cache_dir).map_err(|error| io_error("cache publish", error))?;
        staging_cleanup.disarm();
    }

    let relative_intermediate =
        project_relative_path(project_dir, &cache_dir.join("intermediate.mov"))?;
    let prepared_media_id = format!("precompose-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|media| media.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Reversed {}", item.label)),
            relative_path: relative_intermediate.clone(),
            kind: MediaKind::Video,
            duration_seconds: span_seconds,
            width: Some(width),
            height: Some(height),
            fps: Some(fps),
            folder_id: media.folder_id.clone(),
        });
    }
    let prepared = &mut project.timeline.tracks[task.track_index].items[task.item_index];
    prepared.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared.properties.remove(REVERSE_PROPERTY);
    prepared
        .properties
        .insert("sourceIn".to_string(), json!(handles.head_seconds));
    prepared.properties.insert(
        "sourceOut".to_string(),
        json!(handles.head_seconds + item.duration_seconds),
    );
    prepared.properties.insert("speed".to_string(), json!(1.0));
    Ok(PrecomposeReport {
        stage: "reverse".to_string(),
        item_id: task.item_id.clone(),
        media_id: media.id,
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: Some("gstreamer-pngenc".to_string()),
        compositor_fallback: None,
        worker_manifest: project_relative_path(project_dir, &cache_dir.join("manifest.json"))?,
        intermediate: relative_intermediate,
    })
}

#[cfg(feature = "ges-render")]
fn encode_frames(
    source: &Path,
    spec: FrameDecodeSpec,
    frames_dir: &Path,
    first_output_index: u32,
    timeout: Duration,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<super::reverse_pipeline::EncodedFrames> {
    super::reverse_pipeline::encode_reversed_frames(
        source,
        spec,
        frames_dir,
        first_output_index,
        timeout,
        &|| cancellation.is_some_and(RenderCancellationToken::is_cancelled),
    )
}

#[cfg(not(feature = "ges-render"))]
struct EncodedFrames {
    rgba_sha256: Vec<String>,
    png_sha256: Vec<String>,
}

#[cfg(not(feature = "ges-render"))]
fn encode_frames(
    _source: &Path,
    _spec: FrameDecodeSpec,
    _frames_dir: &Path,
    _first_output_index: u32,
    _timeout: Duration,
    _cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<EncodedFrames> {
    Err(vec![crate::render_pipeline::error::PipelineError::new(
        crate::render_pipeline::error::PipelineErrorCode::RenderBackendFailed,
        "precompose.reverse",
        "Reversed clips require the GStreamer render runtime.",
        "Render with the GStreamer runtime installed.",
    )])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_plan_reads_the_span_that_ends_at_the_head_handle() {
        // Source 1-3 s at speed 1 with 0.5 s handles at 24 fps.
        let plan = reversed_decode_plan(3.0, 1.0, 0.5, 3.0, 24.0).expect("plan");
        assert_eq!(plan.frame_count, 72);
        assert_eq!(plan.held_frames, 0);
        assert!((plan.decode_start_seconds - 0.5).abs() < 1e-9);
        assert!((plan.decode_stop_seconds - 3.5).abs() < 1e-9);

        // Speed 2 over 1 s with 0.25 s handles reads source 0.5-3.5 s.
        let plan = reversed_decode_plan(3.0, 2.0, 0.25, 1.5, 24.0).expect("plan");
        assert_eq!((plan.frame_count, plan.held_frames), (36, 0));
        assert!((plan.decode_start_seconds - 0.5).abs() < 1e-9);
        assert!((plan.decode_stop_seconds - 3.5).abs() < 1e-9);
    }

    #[test]
    fn decode_plan_holds_frames_that_would_start_before_the_media() {
        // Source 0.25-1.01 s with its whole 0.25 s tail: 1.01 s at 24 fps
        // rounds up to 25 frames, so the last frame would start before 0.
        let plan = reversed_decode_plan(1.01, 1.0, 0.0, 1.01, 24.0).expect("plan");
        assert_eq!((plan.frame_count, plan.held_frames), (25, 1));
        assert!((plan.decode_start_seconds - 0.01).abs() < 1e-9, "{plan:?}");
        assert!((plan.decode_stop_seconds - 1.01).abs() < 1e-9);
        assert!(reversed_decode_plan(0.01, 1.0, 0.0, 1.0, 30.0).is_none());
    }
}
