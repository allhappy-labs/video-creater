//! Reversed audio intermediates.
//!
//! A reversed audio clip reads source `sourceOut - t * speed`. Render
//! preparation decodes the source span the clip and its audio transition
//! handles read, forwards at speed 1 into a 48 kHz PCM16 WAV that keeps the
//! source channel count, then writes the sample frames back to front as a
//! cached WAV (the spike's Strategy B for audio).
//!
//! The WAV's second `p` is source second `stop - p`, so the prepared clip is
//! reverse-free, reads `[stop - sourceOut, stop - sourceIn]` forwards and
//! keeps its speed. It runs before denoising and retiming, which then work
//! on the reversed intermediate like on any other audio clip.

use super::audio_retime::audio_transition_handles;
use super::cache::fingerprint;
use super::{ensure_not_cancelled, project_relative_path, seconds_to_micros, PrecomposeReport};
use crate::project::model::{
    MediaAsset, MediaKind, TimelineItemKind, TimelineSource, VideoProject,
};
use crate::project::reverse::{is_reversed, REVERSE_PROPERTY};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use hound::{WavReader, WavWriter};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const CACHE_VERSION: u32 = 1;
const ALGORITHM: &str = "gstreamer-forward-pcm16-48k-sample-reverse-v1";
const EPSILON: f64 = 0.000_001;
/// How far the decoded intermediate may fall short of the planned span.
const DURATION_TOLERANCE_SECONDS: f64 = 0.05;
const CHUNK_FRAMES: u32 = 48_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fingerprint<'a> {
    schema_version: u32,
    source_sha256: &'a str,
    start_micros: u64,
    stop_micros: u64,
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
    sample_rate: u32,
    channels: u16,
    duration_seconds: f64,
}

/// Replaces every reversed audio clip on an enabled track with a reverse-free
/// clip of a cached, reversed WAV.
pub(super) fn prepare_reversed_audio(
    project_dir: &Path,
    project: &mut VideoProject,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<PrecomposeReport>> {
    let handles = audio_transition_handles(project);
    let tasks = project
        .timeline
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.enabled)
        .flat_map(|(track_index, track)| {
            track
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.kind == TimelineItemKind::AudioClip && is_reversed(item))
                .map(move |(item_index, item)| (track_index, item_index, item.id.clone()))
        })
        .collect::<Vec<_>>();
    let mut reports = Vec::with_capacity(tasks.len());
    for (track_index, item_index, item_id) in tasks {
        ensure_not_cancelled(cancellation)?;
        let item_handles = handles
            .get(&(track_index, item_id))
            .copied()
            .unwrap_or_default();
        reports.push(prepare_task(
            project_dir,
            project,
            (track_index, item_index),
            item_handles,
            cancellation,
        )?);
    }
    Ok(reports)
}

fn prepare_task(
    project_dir: &Path,
    project: &mut VideoProject,
    (track_index, item_index): (usize, usize),
    (head_seconds, tail_seconds): (f64, f64),
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<PrecomposeReport> {
    let item = project.timeline.tracks[track_index].items[item_index].clone();
    let fail = |message: &str| vec![reverse_error(&item.id, message)];
    let TimelineSource::Media { media_id } = &item.source else {
        return Err(fail("Reversed audio requires a media-backed audio clip."));
    };
    let media = project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .cloned()
        .ok_or_else(|| fail("Reversed audio media was not found."))?;
    let speed = number(&item, "speed").unwrap_or(1.0);
    let source_in = number(&item, "sourceIn").unwrap_or(0.0);
    let source_out =
        number(&item, "sourceOut").unwrap_or(source_in + item.duration_seconds * speed);
    if !speed.is_finite()
        || !(0.1..=8.0).contains(&speed)
        || !source_in.is_finite()
        || !source_out.is_finite()
        || source_in < 0.0
        || source_out <= source_in
        || (media.duration_seconds > 0.0 && source_out > media.duration_seconds + EPSILON)
    {
        return Err(fail(
            "Reversed audio source range or playback speed is invalid.",
        ));
    }
    let start = (source_in - tail_seconds * speed).max(0.0);
    let mut stop = source_out + head_seconds * speed;
    if media.duration_seconds > 0.0 {
        stop = stop.min(media.duration_seconds);
    }
    let source_path = project_dir.join(&media.relative_path);
    let io_error =
        |what: &str, error: std::io::Error| fail(&format!("Reversed audio {what} failed: {error}"));
    let source_sha256 = super::cache::sha256_file_cancellable(&source_path, || {
        cancellation.is_some_and(RenderCancellationToken::is_cancelled)
    })
    .map_err(|error| io_error("source read", error))?;
    let fingerprint = fingerprint(&Fingerprint {
        schema_version: CACHE_VERSION,
        source_sha256: &source_sha256,
        start_micros: seconds_to_micros(start)?,
        stop_micros: seconds_to_micros(stop)?,
        algorithm: ALGORITHM,
    })?;
    let cache_dir = project_dir
        .join("cache/audio-reverse/v1/sha256")
        .join(&fingerprint[..2])
        .join(&fingerprint);
    let output_path = cache_dir.join("output.wav");
    let manifest_path = cache_dir.join("manifest.json");
    let cache_hit = read_manifest(&manifest_path, &fingerprint).is_some() && output_path.is_file();
    if !cache_hit {
        fs::create_dir_all(cache_dir.parent().unwrap_or(project_dir))
            .map_err(|error| io_error("cache", error))?;
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(|error| io_error("stale cache", error))?;
        }
        let staging =
            cache_dir.with_file_name(format!(".{fingerprint}.staging-{}", Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|error| io_error("staging", error))?;
        let is_cancelled = || cancellation.is_some_and(RenderCancellationToken::is_cancelled);
        let forward = staging.join("forward.wav");
        let written = decode_forward(
            &source_path,
            &forward,
            (start, stop, media.duration_seconds),
            &is_cancelled,
        )
        .map_err(|message| fail(&message))
        .and_then(|_| {
            reverse_wav(&forward, &staging.join("output.wav"), &is_cancelled)
                .map_err(|message| fail(&message))
        })
        .and_then(|(sample_rate, channels, frames)| {
            let _ = fs::remove_file(&forward);
            let manifest = Manifest {
                schema_version: CACHE_VERSION,
                fingerprint: fingerprint.clone(),
                algorithm: ALGORITHM.to_string(),
                source_sha256: source_sha256.clone(),
                source_start_seconds: start,
                source_stop_seconds: stop,
                sample_rate,
                channels,
                duration_seconds: frames as f64 / f64::from(sample_rate),
            };
            let bytes = serde_json::to_vec_pretty(&manifest)
                .map_err(|error| fail(&format!("Reversed audio manifest failed: {error}")))?;
            fs::write(staging.join("manifest.json"), bytes)
                .map_err(|error| io_error("manifest", error))
        })
        .and_then(|()| {
            fs::rename(&staging, &cache_dir).map_err(|error| io_error("cache publish", error))
        });
        if let Err(errors) = written {
            let _ = fs::remove_dir_all(&staging);
            return Err(errors);
        }
    }
    let manifest = read_manifest(&manifest_path, &fingerprint)
        .ok_or_else(|| fail("Reversed audio manifest is invalid."))?;
    let planned_seconds = stop - start;
    if manifest.duration_seconds + DURATION_TOLERANCE_SECONDS < planned_seconds {
        return Err(fail(&format!(
            "Reversed audio is {:.3} s long; {planned_seconds:.3} s were expected.",
            manifest.duration_seconds
        )));
    }
    let relative_output = project_relative_path(project_dir, &output_path)?;
    let prepared_media_id = format!("audio-reverse-{fingerprint}");
    if !project
        .media
        .iter()
        .any(|candidate| candidate.id == prepared_media_id)
    {
        project.media.push(MediaAsset {
            id: prepared_media_id.clone(),
            name: Some(format!("Reversed {}", item.label)),
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
    let prepared = &mut project.timeline.tracks[track_index].items[item_index];
    prepared.source = TimelineSource::Media {
        media_id: prepared_media_id.clone(),
    };
    prepared.properties.remove(REVERSE_PROPERTY);
    prepared
        .properties
        .insert("sourceIn".to_string(), json!(stop - source_out));
    prepared
        .properties
        .insert("sourceOut".to_string(), json!(stop - source_in));
    Ok(PrecomposeReport {
        stage: "audioReverse".to_string(),
        item_id: item.id.clone(),
        media_id: media.id,
        prepared_media_id,
        fingerprint,
        cache_hit,
        expressions_enabled: false,
        compositor_backend: Some("gstreamer-pcm16-sample-reverse".to_string()),
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
fn decode_forward(
    source: &Path,
    destination: &Path,
    (start, stop, media_duration): (f64, f64, f64),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(u32, u16, u64), String> {
    super::audio_retime_pipeline::retime_to_wav(
        source,
        destination,
        super::audio_retime_pipeline::RetimeRange {
            start,
            stop,
            speed: 1.0,
            media_duration,
        },
        is_cancelled,
    )
}

#[cfg(not(feature = "ges-render"))]
fn decode_forward(
    _source: &Path,
    _destination: &Path,
    _range: (f64, f64, f64),
    _is_cancelled: &dyn Fn() -> bool,
) -> Result<(u32, u16, u64), String> {
    Err("Reversed audio requires the GStreamer render runtime.".to_string())
}

/// Writes the sample frames of the PCM16 WAV `source` back to front, keeping
/// its channel layout. Returns `(sample rate, channels, frames)`.
fn reverse_wav(
    source: &Path,
    destination: &Path,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(u32, u16, u64), String> {
    let mut reader = WavReader::open(source)
        .map_err(|error| format!("Decoded audio could not be opened: {error}"))?;
    let spec = reader.spec();
    if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int {
        return Err("Decoded audio is not 16-bit PCM.".to_string());
    }
    let channels = usize::from(spec.channels.max(1));
    let total = reader.duration();
    let mut writer = WavWriter::create(destination, spec)
        .map_err(|error| format!("Reversed audio WAV could not be created: {error}"))?;
    let mut end = total;
    let mut chunk = Vec::with_capacity(CHUNK_FRAMES as usize * channels);
    while end > 0 {
        if is_cancelled() {
            return Err("Render preparation was cancelled.".to_string());
        }
        let start = end.saturating_sub(CHUNK_FRAMES);
        reader
            .seek(start)
            .map_err(|error| format!("Decoded audio could not be read: {error}"))?;
        chunk.clear();
        for sample in reader
            .samples::<i16>()
            .take((end - start) as usize * channels)
        {
            chunk
                .push(sample.map_err(|error| format!("Decoded audio could not be read: {error}"))?);
        }
        for frame in chunk.chunks_exact(channels).rev() {
            for sample in frame {
                writer
                    .write_sample(*sample)
                    .map_err(|error| format!("Reversed audio WAV write failed: {error}"))?;
            }
        }
        end = start;
    }
    writer
        .finalize()
        .map_err(|error| format!("Reversed audio WAV could not be finished: {error}"))?;
    Ok((spec.sample_rate, spec.channels, u64::from(total)))
}

fn number(item: &crate::project::model::TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(serde_json::Value::as_f64)
}

fn reverse_error(item_id: &str, message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.audioReverse.{item_id}"),
        message,
        "Retry the render; if it keeps failing, check that the clip's media has a readable audio stream.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_wav_keeps_channels_and_mirrors_frames() {
        let dir = tempfile::tempdir().expect("temp dir");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let forward = dir.path().join("forward.wav");
        let frames = CHUNK_FRAMES + 5;
        let mut writer = WavWriter::create(&forward, spec).expect("forward wav");
        for frame in 0..frames {
            // Left counts up, right counts down, so a swapped layout shows.
            writer.write_sample((frame % 30_000) as i16).expect("left");
            writer
                .write_sample(-((frame % 30_000) as i16))
                .expect("right");
        }
        writer.finalize().expect("finish forward");

        let reversed = dir.path().join("reversed.wav");
        assert_eq!(
            reverse_wav(&forward, &reversed, &|| false).expect("reverse"),
            (48_000, 2, u64::from(frames))
        );
        let mut reader = WavReader::open(&reversed).expect("reversed wav");
        assert_eq!(reader.spec(), spec);
        let samples = reader
            .samples::<i16>()
            .map(|sample| sample.expect("sample"))
            .collect::<Vec<_>>();
        assert_eq!(samples.len(), frames as usize * 2);
        for (output, pair) in samples.chunks_exact(2).enumerate() {
            let source = ((frames - 1 - output as u32) % 30_000) as i16;
            assert_eq!(pair, [source, -source], "output frame {output}");
        }
    }
}
