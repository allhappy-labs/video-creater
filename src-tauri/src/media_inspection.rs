use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use hound::{SampleFormat, WavReader};
use serde::{Deserialize, Serialize};

use crate::precompose::compatibility::extract_compatibility_frames;
use crate::precompose::decode_to_wav;
use crate::project::model::{MediaAnalysisMoment, MediaSilenceRange};
use crate::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use crate::render_pipeline::probe::MediaProbe;
use video_creater_compatibility_protocol::CompatibilityFrameRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInspectionReport {
    pub kind: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub image: ImageMetrics,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInspectionReport {
    pub kind: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub media: MediaProbe,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDirectoryInspectionReport {
    pub kind: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub render_directory: Option<RenderDirectoryInspection>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDirectoryInspection {
    pub preview: Option<ImageInspectionReport>,
    pub frame_sequence: Option<FrameSequenceInspection>,
    pub video: Option<VideoInspectionReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSequenceInspection {
    pub directory: PathBuf,
    pub frame_count: usize,
    pub first_frame: PathBuf,
    pub middle_frame: PathBuf,
    pub last_frame: PathBuf,
    pub first_frame_image: ImageInspectionReport,
    pub middle_frame_image: ImageInspectionReport,
    pub last_frame_image: ImageInspectionReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetrics {
    pub width: u32,
    pub height: u32,
    pub total_pixels: u64,
    pub alpha_coverage: f64,
    pub visible_sample_count: u64,
    pub average_saturation: f64,
    pub luminance_mean: f64,
    pub luminance_stddev: f64,
    pub dark_pixel_ratio: f64,
    pub bright_pixel_ratio: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioWindowMetrics {
    #[serde(default)]
    pub start_seconds: f64,
    #[serde(default)]
    pub end_seconds: f64,
    pub mean_volume_db: f64,
    pub max_volume_db: f64,
}

pub fn inspect_image(path: &Path) -> Result<ImageInspectionReport> {
    let image = image::open(path)
        .with_context(|| format!("Could not decode PNG image at {}", path.display()))?
        .to_rgba8();
    let metadata = fs::metadata(path)
        .with_context(|| format!("Could not stat image at {}", path.display()))?;
    let metrics = image_metrics(&image);
    let warnings = image_warnings(&metrics);

    Ok(ImageInspectionReport {
        kind: "image".to_string(),
        path: path.to_path_buf(),
        size_bytes: metadata.len(),
        image: metrics,
        warnings,
    })
}

pub fn inspect_video(path: &Path) -> Result<VideoInspectionReport> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Could not stat video at {}", path.display()))?;
    let probe_path = path
        .canonicalize()
        .with_context(|| format!("Could not resolve video path {}", path.display()))?;
    let mut warnings = Vec::new();
    let (media, _probe_output) = probe_media_with_gstreamer(
        &probe_path,
        Duration::from_secs(30),
        "mediaInspection.video",
    )
    .map_err(|errors| anyhow!("GStreamer media probe failed: {errors:?}"))?;
    if media.video.is_none() {
        warnings.push("missing_video_stream".to_string());
    }

    Ok(VideoInspectionReport {
        kind: "video".to_string(),
        path: path.to_path_buf(),
        size_bytes: metadata.len(),
        media,
        warnings,
    })
}

pub fn derive_media_analysis_moments(
    media_id: &str,
    report: &VideoInspectionReport,
    preview_metrics: Option<&ImageMetrics>,
) -> Vec<MediaAnalysisMoment> {
    derive_media_analysis_moments_with_sampled_frames(media_id, report, preview_metrics, &[])
}

pub fn derive_media_analysis_moments_with_sampled_frames(
    media_id: &str,
    report: &VideoInspectionReport,
    preview_metrics: Option<&ImageMetrics>,
    sampled_frame_metrics: &[ImageMetrics],
) -> Vec<MediaAnalysisMoment> {
    derive_media_analysis_moments_with_sampled_signals(
        media_id,
        report,
        preview_metrics,
        sampled_frame_metrics,
        &[],
    )
}

pub fn derive_media_analysis_moments_with_sampled_signals(
    media_id: &str,
    report: &VideoInspectionReport,
    preview_metrics: Option<&ImageMetrics>,
    sampled_frame_metrics: &[ImageMetrics],
    sampled_audio_metrics: &[AudioWindowMetrics],
) -> Vec<MediaAnalysisMoment> {
    let Some(duration_seconds) = report.media.duration_seconds else {
        return Vec::new();
    };
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Vec::new();
    }

    let fps_score = report
        .media
        .video
        .as_ref()
        .and_then(|video| video.fps)
        .map(|fps| ((fps - 24.0) / 36.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let contrast_score = preview_metrics
        .map(|metrics| (metrics.luminance_stddev / 80.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let saturation_score = preview_metrics
        .map(|metrics| metrics.average_saturation.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let frame_energy = sampled_frame_energy(sampled_frame_metrics);
    let frame_change_score = frame_energy.change_score;
    let frame_contrast_score = frame_energy.contrast_score;
    let frame_saturation_score = frame_energy.saturation_score;
    let audio_energy = sampled_audio_energy(sampled_audio_metrics);
    let audio_loudness_score = audio_energy.loudness_score;
    let audio_change_score = audio_energy.change_score;
    let visual_action_score = if sampled_frame_metrics.len() >= 2 {
        (fps_score * 0.18
            + contrast_score * 0.08
            + saturation_score * 0.04
            + frame_change_score * 0.45
            + frame_contrast_score * 0.15
            + frame_saturation_score * 0.1)
            .clamp(0.0, 1.0)
    } else {
        (fps_score * 0.45 + contrast_score * 0.35 + saturation_score * 0.2).clamp(0.0, 1.0)
    };
    let audio_energy_score = if report.media.audio.is_some() {
        if sampled_audio_metrics.len() >= 2 {
            (0.26
                + contrast_score * 0.08
                + fps_score * 0.08
                + frame_change_score * 0.08
                + audio_loudness_score * 0.28
                + audio_change_score * 0.22)
                .clamp(0.0, 1.0)
        } else {
            (0.52 + contrast_score * 0.16 + fps_score * 0.14 + frame_change_score * 0.12)
                .clamp(0.0, 1.0)
        }
    } else {
        0.0
    };

    if visual_action_score < 0.35 && audio_energy_score < 0.35 {
        return Vec::new();
    }

    let window_seconds = duration_seconds.min(4.0);
    let sampled_frame_detail = if sampled_frame_metrics.len() >= 2 {
        " plus sampled-frame visual change analysis"
    } else {
        ""
    };
    let sampled_audio_detail = if sampled_audio_metrics.len() >= 2 {
        " plus sampled-audio loudness/change analysis"
    } else {
        ""
    };
    let segment_signals = sampled_segment_signals(sampled_frame_metrics, sampled_audio_metrics);
    let transformation_detail = if segment_signals.is_empty() {
        sampled_transformation_detail(frame_change_score, audio_change_score)
    } else {
        ""
    };
    let analysis_detail =
        format!("{sampled_frame_detail}{sampled_audio_detail}{transformation_detail}");

    let mut windows = vec![(
        0.0,
        window_seconds,
        "opening",
        0_usize,
        1.0,
        format!("early probed video cadence and preview image energy{analysis_detail}"),
    )];

    if duration_seconds > 16.0 {
        let middle_in = ((duration_seconds - window_seconds) / 2.0).max(window_seconds);
        windows.push((
            middle_in,
            (middle_in + window_seconds).min(duration_seconds),
            "middle",
            1_usize,
            0.96,
            format!("midpoint probed video cadence and preview image energy{analysis_detail}"),
        ));
    }

    if duration_seconds > 8.0 {
        let closing_in = (duration_seconds - window_seconds).max(0.0);
        windows.push((
            closing_in,
            duration_seconds,
            "closing",
            2_usize,
            0.92,
            format!("late probed video cadence and preview image energy{analysis_detail}"),
        ));
    }

    let mut moments = Vec::new();
    for (source_in, source_out, segment, segment_index, weight, mut detail) in windows {
        if source_out <= source_in
            || moments
                .iter()
                .any(|moment: &MediaAnalysisMoment| (moment.source_in - source_in).abs() < 0.001)
        {
            continue;
        }

        let visual_weight = segment_signals
            .get(segment_index)
            .map(|signal| sampled_segment_score_weight(signal.visual_emphasis_score))
            .unwrap_or(1.0);
        let audio_weight = segment_signals
            .get(segment_index)
            .map(|signal| sampled_segment_score_weight(signal.audio_emphasis_score))
            .unwrap_or(1.0);
        if let Some(signal) = segment_signals.get(segment_index) {
            detail.push_str(sampled_transformation_detail(
                signal.frame_entry_change_score,
                signal.audio_entry_change_score,
            ));
        }

        moments.push(MediaAnalysisMoment {
            media_id: media_id.to_string(),
            source_in,
            source_out,
            visual_action_score: rounded_score(visual_action_score * weight * visual_weight),
            audio_energy_score: rounded_score(audio_energy_score * weight * audio_weight),
            label: format!("media inspection {segment}: {detail}"),
        });
    }

    moments
}

#[derive(Debug, Clone, Copy, Default)]
struct SampledSegmentSignal {
    visual_emphasis_score: f64,
    audio_emphasis_score: f64,
    frame_entry_change_score: f64,
    audio_entry_change_score: f64,
}

fn sampled_segment_signals(
    frame_metrics: &[ImageMetrics],
    audio_metrics: &[AudioWindowMetrics],
) -> Vec<SampledSegmentSignal> {
    let segment_count = frame_metrics.len().max(audio_metrics.len());
    if segment_count < 3 {
        return Vec::new();
    }

    (0..segment_count)
        .map(|index| SampledSegmentSignal {
            visual_emphasis_score: sampled_frame_segment_emphasis(frame_metrics, index),
            audio_emphasis_score: sampled_audio_segment_emphasis(audio_metrics, index),
            frame_entry_change_score: sampled_frame_segment_entry_change(frame_metrics, index),
            audio_entry_change_score: sampled_audio_segment_entry_change(audio_metrics, index),
        })
        .collect()
}

fn sampled_segment_score_weight(score: f64) -> f64 {
    (0.7 + score.clamp(0.0, 1.0) * 0.6).clamp(0.7, 1.3)
}

fn sampled_frame_segment_emphasis(metrics: &[ImageMetrics], index: usize) -> f64 {
    let Some(sample) = metrics.get(index) else {
        return 0.0;
    };
    let contrast = (sample.luminance_stddev / 80.0).clamp(0.0, 1.0);
    let saturation = sample.average_saturation.clamp(0.0, 1.0);
    let bright = sample.bright_pixel_ratio.clamp(0.0, 1.0);
    let visible = (1.0 - sample.dark_pixel_ratio.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let frame_quality =
        (contrast * 0.32 + saturation * 0.28 + bright * 0.24 + visible * 0.16).clamp(0.0, 1.0);
    (sampled_frame_segment_change(metrics, index) * 0.45 + frame_quality * 0.55).clamp(0.0, 1.0)
}

fn sampled_frame_segment_change(metrics: &[ImageMetrics], index: usize) -> f64 {
    if metrics.len() < 2 || index >= metrics.len() {
        return 0.0;
    }
    let previous = index
        .checked_sub(1)
        .and_then(|previous| metrics.get(previous))
        .map(|previous| frame_metric_delta(previous, &metrics[index]))
        .unwrap_or(0.0);
    let next = metrics
        .get(index + 1)
        .map(|next| frame_metric_delta(&metrics[index], next))
        .unwrap_or(0.0);
    previous.max(next)
}

fn sampled_frame_segment_entry_change(metrics: &[ImageMetrics], index: usize) -> f64 {
    if metrics.len() < 2 || index == 0 || index >= metrics.len() {
        return 0.0;
    }
    frame_metric_delta(&metrics[index - 1], &metrics[index])
}

fn sampled_audio_segment_emphasis(metrics: &[AudioWindowMetrics], index: usize) -> f64 {
    let Some(sample) = metrics.get(index) else {
        return 0.0;
    };
    (audio_loudness_score(sample.mean_volume_db) * 0.62
        + sampled_audio_segment_change(metrics, index) * 0.38)
        .clamp(0.0, 1.0)
}

fn sampled_audio_segment_change(metrics: &[AudioWindowMetrics], index: usize) -> f64 {
    if metrics.len() < 2 || index >= metrics.len() {
        return 0.0;
    }
    let previous = index
        .checked_sub(1)
        .and_then(|previous| metrics.get(previous))
        .map(|previous| audio_metric_delta(previous, &metrics[index]))
        .unwrap_or(0.0);
    let next = metrics
        .get(index + 1)
        .map(|next| audio_metric_delta(&metrics[index], next))
        .unwrap_or(0.0);
    previous.max(next)
}

fn sampled_audio_segment_entry_change(metrics: &[AudioWindowMetrics], index: usize) -> f64 {
    if metrics.len() < 2 || index == 0 || index >= metrics.len() {
        return 0.0;
    }
    audio_metric_delta(&metrics[index - 1], &metrics[index])
}

fn sampled_transformation_detail(frame_change_score: f64, audio_change_score: f64) -> &'static str {
    if frame_change_score >= 0.58 && audio_change_score >= 0.58 {
        " plus before after transformation payoff reveal signal"
    } else if frame_change_score >= 0.58 {
        " plus before after transformation visual reveal signal"
    } else if audio_change_score >= 0.58 {
        " plus audio payoff reveal signal"
    } else {
        ""
    }
}

pub fn sample_video_audio_window_metrics(
    path: &Path,
    duration_seconds: Option<f64>,
) -> Vec<AudioWindowMetrics> {
    let Some(duration_seconds) = duration_seconds else {
        return Vec::new();
    };
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Vec::new();
    }

    let decoded = std::env::temp_dir().join(format!(
        "video-creater-media-audio-{}.wav",
        uuid::Uuid::new_v4()
    ));
    if decode_to_wav(path, &decoded, "media-inspection").is_err() {
        return Vec::new();
    }
    let decoded_audio = read_normalized_wav(&decoded);
    let _ = fs::remove_file(&decoded);
    let Ok((samples, sample_rate, channels)) = decoded_audio else {
        return Vec::new();
    };
    let window_seconds = duration_seconds.min(1.5);
    let sample_times = [
        (duration_seconds * 0.15).clamp(0.0, duration_seconds),
        (duration_seconds * 0.50).clamp(0.0, duration_seconds),
        (duration_seconds * 0.85).clamp(0.0, duration_seconds),
    ];
    let mut metrics = Vec::new();
    for sample_time in sample_times {
        let start_time = (sample_time - window_seconds / 2.0).clamp(0.0, duration_seconds);
        let start_sample = (start_time * f64::from(sample_rate)).round() as usize * channels;
        let end_sample =
            ((start_time + window_seconds) * f64::from(sample_rate)).round() as usize * channels;
        if let Some(window) =
            samples.get(start_sample.min(samples.len())..end_sample.min(samples.len()))
        {
            if !window.is_empty() {
                let peak = window
                    .iter()
                    .map(|sample| sample.abs())
                    .fold(0.0_f64, f64::max);
                let rms = (window.iter().map(|sample| sample * sample).sum::<f64>()
                    / window.len() as f64)
                    .sqrt();
                metrics.push(AudioWindowMetrics {
                    start_seconds: round_media_seconds(start_time),
                    end_seconds: round_media_seconds(
                        (start_time + window_seconds).min(duration_seconds),
                    ),
                    mean_volume_db: amplitude_db(rms),
                    max_volume_db: amplitude_db(peak),
                });
            }
        }
    }

    metrics
}

pub fn sample_video_frame_metrics(path: &Path, duration_seconds: Option<f64>) -> Vec<ImageMetrics> {
    let Some(duration_seconds) = duration_seconds else {
        return Vec::new();
    };
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Vec::new();
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let samples_dir = std::env::temp_dir().join(format!(
        "video-creater-media-analysis-{}-{unique}",
        std::process::id()
    ));
    if fs::create_dir_all(&samples_dir).is_err() {
        return Vec::new();
    }

    let sample_times = [
        (duration_seconds * 0.15).clamp(0.0, duration_seconds),
        (duration_seconds * 0.50).clamp(0.0, duration_seconds),
        (duration_seconds * 0.85).clamp(0.0, duration_seconds),
    ];
    let requests = sample_times
        .iter()
        .enumerate()
        .map(|(index, sample_time)| CompatibilityFrameRequest {
            time_seconds: *sample_time,
            output_path: samples_dir
                .join(format!("sample-{index}.png"))
                .to_string_lossy()
                .into_owned(),
        })
        .collect::<Vec<_>>();
    let mut metrics = Vec::new();
    if extract_compatibility_frames(path, &samples_dir, 640, 360, requests.clone()).is_ok() {
        for request in requests {
            if let Ok(report) = inspect_image(Path::new(&request.output_path)) {
                metrics.push(report.image);
            }
        }
    }

    let _ = fs::remove_dir_all(&samples_dir);
    metrics
}

fn read_normalized_wav(path: &Path) -> Result<(Vec<f64>, u32, usize)> {
    let mut reader = WavReader::open(path).map_err(|error| anyhow!(error))?;
    let spec = reader.spec();
    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, bits) if bits <= 16 => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| f64::from(value) / 32_768.0))
            .collect::<Result<Vec<_>, _>>(),
        (SampleFormat::Int, _) => reader
            .samples::<i32>()
            .map(|sample| sample.map(|value| value as f64 / 2_147_483_648.0))
            .collect::<Result<Vec<_>, _>>(),
        (SampleFormat::Float, _) => reader
            .samples::<f32>()
            .map(|sample| sample.map(f64::from))
            .collect::<Result<Vec<_>, _>>(),
    }
    .map_err(|error| anyhow!(error))?;
    Ok((samples, spec.sample_rate, usize::from(spec.channels.max(1))))
}

fn amplitude_db(amplitude: f64) -> f64 {
    if amplitude > 0.0 {
        20.0 * amplitude.log10()
    } else {
        f64::NEG_INFINITY
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SampledFrameEnergy {
    change_score: f64,
    contrast_score: f64,
    saturation_score: f64,
}

fn sampled_frame_energy(metrics: &[ImageMetrics]) -> SampledFrameEnergy {
    if metrics.is_empty() {
        return SampledFrameEnergy::default();
    }

    let contrast_score = metrics
        .iter()
        .map(|metrics| (metrics.luminance_stddev / 80.0).clamp(0.0, 1.0))
        .sum::<f64>()
        / metrics.len() as f64;
    let saturation_score = metrics
        .iter()
        .map(|metrics| metrics.average_saturation.clamp(0.0, 1.0))
        .sum::<f64>()
        / metrics.len() as f64;
    let change_score = if metrics.len() < 2 {
        0.0
    } else {
        metrics
            .windows(2)
            .map(|window| frame_metric_delta(&window[0], &window[1]))
            .sum::<f64>()
            / (metrics.len() - 1) as f64
    };

    SampledFrameEnergy {
        change_score,
        contrast_score,
        saturation_score,
    }
}

fn frame_metric_delta(left: &ImageMetrics, right: &ImageMetrics) -> f64 {
    let luminance_delta =
        ((left.luminance_mean - right.luminance_mean).abs() / 120.0).clamp(0.0, 1.0);
    let contrast_delta =
        ((left.luminance_stddev - right.luminance_stddev).abs() / 80.0).clamp(0.0, 1.0);
    let saturation_delta = (left.average_saturation - right.average_saturation)
        .abs()
        .clamp(0.0, 1.0);
    let dark_delta = (left.dark_pixel_ratio - right.dark_pixel_ratio)
        .abs()
        .clamp(0.0, 1.0);
    let bright_delta = (left.bright_pixel_ratio - right.bright_pixel_ratio)
        .abs()
        .clamp(0.0, 1.0);

    (luminance_delta * 0.38
        + contrast_delta * 0.22
        + saturation_delta * 0.18
        + dark_delta * 0.11
        + bright_delta * 0.11)
        .clamp(0.0, 1.0)
}

pub fn derive_media_silence_ranges(
    media_id: &str,
    sampled_audio_metrics: &[AudioWindowMetrics],
) -> Vec<MediaSilenceRange> {
    let loud_windows = sampled_audio_metrics
        .iter()
        .filter(|metrics| {
            metrics.start_seconds.is_finite()
                && metrics.end_seconds.is_finite()
                && metrics.end_seconds > metrics.start_seconds
                && metrics.mean_volume_db.is_finite()
                && metrics.max_volume_db.is_finite()
                && metrics.mean_volume_db > -35.0
        })
        .collect::<Vec<_>>();
    if loud_windows.is_empty() {
        return Vec::new();
    }

    let speech_floor_db = loud_windows
        .iter()
        .map(|metrics| metrics.mean_volume_db)
        .sum::<f64>()
        / loud_windows.len() as f64;
    let silence_threshold_db = (speech_floor_db - 24.0).min(-38.0);

    sampled_audio_metrics
        .iter()
        .filter(|metrics| {
            metrics.start_seconds.is_finite()
                && metrics.end_seconds.is_finite()
                && metrics.end_seconds > metrics.start_seconds
                && metrics.mean_volume_db.is_finite()
                && metrics.max_volume_db.is_finite()
                && metrics.mean_volume_db <= silence_threshold_db
                && metrics.max_volume_db <= silence_threshold_db + 10.0
                && metrics.end_seconds - metrics.start_seconds >= 1.0
        })
        .map(|metrics| {
            let quiet_margin = (silence_threshold_db - metrics.mean_volume_db).max(0.0);
            MediaSilenceRange {
                media_id: media_id.to_string(),
                source_in: round_media_seconds(metrics.start_seconds),
                source_out: round_media_seconds(metrics.end_seconds),
                confidence: rounded_score((0.72 + quiet_margin / 40.0).clamp(0.0, 0.98)),
                label: "quiet speech-free section from sampled audio analysis".to_string(),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
struct SampledAudioEnergy {
    loudness_score: f64,
    change_score: f64,
}

fn sampled_audio_energy(metrics: &[AudioWindowMetrics]) -> SampledAudioEnergy {
    if metrics.is_empty() {
        return SampledAudioEnergy::default();
    }

    let loudness_score = metrics
        .iter()
        .map(|metrics| audio_loudness_score(metrics.mean_volume_db))
        .sum::<f64>()
        / metrics.len() as f64;
    let change_score = if metrics.len() < 2 {
        0.0
    } else {
        metrics
            .windows(2)
            .map(|window| audio_metric_delta(&window[0], &window[1]))
            .sum::<f64>()
            / (metrics.len() - 1) as f64
    };

    SampledAudioEnergy {
        loudness_score,
        change_score,
    }
}

fn audio_loudness_score(mean_volume_db: f64) -> f64 {
    if !mean_volume_db.is_finite() {
        return 0.0;
    }
    ((mean_volume_db + 55.0) / 35.0).clamp(0.0, 1.0)
}

fn audio_metric_delta(left: &AudioWindowMetrics, right: &AudioWindowMetrics) -> f64 {
    if !left.mean_volume_db.is_finite()
        || !right.mean_volume_db.is_finite()
        || !left.max_volume_db.is_finite()
        || !right.max_volume_db.is_finite()
    {
        return 0.0;
    }

    let mean_delta = ((left.mean_volume_db - right.mean_volume_db).abs() / 28.0).clamp(0.0, 1.0);
    let peak_delta = ((left.max_volume_db - right.max_volume_db).abs() / 24.0).clamp(0.0, 1.0);
    (mean_delta * 0.72 + peak_delta * 0.28).clamp(0.0, 1.0)
}

fn round_media_seconds(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn rounded_score(value: f64) -> f64 {
    let clamped = value.clamp(0.0, 1.0);
    (clamped * 1000.0).round() / 1000.0
}

pub fn inspect_render_directory(path: &Path) -> Result<RenderDirectoryInspectionReport> {
    if !path.is_dir() {
        return Err(anyhow!("{} is not a directory", path.display()));
    }

    let mut warnings = Vec::new();
    let preview = find_first_matching(path, |candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "preview.png")
    })
    .map(|preview_path| inspect_image(&preview_path))
    .transpose()?;

    let frame_sequence = find_frame_directory(path)
        .map(|frames_dir| inspect_frame_sequence(&frames_dir))
        .transpose()?;

    let video = find_first_matching(path, |candidate| {
        candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "mp4" | "mov" | "webm"
                )
            })
    })
    .and_then(|video_path| match inspect_video(&video_path) {
        Ok(report) => Some(report),
        Err(error) => {
            warnings.push(format!("video_probe_failed: {error}"));
            None
        }
    });

    if preview.is_none() {
        warnings.push("missing_preview_png".to_string());
    }
    if frame_sequence.is_none() {
        warnings.push("missing_frame_sequence".to_string());
    }

    Ok(RenderDirectoryInspectionReport {
        kind: "renderDir".to_string(),
        path: path.to_path_buf(),
        size_bytes: directory_size_bytes(path)?,
        render_directory: Some(RenderDirectoryInspection {
            preview,
            frame_sequence,
            video,
        }),
        warnings,
    })
}

fn inspect_frame_sequence(frames_dir: &Path) -> Result<FrameSequenceInspection> {
    let mut frames = fs::read_dir(frames_dir)
        .with_context(|| format!("Could not read frames directory {}", frames_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("frame-") && name.ends_with(".png"))
        })
        .collect::<Vec<_>>();
    frames.sort();

    if frames.is_empty() {
        return Err(anyhow!(
            "Frames directory {} does not contain frame-*.png files",
            frames_dir.display()
        ));
    }

    let first_frame = frames[0].clone();
    let middle_frame = frames[frames.len() / 2].clone();
    let last_frame = frames[frames.len() - 1].clone();

    Ok(FrameSequenceInspection {
        directory: frames_dir.to_path_buf(),
        frame_count: frames.len(),
        first_frame_image: inspect_image(&first_frame)?,
        middle_frame_image: inspect_image(&middle_frame)?,
        last_frame_image: inspect_image(&last_frame)?,
        first_frame,
        middle_frame,
        last_frame,
    })
}

fn image_metrics(image: &image::RgbaImage) -> ImageMetrics {
    let width = image.width();
    let height = image.height();
    let total_pixels = u64::from(width) * u64::from(height);
    let mut covered_pixels = 0_u64;
    let mut visible_sample_count = 0_u64;
    let mut saturation_sum = 0.0;
    let mut luminance_mean = 0.0;
    let mut luminance_m2 = 0.0;
    let mut dark_pixels = 0_u64;
    let mut bright_pixels = 0_u64;

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha >= 32 {
            covered_pixels += 1;
        }
        if alpha < 32 || red.max(green).max(blue) < 48 {
            continue;
        }

        let max_channel = red.max(green).max(blue) as f64;
        let min_channel = red.min(green).min(blue) as f64;
        if max_channel > f64::EPSILON {
            saturation_sum += (max_channel - min_channel) / max_channel;
        }

        let luminance = red as f64 * 0.2126 + green as f64 * 0.7152 + blue as f64 * 0.0722;
        if luminance < 64.0 {
            dark_pixels += 1;
        }
        if luminance > 220.0 {
            bright_pixels += 1;
        }
        visible_sample_count += 1;
        let delta = luminance - luminance_mean;
        luminance_mean += delta / visible_sample_count as f64;
        let delta_after_mean = luminance - luminance_mean;
        luminance_m2 += delta * delta_after_mean;
    }

    let visible = visible_sample_count.max(1) as f64;
    ImageMetrics {
        width,
        height,
        total_pixels,
        alpha_coverage: if total_pixels == 0 {
            0.0
        } else {
            covered_pixels as f64 / total_pixels as f64
        },
        visible_sample_count,
        average_saturation: if visible_sample_count == 0 {
            0.0
        } else {
            saturation_sum / visible
        },
        luminance_mean: if visible_sample_count == 0 {
            0.0
        } else {
            luminance_mean
        },
        luminance_stddev: if visible_sample_count < 2 {
            0.0
        } else {
            (luminance_m2 / visible).sqrt()
        },
        dark_pixel_ratio: if visible_sample_count == 0 {
            0.0
        } else {
            dark_pixels as f64 / visible
        },
        bright_pixel_ratio: if visible_sample_count == 0 {
            0.0
        } else {
            bright_pixels as f64 / visible
        },
    }
}

fn image_warnings(metrics: &ImageMetrics) -> Vec<String> {
    let mut warnings = Vec::new();
    if metrics.visible_sample_count == 0 {
        warnings.push("no_visible_pixels".to_string());
    }
    if metrics.visible_sample_count > 0 && metrics.average_saturation < 0.04 {
        warnings.push("low_saturation".to_string());
    }
    if metrics.visible_sample_count > 0 && metrics.luminance_stddev < 8.0 {
        warnings.push("low_luminance_contrast".to_string());
    }
    warnings
}

fn find_frame_directory(root: &Path) -> Option<PathBuf> {
    if root.join("frames").is_dir() {
        return Some(root.join("frames"));
    }

    find_first_directory_matching(root, |candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "frames")
    })
}

fn find_first_matching<F>(root: &Path, matches: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut files = walk_files(root).ok()?;
    files.sort();
    files.into_iter().find(|path| matches(path))
}

fn find_first_directory_matching<F>(root: &Path, matches: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut directories = walk_directories(root).ok()?;
    directories.sort();
    directories.into_iter().find(|path| matches(path))
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)
            .with_context(|| format!("Could not read directory {}", path.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else {
                files.push(entry_path);
            }
        }
    }
    Ok(files)
}

fn walk_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut directories = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)
            .with_context(|| format!("Could not read directory {}", path.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path.clone());
                directories.push(entry_path);
            }
        }
    }
    Ok(directories)
}

fn directory_size_bytes(root: &Path) -> Result<u64> {
    Ok(walk_files(root)?
        .iter()
        .filter_map(|path| fs::metadata(path).ok().map(|metadata| metadata.len()))
        .sum())
}

#[cfg(test)]
mod tests {
    use super::amplitude_db;

    #[test]
    fn converts_normalized_amplitudes_to_decibels() {
        assert_eq!(amplitude_db(1.0), 0.0);
        assert!((amplitude_db(0.5) + 6.0206).abs() < 0.001);
        assert!(amplitude_db(0.0).is_infinite() && amplitude_db(0.0).is_sign_negative());
    }
}
