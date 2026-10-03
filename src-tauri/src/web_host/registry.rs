use std::collections::{BTreeMap, BTreeSet};

use crate::app_service::operation::{
    AuthorizationScope, MutationClass, OperationDescriptor, RemoteSupport, OPERATION_INVENTORY,
};

use super::rpc::RpcEnvelope;

const WEB_HOST_OPERATION_INVENTORY: &[OperationDescriptor] = &[
    OperationDescriptor {
        name: "remote_create_project",
        support: RemoteSupport::Remote,
        authorization: AuthorizationScope::HostAdmin,
        mutation: MutationClass::HostMutation,
        requires_project: false,
        requires_revision: false,
        max_request_bytes: 1_048_576,
        browser_replacement: None,
    },
    OperationDescriptor {
        name: "remote_list_projects",
        support: RemoteSupport::Remote,
        authorization: AuthorizationScope::Session,
        mutation: MutationClass::Read,
        requires_project: false,
        requires_revision: false,
        max_request_bytes: 65_536,
        browser_replacement: None,
    },
];

/// Operations with a concrete production dispatcher implementation. Keeping
/// this list next to the registry prevents metadata from advertising desktop
/// commands that the headless host cannot execute.
pub const WEB_HOST_DISPATCHED_OPERATIONS: &[&str] = &[
    "get_platform_info",
    "get_remote_access_status",
    "get_app_preferences",
    "update_app_preferences",
    "list_transcription_models",
    "get_active_transcription_model",
    "get_transcription_runtime_status",
    "get_production_speech_model_status",
    "get_export_profile_availability_report",
    "list_visual_effect_catalog",
    "remote_list_projects",
    "remote_create_project",
    "load_split_project_from_folder",
    "read_project_snapshot_from_split_project_folder",
    "save_split_project_to_folder",
    "apply_project_action_to_split_project_folder",
    "apply_project_actions_to_split_project_folder",
    "load_job_progress_from_split_project_folder",
    "load_render_attempt_in_split_project_folder",
    "recover_render_attempt_in_split_project_folder",
    "reconcile_temporal_jobs_in_split_project_folder",
    "load_render_pipeline_report_from_split_project_folder",
    "render_media_to_split_project_folder",
    "cache_timeline_filmstrip_in_split_project_folder",
    "capture_canonical_preview_frame_in_split_project_folder",
    "prepare_project_preview",
    "load_agent_sessions_from_split_project_folder",
    "load_app_server_conversation_history_from_split_project_folder",
    "apply_agent_session_action_to_split_project_folder",
    "start_codex_conversation_edit_for_project",
    "apply_codex_conversation_proposal",
    "undo_latest_codex_conversation_edit",
    "cancel_codex_conversation_edit_for_project",
    "cancel_codex_video_edit_for_project",
    "cancel_generate_media_in_process",
    "cancel_generate_media_provider_request_in_split_project_folder",
    "cancel_render_job_in_split_project_folder",
    "validate_split_project_folder",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    UnknownOperation,
    BodyTooLarge,
    Forbidden,
    ProjectRequired,
    ExpectedRevisionRequired,
    EditorLeaseRequired,
}

pub struct RpcRegistry {
    operations: BTreeMap<&'static str, &'static OperationDescriptor>,
}

impl RpcRegistry {
    pub fn from_inventory() -> Self {
        Self {
            operations: OPERATION_INVENTORY
                .iter()
                .chain(WEB_HOST_OPERATION_INVENTORY)
                .filter(|operation| operation.support == RemoteSupport::Remote)
                .filter(|operation| WEB_HOST_DISPATCHED_OPERATIONS.contains(&operation.name))
                .map(|operation| (operation.name, operation))
                .collect(),
        }
    }

    pub fn operation(&self, name: &str) -> Option<&'static OperationDescriptor> {
        self.operations.get(name).copied()
    }

    pub fn validate(
        &self,
        request: &RpcEnvelope,
        scopes: &BTreeSet<AuthorizationScope>,
        body_bytes: usize,
    ) -> Result<&'static OperationDescriptor, RegistryError> {
        let operation = self
            .operation(&request.operation)
            .ok_or(RegistryError::UnknownOperation)?;
        if body_bytes > operation.max_request_bytes {
            return Err(RegistryError::BodyTooLarge);
        }
        if !scope_allows(scopes, operation.authorization) {
            return Err(RegistryError::Forbidden);
        }
        if operation.requires_project && request.project_id.as_deref().is_none_or(str::is_empty) {
            return Err(RegistryError::ProjectRequired);
        }
        if operation.requires_revision && request.expected_revision.is_none() {
            return Err(RegistryError::ExpectedRevisionRequired);
        }
        if operation.requires_project
            && operation.mutation != MutationClass::Read
            && request
                .editor_lease_token
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(RegistryError::EditorLeaseRequired);
        }
        Ok(operation)
    }
}

fn scope_allows(granted: &BTreeSet<AuthorizationScope>, required: AuthorizationScope) -> bool {
    granted.contains(&AuthorizationScope::HostAdmin)
        || granted.contains(&required)
        || (required == AuthorizationScope::ProjectRead
            && granted.contains(&AuthorizationScope::ProjectWrite))
}
