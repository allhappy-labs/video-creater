use crate::project::model::{GeneratedAsset, MediaAsset, VideoProject};
use serde_json::Value;
use thiserror::Error;

const FLOAT_TOLERANCE: f64 = 0.001;

#[derive(Debug, Error, PartialEq)]
pub enum GenerationCapabilityError {
    #[error("generation model capability {field} is invalid: {reason}")]
    InvalidCapability { field: &'static str, reason: String },
    #[error(
        "generation asset model {asset_provider}:{asset_model} does not match resolved model {resolved_provider}:{resolved_model}"
    )]
    ModelMismatch {
        asset_provider: String,
        asset_model: String,
        resolved_provider: String,
        resolved_model: String,
    },
    #[error("duration {requested}s is not supported by this model")]
    UnsupportedDuration { requested: f64 },
    #[error("duration {requested}s is below the model minimum of {minimum}s")]
    DurationBelowMinimum { requested: f64, minimum: f64 },
    #[error("duration {requested}s exceeds the model maximum of {maximum}s")]
    DurationAboveMaximum { requested: f64, maximum: f64 },
    #[error("{field} value is not supported by this model: {value}")]
    UnsupportedValue { field: &'static str, value: String },
    #[error("numImages must be between 1 and {maximum}; got {requested}")]
    ImageCountOutOfRange { requested: u32, maximum: u64 },
    #[error("prompt has {actual} characters; this model requires at least {minimum}")]
    PromptTooShort { actual: usize, minimum: u64 },
    #[error("generation model requires {field}")]
    MissingRequiredReference { field: &'static str },
    #[error("generation model does not support {field}")]
    UnsupportedReference { field: &'static str },
    #[error("generation model accepts at most {maximum} {kind} references; got {actual}")]
    ReferenceLimitExceeded {
        kind: &'static str,
        actual: usize,
        maximum: u64,
    },
    #[error("generation model accepts at most {maximum} references total; got {actual}")]
    TotalReferenceLimitExceeded { actual: usize, maximum: u64 },
    #[error("frame references and typed media references cannot be used together")]
    ConflictingReferenceModes,
    #[error(
        "combined {kind} reference duration {actual_seconds}s exceeds the model maximum of {maximum_seconds}s"
    )]
    CombinedReferenceDurationExceeded {
        kind: &'static str,
        actual_seconds: f64,
        maximum_seconds: f64,
    },
    #[error("generation reference media was not found: {0}")]
    MissingReferenceMedia(String),
    #[error("generation reference media has an invalid duration: {0}")]
    InvalidReferenceMediaDuration(String),
    #[error("source trim range is invalid: {0}")]
    InvalidSourceTrimRange(String),
    #[error(
        "source video duration {actual_seconds}s exceeds the model maximum of {maximum_seconds}s"
    )]
    SourceVideoDurationExceeded {
        actual_seconds: f64,
        maximum_seconds: f64,
    },
}

pub fn validate_generation_asset_capabilities(
    project: &VideoProject,
    asset: &GeneratedAsset,
    resolved_model: &Value,
) -> Result<(), GenerationCapabilityError> {
    validate_model_identity(asset, resolved_model)?;
    let source_video_edit = asset.references.source_video_media_ref.is_some()
        && (optional_bool(resolved_model, "requiresSourceVideo")?.unwrap_or(false)
            || optional_bool(resolved_model, "supportsSourceVideo")?.unwrap_or(false));
    if !source_video_edit {
        validate_duration(asset, resolved_model)?;
        validate_string_setting(
            asset.settings.resolution.as_deref(),
            resolved_model,
            "resolutions",
            "resolution",
        )?;
        validate_string_setting(
            asset.settings.aspect_ratio.as_deref(),
            resolved_model,
            "aspectRatios",
            "aspectRatio",
        )?;
        validate_string_setting(
            asset.settings.quality.as_deref(),
            resolved_model,
            "qualities",
            "quality",
        )?;
    }
    validate_image_count(asset, resolved_model)?;
    validate_prompt(asset, resolved_model)?;
    validate_references(project, asset, resolved_model)?;
    validate_source_trim_ranges(project, asset, resolved_model)?;
    Ok(())
}

fn validate_model_identity(
    asset: &GeneratedAsset,
    model: &Value,
) -> Result<(), GenerationCapabilityError> {
    let resolved_provider = required_top_level_string(model, "provider")?;
    let resolved_model = required_top_level_string(model, "id")?;
    if asset.model.provider.trim() != resolved_provider || asset.model.id.trim() != resolved_model {
        return Err(GenerationCapabilityError::ModelMismatch {
            asset_provider: asset.model.provider.clone(),
            asset_model: asset.model.id.clone(),
            resolved_provider: resolved_provider.to_string(),
            resolved_model: resolved_model.to_string(),
        });
    }
    Ok(())
}

fn validate_duration(
    asset: &GeneratedAsset,
    model: &Value,
) -> Result<(), GenerationCapabilityError> {
    let allowed = optional_number_list(model, "durations")?;
    let minimum = optional_nonnegative_number(model, "minSeconds")?;
    let maximum = optional_nonnegative_number(model, "maxSeconds")?;
    if minimum.zip(maximum).is_some_and(|(min, max)| min > max) {
        return Err(invalid_capability(
            "minSeconds",
            "must not exceed maxSeconds",
        ));
    }
    let Some(requested) = asset.settings.duration_seconds else {
        return Ok(());
    };
    if !requested.is_finite() || requested <= 0.0 {
        return Err(GenerationCapabilityError::UnsupportedDuration { requested });
    }
    if let Some(allowed) = allowed {
        if !allowed
            .iter()
            .any(|candidate| (candidate - requested).abs() <= FLOAT_TOLERANCE)
        {
            return Err(GenerationCapabilityError::UnsupportedDuration { requested });
        }
    }
    if let Some(minimum) = minimum {
        if requested + FLOAT_TOLERANCE < minimum {
            return Err(GenerationCapabilityError::DurationBelowMinimum { requested, minimum });
        }
    }
    if let Some(maximum) = maximum {
        if requested - FLOAT_TOLERANCE > maximum {
            return Err(GenerationCapabilityError::DurationAboveMaximum { requested, maximum });
        }
    }
    Ok(())
}

fn validate_string_setting(
    requested: Option<&str>,
    model: &Value,
    capability_field: &'static str,
    error_field: &'static str,
) -> Result<(), GenerationCapabilityError> {
    let allowed = optional_string_list(model, capability_field)?;
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if allowed.is_some_and(|allowed| !allowed.contains(&requested)) {
        return Err(GenerationCapabilityError::UnsupportedValue {
            field: error_field,
            value: requested.to_string(),
        });
    }
    Ok(())
}

fn validate_image_count(
    asset: &GeneratedAsset,
    model: &Value,
) -> Result<(), GenerationCapabilityError> {
    let model_kind = optional_top_level_string(model, "kind")?;
    let maximum =
        optional_u64(model, "maxImages")?.or_else(|| (model_kind == Some("image")).then_some(1));
    if maximum == Some(0) {
        return Err(invalid_capability("maxImages", "must be greater than zero"));
    }
    let Some(requested) = asset.settings.num_images else {
        return Ok(());
    };
    if requested == 0 || maximum.is_some_and(|maximum| u64::from(requested) > maximum) {
        return Err(GenerationCapabilityError::ImageCountOutOfRange {
            requested,
            maximum: maximum.unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

fn validate_prompt(asset: &GeneratedAsset, model: &Value) -> Result<(), GenerationCapabilityError> {
    let Some(minimum) = optional_u64(model, "minPromptLength")? else {
        return Ok(());
    };
    let actual = asset.prompt.trim().chars().count();
    if actual < minimum as usize {
        return Err(GenerationCapabilityError::PromptTooShort { actual, minimum });
    }
    Ok(())
}

fn validate_references(
    project: &VideoProject,
    asset: &GeneratedAsset,
    model: &Value,
) -> Result<(), GenerationCapabilityError> {
    let references = &asset.references;
    let source_video = nonblank(references.source_video_media_ref.as_deref());
    let first_frame = nonblank(references.first_frame_media_id.as_deref());
    let last_frame = nonblank(references.last_frame_media_id.as_deref());
    let image_count = references.reference_image_media_refs.len();
    let video_count = references.reference_video_media_refs.len();
    let audio_count = references.reference_audio_media_refs.len();
    let total_count = image_count + video_count + audio_count;
    let supports_first_frame = optional_bool(model, "supportsFirstFrame")?.unwrap_or(false);
    let supports_last_frame = optional_bool(model, "supportsLastFrame")?.unwrap_or(false);

    if optional_bool(model, "requiresSourceVideo")?.unwrap_or(false) && source_video.is_none() {
        return Err(GenerationCapabilityError::MissingRequiredReference {
            field: "sourceVideoMediaRef",
        });
    }
    if optional_bool(model, "supportsSourceVideo")? == Some(false) && source_video.is_some() {
        return Err(GenerationCapabilityError::UnsupportedReference {
            field: "sourceVideoMediaRef",
        });
    }
    if optional_bool(model, "requiresReferenceImage")?.unwrap_or(false) && image_count == 0 {
        return Err(GenerationCapabilityError::MissingRequiredReference {
            field: "referenceImageMediaRefs",
        });
    }
    if first_frame.is_some() && !supports_first_frame {
        return Err(GenerationCapabilityError::UnsupportedReference {
            field: "firstFrameMediaId",
        });
    }
    if last_frame.is_some() && !supports_last_frame {
        return Err(GenerationCapabilityError::UnsupportedReference {
            field: "lastFrameMediaId",
        });
    }
    if optional_bool(model, "supportsReferences")? == Some(false) && total_count > 0 {
        return Err(GenerationCapabilityError::UnsupportedReference {
            field: "typed media references",
        });
    }
    if optional_bool(model, "supportsImageReference")? == Some(false) && image_count > 0 {
        return Err(GenerationCapabilityError::UnsupportedReference {
            field: "referenceImageMediaRefs",
        });
    }

    validate_reference_count(model, "maxReferenceImages", "image", image_count)?;
    validate_reference_count(model, "maxReferenceVideos", "video", video_count)?;
    validate_reference_count(model, "maxReferenceAudios", "audio", audio_count)?;
    if let Some(maximum) = optional_u64(model, "maxTotalReferences")? {
        if total_count as u64 > maximum {
            return Err(GenerationCapabilityError::TotalReferenceLimitExceeded {
                actual: total_count,
                maximum,
            });
        }
    }
    if optional_bool(model, "framesAndReferencesExclusive")?.unwrap_or(false)
        && (first_frame.is_some() || last_frame.is_some())
        && total_count > 0
    {
        return Err(GenerationCapabilityError::ConflictingReferenceModes);
    }

    validate_combined_reference_duration(
        project,
        &references.reference_video_media_refs,
        model,
        "maxCombinedVideoRefSeconds",
        "video",
    )?;
    validate_combined_reference_duration(
        project,
        &references.reference_audio_media_refs,
        model,
        "maxCombinedAudioRefSeconds",
        "audio",
    )?;
    Ok(())
}

fn validate_reference_count(
    model: &Value,
    field: &'static str,
    kind: &'static str,
    actual: usize,
) -> Result<(), GenerationCapabilityError> {
    if let Some(maximum) = optional_u64(model, field)? {
        if actual as u64 > maximum {
            return Err(GenerationCapabilityError::ReferenceLimitExceeded {
                kind,
                actual,
                maximum,
            });
        }
    }
    Ok(())
}

fn validate_combined_reference_duration(
    project: &VideoProject,
    media_ids: &[String],
    model: &Value,
    field: &'static str,
    kind: &'static str,
) -> Result<(), GenerationCapabilityError> {
    let Some(maximum_seconds) = optional_nonnegative_number(model, field)? else {
        return Ok(());
    };
    let mut actual_seconds = 0.0;
    for media_id in media_ids {
        let media = media_by_id(project, media_id)?;
        if !media.duration_seconds.is_finite() || media.duration_seconds < 0.0 {
            return Err(GenerationCapabilityError::InvalidReferenceMediaDuration(
                media_id.clone(),
            ));
        }
        actual_seconds += media.duration_seconds;
    }
    if actual_seconds - FLOAT_TOLERANCE > maximum_seconds {
        return Err(
            GenerationCapabilityError::CombinedReferenceDurationExceeded {
                kind,
                actual_seconds,
                maximum_seconds,
            },
        );
    }
    Ok(())
}

fn validate_source_trim_ranges(
    project: &VideoProject,
    asset: &GeneratedAsset,
    model: &Value,
) -> Result<(), GenerationCapabilityError> {
    let settings = &asset.settings;
    let seconds = range_pair(
        settings.video_source_start_seconds,
        settings.video_source_end_seconds,
        "videoSourceStartSeconds/videoSourceEndSeconds",
    )?;
    let frames = frame_range_pair(
        settings.video_source_start_frame,
        settings.video_source_end_frame,
    )?;
    if seconds.is_some() && frames.is_some() {
        return Err(GenerationCapabilityError::InvalidSourceTrimRange(
            "frame and second ranges cannot both be set".to_string(),
        ));
    }

    let source_media = nonblank(asset.references.source_video_media_ref.as_deref())
        .map(|media_id| media_by_id(project, media_id))
        .transpose()?;
    if let (Some((_, end)), Some(media)) = (seconds, source_media) {
        if media.duration_seconds.is_finite()
            && media.duration_seconds >= 0.0
            && end - media.duration_seconds > FLOAT_TOLERANCE
        {
            return Err(GenerationCapabilityError::InvalidSourceTrimRange(format!(
                "end {end}s exceeds source duration {}s",
                media.duration_seconds
            )));
        }
    }

    let Some(maximum_seconds) = optional_nonnegative_number(model, "maxSourceVideoSeconds")? else {
        return Ok(());
    };
    let actual_seconds = if let Some((start, end)) = seconds {
        Some(end - start)
    } else if let (Some((start, end)), Some(media)) = (frames, source_media) {
        media
            .fps
            .filter(|fps| fps.is_finite() && *fps > 0.0)
            .map(|fps| (end - start) as f64 / fps)
    } else {
        source_media.map(|media| media.duration_seconds)
    };
    if let Some(actual_seconds) = actual_seconds {
        if !actual_seconds.is_finite() || actual_seconds < 0.0 {
            return Err(GenerationCapabilityError::InvalidSourceTrimRange(
                "source duration must be finite and nonnegative".to_string(),
            ));
        }
        if actual_seconds - FLOAT_TOLERANCE > maximum_seconds {
            return Err(GenerationCapabilityError::SourceVideoDurationExceeded {
                actual_seconds,
                maximum_seconds,
            });
        }
    }
    Ok(())
}

fn range_pair(
    start: Option<f64>,
    end: Option<f64>,
    field: &str,
) -> Result<Option<(f64, f64)>, GenerationCapabilityError> {
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end))
            if start.is_finite() && end.is_finite() && start >= 0.0 && end > start =>
        {
            Ok(Some((start, end)))
        }
        _ => Err(GenerationCapabilityError::InvalidSourceTrimRange(format!(
            "{field} must be a complete finite increasing range"
        ))),
    }
}

fn frame_range_pair(
    start: Option<u64>,
    end: Option<u64>,
) -> Result<Option<(u64, u64)>, GenerationCapabilityError> {
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if end > start => Ok(Some((start, end))),
        _ => Err(GenerationCapabilityError::InvalidSourceTrimRange(
            "videoSourceStartFrame/videoSourceEndFrame must be a complete increasing range"
                .to_string(),
        )),
    }
}

fn media_by_id<'a>(
    project: &'a VideoProject,
    media_id: &str,
) -> Result<&'a MediaAsset, GenerationCapabilityError> {
    project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .ok_or_else(|| GenerationCapabilityError::MissingReferenceMedia(media_id.to_string()))
}

fn required_top_level_string<'a>(
    model: &'a Value,
    field: &'static str,
) -> Result<&'a str, GenerationCapabilityError> {
    optional_top_level_string(model, field)?.ok_or_else(|| invalid_capability(field, "is required"))
}

fn optional_top_level_string<'a>(
    model: &'a Value,
    field: &'static str,
) -> Result<Option<&'a str>, GenerationCapabilityError> {
    match model.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.trim())),
        Some(_) => Err(invalid_capability(field, "must be a nonblank string")),
    }
}

fn optional_bool(
    model: &Value,
    field: &'static str,
) -> Result<Option<bool>, GenerationCapabilityError> {
    match capability_value(model, field)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(invalid_capability(field, "must be a boolean")),
    }
}

fn optional_u64(
    model: &Value,
    field: &'static str,
) -> Result<Option<u64>, GenerationCapabilityError> {
    match capability_value(model, field)? {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid_capability(field, "must be a nonnegative integer")),
    }
}

fn optional_nonnegative_number(
    model: &Value,
    field: &'static str,
) -> Result<Option<f64>, GenerationCapabilityError> {
    match capability_value(model, field)? {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(Some)
            .ok_or_else(|| invalid_capability(field, "must be a finite nonnegative number")),
    }
}

fn optional_number_list(
    model: &Value,
    field: &'static str,
) -> Result<Option<Vec<f64>>, GenerationCapabilityError> {
    let Some(value) = capability_value(model, field)? else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_array()
        .ok_or_else(|| invalid_capability(field, "must be an array of positive numbers"))?;
    if values.is_empty() {
        return Err(invalid_capability(field, "must not be empty"));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && *value > 0.0)
                .ok_or_else(|| invalid_capability(field, "contains an invalid number"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn optional_string_list<'a>(
    model: &'a Value,
    field: &'static str,
) -> Result<Option<Vec<&'a str>>, GenerationCapabilityError> {
    let Some(value) = capability_value(model, field)? else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_array()
        .ok_or_else(|| invalid_capability(field, "must be an array of nonblank strings"))?;
    if values.is_empty() {
        return Err(invalid_capability(field, "must not be empty"));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid_capability(field, "contains an invalid string"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn capability_value<'a>(
    model: &'a Value,
    field: &'static str,
) -> Result<Option<&'a Value>, GenerationCapabilityError> {
    if let Some(ui_capabilities) = model.get("uiCapabilities") {
        let ui_capabilities = ui_capabilities.as_object().ok_or_else(|| {
            invalid_capability("uiCapabilities", "must be an object when present")
        })?;
        if let Some(value) = ui_capabilities.get(field) {
            return Ok(Some(value));
        }
    }
    Ok(model.get(field))
}

fn nonblank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn invalid_capability(field: &'static str, reason: impl Into<String>) -> GenerationCapabilityError {
    GenerationCapabilityError::InvalidCapability {
        field,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::{
        GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus, GenerationModel,
        MediaAsset, MediaKind,
    };
    use serde_json::json;

    fn project() -> VideoProject {
        let mut project = VideoProject::new_empty(
            "project-1".to_string(),
            "Capability tests".to_string(),
            "2026-07-12T00:00:00Z".to_string(),
        );
        project.media = vec![
            media("image-1", MediaKind::Image, 0.0, None),
            media("image-2", MediaKind::Image, 0.0, None),
            media("video-1", MediaKind::Video, 8.0, Some(24.0)),
            media("video-2", MediaKind::Video, 9.0, Some(30.0)),
            media("audio-1", MediaKind::Audio, 7.0, None),
            media("audio-2", MediaKind::Audio, 10.0, None),
        ];
        project
    }

    fn media(id: &str, kind: MediaKind, duration_seconds: f64, fps: Option<f64>) -> MediaAsset {
        MediaAsset {
            id: id.to_string(),
            name: None,
            relative_path: format!("media/{id}"),
            kind,
            duration_seconds,
            width: None,
            height: None,
            fps,
            folder_id: None,
        }
    }

    fn asset() -> GeneratedAsset {
        GeneratedAsset {
            schema_version: 1,
            id: "generated-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Queued,
            name: None,
            target_folder_id: None,
            placement_intent: None,
            prompt: "A cinematic sunrise".to_string(),
            model: GenerationModel {
                provider: "provider".to_string(),
                id: "model".to_string(),
            },
            references: GeneratedAssetReferences::default(),
            settings: GeneratedAssetSettings::default(),
            outputs: Vec::new(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        }
    }

    fn model() -> Value {
        json!({
            "provider": "provider",
            "id": "model",
            "kind": "video"
        })
    }

    #[test]
    fn accepts_a_valid_bounded_request() {
        let mut asset = asset();
        asset.settings.duration_seconds = Some(5.0);
        asset.settings.resolution = Some("720p".to_string());
        asset.settings.aspect_ratio = Some("16:9".to_string());
        asset.settings.num_images = Some(1);
        asset.settings.quality = Some("high".to_string());
        asset.references.source_video_media_ref = Some("video-1".to_string());
        asset.references.first_frame_media_id = Some("image-1".to_string());
        asset.references.reference_image_media_refs = vec!["image-2".to_string()];
        let model = json!({
            "provider": "provider",
            "id": "model",
            "kind": "image",
            "durations": [5, 10],
            "minSeconds": 3,
            "maxSeconds": 10,
            "resolutions": ["720p", "1080p"],
            "aspectRatios": ["16:9", "9:16"],
            "qualities": ["high"],
            "maxImages": 2,
            "minPromptLength": 5,
            "requiresSourceVideo": true,
            "supportsSourceVideo": true,
            "supportsFirstFrame": true,
            "maxReferenceImages": 2,
            "maxTotalReferences": 2,
            "maxSourceVideoSeconds": 10
        });
        assert_eq!(
            validate_generation_asset_capabilities(&project(), &asset, &model),
            Ok(())
        );
    }

    #[test]
    fn rejects_model_identity_and_malformed_capabilities() {
        let cases = [
            (
                json!({"provider":"other","id":"model"}),
                "does not match resolved model",
            ),
            (
                json!({"provider":"provider","id":"model","durations":"5"}),
                "durations is invalid",
            ),
            (
                json!({"provider":"provider","id":"model","supportsFirstFrame":"yes"}),
                "supportsFirstFrame is invalid",
            ),
            (
                json!({"provider":"provider","id":"model","minSeconds":10,"maxSeconds":5}),
                "minSeconds is invalid",
            ),
        ];
        for (model, expected) in cases {
            let error = validate_generation_asset_capabilities(&project(), &asset(), &model)
                .expect_err("case must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn enforces_duration_bounds_table() {
        let cases = [
            (
                7.0,
                json!({"provider":"provider","id":"model","durations":[5,10]}),
                "not supported",
            ),
            (
                2.0,
                json!({"provider":"provider","id":"model","minSeconds":3}),
                "below the model minimum",
            ),
            (
                11.0,
                json!({"provider":"provider","id":"model","maxSeconds":10}),
                "exceeds the model maximum",
            ),
        ];
        for (duration, model, expected) in cases {
            let mut asset = asset();
            asset.settings.duration_seconds = Some(duration);
            let error = validate_generation_asset_capabilities(&project(), &asset, &model)
                .expect_err("duration must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn nested_ui_capabilities_are_enforced_and_preferred() {
        let mut asset = asset();
        asset.settings.duration_seconds = Some(7.0);
        asset.settings.resolution = Some("1080p".to_string());
        let nested_only = json!({
            "provider":"provider",
            "id":"model",
            "kind":"video",
            "uiCapabilities": {
                "durations":[5],
                "resolutions":["720p"]
            }
        });
        let error = validate_generation_asset_capabilities(&project(), &asset, &nested_only)
            .expect_err("nested duration must be enforced");
        assert!(matches!(
            error,
            GenerationCapabilityError::UnsupportedDuration { requested: 7.0 }
        ));

        let nested_preferred = json!({
            "provider":"provider",
            "id":"model",
            "kind":"video",
            "durations":[7],
            "uiCapabilities": { "durations":[5] }
        });
        let error = validate_generation_asset_capabilities(&project(), &asset, &nested_preferred)
            .expect_err("nested capability must override top-level capability");
        assert!(matches!(
            error,
            GenerationCapabilityError::UnsupportedDuration { requested: 7.0 }
        ));
    }

    #[test]
    fn enforces_enumerated_settings_and_image_count() {
        let mut asset = asset();
        type AssetMutationCase = (&'static str, Box<dyn Fn(&mut GeneratedAsset)>);
        let cases: Vec<AssetMutationCase> = vec![
            (
                "resolution",
                Box::new(|asset| asset.settings.resolution = Some("4k".to_string())),
            ),
            (
                "aspectRatio",
                Box::new(|asset| asset.settings.aspect_ratio = Some("1:1".to_string())),
            ),
            (
                "quality",
                Box::new(|asset| asset.settings.quality = Some("draft".to_string())),
            ),
        ];
        let bounded = json!({
            "provider":"provider",
            "id":"model",
            "resolutions":["720p"],
            "aspectRatios":["16:9"],
            "qualities":["high"]
        });
        for (field, mutate) in cases {
            asset.settings = GeneratedAssetSettings::default();
            mutate(&mut asset);
            let error = validate_generation_asset_capabilities(&project(), &asset, &bounded)
                .expect_err("enumerated setting must fail");
            assert!(error.to_string().contains(field), "{error}");
        }

        asset.settings = GeneratedAssetSettings {
            num_images: Some(3),
            ..GeneratedAssetSettings::default()
        };
        let error = validate_generation_asset_capabilities(
            &project(),
            &asset,
            &json!({"provider":"provider","id":"model","kind":"image","maxImages":2}),
        )
        .expect_err("image count must fail");
        assert!(matches!(
            error,
            GenerationCapabilityError::ImageCountOutOfRange { .. }
        ));
    }

    #[test]
    fn enforces_prompt_and_required_or_unsupported_references() {
        let mut asset = asset();
        asset.prompt = "short".to_string();
        let error = validate_generation_asset_capabilities(
            &project(),
            &asset,
            &json!({"provider":"provider","id":"model","minPromptLength":6}),
        )
        .expect_err("prompt must fail");
        assert!(matches!(
            error,
            GenerationCapabilityError::PromptTooShort { .. }
        ));

        asset.prompt = "long enough".to_string();
        for (model, expected_field) in [
            (
                json!({"provider":"provider","id":"model","requiresSourceVideo":true}),
                "sourceVideoMediaRef",
            ),
            (
                json!({"provider":"provider","id":"model","requiresReferenceImage":true}),
                "referenceImageMediaRefs",
            ),
        ] {
            let error = validate_generation_asset_capabilities(&project(), &asset, &model)
                .expect_err("required reference must fail");
            assert!(error.to_string().contains(expected_field), "{error}");
        }

        asset.references.first_frame_media_id = Some("image-1".to_string());
        let error = validate_generation_asset_capabilities(&project(), &asset, &model())
            .expect_err("unsupported frame must fail");
        assert!(matches!(
            error,
            GenerationCapabilityError::UnsupportedReference {
                field: "firstFrameMediaId"
            }
        ));
    }

    #[test]
    fn enforces_reference_counts_total_and_exclusivity() {
        let mut asset = asset();
        asset.references.reference_image_media_refs =
            vec!["image-1".to_string(), "image-2".to_string()];
        asset.references.reference_video_media_refs = vec!["video-1".to_string()];
        let cases = [
            (
                json!({"provider":"provider","id":"model","maxReferenceImages":1}),
                "at most 1 image",
            ),
            (
                json!({"provider":"provider","id":"model","maxTotalReferences":2}),
                "at most 2 references total",
            ),
        ];
        for (model, expected) in cases {
            let error = validate_generation_asset_capabilities(&project(), &asset, &model)
                .expect_err("reference limit must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }

        asset.references.reference_image_media_refs = vec!["image-1".to_string()];
        asset.references.reference_video_media_refs.clear();
        asset.references.first_frame_media_id = Some("image-2".to_string());
        let error = validate_generation_asset_capabilities(
            &project(),
            &asset,
            &json!({
                "provider":"provider",
                "id":"model",
                "supportsFirstFrame":true,
                "framesAndReferencesExclusive":true
            }),
        )
        .expect_err("exclusive modes must fail");
        assert_eq!(error, GenerationCapabilityError::ConflictingReferenceModes);
    }

    #[test]
    fn enforces_combined_reference_durations() {
        let mut asset = asset();
        asset.references.reference_video_media_refs =
            vec!["video-1".to_string(), "video-2".to_string()];
        asset.references.reference_audio_media_refs =
            vec!["audio-1".to_string(), "audio-2".to_string()];
        for (field, maximum, expected_kind) in [
            ("maxCombinedVideoRefSeconds", 16, "video"),
            ("maxCombinedAudioRefSeconds", 16, "audio"),
        ] {
            let model = json!({
                "provider":"provider",
                "id":"model",
                (field): maximum
            });
            let error = validate_generation_asset_capabilities(&project(), &asset, &model)
                .expect_err("combined duration must fail");
            assert!(matches!(
                error,
                GenerationCapabilityError::CombinedReferenceDurationExceeded { kind, .. }
                    if kind == expected_kind
            ));
        }
    }

    #[test]
    fn validates_source_ranges_and_effective_source_duration() {
        let bounded = json!({
            "provider":"provider",
            "id":"model",
            "requiresSourceVideo":true,
            "maxSourceVideoSeconds":5
        });
        let cases = [
            (Some(2.0), None, "complete finite increasing range"),
            (Some(4.0), Some(2.0), "complete finite increasing range"),
            (Some(2.0), Some(7.5), "exceeds the model maximum"),
            (Some(2.0), Some(12.0), "exceeds source duration"),
        ];
        for (start, end, expected) in cases {
            let mut asset = asset();
            asset.references.source_video_media_ref = Some("video-1".to_string());
            asset.settings.video_source_start_seconds = start;
            asset.settings.video_source_end_seconds = end;
            let error = validate_generation_asset_capabilities(&project(), &asset, &bounded)
                .expect_err("range must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }

        let mut valid = asset();
        valid.references.source_video_media_ref = Some("video-1".to_string());
        valid.settings.video_source_start_seconds = Some(2.0);
        valid.settings.video_source_end_seconds = Some(7.0);
        assert_eq!(
            validate_generation_asset_capabilities(&project(), &valid, &bounded),
            Ok(())
        );
    }
}
