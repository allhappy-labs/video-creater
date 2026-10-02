use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use super::context::RequestContext;
use super::error::ServiceError;
use super::events::{EventSink, ServiceEvent};
use super::operation::AuthorizationScope;
use crate::project::action::ProjectAction;
use crate::project::job_progress::{read_job_progress_snapshots, JobProgressSnapshot};
use crate::project::model::{RenderSettings, VideoProject};
use crate::project::split::{
    apply_project_actions_to_split_project_if_revision, load_agent_session_manifest,
    load_app_server_conversation_history, load_split_project,
    replace_existing_split_project_if_revision, replace_split_project_if_revision,
    split_project_manifest_path, validate_split_project, validate_split_project_write_path,
    ProjectActionWriteResult, ProjectValidationReport, SplitAgentSessionManifest,
    SplitAppServerConversationFile, SplitProjectError, SplitProjectLayout,
    SPLIT_PROJECT_SCHEMA_VERSION,
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
        self.save_impl(context, project_dir, project, false)
    }

    /// Canonical snapshot restore: never creates a project or activates a desktop session.
    pub fn save_existing(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        project: VideoProject,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        self.save_impl(context, project_dir, project, true)
    }

    fn save_impl(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        project: VideoProject,
        require_existing: bool,
    ) -> Result<ProjectActionWriteResult, ServiceError> {
        require_mutation(context, &project.id)?;
        let expected_revision = context.expected_revision.ok_or_else(|| {
            ServiceError::invalid_input("expected revision is required for project mutation")
        })?;
        let result = if require_existing {
            replace_existing_split_project_if_revision(project_dir, project, expected_revision)
        } else {
            replace_split_project_if_revision(project_dir, project, expected_revision)
        }
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
        let project_id = context
            .project_id
            .as_deref()
            .ok_or_else(|| ServiceError::invalid_input("project ID is required"))?;
        let result = apply_project_actions_to_split_project_if_revision(
            project_dir,
            actions,
            project_id,
            expected_revision,
        )
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
        self.job_progress_with_read_hook(context, project_dir, || {})
    }

    fn job_progress_with_read_hook(
        &self,
        context: &RequestContext,
        project_dir: &Path,
        read_hook: impl FnOnce(),
    ) -> Result<Vec<JobProgressSnapshot>, ServiceError> {
        require_scope(context, AuthorizationScope::ProjectRead)?;
        let identity = read_project_identity(project_dir)?;
        require_project(context, &identity.id)?;
        read_hook();
        let snapshots = read_job_progress_snapshots(project_dir).map_err(ServiceError::internal)?;
        // A package replacement or symlink retarget must never publish another project's data.
        let after = read_project_identity(project_dir)?;
        if !identity.same_project(&after) {
            return Err(ServiceError::forbidden());
        }
        Ok(snapshots)
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
        SplitProjectError::ProjectIdentityMismatch => ServiceError::forbidden(),
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

/// Manifest-only identity for short reads. This never recovers a package, reads sidecars,
/// activates a session, or acquires a project/storage lease.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectReadIdentity {
    schema_version: u32,
    layout: SplitProjectLayout,
    pub id: String,
    #[serde(default)]
    pub content_revision: u64,
    #[serde(skip)]
    canonical_path: PathBuf,
    #[cfg(unix)]
    #[serde(skip)]
    directory_key: (u64, u64),
}

impl ProjectReadIdentity {
    fn same_project(&self, other: &Self) -> bool {
        let matches = self.id == other.id && self.canonical_path == other.canonical_path;
        #[cfg(unix)]
        let matches = matches && self.directory_key == other.directory_key;
        matches
    }
}

pub fn read_project_identity(project_dir: &Path) -> Result<ProjectReadIdentity, ServiceError> {
    let manifest = split_project_manifest_path(project_dir);
    validate_split_project_write_path(project_dir, &manifest).map_err(|error| match error {
        SplitProjectError::UnsafeProjectSymlink { .. } => ServiceError::forbidden(),
        other => map_split_error(other),
    })?;
    let missing = |error: std::io::Error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ServiceError::not_found("project")
        } else {
            ServiceError::internal(error.to_string())
        }
    };
    let directory = std::fs::metadata(project_dir).map_err(missing)?;
    if !directory.is_dir() {
        return Err(ServiceError::not_found("project"));
    }
    let canonical_path = project_dir.canonicalize().map_err(missing)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    let file = options.open(&manifest).map_err(missing)?;
    let metadata = file.metadata().map_err(missing)?;
    if !metadata.is_file() {
        return Err(ServiceError::invalid_input("project manifest is invalid"));
    }
    // Deserialize only identity fields; unknown manifest content is streamed and discarded.
    let mut identity: ProjectReadIdentity =
        serde_json::from_reader(file).map_err(|error| ServiceError::internal(error.to_string()))?;
    if identity.schema_version != SPLIT_PROJECT_SCHEMA_VERSION
        || identity.layout != SplitProjectLayout::Split
    {
        return Err(ServiceError::invalid_input(
            "project manifest is not a split project",
        ));
    }
    identity.canonical_path = canonical_path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        identity.directory_key = (directory.dev(), directory.ino());
        let current = std::fs::symlink_metadata(project_dir).map_err(missing)?;
        if current.file_type().is_symlink()
            || (current.dev(), current.ino()) != identity.directory_key
        {
            return Err(ServiceError::forbidden());
        }
    }
    Ok(identity)
}

#[cfg(test)]
mod existing_save_tests {
    use super::*;
    use crate::app_service::context::ClientKind;
    use crate::app_service::events::FakeEventSink;
    use crate::project::fixtures::sample_project;
    use crate::project::mutation::acquire_split_project_mutation_lease;
    use crate::project::split::save_split_project;
    use std::collections::BTreeSet;
    use std::time::Duration;

    fn context(project: &VideoProject) -> RequestContext {
        RequestContext::new(
            ClientKind::Test,
            "existing-save",
            Some(project.id.clone()),
            Some(project.content_revision),
            BTreeSet::from([AuthorizationScope::ProjectWrite]),
            None,
        )
        .unwrap()
    }

    fn zero_revision_project(path: &Path) -> VideoProject {
        save_split_project(path, &sample_project()).unwrap();
        let manifest_path = split_project_manifest_path(path);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        manifest["contentRevision"] = json!(0);
        std::fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        load_split_project(path).unwrap()
    }

    #[test]
    fn existing_only_save_checks_manifest_after_acquiring_the_mutation_lease() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("project");
        let project = zero_revision_project(&path);
        let events = Arc::new(FakeEventSink::default());
        let service = ProjectService::new(events.clone());
        let lease = acquire_split_project_mutation_lease(&path).unwrap();
        let worker_path = path.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = service.save_existing(&context(&project), &worker_path, project);
            done_tx.send(result).unwrap();
        });
        let started = started_rx.recv_timeout(Duration::from_secs(5));
        let blocked = done_rx.recv_timeout(Duration::from_millis(100));
        std::fs::remove_file(split_project_manifest_path(&path)).unwrap();
        drop(lease);
        worker.join().unwrap();
        assert!(started.is_ok());
        assert!(
            blocked.is_err(),
            "save must wait for the canonical mutation lease"
        );
        let result = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            result.is_err(),
            "snapshot save must not recreate a missing canonical manifest"
        );
        assert!(!split_project_manifest_path(&path).exists());
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn existing_only_save_accepts_a_valid_revision_zero_package() {
        let root = tempfile::tempdir().unwrap();
        let project = zero_revision_project(root.path());
        let events = Arc::new(FakeEventSink::default());
        let service = ProjectService::new(events.clone());
        #[cfg(unix)]
        let before = {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(root.path()).unwrap();
            (metadata.dev(), metadata.ino())
        };
        let result = service
            .save_existing(&context(&project), root.path(), project)
            .unwrap();
        assert_eq!(result.project.content_revision, 1);
        assert_eq!(events.recorded().len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(root.path()).unwrap();
            assert_eq!((metadata.dev(), metadata.ino()), before);
        }
    }
}

#[cfg(test)]
mod progress_identity_tests {
    use super::*;
    use crate::app_service::context::ClientKind;
    use crate::app_service::error::ServiceErrorCode;
    use crate::app_service::events::FakeEventSink;
    use crate::project::fixtures::sample_project;
    use crate::project::split::save_split_project;
    use std::collections::BTreeSet;

    #[cfg(unix)]
    #[test]
    fn progress_rejects_a_same_id_package_replaced_during_the_read() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("project");
        let replacement = root.path().join("replacement");
        let project = save_split_project(&path, &sample_project())
            .unwrap()
            .project;
        save_split_project(&replacement, &project).unwrap();
        let context = RequestContext::new(
            ClientKind::Test,
            "progress-replacement",
            Some(project.id),
            None,
            BTreeSet::from([AuthorizationScope::ProjectRead]),
            None,
        )
        .unwrap();
        let service = ProjectService::new(Arc::new(FakeEventSink::default()));
        let result = service.job_progress_with_read_hook(&context, &path, || {
            std::fs::rename(&path, root.path().join("old-project")).unwrap();
            std::fs::rename(&replacement, &path).unwrap();
        });
        assert_eq!(result.unwrap_err().code(), ServiceErrorCode::Forbidden);
    }
}
