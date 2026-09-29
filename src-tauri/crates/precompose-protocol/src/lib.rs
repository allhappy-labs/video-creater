//! Versioned messages exchanged with the isolated visual-precompose worker.
//!
//! The parent sends exactly one JSON [`PrecomposeRequest`] on stdin. The worker
//! responds with newline-delimited JSON [`WorkerEvent`] values on stdout. No
//! diagnostic text may be written to stdout because every non-empty line is a
//! protocol record.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const PRECOMPOSE_PROTOCOL_NAME: &str = "video-creater.precompose";
pub const PRECOMPOSE_PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrecomposeRequest {
    pub protocol: String,
    pub schema_version: u32,
    pub request_id: String,
    pub cache_key: String,
    pub operation: PrecomposeOperation,
    pub source: LottieSource,
    pub render: LottieRenderSpec,
    pub inputs: LottieRuntimeInputs,
    pub expressions: ExpressionPolicy,
    pub budgets: WorkerBudgets,
    pub output: WorkerOutput,
}

impl PrecomposeRequest {
    pub fn validate(&self) -> Result<(), RequestValidationError> {
        if self.protocol != PRECOMPOSE_PROTOCOL_NAME {
            return Err(RequestValidationError::new(
                "protocol",
                "unsupportedProtocol",
                format!(
                    "protocol must be {PRECOMPOSE_PROTOCOL_NAME:?}, got {:?}",
                    self.protocol
                ),
            ));
        }
        if self.schema_version != PRECOMPOSE_PROTOCOL_VERSION {
            return Err(RequestValidationError::new(
                "schemaVersion",
                "unsupportedSchemaVersion",
                format!(
                    "schemaVersion must be {PRECOMPOSE_PROTOCOL_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }
        require_non_blank("requestId", &self.request_id)?;
        require_hex_sha256("cacheKey", &self.cache_key)?;
        require_non_blank("source.path", &self.source.path)?;
        require_hex_sha256("source.sha256", &self.source.sha256)?;
        if self
            .source
            .animation_id
            .as_deref()
            .is_some_and(str::is_empty)
        {
            return Err(RequestValidationError::new(
                "source.animationId",
                "blankValue",
                "animationId must be omitted or non-empty",
            ));
        }
        if self.render.width == 0 || self.render.width > 16_384 {
            return Err(RequestValidationError::new(
                "render.width",
                "outOfRange",
                "width must be between 1 and 16384 pixels",
            ));
        }
        if self.render.height == 0 || self.render.height > 16_384 {
            return Err(RequestValidationError::new(
                "render.height",
                "outOfRange",
                "height must be between 1 and 16384 pixels",
            ));
        }
        if self.render.fps.numerator == 0 || self.render.fps.denominator == 0 {
            return Err(RequestValidationError::new(
                "render.fps",
                "invalidFrameRate",
                "frame-rate numerator and denominator must both be non-zero",
            ));
        }
        if self.render.frame_count == 0 {
            return Err(RequestValidationError::new(
                "render.frameCount",
                "outOfRange",
                "frameCount must be at least 1",
            ));
        }
        self.inputs.validate()?;
        if self.render.playback_rate_micros < 100_000
            || self.render.playback_rate_micros > 8_000_000
        {
            return Err(RequestValidationError::new(
                "render.playbackRateMicros",
                "outOfRange",
                "playbackRateMicros must represent a rate from 0.1x through 8x",
            ));
        }
        if !self.expressions.enabled {
            return Err(RequestValidationError::new(
                "expressions.enabled",
                "expressionsRequired",
                "Lottie expression evaluation must be enabled for every bake request",
            ));
        }
        if self.budgets.max_frames == 0
            || self.budgets.max_pixels_per_frame == 0
            || self.budgets.max_source_bytes == 0
            || self.budgets.max_archive_entries == 0
            || self.budgets.max_expanded_archive_bytes == 0
            || self.budgets.max_compression_ratio == 0
            || self.budgets.max_wall_time_ms == 0
            || self.budgets.max_memory_bytes == 0
            || self.budgets.max_output_bytes == 0
        {
            return Err(RequestValidationError::new(
                "budgets",
                "invalidBudget",
                "all worker budgets must be greater than zero",
            ));
        }
        if self.render.frame_count > self.budgets.max_frames {
            return Err(RequestValidationError::new(
                "render.frameCount",
                "frameBudgetExceeded",
                "frameCount exceeds budgets.maxFrames",
            ));
        }
        let pixels = u64::from(self.render.width) * u64::from(self.render.height);
        if pixels > self.budgets.max_pixels_per_frame {
            return Err(RequestValidationError::new(
                "render",
                "pixelBudgetExceeded",
                "render dimensions exceed budgets.maxPixelsPerFrame",
            ));
        }
        require_non_blank("output.stagingDir", &self.output.staging_dir)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieRuntimeInputs {
    pub theme_id: Option<String>,
    pub slots: Vec<LottieSlotInput>,
    pub marker: Option<String>,
    pub segment: Option<LottieSegmentInput>,
    pub state_machine: Option<LottieStateMachineInput>,
}

impl LottieRuntimeInputs {
    fn validate(&self) -> Result<(), RequestValidationError> {
        for (field, value) in [
            ("inputs.themeId", self.theme_id.as_deref()),
            ("inputs.marker", self.marker.as_deref()),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(RequestValidationError::new(
                    field,
                    "blankValue",
                    format!("{field} must be omitted or non-empty"),
                ));
            }
        }
        if self.marker.is_some() && self.segment.is_some() {
            return Err(RequestValidationError::new(
                "inputs",
                "conflictingSelection",
                "marker and segment cannot both be selected",
            ));
        }
        if let Some(segment) = &self.segment {
            if segment.end_frame_micros <= segment.start_frame_micros {
                return Err(RequestValidationError::new(
                    "inputs.segment",
                    "invalidSegment",
                    "segment end must be greater than its start",
                ));
            }
        }
        let mut slot_ids = std::collections::BTreeSet::new();
        for slot in &self.slots {
            require_non_blank("inputs.slots.id", &slot.id)?;
            if !slot_ids.insert(slot.id.as_str()) {
                return Err(RequestValidationError::new(
                    "inputs.slots.id",
                    "duplicateInput",
                    format!("slot {:?} is duplicated", slot.id),
                ));
            }
            if let LottieSlotValue::Color { rgba } = &slot.value {
                if rgba.iter().any(|channel| *channel > 1_000_000) {
                    return Err(RequestValidationError::new(
                        "inputs.slots.value",
                        "outOfRange",
                        "color channels must be between 0 and 1000000",
                    ));
                }
            }
        }
        if let Some(machine) = &self.state_machine {
            require_non_blank("inputs.stateMachine.id", &machine.id)?;
            let mut names = std::collections::BTreeSet::new();
            for input in &machine.inputs {
                require_non_blank("inputs.stateMachine.inputs.name", input.name())?;
                if !names.insert(input.name()) {
                    return Err(RequestValidationError::new(
                        "inputs.stateMachine.inputs.name",
                        "duplicateInput",
                        format!("state input {:?} is duplicated", input.name()),
                    ));
                }
            }
            for event in &machine.events {
                require_non_blank("inputs.stateMachine.events", event)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieSlotInput {
    pub id: String,
    pub value: LottieSlotValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LottieSlotValue {
    Color { rgba: [u32; 4] },
    Scalar { value_micros: i64 },
    Vector { x_micros: i64, y_micros: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieSegmentInput {
    pub start_frame_micros: u64,
    pub end_frame_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieStateMachineInput {
    pub id: String,
    pub inputs: Vec<LottieStateInputValue>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LottieStateInputValue {
    Numeric { name: String, value_micros: i64 },
    String { name: String, value: String },
    Boolean { name: String, value: bool },
}

impl LottieStateInputValue {
    pub fn name(&self) -> &str {
        match self {
            Self::Numeric { name, .. } | Self::String { name, .. } | Self::Boolean { name, .. } => {
                name
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PrecomposeOperation {
    BakeLottieRgba,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LottieSourceFormat {
    LottieJson,
    DotLottie,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieSource {
    pub path: String,
    pub sha256: String,
    pub format: LottieSourceFormat,
    pub animation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LottieRenderSpec {
    pub width: u32,
    pub height: u32,
    pub fps: FrameRate,
    pub first_frame: u64,
    pub frame_count: u32,
    pub source_start_micros: u64,
    pub playback_rate_micros: u32,
    pub looping: bool,
    pub alpha_mode: AlphaMode,
    pub color_space: ColorSpace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AlphaMode {
    Straight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ColorSpace {
    Srgb,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionPolicy {
    /// This field is deliberately required in JSON and validation rejects
    /// `false`. Expressions are part of the supported Lottie contract rather
    /// than an optional, silently degraded rendering mode.
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerBudgets {
    pub max_frames: u32,
    pub max_pixels_per_frame: u64,
    pub max_source_bytes: u64,
    pub max_archive_entries: u32,
    pub max_expanded_archive_bytes: u64,
    pub max_compression_ratio: u32,
    pub max_wall_time_ms: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerOutput {
    pub staging_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WorkerEvent {
    Progress {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        phase: WorkerPhase,
        completed: u32,
        total: u32,
        message: String,
    },
    Completed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        result: PrecomposeResult,
    },
    Failed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        error: WorkerError,
    },
}

impl WorkerEvent {
    pub fn progress(
        request_id: impl Into<String>,
        phase: WorkerPhase,
        completed: u32,
        total: u32,
        message: impl Into<String>,
    ) -> Self {
        Self::Progress {
            protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
            schema_version: PRECOMPOSE_PROTOCOL_VERSION,
            request_id: Some(request_id.into()),
            phase,
            completed,
            total,
            message: message.into(),
        }
    }

    pub fn failed(request_id: Option<String>, error: WorkerError) -> Self {
        Self::Failed {
            protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
            schema_version: PRECOMPOSE_PROTOCOL_VERSION,
            request_id,
            error,
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkerPhase {
    Accepted,
    Loading,
    Rasterizing,
    Checksumming,
    WritingManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrecomposeResult {
    pub manifest_path: String,
    pub frame_count: u32,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerError {
    pub code: WorkerErrorCode,
    pub message: String,
    pub field: Option<String>,
    pub retryable: bool,
}

impl WorkerError {
    pub fn invalid_request(error: RequestValidationError) -> Self {
        Self {
            code: WorkerErrorCode::InvalidRequest,
            message: error.message,
            field: Some(error.field),
            retryable: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkerErrorCode {
    InvalidRequest,
    RendererUnavailable,
    SourceInvalid,
    ExpressionEvaluationFailed,
    BudgetExceeded,
    Io,
    ChecksumMismatch,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestValidationError {
    pub field: String,
    pub code: String,
    pub message: String,
}

impl RequestValidationError {
    fn new(field: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for RequestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}): {}",
            self.field, self.code, self.message
        )
    }
}

impl std::error::Error for RequestValidationError {}

fn require_non_blank(field: &str, value: &str) -> Result<(), RequestValidationError> {
    if value.trim().is_empty() {
        return Err(RequestValidationError::new(
            field,
            "blankValue",
            format!("{field} must not be blank"),
        ));
    }
    Ok(())
}

fn require_hex_sha256(field: &str, value: &str) -> Result<(), RequestValidationError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RequestValidationError::new(
            field,
            "invalidSha256",
            format!("{field} must be a lowercase 64-character SHA-256 hex digest"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> PrecomposeRequest {
        PrecomposeRequest {
            protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
            schema_version: PRECOMPOSE_PROTOCOL_VERSION,
            request_id: "request-1".to_string(),
            cache_key: "a".repeat(64),
            operation: PrecomposeOperation::BakeLottieRgba,
            source: LottieSource {
                path: "/project/media/title.lottie".to_string(),
                sha256: "b".repeat(64),
                format: LottieSourceFormat::DotLottie,
                animation_id: Some("main".to_string()),
            },
            render: LottieRenderSpec {
                width: 1920,
                height: 1080,
                fps: FrameRate {
                    numerator: 30_000,
                    denominator: 1_001,
                },
                first_frame: 0,
                frame_count: 90,
                source_start_micros: 0,
                playback_rate_micros: 1_000_000,
                looping: true,
                alpha_mode: AlphaMode::Straight,
                color_space: ColorSpace::Srgb,
            },
            inputs: LottieRuntimeInputs::default(),
            expressions: ExpressionPolicy { enabled: true },
            budgets: WorkerBudgets {
                max_frames: 300,
                max_pixels_per_frame: 1920 * 1080,
                max_source_bytes: 16 * 1024 * 1024,
                max_archive_entries: 256,
                max_expanded_archive_bytes: 64 * 1024 * 1024,
                max_compression_ratio: 100,
                max_wall_time_ms: 30_000,
                max_memory_bytes: 512 * 1024 * 1024,
                max_output_bytes: 1024 * 1024 * 1024,
            },
            output: WorkerOutput {
                staging_dir: "/project/cache/.staging/request-1".to_string(),
            },
        }
    }

    #[test]
    fn valid_request_round_trips_with_versioned_camel_case_json() {
        let request = valid_request();
        request.validate().expect("valid request");

        let json = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(json["protocol"], PRECOMPOSE_PROTOCOL_NAME);
        assert_eq!(json["schemaVersion"], PRECOMPOSE_PROTOCOL_VERSION);
        assert_eq!(json["operation"], "bakeLottieRgba");
        assert_eq!(json["expressions"]["enabled"], true);

        let decoded: PrecomposeRequest = serde_json::from_value(json).expect("decode request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn expressions_field_is_required_during_deserialization() {
        let mut json = serde_json::to_value(valid_request()).expect("serialize request");
        json.as_object_mut()
            .expect("request object")
            .remove("expressions");

        let error = serde_json::from_value::<PrecomposeRequest>(json)
            .expect_err("missing expression policy must fail");
        assert!(error.to_string().contains("expressions"));
    }

    #[test]
    fn disabled_expressions_are_rejected_without_degraded_fallback() {
        let mut request = valid_request();
        request.expressions.enabled = false;

        let error = request
            .validate()
            .expect_err("disabled expressions must fail validation");
        assert_eq!(error.field, "expressions.enabled");
        assert_eq!(error.code, "expressionsRequired");
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let mut request = valid_request();
        request.schema_version += 1;

        let error = request
            .validate()
            .expect_err("unsupported schema must fail validation");
        assert_eq!(error.field, "schemaVersion");
        assert_eq!(error.code, "unsupportedSchemaVersion");
    }

    #[test]
    fn worker_events_form_independent_ndjson_records() {
        let events = [
            WorkerEvent::progress("request-1", WorkerPhase::Accepted, 0, 90, "accepted"),
            WorkerEvent::failed(
                Some("request-1".to_string()),
                WorkerError {
                    code: WorkerErrorCode::RendererUnavailable,
                    message: "renderer is not linked".to_string(),
                    field: None,
                    retryable: false,
                },
            ),
        ];
        let ndjson = events
            .iter()
            .map(|event| serde_json::to_string(event).expect("serialize event"))
            .collect::<Vec<_>>()
            .join("\n");
        let decoded = ndjson
            .lines()
            .map(|line| serde_json::from_str::<WorkerEvent>(line).expect("decode event"))
            .collect::<Vec<_>>();

        assert_eq!(decoded, events);
        assert!(!decoded[0].is_final());
        assert!(decoded[1].is_final());
    }
}
