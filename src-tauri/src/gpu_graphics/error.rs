use crate::render_pipeline::error::{PipelineError, PipelineErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GpuGraphicsErrorCode {
    GpuGraphicsSchemaUnsupported,
    GpuGraphicsInvalidTiming,
    GpuGraphicsInvalidDimensions,
    GpuGraphicsFrameBudgetExceeded,
    GpuGraphicsTextEmpty,
    GpuGraphicsShaderSourceTooLarge,
    GpuGraphicsShaderEntryMissing,
    GpuGraphicsShaderUnsupportedFeature,
    GpuGraphicsShaderCompileFailed,
    GpuGraphicsUniformInvalid,
    GpuGraphicsPrimitiveUnsupported,
    GpuGraphicsPrimitiveInvalid,
    GpuGraphicsCameraInvalid,
    GpuGraphicsDeviceUnavailable,
    GpuGraphicsRenderFailed,
    GpuGraphicsReadbackFailed,
    GpuGraphicsArtifactWriteFailed,
    GpuGraphicsVisualQaFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuGraphicsError {
    pub code: GpuGraphicsErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl GpuGraphicsError {
    pub fn new(
        code: GpuGraphicsErrorCode,
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
}

pub type GpuGraphicsResult<T> = Result<T, Vec<GpuGraphicsError>>;

pub fn gpu_errors_to_pipeline_errors(errors: Vec<GpuGraphicsError>) -> Vec<PipelineError> {
    errors
        .into_iter()
        .map(|error| {
            let mut details = error.details;
            details.insert(
                "gpuGraphicsCode".to_string(),
                gpu_error_code_label(&error.code),
            );
            PipelineError {
                code: PipelineErrorCode::PipelineInputInvalid,
                path: error.path,
                message: error.message,
                fix: error.fix,
                details,
            }
        })
        .collect()
}

pub fn gpu_error_code_label(code: &GpuGraphicsErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "GPU_GRAPHICS_RENDER_FAILED".to_string())
}
