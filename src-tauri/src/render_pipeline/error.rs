use crate::graphics::error::{ActionableError, GraphicsErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PipelineErrorCode {
    GraphicsSchemaUnsupported,
    GraphicsEmptyNodeId,
    GraphicsUnsupportedPrimitive,
    GraphicsInvalidTiming,
    GraphicsInvalidDimensions,
    GraphicsInvalidColor,
    GraphicsTextEmpty,
    GraphicsTextOverflow,
    GraphicsTextUnreadable,
    GraphicsNodeOutOfSafeZone,
    GraphicsFrameCoverageExceeded,
    GraphicsImageRefMissing,
    GraphicsImageRefUnauthorized,
    GraphicsImageDecodeFailed,
    GraphicsTemplateParamMissing,
    GraphicsTemplateParamInvalid,
    GraphicsVisualQaFailed,
    GraphicsRenderFailed,
    RenderOverlayOutOfRange,
    RenderFrameSequenceEmpty,
    RenderPlanInvalidClip,
    RenderPlanInvalidOverlay,
    RenderBackendUnavailable,
    RenderBackendFailed,
    RenderBackendTimeout,
    RenderBackendPolicyDenied,
    RenderProbeInvalidJson,
    RenderProbeValidationFailed,
    RenderArtifactMissing,
    RenderArtifactEmpty,
    RenderStorageInsufficient,
    PipelineInputInvalid,
    PipelineReportWriteFailed,
    CodexTransportFailed,
    CodexProposalInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineError {
    pub code: PipelineErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl PipelineError {
    pub fn new(
        code: PipelineErrorCode,
        path: impl Into<String>,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
            fix: fix.into(),
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn from_graphics_errors(errors: Vec<ActionableError>) -> Vec<Self> {
        errors
            .into_iter()
            .map(|error| {
                let ActionableError {
                    code,
                    path,
                    message,
                    fix,
                    mut details,
                    cause,
                } = error;

                details.insert("graphicsCode".to_string(), graphics_error_code_label(&code));
                if let Some(cause) = cause {
                    details.insert(
                        "graphicsCause".to_string(),
                        graphics_error_code_label(&cause),
                    );
                }

                Self {
                    code: pipeline_error_code_from_graphics(&code),
                    path,
                    message,
                    fix,
                    details,
                }
            })
            .collect()
    }
}

pub type PipelineResult<T> = Result<T, Vec<PipelineError>>;

fn pipeline_error_code_from_graphics(code: &GraphicsErrorCode) -> PipelineErrorCode {
    match code {
        GraphicsErrorCode::GraphicsSchemaUnsupported => {
            PipelineErrorCode::GraphicsSchemaUnsupported
        }
        GraphicsErrorCode::GraphicsEmptyNodeId => PipelineErrorCode::GraphicsEmptyNodeId,
        GraphicsErrorCode::GraphicsUnsupportedPrimitive => {
            PipelineErrorCode::GraphicsUnsupportedPrimitive
        }
        GraphicsErrorCode::GraphicsInvalidTiming => PipelineErrorCode::GraphicsInvalidTiming,
        GraphicsErrorCode::GraphicsInvalidDimensions => {
            PipelineErrorCode::GraphicsInvalidDimensions
        }
        GraphicsErrorCode::GraphicsInvalidColor => PipelineErrorCode::GraphicsInvalidColor,
        GraphicsErrorCode::GraphicsTextEmpty => PipelineErrorCode::GraphicsTextEmpty,
        GraphicsErrorCode::GraphicsTextOverflow => PipelineErrorCode::GraphicsTextOverflow,
        GraphicsErrorCode::GraphicsTextUnreadable => PipelineErrorCode::GraphicsTextUnreadable,
        GraphicsErrorCode::GraphicsNodeOutOfSafeZone => {
            PipelineErrorCode::GraphicsNodeOutOfSafeZone
        }
        GraphicsErrorCode::GraphicsFrameCoverageExceeded => {
            PipelineErrorCode::GraphicsFrameCoverageExceeded
        }
        GraphicsErrorCode::GraphicsImageRefMissing => PipelineErrorCode::GraphicsImageRefMissing,
        GraphicsErrorCode::GraphicsImageRefUnauthorized => {
            PipelineErrorCode::GraphicsImageRefUnauthorized
        }
        GraphicsErrorCode::GraphicsImageDecodeFailed => {
            PipelineErrorCode::GraphicsImageDecodeFailed
        }
        GraphicsErrorCode::GraphicsTemplateParamMissing => {
            PipelineErrorCode::GraphicsTemplateParamMissing
        }
        GraphicsErrorCode::GraphicsTemplateParamInvalid => {
            PipelineErrorCode::GraphicsTemplateParamInvalid
        }
        GraphicsErrorCode::GraphicsVisualQaFailed => PipelineErrorCode::GraphicsVisualQaFailed,
        GraphicsErrorCode::GraphicsRenderFailed => PipelineErrorCode::GraphicsRenderFailed,
        GraphicsErrorCode::RenderOverlayOutOfRange => PipelineErrorCode::RenderOverlayOutOfRange,
        GraphicsErrorCode::RenderFrameSequenceEmpty => PipelineErrorCode::RenderFrameSequenceEmpty,
        GraphicsErrorCode::RenderBackendUnavailable => PipelineErrorCode::RenderBackendUnavailable,
        GraphicsErrorCode::RenderBackendFailed => PipelineErrorCode::RenderBackendFailed,
    }
}

fn graphics_error_code_label(code: &GraphicsErrorCode) -> String {
    match code {
        GraphicsErrorCode::GraphicsSchemaUnsupported => "GRAPHICS_SCHEMA_UNSUPPORTED",
        GraphicsErrorCode::GraphicsEmptyNodeId => "GRAPHICS_EMPTY_NODE_ID",
        GraphicsErrorCode::GraphicsUnsupportedPrimitive => "GRAPHICS_UNSUPPORTED_PRIMITIVE",
        GraphicsErrorCode::GraphicsInvalidTiming => "GRAPHICS_INVALID_TIMING",
        GraphicsErrorCode::GraphicsInvalidDimensions => "GRAPHICS_INVALID_DIMENSIONS",
        GraphicsErrorCode::GraphicsInvalidColor => "GRAPHICS_INVALID_COLOR",
        GraphicsErrorCode::GraphicsTextEmpty => "GRAPHICS_TEXT_EMPTY",
        GraphicsErrorCode::GraphicsTextOverflow => "GRAPHICS_TEXT_OVERFLOW",
        GraphicsErrorCode::GraphicsTextUnreadable => "GRAPHICS_TEXT_UNREADABLE",
        GraphicsErrorCode::GraphicsNodeOutOfSafeZone => "GRAPHICS_NODE_OUT_OF_SAFE_ZONE",
        GraphicsErrorCode::GraphicsFrameCoverageExceeded => "GRAPHICS_FRAME_COVERAGE_EXCEEDED",
        GraphicsErrorCode::GraphicsImageRefMissing => "GRAPHICS_IMAGE_REF_MISSING",
        GraphicsErrorCode::GraphicsImageRefUnauthorized => "GRAPHICS_IMAGE_REF_UNAUTHORIZED",
        GraphicsErrorCode::GraphicsImageDecodeFailed => "GRAPHICS_IMAGE_DECODE_FAILED",
        GraphicsErrorCode::GraphicsTemplateParamMissing => "GRAPHICS_TEMPLATE_PARAM_MISSING",
        GraphicsErrorCode::GraphicsTemplateParamInvalid => "GRAPHICS_TEMPLATE_PARAM_INVALID",
        GraphicsErrorCode::GraphicsVisualQaFailed => "GRAPHICS_VISUAL_QA_FAILED",
        GraphicsErrorCode::GraphicsRenderFailed => "GRAPHICS_RENDER_FAILED",
        GraphicsErrorCode::RenderOverlayOutOfRange => "RENDER_OVERLAY_OUT_OF_RANGE",
        GraphicsErrorCode::RenderFrameSequenceEmpty => "RENDER_FRAME_SEQUENCE_EMPTY",
        GraphicsErrorCode::RenderBackendUnavailable => "RENDER_BACKEND_UNAVAILABLE",
        GraphicsErrorCode::RenderBackendFailed => "RENDER_BACKEND_FAILED",
    }
    .to_string()
}
