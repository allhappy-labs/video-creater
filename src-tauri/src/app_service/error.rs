use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceErrorCode {
    InvalidInput,
    UnavailableCapability,
    Unauthorized,
    Forbidden,
    RevisionConflict,
    LeaseConflict,
    NotFound,
    Busy,
    Cancelled,
    Internal,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceError {
    code: ServiceErrorCode,
    message: String,
    retryable: bool,
    #[serde(skip)]
    internal_detail: Option<String>,
}

impl ServiceError {
    fn public(code: ServiceErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: redact_paths(&message.into()),
            retryable,
            internal_detail: None,
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::public(ServiceErrorCode::InvalidInput, message, false)
    }

    pub fn unavailable(capability: &str) -> Self {
        Self::public(
            ServiceErrorCode::UnavailableCapability,
            format!("{capability} is unavailable"),
            true,
        )
    }

    pub fn unauthorized() -> Self {
        Self::public(
            ServiceErrorCode::Unauthorized,
            "authentication required",
            false,
        )
    }

    pub fn forbidden() -> Self {
        Self::public(
            ServiceErrorCode::Forbidden,
            "operation is not permitted",
            false,
        )
    }

    pub fn revision_conflict(expected: u64, actual: u64) -> Self {
        Self::public(
            ServiceErrorCode::RevisionConflict,
            format!("project revision changed (expected {expected}, actual {actual})"),
            true,
        )
    }

    pub fn lease_conflict() -> Self {
        Self::public(
            ServiceErrorCode::LeaseConflict,
            "editor lease is held by another client",
            true,
        )
    }

    pub fn not_found(resource: &str) -> Self {
        Self::public(
            ServiceErrorCode::NotFound,
            format!("{resource} was not found"),
            false,
        )
    }

    pub fn busy(resource: &str) -> Self {
        Self::public(ServiceErrorCode::Busy, format!("{resource} is busy"), true)
    }

    pub fn cancelled() -> Self {
        Self::public(
            ServiceErrorCode::Cancelled,
            "operation was cancelled",
            false,
        )
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            code: ServiceErrorCode::Internal,
            message: "an internal error occurred".into(),
            retryable: false,
            internal_detail: Some(detail.into()),
        }
    }

    pub fn code(&self) -> ServiceErrorCode {
        self.code
    }

    pub fn internal_detail(&self) -> Option<&str> {
        self.internal_detail.as_deref()
    }
}

impl fmt::Debug for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceError")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("retryable", &self.retryable)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ServiceError {}

fn redact_paths(message: &str) -> String {
    message
        .split_whitespace()
        .map(|word| {
            if word.starts_with('/')
                || (word.len() > 2
                    && word.as_bytes()[1] == b':'
                    && matches!(word.as_bytes()[2], b'/' | b'\\'))
            {
                "[path]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
