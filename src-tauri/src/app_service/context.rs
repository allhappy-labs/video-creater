use std::collections::BTreeSet;
use std::fmt;

use super::error::ServiceError;
use super::operation::AuthorizationScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Desktop,
    Browser,
    Test,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RequestContext {
    pub client_kind: ClientKind,
    pub request_id: String,
    pub project_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub authorization_scopes: BTreeSet<AuthorizationScope>,
    editor_lease_token: Option<String>,
}

impl RequestContext {
    pub fn new(
        client_kind: ClientKind,
        request_id: impl Into<String>,
        project_id: Option<String>,
        expected_revision: Option<u64>,
        authorization_scopes: BTreeSet<AuthorizationScope>,
        editor_lease_token: Option<String>,
    ) -> Result<Self, ServiceError> {
        let request_id = request_id.into();
        if request_id.is_empty() || request_id.len() > 128 || !request_id.is_ascii() {
            return Err(ServiceError::invalid_input("request ID is invalid"));
        }
        if project_id.as_ref().is_some_and(|id| {
            id.is_empty()
                || id.len() > 128
                || !id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "-_.".contains(ch))
        }) {
            return Err(ServiceError::invalid_input("project ID is invalid"));
        }
        if editor_lease_token
            .as_ref()
            .is_some_and(|token| token.is_empty() || token.len() > 512)
        {
            return Err(ServiceError::invalid_input("editor lease token is invalid"));
        }
        Ok(Self {
            client_kind,
            request_id,
            project_id,
            expected_revision,
            authorization_scopes,
            editor_lease_token,
        })
    }

    pub fn allows(&self, scope: AuthorizationScope) -> bool {
        self.authorization_scopes.contains(&scope)
            || self
                .authorization_scopes
                .contains(&AuthorizationScope::HostAdmin)
    }

    pub fn editor_lease_token(&self) -> Option<&str> {
        self.editor_lease_token.as_deref()
    }
}

impl fmt::Debug for RequestContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestContext")
            .field("client_kind", &self.client_kind)
            .field("request_id", &self.request_id)
            .field("project_id", &self.project_id)
            .field("expected_revision", &self.expected_revision)
            .field("authorization_scopes", &self.authorization_scopes)
            .field(
                "editor_lease_token",
                &self.editor_lease_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}
