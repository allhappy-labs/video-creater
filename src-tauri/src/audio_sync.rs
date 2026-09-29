use crate::precompose::decode_to_wav;
use crate::project::action::{ProjectAction, ProjectActionAudioSync, ProjectActionMove};
use crate::project::model::{
    MediaAsset, TimelineItem, TimelineSource, TimelineTrack, VideoProject,
};
use crate::project::split::resolve_project_relative_path;
use hound::{SampleFormat, WavReader};
use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DEFAULT_HOP_SECONDS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSyncCorrelationResult {
    pub lag_hops: i32,
    pub confidence: f64,
}

pub struct AudioSyncCorrelator;

impl AudioSyncCorrelator {
    pub const MIN_OVERLAP: usize = 16;

    pub fn correlate(
        reference: &[f32],
        target: &[f32],
        max_lag_hops: usize,
    ) -> Option<AudioSyncCorrelationResult> {
        if reference.is_empty() || target.is_empty() || max_lag_hops == 0 {
            return None;
        }

        let max_lag = i32::try_from(max_lag_hops).ok()?;
        let mut best: Option<AudioSyncCorrelationResult> = None;
        for lag in -max_lag..=max_lag {
            let target_start = 0.max(-lag) as usize;
            let target_end = target
                .len()
                .min((reference.len() as i32 - lag).max(0) as usize);
            if target_end <= target_start {
                continue;
            }
            let count = target_end - target_start;
            if count < Self::MIN_OVERLAP {
                continue;
            }

            let reference_start = (target_start as i32 + lag) as usize;
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut sum_xx = 0.0;
            let mut sum_yy = 0.0;
            let mut sum_xy = 0.0;
            for index in 0..count {
                let x = target[target_start + index] as f64;
                let y = reference[reference_start + index] as f64;
                sum_x += x;
                sum_y += y;
                sum_xx += x * x;
                sum_yy += y * y;
                sum_xy += x * y;
            }

            let count = count as f64;
            let covariance = sum_xy - (sum_x * sum_y / count);
            let variance_x = sum_xx - (sum_x * sum_x / count);
            let variance_y = sum_yy - (sum_y * sum_y / count);
            let denominator = (variance_x * variance_y).sqrt();
            if denominator <= f64::EPSILON {
                continue;
            }

            let confidence = (covariance / denominator).max(0.0);
            if best
                .as_ref()
                .map(|current| confidence > current.confidence)
                .unwrap_or(true)
            {
                best = Some(AudioSyncCorrelationResult {
                    lag_hops: lag,
                    confidence,
                });
            }
        }

        best
    }
}

#[derive(Debug, Clone)]
pub struct AudioSyncRequest {
    pub reference_clip_id: String,
    pub target_clip_ids: Vec<String>,
    pub project_dir: Option<PathBuf>,
    pub search_window_seconds: f64,
    pub min_confidence: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSyncClipResult {
    pub clip_id: String,
    pub offset_seconds: f64,
    pub start_seconds: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSyncFailure {
    pub clip_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSyncReport {
    pub reference_clip_id: String,
    pub synced: Vec<AudioSyncClipResult>,
    pub failed: Vec<AudioSyncFailure>,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Error, PartialEq)]
pub enum AudioSyncError {
    #[error("reference clip not found: {0}")]
    ReferenceClipNotFound(String),
    #[error("reference clip has no audio envelope")]
    ReferenceEnvelopeUnavailable,
    #[error("searchWindowSeconds must be > 0")]
    InvalidSearchWindow,
    #[error("minConfidence must be between 0 and 1")]
    InvalidMinConfidence,
}

#[derive(Debug, Clone)]
struct AudioEnvelope {
    samples: Vec<f32>,
    hop_seconds: f64,
}

pub fn build_audio_sync_report(
    project: &VideoProject,
    request: AudioSyncRequest,
) -> Result<AudioSyncReport, AudioSyncError> {
    if !request.search_window_seconds.is_finite() || request.search_window_seconds <= 0.0 {
        return Err(AudioSyncError::InvalidSearchWindow);
    }
    if !request.min_confidence.is_finite()
        || request.min_confidence < 0.0
        || request.min_confidence > 1.0
    {
        return Err(AudioSyncError::InvalidMinConfidence);
    }

    let (reference_track, reference_item) = find_item(project, &request.reference_clip_id)
        .ok_or_else(|| AudioSyncError::ReferenceClipNotFound(request.reference_clip_id.clone()))?;
    let reference_envelope =
        envelope_for_item(project, request.project_dir.as_deref(), reference_item)
            .ok_or(AudioSyncError::ReferenceEnvelopeUnavailable)?;

    let mut moves = Vec::new();
    let mut metadata_actions = Vec::new();
    let mut synced = Vec::new();
    let mut failed = Vec::new();
    for target_id in dedupe_targets(&request.target_clip_ids, &request.reference_clip_id) {
        let Some((target_track, target_item)) = find_item(project, &target_id) else {
            failed.push(AudioSyncFailure {
                clip_id: target_id,
                reason: "Clip not found.".to_string(),
            });
            continue;
        };
        let Some(target_envelope) =
            envelope_for_item(project, request.project_dir.as_deref(), target_item)
        else {
            failed.push(AudioSyncFailure {
                clip_id: target_id,
                reason: "Clip has no audio envelope.".to_string(),
            });
            continue;
        };

        let hop_seconds = reference_envelope
            .hop_seconds
            .min(target_envelope.hop_seconds)
            .max(0.001);
        let reference_samples = resample_envelope(&reference_envelope, hop_seconds);
        let target_samples = resample_envelope(&target_envelope, hop_seconds);
        let max_lag_hops = (request.search_window_seconds / hop_seconds)
            .ceil()
            .max(1.0) as usize;
        let Some(match_result) =
            AudioSyncCorrelator::correlate(&reference_samples, &target_samples, max_lag_hops)
        else {
            failed.push(AudioSyncFailure {
                clip_id: target_id,
                reason: "No overlapping waveform match.".to_string(),
            });
            continue;
        };
        if match_result.confidence < request.min_confidence {
            failed.push(AudioSyncFailure {
                clip_id: target_id,
                reason: "No confident alignment.".to_string(),
            });
            continue;
        }

        let start_seconds = round_millis(
            reference_item.start_seconds + f64::from(match_result.lag_hops) * hop_seconds,
        );
        if start_seconds < 0.0 {
            failed.push(AudioSyncFailure {
                clip_id: target_id,
                reason: "Alignment falls before the timeline start.".to_string(),
            });
            continue;
        }
        let offset_seconds = round_millis(start_seconds - target_item.start_seconds);
        moves.push(ProjectActionMove {
            item_id: target_item.id.clone(),
            target_track_id: target_track.id.clone(),
            start_seconds,
        });
        metadata_actions.push(ProjectAction::UpdateAudioSync {
            item_id: target_item.id.clone(),
            sync: ProjectActionAudioSync {
                reference_clip_id: reference_item.id.clone(),
                offset_seconds,
                confidence: round_confidence(match_result.confidence),
                synced_at_start_seconds: start_seconds,
            },
        });
        synced.push(AudioSyncClipResult {
            clip_id: target_item.id.clone(),
            offset_seconds,
            start_seconds,
            confidence: round_confidence(match_result.confidence),
        });
    }

    let mut actions = if moves.is_empty() {
        Vec::new()
    } else {
        vec![ProjectAction::MoveItems { moves }]
    };
    actions.extend(metadata_actions);

    Ok(AudioSyncReport {
        reference_clip_id: reference_track
            .items
            .iter()
            .find(|item| item.id == request.reference_clip_id)
            .map(|item| item.id.clone())
            .unwrap_or(request.reference_clip_id),
        synced,
        failed,
        actions,
    })
}

fn dedupe_targets(targets: &[String], reference_clip_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    for target in targets {
        if target == reference_clip_id || result.iter().any(|existing| existing == target) {
            continue;
        }
        result.push(target.clone());
    }
    result
}

fn find_item<'a>(
    project: &'a VideoProject,
    item_id: &str,
) -> Option<(&'a TimelineTrack, &'a TimelineItem)> {
    project.timeline.tracks.iter().find_map(|track| {
        track
            .items
            .iter()
            .find(|item| item.id == item_id)
            .map(|item| (track, item))
    })
}

fn envelope_for_item(
    project: &VideoProject,
    project_dir: Option<&Path>,
    item: &TimelineItem,
) -> Option<AudioEnvelope> {
    waveform_peaks_envelope(item).or_else(|| media_envelope(project, project_dir, item))
}

fn waveform_peaks_envelope(item: &TimelineItem) -> Option<AudioEnvelope> {
    let peaks = item.properties.get("waveformPeaks")?.as_array()?;
    let samples = peaks
        .iter()
        .filter_map(|peak| peak.as_f64())
        .filter(|peak| peak.is_finite())
        .map(|peak| peak.clamp(0.0, 1.0) as f32)
        .collect::<Vec<_>>();
    if samples.len() < AudioSyncCorrelator::MIN_OVERLAP || item.duration_seconds <= 0.0 {
        return None;
    }
    Some(AudioEnvelope {
        hop_seconds: item.duration_seconds / samples.len() as f64,
        samples,
    })
}

fn media_envelope(
    project: &VideoProject,
    project_dir: Option<&Path>,
    item: &TimelineItem,
) -> Option<AudioEnvelope> {
    let project_dir = project_dir?;
    let media_id = match &item.source {
        TimelineSource::Media { media_id } => media_id,
        TimelineSource::Generated { .. }
        | TimelineSource::Timeline { .. }
        | TimelineSource::Text { .. } => return None,
    };
    let media = project.media.iter().find(|asset| asset.id == *media_id)?;
    let media_path = resolve_media_path(project_dir, media).ok()?;
    extract_native_audio_envelope(&media_path, source_in_seconds(item), item.duration_seconds).ok()
}

fn resolve_media_path(project_dir: &Path, media: &MediaAsset) -> Result<PathBuf, String> {
    resolve_project_relative_path(project_dir, &media.relative_path)
        .map_err(|error| error.to_string())
}

fn extract_native_audio_envelope(
    media_path: &Path,
    source_in_seconds: f64,
    duration_seconds: f64,
) -> Result<AudioEnvelope, String> {
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err("duration must be positive".to_string());
    }
    let decoded = std::env::temp_dir().join(format!(
        "video-creater-audio-sync-{}.wav",
        uuid::Uuid::new_v4()
    ));
    decode_to_wav(media_path, &decoded, "audio-sync").map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let result = (|| {
        let mut reader = WavReader::open(&decoded).map_err(|error| error.to_string())?;
        let spec = reader.spec();
        let samples = match (spec.sample_format, spec.bits_per_sample) {
            (SampleFormat::Int, bits) if bits <= 16 => reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| f32::from(value) / 32_768.0))
                .collect::<Result<Vec<_>, _>>(),
            (SampleFormat::Int, _) => reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / 2_147_483_648.0))
                .collect::<Result<Vec<_>, _>>(),
            (SampleFormat::Float, _) => reader.samples::<f32>().collect::<Result<Vec<_>, _>>(),
        }
        .map_err(|error| error.to_string())?;
        let start = (source_in_seconds.max(0.0) * f64::from(spec.sample_rate)).round() as usize;
        let end = ((source_in_seconds.max(0.0) + duration_seconds) * f64::from(spec.sample_rate))
            .round()
            .min(samples.len() as f64) as usize;
        let trimmed = samples
            .get(start.min(samples.len())..end.max(start).min(samples.len()))
            .unwrap_or_default();
        pcm_samples_to_envelope(trimmed, spec.sample_rate, DEFAULT_HOP_SECONDS)
    })();
    let _ = std::fs::remove_file(decoded);
    result
}

fn pcm_samples_to_envelope(
    samples: &[f32],
    sample_rate: u32,
    hop_seconds: f64,
) -> Result<AudioEnvelope, String> {
    let hop_samples = ((sample_rate as f64 * hop_seconds).round() as usize).max(1);
    let envelope = samples
        .chunks(hop_samples)
        .map(|chunk| {
            chunk
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max)
        })
        .collect::<Vec<_>>();
    if envelope.len() < AudioSyncCorrelator::MIN_OVERLAP {
        return Err("audio envelope is too short".to_string());
    }
    Ok(AudioEnvelope {
        samples: envelope,
        hop_seconds,
    })
}

fn resample_envelope(envelope: &AudioEnvelope, target_hop_seconds: f64) -> Vec<f32> {
    if (envelope.hop_seconds - target_hop_seconds).abs() < 0.000_001 {
        return envelope.samples.clone();
    }
    let duration_seconds = envelope.samples.len() as f64 * envelope.hop_seconds;
    let output_len = (duration_seconds / target_hop_seconds).round().max(1.0) as usize;
    (0..output_len)
        .map(|index| {
            let time = (index as f64 + 0.5) * target_hop_seconds;
            let source_index = (time / envelope.hop_seconds)
                .floor()
                .clamp(0.0, (envelope.samples.len() - 1) as f64)
                as usize;
            envelope.samples[source_index]
        })
        .collect()
}

fn source_in_seconds(item: &TimelineItem) -> f64 {
    item.properties
        .get("sourceIn")
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0)
}

fn round_millis(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn round_confidence(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
