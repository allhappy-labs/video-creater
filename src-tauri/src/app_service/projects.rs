use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::context::RequestContext;
use super::error::ServiceError;
use super::events::{EventSink, ServiceEvent};
use super::operation::AuthorizationScope;
use crate::project::action::{apply_project_action, ProjectAction};
use crate::project::job_progress::{read_job_progress_snapshots, JobProgressSnapshot};
use crate::project::model::{RenderSettings, VideoProject};
use crate::project::split::{
    load_agent_session_manifest, load_app_server_conversation_history, load_split_project,
    replace_split_project_if_revision, validate_split_project, ProjectActionWriteResult,
    ProjectValidationReport, SplitAgentSessionManifest, SplitAppServerConversationFile,
    SplitProjectError,
};

#[derive(Clone)]
pub struct ProjectService {
    events: Arc<dyn EventSink>,
}

impl ProjectService {
    pub fn new(events: Arc<dyn EventSink>) -> Self {
        Self { events }
    }

    pub fn create_empty(&self, id: String, name: String, created_at: String) -> VideoProject {
        VideoProject::new_empty(id, name, created_at)
    }

    pub fn load(
        &self,
        context: &RequestContext,
        project_dir: &Path,
    ) -> Result<VideoProject, ServiceError> {
        require_scope(context, AuthorizationScope::ProjectRead)?;
        let project = load_split_project(project_dir).map_err(map_split_error)?;
        require_project(context, &project.id)?;
        Ok(project)
    }

    pub fn authorize_loaded(
        &self,
        context: &RequestContext,
        project: VideoProject,
    ) -> Result<VideoProject, ServiceError> {
        require_scope(context, AuthorizationScope::ProjectRead)?;
        require_project(context, &project.id)?;
        Ok(project)
    }

    pub fn save(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        project: VideoProject,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        require_mutation(context, &project.id)?;
        let expected_revision = context.expected_revision.ok_or_else(|| {
            ServiceError::invalid_input("expected revision is required for project mutation")
        })?;
        let result = replace_split_project_if_revision(project_dir, project, expected_revision)
            .map_err(map_split_error)?;
        self.publish_changed(context, &result)?;
        Ok(result)
    }

    pub fn apply_action(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        action: ProjectAction,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        self.apply_actions(context, project_dir, vec![action])
    }

    pub fn apply_actions(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        actions: Vec<ProjectAction>,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        require_scope(context, AuthorizationScope::ProjectWrite)?;
        let expected_revision = context.expected_revision.ok_or_else(|| {
            ServiceError::invalid_input("expected revision is required for project mutation")
        })?;
        let mut project = load_split_project(project_dir).map_err(map_split_error)?;
        require_project(context, &project.id)?;
        for action in actions {
            apply_project_action(&mut project, action)
                .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
        }
        let result = replace_split_project_if_revision(project_dir, project, expected_revision)
            .map_err(map_split_error)?;
        self.publish_changed(context, &result)?;
        Ok(result)
    }

    pub fn update_settings(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        name: String,
        render_settings: RenderSettings,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        self.apply_action(
            context,
            project_dir,
            ProjectAction::UpdateProjectSettings {
                name,
                render_settings,
            },
        )
    }

    pub fn validate(
        &self,
        context: &RequestContext,
        project_dir: &Path,
    ) -> Result<ProjectValidationReport, ServiceError> {
        require_scope(context, AuthorizationScope::ProjectRead)?;
        validate_split_project(project_dir).map_err(map_split_error)
    }

    pub fn conversation_history(
        &self,
        context: &RequestContext,
        project_dir: &Path,
    ) -> Result<SplitAppServerConversationFile, ServiceError> {
        self.load(context, project_dir)?;
        load_app_server_conversation_history(project_dir).map_err(map_split_error)
    }

    pub fn agent_sessions(
        &self,
        context: &RequestContext,
        project_dir: &Path,
    ) -> Result<SplitAgentSessionManifest, ServiceError> {
        self.load(context, project_dir)?;
        load_agent_session_manifest(project_dir).map_err(map_split_error)
    }

    pub fn job_progress(
        &self,
        context: &RequestContext,
        project_dir: &Path,
    ) -> Result<Vec<JobProgressSnapshot>, ServiceError> {
        self.load(context, project_dir)?;
        read_job_progress_snapshots(project_dir).map_err(ServiceError::internal)
    }

    fn publish_changed(
        &self,
        context: &RequestContext,
        result: &ProjectActionWriteResult,
    ) -> Result<(), ServiceError> {
        self.events.publish(ServiceEvent::new(
            "project.changed",
            context.project_id.as_deref(),
            json!({
                "requestId": context.request_id,
                "contentRevision": result.project.content_revision,
            }),
        ))?;
        Ok(())
    }
}

fn require_scope(context: &RequestContext, scope: AuthorizationScope) -> Result<(), ServiceError> {
    if context.allows(scope) {
        Ok(())
    } else {
        Err(ServiceError::forbidden())
    }
}

fn require_project(context: &RequestContext, actual_project_id: &str) -> Result<(), ServiceError> {
    match context.project_id.as_deref() {
        Some(project_id) if project_id == actual_project_id => Ok(()),
        Some(_) => Err(ServiceError::forbidden()),
        None => Err(ServiceError::invalid_input("project ID is required")),
    }
}

fn require_mutation(context: &RequestContext, project_id: &str) -> Result<(), ServiceError> {
    require_scope(context, AuthorizationScope::ProjectWrite)?;
    require_project(context, project_id)
}

fn map_split_error(error: SplitProjectError) -> ServiceError {
    match error {
        SplitProjectError::RevisionConflict { expected, actual } => {
            ServiceError::revision_conflict(expected, actual)
        }
        SplitProjectError::AlreadySplitProject
        | SplitProjectError::UnsafeManifestPath { .. }
        | SplitProjectError::UnsafeSidecarId { .. }
        | SplitProjectError::ProjectAction { .. } => ServiceError::invalid_input(error.to_string()),
        SplitProjectError::UnsafeProjectSymlink { .. }
        | SplitProjectError::Io { .. }
        | SplitProjectError::Json { .. } => ServiceError::internal(error.to_string()),
    }
}
