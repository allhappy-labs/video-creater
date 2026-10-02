//! One application boundary for desktop and authorized host render adapters.
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::projects::read_project_identity;
use super::{context::RequestContext, error::ServiceError, operation::AuthorizationScope};
use crate::render_pipeline::project_export::{
    admit_media_render, read_media_render_attempt, MediaRenderAdmission, MediaRenderAttempt,
    MediaRenderInput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderAttemptQuery {
    pub job_id: String,
    pub attempt_id: String,
}

pub struct RenderJobService;

impl RenderJobService {
    pub fn admit(
        context: &RequestContext,
        project_dir: &Path,
        input: MediaRenderInput,
    ) -> Result<MediaRenderAdmission, ServiceError> {
        Self::authorize(context, project_dir, AuthorizationScope::ProjectWrite)?;
        if context.project_id.as_deref() != Some(input.project_id.as_str()) {
            return Err(ServiceError::forbidden());
        }
        let revision = context.expected_revision.ok_or_else(|| {
            ServiceError::invalid_input("expected revision is required for render admission")
        })?;
        admit_media_render(project_dir, revision, input)
    }

    pub fn attempt(
        context: &RequestContext,
        project_dir: &Path,
        query: RenderAttemptQuery,
    ) -> Result<MediaRenderAttempt, ServiceError> {
        Self::authorize(context, project_dir, AuthorizationScope::ProjectRead)?;
        read_media_render_attempt(project_dir, &query.job_id, &query.attempt_id)
    }

    pub fn recover(
        context: &RequestContext,
        project_dir: &Path,
        query: RenderAttemptQuery,
    ) -> Result<MediaRenderAttempt, ServiceError> {
        Self::authorize(context, project_dir, AuthorizationScope::ProjectWrite)?;
        crate::render_pipeline::project_export::recover_media_render_attempt(
            project_dir,
            &query.job_id,
            &query.attempt_id,
        )
    }

    fn authorize(
        context: &RequestContext,
        project_dir: &Path,
        scope: AuthorizationScope,
    ) -> Result<(), ServiceError> {
        if !context.allows(scope) {
            return Err(ServiceError::forbidden());
        }
        let identity = read_project_identity(project_dir)?;
        if context.project_id.as_deref() != Some(identity.id.as_str()) {
            return Err(ServiceError::forbidden());
        }
        Ok(())
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::app_service::{context::ClientKind, operation::find_operation};
    use crate::project::{fixtures::sample_project, split::save_split_project};
    use serde_json::{json, Value};
    use std::collections::BTreeSet;

    #[test]
    fn render_job_wire_contract_matches_checked_frontend_bundle() {
        let bundle: Value =
            serde_json::from_str(include_str!("../../../contracts/render-jobs-v1.json")).unwrap();
        let input: MediaRenderInput = serde_json::from_value(bundle["input"].clone()).unwrap();
        assert_eq!(serde_json::to_value(input).unwrap(), bundle["input"]);
        for attempt in bundle["attempts"].as_array().unwrap() {
            let parsed: MediaRenderAttempt = serde_json::from_value(attempt.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), *attempt);
        }
        for operation in bundle["operations"].as_object().unwrap().values() {
            assert!(find_operation(operation.as_str().unwrap()).is_some());
        }
        let admission = MediaRenderAdmission {
            admission_protocol: 1,
            project: sample_project(),
            job_id: "job".into(),
            attempt_id: "render-attempt/1".into(),
            source_revision: 1,
        };
        let value = serde_json::to_value(admission).unwrap();
        let fields: Vec<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|s| json!(s))
            .collect();
        assert_eq!(json!(fields), bundle["admissionFields"]);
        let mut unknown = bundle["input"].clone();
        unknown["surprise"] = json!(true);
        assert!(serde_json::from_value::<MediaRenderInput>(unknown).is_err());
    }

    #[test]
    fn render_service_rejects_missing_scope_and_wrong_project_before_attempt_read() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("project");
        let project = save_split_project(&dir, &sample_project()).unwrap().project;
        let query = RenderAttemptQuery {
            job_id: "job".into(),
            attempt_id: "render-attempt/1".into(),
        };
        for (id, scopes) in [
            (project.id, BTreeSet::new()),
            (
                "different".into(),
                BTreeSet::from([AuthorizationScope::ProjectRead]),
            ),
        ] {
            let context =
                RequestContext::new(ClientKind::Test, "request", Some(id), None, scopes, None)
                    .unwrap();
            assert_eq!(
                RenderJobService::attempt(&context, &dir, query.clone())
                    .unwrap_err()
                    .code(),
                super::super::error::ServiceErrorCode::Forbidden
            );
        }
    }
}
