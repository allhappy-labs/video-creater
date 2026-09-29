use serde::{Deserialize, Serialize};
use std::path::Path;

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub duration_seconds: Option<f64>,
    pub size_bytes: Option<u64>,
    pub video: Option<VideoProbe>,
    pub audio: Option<AudioProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoProbe {
    pub codec_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioProbe {
    pub codec_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedMedia {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_required: bool,
    pub audio_required: bool,
    pub non_empty_required: bool,
    pub expected_duration_seconds: Option<f64>,
    pub duration_tolerance_seconds: Option<f64>,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

pub fn validate_rendered_media(
    artifact_path: &Path,
    probe: &MediaProbe,
    expected: &ExpectedMedia,
) -> PipelineResult<()> {
    let mut errors = Vec::new();
    let artifact_path = artifact_path.display().to_string();

    if let Some(expected_container) = expected.container.as_deref() {
        let actual_container = normalized_container(
            probe.container_name.as_deref(),
            Path::new(&artifact_path)
                .extension()
                .and_then(|value| value.to_str()),
        );
        let expected_container = normalized_container_name(expected_container);
        if actual_container.as_deref() != Some(expected_container.as_str()) {
            errors.push(
                validation_error(
                    "format.container",
                    "Rendered media container does not match the selected export profile.",
                    "Render with the container selected by the export profile.",
                    &artifact_path,
                )
                .with_detail(
                    "actual",
                    actual_container.unwrap_or_else(|| "unknown".to_string()),
                )
                .with_detail("expected", expected_container),
            );
        }
    }

    if expected.video_required && probe.video.is_none() {
        errors.push(validation_error(
            "streams.video",
            "Rendered media does not contain a video stream.",
            "Render a video stream or mark video as optional for this artifact.",
            &artifact_path,
        ));
    }

    if expected.audio_required && probe.audio.is_none() {
        errors.push(validation_error(
            "streams.audio",
            "Rendered media does not contain an audio stream.",
            "Render an audio stream or mark audio as optional for this artifact.",
            &artifact_path,
        ));
    }

    if expected.non_empty_required && probe.size_bytes.unwrap_or(0) == 0 {
        errors.push(validation_error(
            "format.size",
            "Rendered media artifact is empty.",
            "Re-run the render and inspect backend logs for write failures.",
            &artifact_path,
        ));
    }

    if let Some(expected_duration) = expected.expected_duration_seconds {
        let tolerance = expected.duration_tolerance_seconds.unwrap_or(0.05).max(0.0);
        match probe.duration_seconds {
            Some(actual_duration)
                if actual_duration.is_finite()
                    && expected_duration.is_finite()
                    && (actual_duration - expected_duration).abs() <= tolerance => {}
            Some(actual_duration) => {
                errors.push(
                    validation_error(
                        "format.duration",
                        "Rendered media duration does not match the expected edit duration.",
                        "Render the selected EDL duration; inspect trim/concat and overlay timing.",
                        &artifact_path,
                    )
                    .with_detail("actual", format!("{actual_duration:.3}"))
                    .with_detail("expected", format!("{expected_duration:.3}"))
                    .with_detail("tolerance", format!("{tolerance:.3}")),
                );
            }
            None => {
                errors.push(
                    validation_error(
                        "format.duration",
                        "Rendered media duration is missing from probe output.",
                        "Run media discovery with format duration enabled or regenerate the artifact.",
                        &artifact_path,
                    )
                    .with_detail("expected", format!("{expected_duration:.3}"))
                    .with_detail("tolerance", format!("{tolerance:.3}")),
                );
            }
        }
    }

    if let Some(video) = &probe.video {
        if let Some(expected_codec) = expected.video_codec.as_deref() {
            let actual_codec = video.codec_name.as_deref().map(normalized_video_codec_name);
            if actual_codec.as_deref() != Some(normalized_video_codec_name(expected_codec).as_str())
            {
                errors.push(
                    validation_error(
                        "streams.video.codec",
                        "Rendered video codec does not match the selected export profile.",
                        "Render with the video codec selected by the export profile.",
                        &artifact_path,
                    )
                    .with_detail(
                        "actual",
                        actual_codec.unwrap_or_else(|| "unknown".to_string()),
                    )
                    .with_detail("expected", normalized_video_codec_name(expected_codec)),
                );
            }
        }
        if let (Some(expected_width), Some(expected_height)) = (expected.width, expected.height) {
            if video.width != Some(expected_width) || video.height != Some(expected_height) {
                errors.push(
                    validation_error(
                        "streams.video.dimensions",
                        "Rendered video dimensions do not match the expected output size.",
                        "Render with the expected output width and height.",
                        &artifact_path,
                    )
                    .with_detail(
                        "actual",
                        format!(
                            "{}x{}",
                            video
                                .width
                                .map(|width| width.to_string())
                                .unwrap_or_else(|| "unknown".to_string()),
                            video
                                .height
                                .map(|height| height.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        ),
                    )
                    .with_detail(
                        "expected",
                        format!("{}x{}", expected_width, expected_height),
                    ),
                );
            }
        } else {
            if let Some(expected_width) = expected.width {
                if video.width != Some(expected_width) {
                    errors.push(
                        validation_error(
                            "streams.video.width",
                            "Rendered video width does not match the expected output width.",
                            "Render with the expected output width.",
                            &artifact_path,
                        )
                        .with_detail(
                            "actual",
                            video
                                .width
                                .map(|width| width.to_string())
                                .unwrap_or_else(|| "unknown".to_string()),
                        )
                        .with_detail("expected", expected_width.to_string()),
                    );
                }
            }

            if let Some(expected_height) = expected.height {
                if video.height != Some(expected_height) {
                    errors.push(
                        validation_error(
                            "streams.video.height",
                            "Rendered video height does not match the expected output height.",
                            "Render with the expected output height.",
                            &artifact_path,
                        )
                        .with_detail(
                            "actual",
                            video
                                .height
                                .map(|height| height.to_string())
                                .unwrap_or_else(|| "unknown".to_string()),
                        )
                        .with_detail("expected", expected_height.to_string()),
                    );
                }
            }
        }
    }

    if let (Some(audio), Some(expected_codec)) = (&probe.audio, expected.audio_codec.as_deref()) {
        let actual_codec = audio.codec_name.as_deref().map(normalized_audio_codec_name);
        if actual_codec.as_deref() != Some(normalized_audio_codec_name(expected_codec).as_str()) {
            errors.push(
                validation_error(
                    "streams.audio.codec",
                    "Rendered audio codec does not match the selected export profile.",
                    "Render with the audio codec selected by the export profile.",
                    &artifact_path,
                )
                .with_detail(
                    "actual",
                    actual_codec.unwrap_or_else(|| "unknown".to_string()),
                )
                .with_detail("expected", normalized_audio_codec_name(expected_codec)),
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn normalized_container(value: Option<&str>, extension: Option<&str>) -> Option<String> {
    let value = value?;
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("webm") || normalized.contains("matroska") {
        return Some("webm".to_string());
    }
    if normalized.contains("quicktime") || normalized.contains("video/quicktime") {
        return Some(
            if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("mp4")) {
                "mp4"
            } else {
                "mov"
            }
            .to_string(),
        );
    }
    if normalized.contains("mp4") || normalized.contains("mov") {
        return Some(
            if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("mov")) {
                "mov"
            } else {
                "mp4"
            }
            .to_string(),
        );
    }
    Some(normalized_container_name(value))
}

fn normalized_container_name(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "quicktime" | "video/quicktime" => "mov".to_string(),
        "matroska" | "video/webm" => "webm".to_string(),
        value => value.to_string(),
    }
}

pub(crate) fn normalized_video_codec_name(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if value.contains("h264") || value == "avc1" || value.contains("x-h264") {
        "h264".to_string()
    } else if value.contains("hevc") || value.contains("h265") || value == "hev1" || value == "hvc1"
    {
        "hevc".to_string()
    } else if value.contains("prores") {
        "prores".to_string()
    } else if value.contains("vp9") {
        "vp9".to_string()
    } else if value.contains("vp8") {
        "vp8".to_string()
    } else {
        value
    }
}

pub(crate) fn normalized_audio_codec_name(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if value.contains("aac") || value == "mp4a" {
        "aac".to_string()
    } else if value.contains("opus") {
        "opus".to_string()
    } else if value.contains("pcm") || value.contains("x-raw") || value == "lpcm" {
        "pcm".to_string()
    } else {
        value
    }
}

fn validation_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
    artifact_path: &str,
) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderProbeValidationFailed,
        path,
        message,
        fix,
    )
    .with_detail("path", artifact_path.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalized_container;

    #[test]
    fn quicktime_family_container_uses_the_canonical_artifact_extension() {
        assert_eq!(
            normalized_container(Some("video/quicktime"), Some("mp4")),
            Some("mp4".to_string())
        );
        assert_eq!(
            normalized_container(Some("video/quicktime"), Some("mov")),
            Some("mov".to_string())
        );
        assert_eq!(
            normalized_container(Some("video/quicktime"), None),
            Some("mov".to_string())
        );
    }
}
