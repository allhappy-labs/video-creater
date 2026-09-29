use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphicsErrorCode {
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
    RenderBackendUnavailable,
    RenderBackendFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionableError {
    pub code: GraphicsErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<GraphicsErrorCode>,
}

impl ActionableError {
    pub fn new(
        code: GraphicsErrorCode,
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
            cause: None,
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn with_cause(mut self, cause: GraphicsErrorCode) -> Self {
        self.cause = Some(cause);
        self
    }
}

pub type ActionableResult<T> = Result<T, Vec<ActionableError>>;
