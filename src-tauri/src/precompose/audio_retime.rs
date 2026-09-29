//! Pitch-preserving retimed audio intermediates.
//!
//! GES 1.24.2 does not accept `scaletempo` as a clip time effect: its `rate`
//! property is read-only (it follows the segment's applied rate), so
//! `ges_base_effect_is_time_effect` is false for `scaletempo rate=…`. Retimed
//! audio clips are therefore prepared before rendering, like denoised audio:
//! a plain pipeline seeks the source at the clip speed through `scaletempo`,
//! which changes tempo without changing pitch, and the result is cached as a
//! 48 kHz PCM16 WAV.
//!
//! The intermediate covers the clip's audio transition handles, so the
//! prepared clip keeps its transitions. It plays at speed 1 from
//! `sourceIn = head` for the clip duration.

use super::{ensure_not_cancelled, project_relative_path, seconds_to_micros, PrecomposeReport};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItemKind, TimelineSource, VideoProject,
};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const CACHE_VERSION: u32 = 1;
const ALGORITHM: &str = "gstreamer-scaletempo-seek-pcm16-48k-v1";
const SPEED_EPSILON: f64 = 0.000_001;
/// How far the decoded intermediate may fall short of the planned span.
const DURATION_TOLERANCE_SECONDS: f64 = 0.05;

#[derive(Debug, Clone)]
struct Task {
    track_index: usize,
    item_index: usize,
    item_id: String,
    media_id: String,
    speed: f64,
    head_seconds: f64,
    tail_seconds: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fingerprint<'a> {
    schema_version: u32,
    source_sha256: &'a str,
    start_micros: u64,
    stop_micros: u64,
    speed_micros: u64,
    algorithm: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    fingerprint: String,
    algorithm: String,
    source_sha256: String,
    source_start_seconds: f64,
    source_stop_seconds: f64,
    speed: f64,
    sample_rate: u32,
    channels: u16,
    duration_seconds: f64,
}

/// Replaces every retimed audio clip with a speed-1 clip of a cached,
/// pitch-preserved intermediate.
pub(super) fn prepare_retimed_audio(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let tasks = collect_tasks(project)?;
    let mut reports = Vec::with_capacity(tasks.len());
    for task in tasks {
        ensure_not_cancelled(cancellation)?;
        reports.push(prepare_task(project_dir, project, &task, cancellation)?);
    }
    Ok(reports)
}

/// Audio transition handles, in timeline seconds, keyed by `(track index,
/// item id)`: `(head, tail)`, half of each transition on its side.
pub(super) fn audio_transition_handles(
    project: &VideoProject,
) -> BTreeMap<(usize, String), (f64, f64)> {
    let mut handles = BTreeMap::<(usize, String), (f64, f64)>::new();
    for transition in crate::render_pipeline::transition_plan::plan_clip_transitions(project) {
        if !transition.audio {
            continue;
        }
        let half = transition.duration_seconds / 2.0;
        handles
            .entry((transition.track_index, transition.left_item_id))
            .or_default()
            .1 = half;
        handles
            .entry((transition.track_index, transition.right_item_id))
            .or_default()
            .0 = half;
    }
    handles
}

fn collect_tasks(project: &VideoProject) -> PipelineResult<Vec<Task>> {
    let handles = audio_transition_handles(project);
    let mut tasks = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }
        for (item_index, item) in track.items.iter().enumerate() {
            if item.kind != TimelineItemKind::AudioClip {
                continue;
            }
            let speed = number(item, "speed").unwrap_or(1.0);
            if (speed - 1.0).abs() < SPEED_EPSILON {
                continue;
            }
            if !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
                return Err(vec![retime_error(
                    &item.id,
                    "Audio clip speed must be between 0.1 and 8.",
                )]);
            }
            let TimelineSource::Media { media_id } = &item.source else {
                return Err(vec![retime_error(
                    &item.id,
                    "Retimed audio requires a media-backed audio clip.",
                )]);
            };
            let (head_seconds, tail_seconds) = handles
                .get(&(track_index, item.id.clone()))
                .copied()
                .unwrap_or_default();
            tasks.push(Task {
                track_index,
                item_index,
                item_id: item.id.clone(),
                media_id: media_id.clone(),
                speed,
                head_seconds,
                tail_seconds,
            });
        }
    }
    Ok(tasks)
}

fn prepare_task(
    project_dir: &Path,
    project: &mut VideoProject,
    task: &Task,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let item = project.timeline.tracks[task.track_index].items[task.item_index].clone();
    let media = project
        .media
        .iter()
        .find(|media| media.id == task.media_id)
        .cloned()
        .ok_or_else(|| {
            vec![retime_error(
                &task.item_id,
                "Retimed audio media was not found.",
            )]
        })?;
    let source_in = number(&item, "sourceIn").unwrap_or(0.0);
    let source_out =
        number(&item, "sourceOut").unwrap_or(source_in + item.duration_seconds * task.speed);
    if !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || (media.duration_seconds > 0.0 && source_out > media.duration_seconds + SPEED_EPSILON)
    {
        return Err(vec![retime_error(
            &task.item_id,
            "Retimed audio source range is outside its media.",
        )]);
    }
    let start = (source_in - task.head_seconds * task.speed).max(0.0);
    let mut stop = source_out + task.tail_seconds * task.speed;
    if media.duration_seconds > 0.0 {
        stop = stop.min(media.duration_seconds);
    }
    let head_seconds = (source_in - start) / task.speed;
    let source_path = project_dir.join(&media.relative_path);
    let source_sha256 = super::cache::sha256_file(&source_path)
        .map_err(|error| io_error(&task.item_id, "source read", error))?;
    let fingerprint = super::cache::fingerprint(&Fingerprint {
        schema_version: CACHE_VERSION,
        source_sha256: &source_sha256,
        start_micros: seconds_to_micros(start)?,
        stop_micros: seconds_to_micros(stop)?,
        speed_micros: seconds_to_micros(task.speed)?,
        algorithm: ALGORITHM,
    })?;
    let cache_dir = project_dir
        .join("cache/audio-retime/v1/sha256")
        .join(&fingerprint[..2])
        .join(&fingerprint);
    let output_path = cache_dir.join("output.wav");
    let manifest_path = cache_dir.join("manifest.json");
    let cache_hit = read_manifest(&manifest_path, &fingerprint).is_some() && output_path.is_file();
    if !cache_hit {
        let parent = cache_dir.parent().unwrap_or(project_dir);
        fs::create_dir_all(parent).map_err(|error| io_error(&task.item_id, "cache", error))?;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
                .map_err(|error| io_error(&task.item_id, "stale cache", error))?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{fingerprint}.staging-{}", Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|error| io_error(&task.item_id, "staging", error))?;
        let staged_output = staging.join("output.wav");
        let result = render_retimed_wav(
            &source_path,
            &staged_output,
            (start, stop, media.duration_seconds),
            task,
            cancellation,
        );
        let written = result.and_then(|(sample_rate, channels, frames)| {
            let manifest = Manifest {
                schema_version: CACHE_VERSION,
                fingerprint: fingerprint.clone(),
                algorithm: ALGORITHM.to_string(),
                source_sha256: source_sha256.clone(),
                source_start_seconds: start,
                source_stop_seconds: stop,
                speed: task.speed,
                sample_rate,
                channels,
                duration_seconds: frames as f64 / f64::from(sample_rate),
            };
            let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
                vec![retime_error(
                    &task.item_id,
                    &format!("Retimed audio manifest failed: {error}"),
                )]
            })?;
            fs::write(staging.join("manifest.json"), bytes)
                .map_err(|error| io_error(&task.item_id, "manifest", error))
        });
        if let Err(errors) = written {
            let _ = fs::remove_dir_all(&staging);
            return Err(errors);
        }
        fs::rename(&staging, &cache_dir)
            .map_err(|error| io_error(&task.item_id, "cache publish", error))?;
    }
    let manifest = read_manifest(&manifest_path, &fingerprint).ok_or_else(|| {
        vec![retime_error(
            &task.item_id,
            "Retimed audio manifest is invalid.",
        )]
    })?;
    let planned_seconds = (stop - start) / task.speed;
    if manifest.duration_seconds + DURATION_TOLERANCE_SECONDS < planned_seconds {
        return Err(vec![retime_error(
            &task.item_id,
            &format!(
                "Retimed audio is {:.3} s long; {planned_seconds:.3} s were expected.",
                manifest.duration_seconds
            ),
        )]);
    }
    let relative_output = project_relative_path(project_dir, &output_path)?;
    let prepared_media_id = format!("audio-retime-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|candidate| candidate.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Retimed {}", item.label)),
            relative_path: relative_output.clone(),
            kind: MediaKind::Audio,
            // Sample rounding may leave the file a frame short of the plan.
            duration_seconds: manifest.duration_seconds.max(planned_seconds),
            width: None,
            height: None,
            fps: None,
            folder_id: media.folder_id.clone(),
        });
    }
    let prepared = &mut project.timeline.tracks[task.track_index].items[task.item_index];
    prepared.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared.properties.remove("speed");
    prepared
        .properties
        .insert("sourceIn".to_string(), json!(head_seconds));
    prepared.properties.insert(
        "sourceOut".to_string(),
        json!(head_seconds + item.duration_seconds),
    );
    prepared.properties.insert(
        "audioRetimePreparation".to_string(),
        json!({ "status": "completed", "algorithm": ALGORITHM, "speed": task.speed, "fingerprint": fingerprint, "artifact": relative_output }),
    );
    Ok(PrecomposeReport {
        stage: "audioRetime".to_string(),
        item_id: task.item_id.clone(),
        media_id: task.media_id.clone(),
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: Some("gstreamer-scaletempo".to_string()),
        compositor_fallback: None,
        worker_manifest: project_relative_path(project_dir, &manifest_path)?,
        intermediate: relative_output,
    })
}

fn read_manifest(path: &Path, fingerprint: &str) -> Option<Manifest> {
    let manifest: Manifest = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (manifest.schema_version == CACHE_VERSION
        && manifest.fingerprint == fingerprint
        && manifest.algorithm == ALGORITHM)
        .then_some(manifest)
}

#[cfg(feature = "ges-render")]
fn render_retimed_wav(
    source: &Path,
    destination: &Path,
    (start, stop, media_duration): (f64, f64, f64),
    task: &Task,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<(u32, u16, u64)> {
    super::audio_retime_pipeline::retime_to_wav(
        source,
        destination,
        super::audio_retime_pipeline::RetimeRange {
            start,
            stop,
            speed: task.speed,
            media_duration,
        },
        &|| cancellation.is_some_and(RenderCancellationToken::is_cancelled),
    )
    .map_err(|message| vec![retime_error(&task.item_id, &message)])
}

#[cfg(not(feature = "ges-render"))]
fn render_retimed_wav(
    _source: &Path,
    _destination: &Path,
    _range: (f64, f64, f64),
    task: &Task,
    _cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<(u32, u16, u64)> {
    Err(vec![retime_error(
        &task.item_id,
        "Retimed audio requires the GStreamer render runtime.",
    )])
}

fn number(item: &crate::project::model::TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(serde_json::Value::as_f64)
}

fn io_error(item_id: &str, what: &str, error: std::io::Error) -> Vec<PipelineError> {
    vec![retime_error(
        item_id,
        &format!("Retimed audio {what} failed: {error}"),
    )]
}

fn retime_error(item_id: &str, message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.audioRetime.{item_id}"),
        message,
        "Retry the render; if it keeps failing, check that the clip's media has a readable audio stream.",
    )
}
