use crate::project::split::validate_split_project;
use crate::settings::agent::{
    probe_claude_cli, probe_codex_app_server, probe_mcp_server, probe_proposal_validator,
};
use crate::settings::providers::{ProviderHealth, ProviderReadinessGroup, ProviderValidationState};
use crate::settings::render_system::{get_render_system_health, RenderSystemHealth};
use crate::settings::skills::{
    mandatory_skill_prompt_bundle, validate_skill_root, verify_bundled_skills, SkillLoadState,
    SkillRootError,
};
use crate::settings::storage::{StorageInventoryItem, StorageScope};
#[cfg(not(target_os = "linux"))]
use crate::transcription::fluidaudio::FluidAudioCoreMlRuntime as PlatformTranscriptionRuntime;
#[cfg(not(target_os = "linux"))]
use crate::transcription::model::FLUID_AUDIO_COREML_RUNTIME_ID as PLATFORM_TRANSCRIPTION_RUNTIME_ID;
#[cfg(target_os = "linux")]
use crate::transcription::model::SHERPA_ONNX_RUNTIME_ID as PLATFORM_TRANSCRIPTION_RUNTIME_ID;
use crate::transcription::model::{
    transcription_model_catalog_entry, ModelInstallStatus, TranscriptionModelArtifactFormat,
};
#[cfg(target_os = "linux")]
use crate::transcription::sherpa_onnx::SherpaOnnxRuntime as PlatformTranscriptionRuntime;

#[cfg(not(target_os = "linux"))]
const PLATFORM_TRANSCRIPTION_ARTIFACT_FORMAT: TranscriptionModelArtifactFormat =
    TranscriptionModelArtifactFormat::CoreMlBundle;
#[cfg(target_os = "linux")]
const PLATFORM_TRANSCRIPTION_ARTIFACT_FORMAT: TranscriptionModelArtifactFormat =
    TranscriptionModelArtifactFormat::SherpaOnnxTransducer;
use crate::transcription::runtime::{
    InstalledModel, ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability,
};
use crate::transcription::store::{
    ModelStoreError, TranscriptionModelStatus, TranscriptionModelStore,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsHealthState {
    Ready,
    ActionRequired,
    NeedsAction,
    Checking,
    Failed,
    NotConfigured,
    NotEnabled,
    NotApplicable,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsComponentHealth {
    pub id: String,
    pub label: String,
    pub state: SettingsHealthState,
    pub summary: String,
    pub action_id: Option<String>,
    pub action_label: Option<String>,
    pub last_checked_at: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_detail: Option<String>,
    pub provenance: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCategoryHealth {
    pub id: String,
    pub state: SettingsHealthState,
    pub items: Vec<SettingsComponentHealth>,
}

impl SettingsCategoryHealth {
    pub fn ready(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: SettingsHealthState::Ready,
            items: Vec::new(),
        }
    }

    pub fn action_required(id: impl Into<String>, diagnostic_code: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            state: SettingsHealthState::ActionRequired,
            items: vec![SettingsComponentHealth {
                id: id.clone(),
                label: id,
                state: SettingsHealthState::ActionRequired,
                summary: "Action required.".to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: String::new(),
                diagnostic_code: Some(diagnostic_code.into()),
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            }],
        }
    }

    fn from_items(id: impl Into<String>, items: Vec<SettingsComponentHealth>) -> Self {
        let state = items
            .iter()
            .map(|item| &item.state)
            .max_by_key(|state| severity(state))
            .cloned()
            .unwrap_or(SettingsHealthState::Unavailable);

        Self {
            id: id.into(),
            state,
            items,
        }
    }

    fn from_items_with_state(
        id: impl Into<String>,
        state: SettingsHealthState,
        items: Vec<SettingsComponentHealth>,
    ) -> Self {
        Self {
            id: id.into(),
            state,
            items,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsHealthSnapshot {
    pub generated_at: String,
    pub overall: SettingsHealthState,
    pub categories: BTreeMap<String, SettingsCategoryHealth>,
}

impl SettingsHealthSnapshot {
    pub fn from_categories(
        generated_at: impl Into<String>,
        categories: Vec<SettingsCategoryHealth>,
    ) -> Self {
        let categories = categories
            .into_iter()
            .map(|category| (category.id.clone(), category))
            .collect::<BTreeMap<_, _>>();
        let overall = categories
            .values()
            .map(|category| &category.state)
            .max_by_key(|state| severity(state))
            .cloned()
            .unwrap_or(SettingsHealthState::Unavailable);

        Self {
            generated_at: generated_at.into(),
            overall,
            categories,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealthSection {
    pub id: String,
    pub label: String,
    pub required: bool,
    pub state: SettingsHealthState,
    pub items: Vec<SettingsComponentHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemHealthSectionId {
    Rendering,
    LocalAi,
    Agent,
    Project,
    Environment,
}

impl SystemHealthSectionId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rendering => "rendering",
            Self::LocalAi => "localAi",
            Self::Agent => "agent",
            Self::Project => "project",
            Self::Environment => "environment",
        }
    }

    pub const fn uses_storage_inventory(self) -> bool {
        matches!(self, Self::Project | Self::Environment)
    }
}

impl TryFrom<&str> for SystemHealthSectionId {
    type Error = SettingsHealthError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "rendering" => Ok(Self::Rendering),
            "localAi" => Ok(Self::LocalAi),
            "agent" => Ok(Self::Agent),
            "project" => Ok(Self::Project),
            "environment" => Ok(Self::Environment),
            _ => Err(SettingsHealthError::UnknownSection(value.to_string())),
        }
    }
}

pub fn collect_system_health_inventory_if_needed<T>(
    section_id: SystemHealthSectionId,
    collect: impl FnOnce() -> T,
) -> Option<T> {
    section_id.uses_storage_inventory().then(collect)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealthSnapshot {
    pub generated_at: String,
    pub overall: SettingsHealthState,
    pub sections: BTreeMap<String, SystemHealthSection>,
}

impl SystemHealthSnapshot {
    pub fn from_sections(
        generated_at: impl Into<String>,
        sections: Vec<SystemHealthSection>,
    ) -> Self {
        let sections = sections
            .into_iter()
            .map(|section| (section.id.clone(), section))
            .collect::<BTreeMap<_, _>>();
        let overall = sections
            .values()
            .filter(|section| section.required)
            .map(|section| readiness_state(&section.state))
            .max_by_key(system_severity)
            .unwrap_or(SettingsHealthState::Ready);

        Self {
            generated_at: generated_at.into(),
            overall,
            sections,
        }
    }
}

fn severity(state: &SettingsHealthState) -> u8 {
    match state {
        SettingsHealthState::Ready => 0,
        SettingsHealthState::NotConfigured
        | SettingsHealthState::NotEnabled
        | SettingsHealthState::NotApplicable => 1,
        SettingsHealthState::Checking => 2,
        SettingsHealthState::Unavailable => 3,
        SettingsHealthState::ActionRequired | SettingsHealthState::NeedsAction => 4,
        SettingsHealthState::Failed => 5,
    }
}

fn system_severity(state: &SettingsHealthState) -> u8 {
    severity(state)
}

fn readiness_state(state: &SettingsHealthState) -> SettingsHealthState {
    match state {
        SettingsHealthState::NotConfigured
        | SettingsHealthState::NotEnabled
        | SettingsHealthState::NotApplicable => SettingsHealthState::Ready,
        state => state.clone(),
    }
}

#[derive(Debug, Error)]
pub enum SettingsHealthError {
    #[error("model store lock failed")]
    ModelStoreLock,
    #[error(transparent)]
    ModelStore(#[from] ModelStoreError),
    #[error("unknown system health section {0}")]
    UnknownSection(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsHealthCommandError {
    pub code: String,
    pub message: String,
    pub detail: String,
}

impl From<SettingsHealthError> for SettingsHealthCommandError {
    fn from(error: SettingsHealthError) -> Self {
        match error {
            SettingsHealthError::ModelStoreLock => Self {
                code: "settings.modelStoreUnavailable".to_string(),
                message: "Settings health is temporarily unavailable.".to_string(),
                detail: "model store lock failed".to_string(),
            },
            SettingsHealthError::ModelStore(error) => Self {
                code: "settings.modelCatalogUnavailable".to_string(),
                message: "Settings model health could not be checked.".to_string(),
                detail: error.to_string(),
            },
            SettingsHealthError::UnknownSection(section_id) => Self {
                code: "settings.systemHealth.unknownSection".to_string(),
                message: "That System Health section could not be refreshed.".to_string(),
                detail: format!("unknown system health section {section_id}"),
            },
        }
    }
}

pub fn build_system_health_snapshot(
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&std::path::Path>,
    storage_inventory: &[StorageInventoryItem],
    claude_executable_override: Option<&str>,
) -> SystemHealthSnapshot {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let sections = [
        SystemHealthSectionId::Rendering,
        SystemHealthSectionId::LocalAi,
        SystemHealthSectionId::Agent,
        SystemHealthSectionId::Project,
        SystemHealthSectionId::Environment,
    ]
    .into_iter()
    .map(|section_id| {
        build_system_health_section_by_id(
            section_id,
            model_state,
            configured_skill_root,
            storage_inventory,
            claude_executable_override,
        )
        .unwrap_or_else(|error| {
            failed_system_health_section(section_id.as_str(), &generated_at, error)
        })
    })
    .collect();

    SystemHealthSnapshot::from_sections(generated_at, sections)
}

pub fn build_system_health_section(
    section_id: &str,
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&std::path::Path>,
    storage_inventory: &[StorageInventoryItem],
    claude_executable_override: Option<&str>,
) -> Result<SystemHealthSection, SettingsHealthError> {
    build_system_health_section_by_id(
        SystemHealthSectionId::try_from(section_id)?,
        model_state,
        configured_skill_root,
        storage_inventory,
        claude_executable_override,
    )
}

pub fn build_system_health_section_by_id(
    section_id: SystemHealthSectionId,
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&std::path::Path>,
    storage_inventory: &[StorageInventoryItem],
    claude_executable_override: Option<&str>,
) -> Result<SystemHealthSection, SettingsHealthError> {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    match section_id {
        SystemHealthSectionId::Rendering => {
            Ok(rendering_system_health_section(get_render_system_health()))
        }
        SystemHealthSectionId::LocalAi => {
            let model_store = model_state
                .lock()
                .map_err(|_| SettingsHealthError::ModelStoreLock)?;
            local_ai_system_health_section(
                &model_store,
                &PlatformTranscriptionRuntime::default_helper_path(),
                &generated_at,
            )
        }
        SystemHealthSectionId::Agent => {
            let category = agent_category_health(
                probe_codex_app_server(),
                probe_claude_cli(claude_executable_override),
                probe_mcp_server(),
                probe_proposal_validator(),
            );
            // The category already knows the two backends are alternatives, so the
            // section reuses its verdict instead of rolling the items up again.
            let state = normalize_system_state(category.state);
            Ok(system_health_section(
                "agent",
                "Agent Runtime",
                false,
                state,
                category.items,
            ))
        }
        SystemHealthSectionId::Project => Ok(project_integrity_system_health_section(
            configured_skill_root,
            storage_inventory,
            &generated_at,
        )),
        SystemHealthSectionId::Environment => {
            let items = storage_category_health(storage_inventory, &generated_at)
                .items
                .into_iter()
                .filter(|item| {
                    item.provenance
                        .get("scope")
                        .is_some_and(|scope| !scope.starts_with("project"))
                })
                .collect::<Vec<_>>();
            let state = aggregate_system_items(&items, SettingsHealthState::Unavailable);
            Ok(system_health_section(
                "environment",
                "Environment",
                false,
                state,
                items,
            ))
        }
    }
}

const REQUIRED_RENDER_CAPABILITY_IDS: [&str; 4] = [
    "render.gstreamerGes",
    "render.pluginPolicy",
    crate::settings::render_system::native_delivery_capability().0,
    "render.compatibilityDecoder",
];

fn rendering_system_health_section(health: RenderSystemHealth) -> SystemHealthSection {
    let mut items = Vec::with_capacity(REQUIRED_RENDER_CAPABILITY_IDS.len());
    for required_id in REQUIRED_RENDER_CAPABILITY_IDS {
        if let Some(item) = health.items.iter().find(|item| item.id == required_id) {
            items.push(item.clone());
        } else {
            items.push(SettingsComponentHealth {
                id: required_id.to_string(),
                label: required_render_capability_label(required_id).to_string(),
                state: SettingsHealthState::Failed,
                summary: "Required rendering diagnostics did not report this capability."
                    .to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: health.checked_at.clone(),
                diagnostic_code: Some("render.requiredCapabilityMissing".to_string()),
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            });
        }
    }
    let state = aggregate_system_items(&items, SettingsHealthState::Failed);
    system_health_section("rendering", "Rendering", true, state, items)
}

fn required_render_capability_label(id: &str) -> &'static str {
    match id {
        "render.gstreamerGes" => "GStreamer / GES",
        "render.pluginPolicy" => "GStreamer plugin policy",
        "render.avfoundation" => "AVFoundation final delivery",
        "render.linuxDelivery" => "Linux codec delivery",
        "render.compatibilityDecoder" => "Compatibility decoder",
        _ => "Rendering capability",
    }
}

fn local_ai_system_health_section(
    store: &TranscriptionModelStore,
    helper_path: &std::path::Path,
    generated_at: &str,
) -> Result<SystemHealthSection, SettingsHealthError> {
    let status = store.active_status()?;
    let required = status.install_status != ModelInstallStatus::Missing;
    let mut model_item = model_component_health(status.clone(), generated_at);
    if !required {
        model_item.state = SettingsHealthState::NotConfigured;
        model_item.summary = "No active local transcription model is installed.".to_string();
    }
    let mut items = vec![model_item];

    if status.install_status == ModelInstallStatus::Ready {
        let entry = transcription_model_catalog_entry(&status.model_id)
            .ok_or_else(|| ModelStoreError::UnsupportedModel(status.model_id.clone()))?;
        let model_dir =
            store.runtime_model_dir(&status.model_id, PLATFORM_TRANSCRIPTION_RUNTIME_ID)?;
        let installed_model = InstalledModel {
            model_id: status.model_id.clone(),
            model_dir,
            artifact_format: ModelArtifactFormat::from(PLATFORM_TRANSCRIPTION_ARTIFACT_FORMAT),
            modality: ModelModality::Transcription,
            runtime_family: PLATFORM_TRANSCRIPTION_RUNTIME_ID.to_string(),
        };
        let runtime = PlatformTranscriptionRuntime::new(helper_path.to_path_buf());
        let capability = if runtime.supports_installed_model(&installed_model)
            && entry.artifact_format == PLATFORM_TRANSCRIPTION_ARTIFACT_FORMAT
        {
            runtime.probe_installed_model(&installed_model)
        } else {
            RuntimeCapability::Unavailable
        };
        let (state, summary, code) = match capability {
            RuntimeCapability::Ready => (
                SettingsHealthState::Ready,
                "The active local model and native helper are ready.",
                None,
            ),
            RuntimeCapability::Unavailable => (
                SettingsHealthState::Failed,
                "The active local model or native helper is unavailable.",
                Some("localAi.runtimeUnavailable".to_string()),
            ),
            RuntimeCapability::UnsupportedPlatform => (
                SettingsHealthState::Failed,
                "The active local model is not supported on this platform.",
                Some("localAi.runtimeUnsupported".to_string()),
            ),
        };
        items.push(SettingsComponentHealth {
            id: "localAi.runtime".to_string(),
            label: "Native transcription runtime".to_string(),
            state,
            summary: summary.to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: code,
            diagnostic_detail: runtime.readiness_diagnostic(&installed_model, &capability),
            provenance: BTreeMap::from([
                ("modelId".to_string(), status.model_id),
                (
                    "runtimeId".to_string(),
                    PLATFORM_TRANSCRIPTION_RUNTIME_ID.to_string(),
                ),
            ]),
        });
    }

    let empty_state = if required {
        SettingsHealthState::Unavailable
    } else {
        SettingsHealthState::NotConfigured
    };
    let state = aggregate_system_items(&items, empty_state);
    Ok(system_health_section(
        "localAi", "Local AI", required, state, items,
    ))
}

fn project_integrity_system_health_section(
    project_root: Option<&std::path::Path>,
    storage_inventory: &[StorageInventoryItem],
    generated_at: &str,
) -> SystemHealthSection {
    let mut storage_items = storage_category_health(storage_inventory, generated_at)
        .items
        .into_iter()
        .filter(|item| {
            item.provenance
                .get("scope")
                .is_some_and(|scope| scope.starts_with("project"))
        })
        .collect::<Vec<_>>();
    let Some(project_root) = project_root else {
        for item in &mut storage_items {
            item.state = SettingsHealthState::NotApplicable;
            item.summary = "Open a project to inspect this item.".to_string();
        }
        storage_items.insert(
            0,
            SettingsComponentHealth {
                id: "project.canonicalSplit".to_string(),
                label: "Canonical split project".to_string(),
                state: SettingsHealthState::NotApplicable,
                summary: "Open a project to validate its canonical files.".to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: generated_at.to_string(),
                diagnostic_code: None,
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            },
        );
        return system_health_section(
            "project",
            "Project Integrity",
            false,
            SettingsHealthState::NotApplicable,
            storage_items,
        );
    };

    let validation_item = match validate_split_project(project_root) {
        Ok(report) if report.ok => SettingsComponentHealth {
            id: "project.canonicalSplit".to_string(),
            label: "Canonical split project".to_string(),
            state: SettingsHealthState::Ready,
            summary: "Canonical project files and references are valid.".to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
            provenance: BTreeMap::new(),
        },
        Ok(report) => SettingsComponentHealth {
            id: "project.canonicalSplit".to_string(),
            label: "Canonical split project".to_string(),
            state: SettingsHealthState::Failed,
            summary: format!(
                "Canonical project validation found {} issue(s).",
                report.issues.len()
            ),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: Some("project.canonicalSplitInvalid".to_string()),
            diagnostic_detail: report.issues.first().map(|issue| issue.message.clone()),
            provenance: BTreeMap::from([(
                "issueCount".to_string(),
                report.issues.len().to_string(),
            )]),
        },
        Err(error) => SettingsComponentHealth {
            id: "project.canonicalSplit".to_string(),
            label: "Canonical split project".to_string(),
            state: SettingsHealthState::Failed,
            summary: "Canonical project validation could not complete.".to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: Some("project.canonicalSplitUnavailable".to_string()),
            diagnostic_detail: Some(error.to_string()),
            provenance: BTreeMap::new(),
        },
    };
    let state = validation_item.state.clone();
    let mut items = vec![validation_item];
    items.extend(
        skills_category_health_for_configured_root(
            None,
            &mandatory_skill_prompt_bundle(),
            generated_at,
        )
        .items,
    );
    items.append(&mut storage_items);
    system_health_section("project", "Project Integrity", true, state, items)
}

fn system_health_section(
    id: &str,
    label: &str,
    required: bool,
    state: SettingsHealthState,
    items: Vec<SettingsComponentHealth>,
) -> SystemHealthSection {
    SystemHealthSection {
        id: id.to_string(),
        label: label.to_string(),
        required,
        state: normalize_system_state(state),
        items: items
            .into_iter()
            .map(|mut item| {
                item.state = normalize_system_state(item.state);
                item
            })
            .collect(),
    }
}

fn aggregate_system_items(
    items: &[SettingsComponentHealth],
    empty_state: SettingsHealthState,
) -> SettingsHealthState {
    items
        .iter()
        .map(|item| &item.state)
        .max_by_key(|state| system_severity(state))
        .cloned()
        .map(normalize_system_state)
        .unwrap_or(empty_state)
}

fn normalize_system_state(state: SettingsHealthState) -> SettingsHealthState {
    match state {
        SettingsHealthState::ActionRequired => SettingsHealthState::NeedsAction,
        state => state,
    }
}

fn failed_system_health_section(
    section_id: &str,
    generated_at: &str,
    error: SettingsHealthError,
) -> SystemHealthSection {
    let command_error = SettingsHealthCommandError::from(error);
    let label = match section_id {
        "rendering" => "Rendering",
        "localAi" => "Local AI",
        "agent" => "Agent Runtime",
        "project" => "Project Integrity",
        "environment" => "Environment",
        _ => "System Health",
    };
    system_health_section(
        section_id,
        label,
        matches!(section_id, "rendering" | "localAi"),
        SettingsHealthState::Failed,
        vec![SettingsComponentHealth {
            id: format!("{section_id}.probe"),
            label: label.to_string(),
            state: SettingsHealthState::Failed,
            summary: command_error.message,
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: Some(command_error.code),
            diagnostic_detail: Some(command_error.detail),
            provenance: BTreeMap::new(),
        }],
    )
}

pub fn build_initial_settings_health_snapshot(
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&std::path::Path>,
    provider_health: &[ProviderHealth],
    claude_executable_override: Option<&str>,
) -> Result<SettingsHealthSnapshot, SettingsHealthError> {
    build_settings_health_snapshot(
        model_state,
        configured_skill_root,
        provider_health,
        None,
        claude_executable_override,
    )
}

pub fn build_settings_health_snapshot(
    model_state: &Mutex<TranscriptionModelStore>,
    configured_skill_root: Option<&std::path::Path>,
    provider_health: &[ProviderHealth],
    storage_inventory: Option<&[StorageInventoryItem]>,
    claude_executable_override: Option<&str>,
) -> Result<SettingsHealthSnapshot, SettingsHealthError> {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let model_store = model_state
        .lock()
        .map_err(|_| SettingsHealthError::ModelStoreLock)?;
    let models = model_store
        .list()?
        .into_iter()
        .map(|status| model_component_health(status, &generated_at))
        .collect();
    drop(model_store);
    let render_system = get_render_system_health();
    let agent = agent_category_health(
        probe_codex_app_server(),
        probe_claude_cli(claude_executable_override),
        probe_mcp_server(),
        probe_proposal_validator(),
    );
    let skills = skills_category_health_for_configured_root(
        configured_skill_root,
        &mandatory_skill_prompt_bundle(),
        &generated_at,
    );

    Ok(SettingsHealthSnapshot::from_categories(
        generated_at.clone(),
        vec![
            SettingsCategoryHealth::from_items_with_state(
                "general",
                render_system.state,
                render_system.items,
            ),
            SettingsCategoryHealth::from_items("models", models),
            agent,
            skills,
            storage_inventory.map_or_else(
                || {
                    unavailable_category(
                        "storage",
                        "Storage",
                        "Storage health requires an explicit inventory.",
                        &generated_at,
                    )
                },
                |inventory| storage_category_health(inventory, &generated_at),
            ),
            provider_category_health(provider_health, &generated_at),
        ],
    ))
}

pub fn get_agent_settings_health(
    claude_executable_override: Option<&str>,
) -> SettingsCategoryHealth {
    agent_category_health(
        probe_codex_app_server(),
        probe_claude_cli(claude_executable_override),
        probe_mcp_server(),
        probe_proposal_validator(),
    )
}

pub fn get_skills_settings_health(
    configured_skill_root: Option<&std::path::Path>,
) -> SettingsCategoryHealth {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    skills_category_health_for_configured_root(
        configured_skill_root,
        &mandatory_skill_prompt_bundle(),
        &generated_at,
    )
}

pub fn storage_category_health(
    inventory: &[StorageInventoryItem],
    generated_at: &str,
) -> SettingsCategoryHealth {
    let items = inventory
        .iter()
        .map(|item| {
            let (state, summary, diagnostic_code, diagnostic_detail) =
                match item.unavailable_reason.as_deref() {
                    Some("Open a project to inspect") => (
                        SettingsHealthState::Unavailable,
                        "Open a project to inspect".to_string(),
                        Some("storage.projectUnavailable".to_string()),
                        None,
                    ),
                    Some(reason) => (
                        SettingsHealthState::Failed,
                        "Storage usage could not be inspected.".to_string(),
                        Some("storage.inventoryUnavailable".to_string()),
                        Some(reason.to_string()),
                    ),
                    None if item.free_bytes.is_none() => (
                        SettingsHealthState::Failed,
                        "Volume free space could not be determined.".to_string(),
                        Some("storage.freeSpaceUnavailable".to_string()),
                        item.path.clone(),
                    ),
                    None => (
                        SettingsHealthState::Ready,
                        format!(
                            "{} bytes used; {} bytes available on this volume.",
                            item.bytes,
                            item.free_bytes.unwrap_or_default()
                        ),
                        None,
                        None,
                    ),
                };
            let mut provenance = BTreeMap::new();
            provenance.insert(
                "scope".to_string(),
                storage_scope_name(item.scope).to_string(),
            );
            provenance.insert("bytes".to_string(), item.bytes.to_string());
            provenance.insert("removable".to_string(), item.removable.to_string());
            if let Some(path) = &item.path {
                provenance.insert("path".to_string(), path.clone());
            }
            if let Some(free_bytes) = item.free_bytes {
                provenance.insert("freeBytes".to_string(), free_bytes.to_string());
            }
            if !item.artifact_ids.is_empty() {
                if let Ok(artifact_ids) = serde_json::to_string(&item.artifact_ids) {
                    provenance.insert("artifactIds".to_string(), artifact_ids);
                }
            }

            SettingsComponentHealth {
                id: item.id.clone(),
                label: storage_scope_label(item.scope).to_string(),
                state,
                summary,
                action_id: Some("storage.refreshInventory".to_string()),
                action_label: Some("Refresh usage".to_string()),
                last_checked_at: generated_at.to_string(),
                diagnostic_code,
                diagnostic_detail,
                provenance,
            }
        })
        .collect();
    SettingsCategoryHealth::from_items("storage", items)
}

pub fn provider_category_health(
    providers: &[ProviderHealth],
    generated_at: &str,
) -> SettingsCategoryHealth {
    let items = providers
        .iter()
        .map(|provider| {
            let group = provider.readiness_group();
            let unvalidated = provider.last_checked_at.is_none();
            let state = if unvalidated {
                SettingsHealthState::Checking
            } else if group == ProviderReadinessGroup::NeedsAttention {
                SettingsHealthState::ActionRequired
            } else {
                SettingsHealthState::Ready
            };
            let mut provenance = BTreeMap::new();
            provenance.insert(
                "group".to_string(),
                provider_readiness_group_name(group).to_string(),
            );
            provenance.insert(
                "credentialSource".to_string(),
                provider_credential_source_name(provider.credential_source).to_string(),
            );
            provenance.insert("configured".to_string(), provider.configured.to_string());
            provenance.insert(
                "validationState".to_string(),
                provider_validation_state_name(provider.validation_state).to_string(),
            );
            provenance.insert(
                "dependentModelIds".to_string(),
                provider.dependent_model_ids.join(","),
            );

            SettingsComponentHealth {
                id: format!("providers.{}", provider.provider),
                label: provider.display_name.clone(),
                state,
                summary: if unvalidated {
                    if provider.configured {
                        "Credential presence detected; live validation has not run.".to_string()
                    } else {
                        "Provider validation and model dependency checks have not run.".to_string()
                    }
                } else {
                    provider_health_summary(provider, group)
                },
                action_id: Some("providers.refresh".to_string()),
                action_label: Some(
                    if group == ProviderReadinessGroup::NeedsAttention && provider.configured {
                        "Retry validation".to_string()
                    } else {
                        "Refresh".to_string()
                    },
                ),
                last_checked_at: provider
                    .last_checked_at
                    .clone()
                    .unwrap_or_else(|| generated_at.to_string()),
                diagnostic_code: provider.diagnostic_code.clone(),
                diagnostic_detail: None,
                provenance,
            }
        })
        .collect();
    SettingsCategoryHealth::from_items("providers", items)
}

fn provider_health_summary(provider: &ProviderHealth, group: ProviderReadinessGroup) -> String {
    match group {
        ProviderReadinessGroup::Configured => match provider.validation_state {
            ProviderValidationState::Available => provider
                .balance_label
                .as_ref()
                .map(|balance| format!("Credential validated; balance {balance}."))
                .unwrap_or_else(|| "Credential validated.".to_string()),
            ProviderValidationState::BalanceUnavailable => {
                "Credential is configured; balance is not available from this provider.".to_string()
            }
            ProviderValidationState::NotChecked => {
                "Credential is configured; validation has not run yet.".to_string()
            }
            ProviderValidationState::Missing
            | ProviderValidationState::Rejected
            | ProviderValidationState::Unavailable => {
                "Provider credential needs attention.".to_string()
            }
        },
        ProviderReadinessGroup::NeedsAttention if !provider.configured => format!(
            "A credential is required by {} enabled model(s).",
            provider.dependent_model_ids.len()
        ),
        ProviderReadinessGroup::NeedsAttention => match provider.validation_state {
            ProviderValidationState::Rejected => {
                "The provider rejected the configured credential.".to_string()
            }
            ProviderValidationState::Unavailable => {
                "Provider validation is temporarily unavailable.".to_string()
            }
            ProviderValidationState::Missing
            | ProviderValidationState::NotChecked
            | ProviderValidationState::Available
            | ProviderValidationState::BalanceUnavailable => {
                "Provider credential needs attention.".to_string()
            }
        },
        ProviderReadinessGroup::Optional => {
            "No enabled generation model currently requires this provider.".to_string()
        }
    }
}

fn provider_readiness_group_name(group: ProviderReadinessGroup) -> &'static str {
    match group {
        ProviderReadinessGroup::Configured => "configured",
        ProviderReadinessGroup::NeedsAttention => "needsAttention",
        ProviderReadinessGroup::Optional => "optional",
    }
}

fn provider_credential_source_name(
    source: crate::provider_credentials::ProviderCredentialSource,
) -> &'static str {
    match source {
        crate::provider_credentials::ProviderCredentialSource::Keychain => "keychain",
        crate::provider_credentials::ProviderCredentialSource::Missing => "missing",
        crate::provider_credentials::ProviderCredentialSource::Unavailable => "unavailable",
    }
}

fn provider_validation_state_name(state: ProviderValidationState) -> &'static str {
    match state {
        ProviderValidationState::NotChecked => "notChecked",
        ProviderValidationState::Available => "available",
        ProviderValidationState::BalanceUnavailable => "balanceUnavailable",
        ProviderValidationState::Missing => "missing",
        ProviderValidationState::Rejected => "rejected",
        ProviderValidationState::Unavailable => "unavailable",
    }
}

fn storage_scope_name(scope: StorageScope) -> &'static str {
    match scope {
        StorageScope::GlobalModels => "globalModels",
        StorageScope::DisposableAppCache => "disposableAppCache",
        StorageScope::ProjectMedia => "projectMedia",
        StorageScope::ProjectTranscripts => "projectTranscripts",
        StorageScope::ProjectRenderArtifacts => "projectRenderArtifacts",
        StorageScope::ProjectWorkflowArtifacts => "projectWorkflowArtifacts",
    }
}

fn storage_scope_label(scope: StorageScope) -> &'static str {
    match scope {
        StorageScope::GlobalModels => "Global models",
        StorageScope::DisposableAppCache => "Disposable application cache",
        StorageScope::ProjectMedia => "Project media",
        StorageScope::ProjectTranscripts => "Project transcripts",
        StorageScope::ProjectRenderArtifacts => "Project render artifacts",
        StorageScope::ProjectWorkflowArtifacts => "Project workflow and log artifacts",
    }
}

/// The two agent backends are alternatives, not requirements, so the category
/// reports the pair as ready whenever *either* one is. An install with only
/// Claude — or only Codex — is a working install, not a broken one.
fn agent_category_health(
    codex: SettingsComponentHealth,
    claude: SettingsComponentHealth,
    mcp_server: SettingsComponentHealth,
    proposal_validator: SettingsComponentHealth,
) -> SettingsCategoryHealth {
    let backend_state = if codex.state == SettingsHealthState::Ready
        || claude.state == SettingsHealthState::Ready
    {
        SettingsHealthState::Ready
    } else {
        worst_state([&codex.state, &claude.state])
    };
    let state = worst_state([&backend_state, &mcp_server.state, &proposal_validator.state]);
    SettingsCategoryHealth::from_items_with_state(
        "agent",
        state,
        vec![codex, claude, mcp_server, proposal_validator],
    )
}

fn worst_state<'a>(
    states: impl IntoIterator<Item = &'a SettingsHealthState>,
) -> SettingsHealthState {
    states
        .into_iter()
        .max_by_key(|state| severity(state))
        .cloned()
        .unwrap_or(SettingsHealthState::Unavailable)
}

fn skills_category_health_for_configured_root(
    configured_root: Option<&std::path::Path>,
    prompt_bundle: &str,
    generated_at: &str,
) -> SettingsCategoryHealth {
    match validate_skill_root(configured_root) {
        Ok(root) => skills_category_health(&root, prompt_bundle, generated_at),
        Err(SkillRootError::Missing) => skill_root_health(
            SettingsHealthState::Unavailable,
            "No project repository is selected for skill verification.",
            "skills.rootMissing",
            "Open a project or provide its repository root before verifying or repairing skills.",
            None,
            generated_at,
        ),
        Err(error) => skill_root_health(
            SettingsHealthState::ActionRequired,
            "The configured project repository root is invalid.",
            "skills.rootInvalid",
            &error.to_string(),
            configured_root,
            generated_at,
        ),
    }
}

fn skill_root_health(
    state: SettingsHealthState,
    summary: &str,
    diagnostic_code: &str,
    diagnostic_detail: &str,
    configured_root: Option<&std::path::Path>,
    generated_at: &str,
) -> SettingsCategoryHealth {
    let mut provenance = BTreeMap::new();
    if let Some(root) = configured_root {
        provenance.insert("configuredRoot".to_string(), root.display().to_string());
    }
    SettingsCategoryHealth::from_items(
        "skills",
        vec![SettingsComponentHealth {
            id: "skills.root".to_string(),
            label: "Project skills".to_string(),
            state,
            summary: summary.to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: Some(diagnostic_code.to_string()),
            diagnostic_detail: Some(diagnostic_detail.to_string()),
            provenance,
        }],
    )
}

fn skills_category_health(
    project_root: &std::path::Path,
    prompt_bundle: &str,
    generated_at: &str,
) -> SettingsCategoryHealth {
    let items = verify_bundled_skills(project_root, prompt_bundle)
        .into_iter()
        .map(|verification| {
            let (state, summary, diagnostic_code, diagnostic_detail) =
                if !verification.prompt_included {
                    (
                        SettingsHealthState::Failed,
                        "Mandatory skill is not included in compiled agent context.",
                        Some("skills.promptMissing".to_string()),
                        Some(format!(
                            "{} is absent from the compiled developer instructions.",
                            verification.id
                        )),
                    )
                } else {
                    match verification.load_state {
                        SkillLoadState::MatchesBundled => (
                            SettingsHealthState::Ready,
                            "Mandatory skill matches the bundled version.",
                            Some("skills.ready".to_string()),
                            None,
                        ),
                        SkillLoadState::Missing => (
                            SettingsHealthState::ActionRequired,
                            "Mandatory skill is missing from disk.",
                            Some("skills.missing".to_string()),
                            Some(format!("{} is missing.", verification.path.display())),
                        ),
                        SkillLoadState::Differs => (
                            SettingsHealthState::ActionRequired,
                            "Mandatory skill differs from the bundled version.",
                            Some("skills.checksumMismatch".to_string()),
                            Some(format!(
                                "{} does not match its bundled checksum.",
                                verification.path.display()
                            )),
                        ),
                        SkillLoadState::Unreadable => (
                            SettingsHealthState::Failed,
                            "Mandatory skill could not be read.",
                            Some("skills.unreadable".to_string()),
                            Some(format!("{} is unreadable.", verification.path.display())),
                        ),
                    }
                };
            let mut provenance = BTreeMap::new();
            provenance.insert(
                "checksum".to_string(),
                verification
                    .checksum
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
            );
            provenance.insert("path".to_string(), verification.path.display().to_string());
            provenance.insert("bundledChecksum".to_string(), verification.bundled_checksum);
            provenance.insert(
                "loadState".to_string(),
                verification.load_state.as_str().to_string(),
            );
            provenance.insert(
                "promptIncluded".to_string(),
                verification.prompt_included.to_string(),
            );

            SettingsComponentHealth {
                id: verification.id,
                label: verification.label,
                state: state.clone(),
                summary: summary.to_string(),
                action_id: (state == SettingsHealthState::ActionRequired)
                    .then(|| "skills.repairBundled".to_string()),
                action_label: (state == SettingsHealthState::ActionRequired)
                    .then(|| "Repair bundled skill".to_string()),
                last_checked_at: generated_at.to_string(),
                diagnostic_code,
                diagnostic_detail,
                provenance,
            }
        })
        .collect();

    SettingsCategoryHealth::from_items("skills", items)
}

fn model_component_health(
    status: TranscriptionModelStatus,
    generated_at: &str,
) -> SettingsComponentHealth {
    let (state, summary, diagnostic_code, diagnostic_detail) = match status.install_status {
        ModelInstallStatus::Missing => (
            SettingsHealthState::ActionRequired,
            "Transcription model is not installed.",
            Some("models.modelMissing".to_string()),
            Some(format!(
                "{} is available in the model catalog but missing from {}.",
                status.display_name, status.local_path
            )),
        ),
        ModelInstallStatus::Downloading => (
            SettingsHealthState::Checking,
            "Transcription model download is in progress.",
            Some("models.modelDownloading".to_string()),
            Some(format!(
                "{} of {} model files downloaded.",
                status.downloaded_files, status.total_files
            )),
        ),
        ModelInstallStatus::Verifying => (
            SettingsHealthState::Checking,
            "Transcription model verification is in progress.",
            Some("models.modelVerifying".to_string()),
            None,
        ),
        ModelInstallStatus::Ready => (
            SettingsHealthState::Ready,
            "Transcription model is installed and verified.",
            None,
            None,
        ),
        ModelInstallStatus::Failed => (
            SettingsHealthState::Failed,
            "Transcription model installation failed verification.",
            Some("models.modelFailed".to_string()),
            Some(format!(
                "The installed model at {} is incomplete or invalid.",
                status.local_path
            )),
        ),
    };
    let mut provenance = BTreeMap::new();
    provenance.insert(
        "source".to_string(),
        "transcriptionModelCatalog".to_string(),
    );
    provenance.insert("modelId".to_string(), status.model_id.clone());
    provenance.insert(
        "installStatus".to_string(),
        model_install_status_name(status.install_status).to_string(),
    );
    provenance.insert("localPath".to_string(), status.local_path);

    SettingsComponentHealth {
        id: status.model_id,
        label: status.display_name,
        state,
        summary: summary.to_string(),
        action_id: None,
        action_label: None,
        last_checked_at: generated_at.to_string(),
        diagnostic_code,
        diagnostic_detail,
        provenance,
    }
}

fn unavailable_category(
    id: &str,
    label: &str,
    summary: &str,
    generated_at: &str,
) -> SettingsCategoryHealth {
    SettingsCategoryHealth::from_items(
        id,
        vec![SettingsComponentHealth {
            id: format!("{id}.health"),
            label: label.to_string(),
            state: SettingsHealthState::Unavailable,
            summary: summary.to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: generated_at.to_string(),
            diagnostic_code: Some(format!("{id}.healthUnavailable")),
            diagnostic_detail: None,
            provenance: BTreeMap::new(),
        }],
    )
}

fn model_install_status_name(status: ModelInstallStatus) -> &'static str {
    match status {
        ModelInstallStatus::Missing => "missing",
        ModelInstallStatus::Downloading => "downloading",
        ModelInstallStatus::Verifying => "verifying",
        ModelInstallStatus::Ready => "ready",
        ModelInstallStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        agent_category_health, build_settings_health_snapshot,
        collect_system_health_inventory_if_needed, failed_system_health_section,
        local_ai_system_health_section, project_integrity_system_health_section,
        provider_category_health, rendering_system_health_section, skills_category_health,
        skills_category_health_for_configured_root, storage_category_health,
        SettingsCategoryHealth, SettingsComponentHealth, SettingsHealthCommandError,
        SettingsHealthError, SettingsHealthSnapshot, SettingsHealthState, SystemHealthSection,
        SystemHealthSectionId, SystemHealthSnapshot,
    };
    use crate::project::fixtures::sample_project;
    use crate::project::split::save_split_project;
    use crate::provider_credentials::ProviderCredentialSource;
    use crate::settings::providers::{ProviderHealth, ProviderValidationState};
    use crate::settings::render_system::RenderSystemHealth;
    use crate::settings::storage::{StorageInventoryItem, StorageScope};
    use crate::transcription::model::{
        parakeet_v3_catalog_entry, required_files_dir_for_entry, safe_model_dir_name,
        InstalledModelManifest, ModelInstallStatus,
    };
    use crate::transcription::store::TranscriptionModelStore;
    use crate::transcription::store::MODEL_MANIFEST_FILE_NAME;

    const TEST_TIMESTAMP: &str = "2026-07-19T12:00:00Z";

    fn health_item(id: &str, state: SettingsHealthState) -> SettingsComponentHealth {
        SettingsComponentHealth {
            id: id.to_string(),
            label: id.to_string(),
            state,
            summary: "Fixture state.".to_string(),
            action_id: None,
            action_label: None,
            last_checked_at: TEST_TIMESTAMP.to_string(),
            diagnostic_code: None,
            diagnostic_detail: None,
            provenance: BTreeMap::new(),
        }
    }

    fn render_health_with_item_state(
        item_id: &str,
        item_state: SettingsHealthState,
    ) -> RenderSystemHealth {
        let item = |id: &str| {
            health_item(
                id,
                if id == item_id {
                    item_state.clone()
                } else {
                    SettingsHealthState::Ready
                },
            )
        };
        RenderSystemHealth {
            checked_at: TEST_TIMESTAMP.to_string(),
            state: SettingsHealthState::Ready,
            composition_ready: true,
            native_delivery_degraded: item_id
                == crate::settings::render_system::native_delivery_capability().0,
            compatibility_degraded: item_id == "render.compatibilityDecoder",
            items: vec![
                item("render.gstreamerGes"),
                item("render.pluginPolicy"),
                item(crate::settings::render_system::native_delivery_capability().0),
                item("render.compatibilityDecoder"),
            ],
        }
    }

    fn materialize_ready_active_model(root: &std::path::Path) -> TranscriptionModelStore {
        // Fixture bytes stand in for real weights, so drop any pinned sizes/hashes while keeping
        // the platform's artifact format and runtime.
        let mut entry = parakeet_v3_catalog_entry();
        for file in &mut entry.required_files {
            file.size_bytes = None;
            file.sha256 = None;
        }
        crate::transcription::model::override_catalog_for_current_test_thread(vec![entry.clone()]);
        let model_dir = root.join(safe_model_dir_name(entry.id));
        let runtime_dir = required_files_dir_for_entry(&entry, &model_dir);
        std::fs::create_dir_all(&runtime_dir).expect("runtime model dir");
        for required in &entry.required_files {
            let path = runtime_dir.join(&required.path);
            std::fs::create_dir_all(path.parent().expect("required file parent"))
                .expect("required file parent");
            std::fs::write(path, b"fixture").expect("required model file");
        }
        std::fs::write(
            runtime_dir.join(crate::transcription::store::COREML_INSPECTION_FILE_NAME),
            b"{}",
        )
        .expect("inspection report");
        let manifest = InstalledModelManifest {
            schema_version: 1,
            model_id: entry.id.to_string(),
            revision: entry.revision.to_string(),
            modality: Some(entry.modality),
            artifact_format: Some(entry.artifact_format),
            source: None,
            installed_files: entry
                .required_files
                .iter()
                .cloned()
                .map(|mut file| {
                    file.size_bytes = Some(7);
                    file.sha256 = Some(
                        "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d"
                            .to_string(),
                    );
                    file
                })
                .collect(),
            installed_at: TEST_TIMESTAMP.to_string(),
            verified_at: Some(TEST_TIMESTAMP.to_string()),
        };
        std::fs::create_dir_all(&model_dir).expect("model dir");
        std::fs::write(
            model_dir.join(MODEL_MANIFEST_FILE_NAME),
            serde_json::to_vec(&manifest).expect("manifest json"),
        )
        .expect("model manifest");

        let store = TranscriptionModelStore::new(root.to_path_buf());
        assert_eq!(
            store.active_status().expect("active model").install_status,
            ModelInstallStatus::Ready
        );
        store
    }

    fn system_section(id: &str, state: SettingsHealthState, required: bool) -> SystemHealthSection {
        SystemHealthSection {
            id: id.to_string(),
            label: id.to_string(),
            required,
            state,
            items: Vec::new(),
        }
    }

    #[test]
    fn optional_and_context_items_do_not_lower_overall_readiness() {
        let snapshot = SystemHealthSnapshot::from_sections(
            "2026-07-19T12:00:00Z",
            vec![
                system_section("rendering", SettingsHealthState::Ready, true),
                system_section("agent", SettingsHealthState::Failed, false),
                system_section("project", SettingsHealthState::NotApplicable, false),
            ],
        );

        assert_eq!(snapshot.overall, SettingsHealthState::Ready);
    }

    #[test]
    fn neutral_required_state_does_not_lower_overall_readiness() {
        let snapshot = SystemHealthSnapshot::from_sections(
            "2026-07-19T12:00:00Z",
            vec![system_section(
                "localAi",
                SettingsHealthState::NotConfigured,
                true,
            )],
        );

        assert_eq!(snapshot.overall, SettingsHealthState::Ready);
    }

    #[test]
    fn native_delivery_failure_cannot_leave_production_system_health_ready() {
        let rendering = rendering_system_health_section(render_health_with_item_state(
            crate::settings::render_system::native_delivery_capability().0,
            SettingsHealthState::Failed,
        ));
        let snapshot = SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![rendering]);

        assert_eq!(
            snapshot.sections["rendering"].state,
            SettingsHealthState::Failed
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Failed);
    }

    #[test]
    fn corrupt_active_model_is_required_and_fails_local_ai_readiness() {
        let root = tempfile::tempdir().expect("model root");
        let entry = parakeet_v3_catalog_entry();
        let model_dir = root.path().join(safe_model_dir_name(entry.id));
        std::fs::create_dir_all(&model_dir).expect("model dir");
        std::fs::write(
            model_dir.join(MODEL_MANIFEST_FILE_NAME),
            b"invalid manifest",
        )
        .expect("corrupt manifest");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());

        let section = local_ai_system_health_section(
            &store,
            &root.path().join("missing-helper"),
            TEST_TIMESTAMP,
        )
        .expect("local AI section");

        assert!(section.required);
        assert_eq!(section.state, SettingsHealthState::Failed);
        assert_eq!(section.items.len(), 1);
        assert_eq!(section.items[0].id, entry.id);
    }

    #[test]
    fn missing_active_helper_fails_an_installed_local_ai_capability() {
        let root = tempfile::tempdir().expect("model root");
        let store = materialize_ready_active_model(root.path());

        let section = local_ai_system_health_section(
            &store,
            &root.path().join("missing-helper"),
            TEST_TIMESTAMP,
        )
        .expect("local AI section");

        assert!(section.required);
        assert_eq!(section.state, SettingsHealthState::Failed);
        assert_eq!(section.items.len(), 2);
        assert_eq!(section.items[1].id, "localAi.runtime");
        assert_eq!(section.items[1].state, SettingsHealthState::Failed);
    }

    #[test]
    fn unreadable_local_ai_capability_cannot_be_readiness_neutral() {
        let section = failed_system_health_section(
            "localAi",
            TEST_TIMESTAMP,
            SettingsHealthError::ModelStoreLock,
        );
        let snapshot = SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![section]);

        assert!(snapshot.sections["localAi"].required);
        assert_eq!(
            snapshot.sections["localAi"].state,
            SettingsHealthState::Failed
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Failed);
    }

    #[test]
    fn invalid_open_split_project_is_required_and_fails_integrity() {
        let project_root = tempfile::tempdir().expect("project root");

        let section =
            project_integrity_system_health_section(Some(project_root.path()), &[], TEST_TIMESTAMP);
        let snapshot = SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![section]);

        assert!(snapshot.sections["project"].required);
        assert_eq!(
            snapshot.sections["project"].state,
            SettingsHealthState::Failed
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Failed);
        assert!(snapshot.sections["project"]
            .items
            .iter()
            .any(|item| item.id == "project.canonicalSplit"));
        assert!(snapshot.sections["project"]
            .items
            .iter()
            .any(|item| item.id.starts_with("skills.")));
    }

    #[test]
    fn valid_ordinary_project_without_repository_guidance_is_ready() {
        let project_root = tempfile::tempdir().expect("project root");
        save_split_project(project_root.path(), &sample_project()).expect("valid split project");

        let section =
            project_integrity_system_health_section(Some(project_root.path()), &[], TEST_TIMESTAMP);
        let snapshot = SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![section]);

        assert!(snapshot.sections["project"].required);
        assert_eq!(
            snapshot.sections["project"].state,
            SettingsHealthState::Ready
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Ready);
        let guidance = snapshot.sections["project"]
            .items
            .iter()
            .find(|item| item.id == "skills.root")
            .expect("visible repository guidance diagnostic");
        assert_eq!(
            guidance.diagnostic_code.as_deref(),
            Some("skills.rootMissing")
        );
        assert!(!guidance.provenance.contains_key("configuredRoot"));
    }

    #[test]
    fn optional_project_storage_failure_does_not_lower_project_readiness() {
        let project_root = tempfile::tempdir().expect("project root");
        save_split_project(project_root.path(), &sample_project()).expect("valid split project");
        let storage = StorageInventoryItem {
            id: "storage.projectMedia".to_string(),
            scope: StorageScope::ProjectMedia,
            path: Some(project_root.path().join("media").display().to_string()),
            bytes: 0,
            free_bytes: None,
            removable: false,
            unavailable_reason: Some("fixture inventory failure".to_string()),
            artifact_ids: Vec::new(),
        };

        let section = project_integrity_system_health_section(
            Some(project_root.path()),
            &[storage],
            TEST_TIMESTAMP,
        );
        let snapshot = SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![section]);

        assert_eq!(
            snapshot.sections["project"].state,
            SettingsHealthState::Ready
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Ready);
        assert!(snapshot.sections["project"]
            .items
            .iter()
            .any(|item| item.id == "storage.projectMedia"
                && item.state == SettingsHealthState::Failed));
    }

    #[test]
    fn absent_local_ai_and_project_context_remain_readiness_neutral() {
        let root = tempfile::tempdir().expect("model root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let local_ai = local_ai_system_health_section(
            &store,
            &root.path().join("missing-helper"),
            TEST_TIMESTAMP,
        )
        .expect("local AI section");
        let project = project_integrity_system_health_section(None, &[], TEST_TIMESTAMP);
        let rendering = rendering_system_health_section(render_health_with_item_state(
            "none",
            SettingsHealthState::Failed,
        ));
        let snapshot =
            SystemHealthSnapshot::from_sections(TEST_TIMESTAMP, vec![rendering, local_ai, project]);

        assert!(!snapshot.sections["localAi"].required);
        assert_eq!(
            snapshot.sections["localAi"].state,
            SettingsHealthState::NotConfigured
        );
        assert!(!snapshot.sections["project"].required);
        assert_eq!(
            snapshot.sections["project"].state,
            SettingsHealthState::NotApplicable
        );
        assert_eq!(snapshot.overall, SettingsHealthState::Ready);
    }

    #[test]
    fn non_storage_section_refreshes_do_not_collect_inventory() {
        let calls = std::cell::Cell::new(0);
        for section_id in [
            SystemHealthSectionId::Rendering,
            SystemHealthSectionId::LocalAi,
            SystemHealthSectionId::Agent,
        ] {
            let inventory = collect_system_health_inventory_if_needed(section_id, || {
                calls.set(calls.get() + 1);
                vec!["walked"]
            });
            assert!(inventory.is_none());
        }

        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn project_and_environment_refreshes_collect_inventory_once_each() {
        let calls = std::cell::Cell::new(0);
        for section_id in [
            SystemHealthSectionId::Project,
            SystemHealthSectionId::Environment,
        ] {
            let inventory = collect_system_health_inventory_if_needed(section_id, || {
                calls.set(calls.get() + 1);
                vec!["walked"]
            });
            assert_eq!(inventory, Some(vec!["walked"]));
        }

        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn production_snapshot_builder_assembles_every_category_from_real_inputs() {
        let root = tempfile::tempdir().expect("snapshot root");
        let skills_root = root.path().join("project");
        std::fs::create_dir_all(&skills_root).expect("skills root");
        std::fs::write(skills_root.join("AGENTS.md"), b"# Fixture\n").expect("agents file");
        for definition in crate::settings::skills::MANDATORY_SKILLS {
            let path = skills_root.join(definition.relative_path);
            std::fs::create_dir_all(path.parent().expect("skill parent"))
                .expect("create skill parent");
            std::fs::write(path, definition.bundled_content).expect("write skill");
        }
        let models =
            std::sync::Mutex::new(TranscriptionModelStore::new(root.path().join("models")));
        let inventory = vec![StorageInventoryItem {
            id: "storage.globalModels".to_string(),
            scope: StorageScope::GlobalModels,
            path: Some(root.path().join("models").display().to_string()),
            bytes: 0,
            free_bytes: Some(1024),
            removable: false,
            unavailable_reason: None,
            artifact_ids: Vec::new(),
        }];

        let snapshot = build_settings_health_snapshot(
            &models,
            Some(&skills_root),
            &[],
            Some(&inventory),
            Some(MISSING_CLAUDE_PATH),
        )
        .expect("production snapshot");

        assert_eq!(
            snapshot
                .categories
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec![
                "agent",
                "general",
                "models",
                "providers",
                "skills",
                "storage"
            ]
        );
        assert!(!snapshot.categories["general"].items.is_empty());
        assert!(!snapshot.categories["agent"].items.is_empty());
        assert_eq!(snapshot.categories["models"].items.len(), 1);
        assert_eq!(snapshot.categories["skills"].items.len(), 3);
        assert_eq!(snapshot.categories["storage"].items.len(), 1);
        assert_eq!(
            snapshot.categories["storage"].items[0].id,
            "storage.globalModels"
        );
    }

    /// Keeps health tests off any real `claude` install.
    const MISSING_CLAUDE_PATH: &str = "/tmp/video-creater-missing-claude-cli/claude";

    fn agent_component(
        id: &str,
        state: SettingsHealthState,
        diagnostic_code: &str,
    ) -> SettingsComponentHealth {
        SettingsComponentHealth {
            id: id.to_string(),
            label: id.to_string(),
            state,
            summary: format!("{id} fixture."),
            action_id: None,
            action_label: None,
            last_checked_at: "2026-09-18T12:00:00Z".to_string(),
            diagnostic_code: Some(diagnostic_code.to_string()),
            diagnostic_detail: None,
            provenance: BTreeMap::new(),
        }
    }

    fn agent_category_with_agents(
        codex: SettingsHealthState,
        claude: SettingsHealthState,
    ) -> SettingsCategoryHealth {
        agent_category_health(
            agent_component("agent.codex", codex, "agent.codex.fixture"),
            agent_component("agent.claude", claude, "agent.claude.fixture"),
            agent_component(
                "agent.mcpServer",
                SettingsHealthState::Ready,
                "agent.mcp.ready",
            ),
            agent_component(
                "agent.proposalValidator",
                SettingsHealthState::Ready,
                "agent.proposalValidator.ready",
            ),
        )
    }

    #[test]
    fn the_agent_category_lists_claude_after_codex() {
        let category =
            agent_category_with_agents(SettingsHealthState::Ready, SettingsHealthState::Ready);

        assert_eq!(
            category
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "agent.codex",
                "agent.claude",
                "agent.mcpServer",
                "agent.proposalValidator"
            ]
        );
    }

    #[test]
    fn the_agent_category_is_ready_when_either_agent_is_ready() {
        assert_eq!(
            agent_category_with_agents(
                SettingsHealthState::ActionRequired,
                SettingsHealthState::Ready
            )
            .state,
            SettingsHealthState::Ready
        );
        assert_eq!(
            agent_category_with_agents(
                SettingsHealthState::Ready,
                SettingsHealthState::NotConfigured
            )
            .state,
            SettingsHealthState::Ready
        );
    }

    #[test]
    fn the_agent_category_reports_the_loudest_problem_when_no_agent_is_ready() {
        assert_eq!(
            agent_category_with_agents(
                SettingsHealthState::ActionRequired,
                SettingsHealthState::NotConfigured
            )
            .state,
            SettingsHealthState::ActionRequired
        );
    }

    #[test]
    fn a_failing_support_component_still_breaks_the_agent_category() {
        let category = agent_category_health(
            agent_component(
                "agent.codex",
                SettingsHealthState::Ready,
                "agent.codex.ready",
            ),
            agent_component(
                "agent.claude",
                SettingsHealthState::Ready,
                "agent.claude.ready",
            ),
            agent_component(
                "agent.mcpServer",
                SettingsHealthState::Failed,
                "agent.mcp.launchFailed",
            ),
            agent_component(
                "agent.proposalValidator",
                SettingsHealthState::Ready,
                "agent.proposalValidator.ready",
            ),
        );

        assert_eq!(category.state, SettingsHealthState::Failed);
    }

    #[test]
    fn agent_health_keeps_mcp_and_validator_ready_when_codex_is_missing() {
        let category = agent_category_health(
            SettingsComponentHealth {
                id: "agent.codex".to_string(),
                label: "Codex app-server".to_string(),
                state: SettingsHealthState::NotConfigured,
                summary: "The bundled Codex runtime is missing from this app installation. It is optional: turns can run on Claude instead."
                    .to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: "2026-07-17T12:00:00Z".to_string(),
                diagnostic_code: Some("agent.codex.missing".to_string()),
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            },
            agent_component(
                "agent.claude",
                SettingsHealthState::NotConfigured,
                "agent.claude.missing",
            ),
            SettingsComponentHealth {
                id: "agent.mcpServer".to_string(),
                label: "Video Creater MCP server".to_string(),
                state: SettingsHealthState::Ready,
                summary: "MCP initialize handshake succeeded.".to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: "2026-07-17T12:00:00Z".to_string(),
                diagnostic_code: Some("agent.mcp.ready".to_string()),
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            },
            SettingsComponentHealth {
                id: "agent.proposalValidator".to_string(),
                label: "Proposal validation".to_string(),
                state: SettingsHealthState::Ready,
                summary: "Proposal fixtures passed.".to_string(),
                action_id: None,
                action_label: None,
                last_checked_at: "2026-07-17T12:00:00Z".to_string(),
                diagnostic_code: Some("agent.proposalValidator.ready".to_string()),
                diagnostic_detail: None,
                provenance: BTreeMap::new(),
            },
        );

        assert_eq!(
            category
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "agent.codex",
                "agent.claude",
                "agent.mcpServer",
                "agent.proposalValidator"
            ]
        );
        // Neither backend is installed, so the category is "not configured" rather than
        // "action required": both rows are now a configuration state, because neither agent
        // is a requirement and a missing one is not a broken install.
        assert_eq!(category.items[0].state, SettingsHealthState::NotConfigured);
        assert_eq!(category.items[1].state, SettingsHealthState::NotConfigured);
        assert_eq!(category.items[2].state, SettingsHealthState::Ready);
        assert_eq!(category.items[3].state, SettingsHealthState::Ready);
        assert_eq!(category.state, SettingsHealthState::NotConfigured);

        let snapshot =
            SettingsHealthSnapshot::from_categories("2026-07-17T12:00:00Z", vec![category]);
        assert_eq!(snapshot.overall, SettingsHealthState::NotConfigured);
    }

    #[test]
    fn snapshot_uses_camel_case_and_rolls_up_the_worst_state() {
        let snapshot = SettingsHealthSnapshot::from_categories(
            "2026-07-16T12:00:00Z",
            vec![
                SettingsCategoryHealth::ready("models"),
                SettingsCategoryHealth::action_required("storage", "storage.lowSpace"),
            ],
        );

        assert_eq!(snapshot.overall, SettingsHealthState::ActionRequired);
        let json = serde_json::to_value(snapshot).expect("serialize");
        assert_eq!(json["generatedAt"], "2026-07-16T12:00:00Z");
        assert_eq!(json["categories"]["storage"]["state"], "actionRequired");
    }

    #[test]
    fn storage_health_preserves_ready_global_rows_and_unavailable_project_rows() {
        let inventory = vec![
            StorageInventoryItem {
                id: "storage.globalModels".to_string(),
                scope: StorageScope::GlobalModels,
                path: Some("/tmp/models".to_string()),
                bytes: 42,
                free_bytes: Some(1_024),
                removable: false,
                unavailable_reason: None,
                artifact_ids: Vec::new(),
            },
            StorageInventoryItem {
                id: "storage.projectMedia".to_string(),
                scope: StorageScope::ProjectMedia,
                path: None,
                bytes: 0,
                free_bytes: None,
                removable: false,
                unavailable_reason: Some("Open a project to inspect".to_string()),
                artifact_ids: Vec::new(),
            },
        ];

        let health = storage_category_health(&inventory, "2026-07-17T12:00:00Z");

        assert_eq!(health.id, "storage");
        assert_eq!(health.state, SettingsHealthState::Unavailable);
        assert_eq!(health.items[0].state, SettingsHealthState::Ready);
        assert_eq!(health.items[0].provenance["bytes"], "42");
        assert_eq!(health.items[0].provenance["freeBytes"], "1024");
        assert_eq!(health.items[1].state, SettingsHealthState::Unavailable);
        assert_eq!(health.items[1].summary, "Open a project to inspect");
        assert_eq!(
            health.items[1].diagnostic_code.as_deref(),
            Some("storage.projectUnavailable")
        );
    }

    #[test]
    fn storage_health_exposes_render_artifact_selection_ids() {
        let health = storage_category_health(
            &[StorageInventoryItem {
                id: "storage.projectRenderArtifacts".to_string(),
                scope: StorageScope::ProjectRenderArtifacts,
                path: Some("/tmp/project/renders".to_string()),
                bytes: 42,
                free_bytes: Some(1_024),
                removable: true,
                unavailable_reason: None,
                artifact_ids: vec!["draft-001".to_string(), "final-002".to_string()],
            }],
            "2026-07-17T12:00:00Z",
        );

        assert_eq!(
            health.items[0].provenance["artifactIds"],
            r#"["draft-001","final-002"]"#,
        );
    }

    #[test]
    fn provider_health_rolls_up_attention_without_penalizing_optional_providers() {
        let health = vec![
            ProviderHealth {
                provider: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                credential_source: ProviderCredentialSource::Keychain,
                configured: true,
                validation_state: ProviderValidationState::BalanceUnavailable,
                account_label: None,
                balance_label: None,
                dependent_model_ids: vec!["openai:gpt-image-2".to_string()],
                last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                diagnostic_code: None,
            },
            ProviderHealth {
                provider: "google".to_string(),
                display_name: "Google".to_string(),
                credential_source: ProviderCredentialSource::Missing,
                configured: false,
                validation_state: ProviderValidationState::Missing,
                account_label: None,
                balance_label: None,
                dependent_model_ids: vec!["google:veo3.1-fast".to_string()],
                last_checked_at: Some("2026-07-17T12:00:00Z".to_string()),
                diagnostic_code: Some("providers.credentialMissing".to_string()),
            },
            ProviderHealth {
                provider: "minimax".to_string(),
                display_name: "MiniMax".to_string(),
                credential_source: ProviderCredentialSource::Missing,
                configured: false,
                validation_state: ProviderValidationState::Missing,
                account_label: None,
                balance_label: None,
                dependent_model_ids: Vec::new(),
                last_checked_at: None,
                diagnostic_code: None,
            },
        ];

        let category = provider_category_health(&health, "2026-07-17T12:00:00Z");

        assert_eq!(category.id, "providers");
        assert_eq!(category.state, SettingsHealthState::ActionRequired);
        assert_eq!(category.items[0].state, SettingsHealthState::Ready);
        assert_eq!(category.items[0].provenance["group"], "configured");
        assert_eq!(category.items[1].state, SettingsHealthState::ActionRequired);
        assert_eq!(category.items[1].provenance["group"], "needsAttention");
        assert_eq!(category.items[2].state, SettingsHealthState::Checking);
        assert_eq!(category.items[2].provenance["group"], "optional");
    }

    #[test]
    fn command_error_serializes_stable_code_message_and_detail() {
        let error = SettingsHealthCommandError::from(SettingsHealthError::ModelStoreLock);

        let json = serde_json::to_value(error).expect("serialize command error");
        assert_eq!(json["code"], "settings.modelStoreUnavailable");
        assert_eq!(
            json["message"],
            "Settings health is temporarily unavailable."
        );
        assert_eq!(json["detail"], "model store lock failed");
    }

    #[test]
    fn skills_health_returns_checksum_path_load_and_prompt_provenance() {
        let root = tempfile::tempdir().expect("temp root");
        for definition in crate::settings::skills::MANDATORY_SKILLS {
            let path = root.path().join(definition.relative_path);
            std::fs::create_dir_all(path.parent().expect("skill parent"))
                .expect("create skill parent");
            std::fs::write(path, definition.bundled_content).expect("write skill");
        }

        let category = skills_category_health(
            root.path(),
            &crate::settings::skills::mandatory_skill_prompt_bundle(),
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(category.state, SettingsHealthState::Ready);
        assert_eq!(category.items.len(), 3);
        for item in category.items {
            assert_eq!(item.state, SettingsHealthState::Ready);
            assert!(item.provenance.contains_key("checksum"));
            assert!(item.provenance.contains_key("path"));
            assert!(item.provenance.contains_key("bundledChecksum"));
            assert_eq!(
                item.provenance.get("loadState").map(String::as_str),
                Some("matchesBundled")
            );
            assert_eq!(
                item.provenance.get("promptIncluded").map(String::as_str),
                Some("true")
            );
        }
    }

    #[test]
    fn skills_health_is_unavailable_without_an_explicit_root() {
        let category = skills_category_health_for_configured_root(
            None,
            &crate::settings::skills::mandatory_skill_prompt_bundle(),
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(category.state, SettingsHealthState::Unavailable);
        assert_eq!(
            category.items[0].diagnostic_code.as_deref(),
            Some("skills.rootMissing")
        );
    }

    #[test]
    fn skills_health_rejects_an_explicit_non_project_root() {
        let non_project = tempfile::tempdir().expect("non-project root");

        let category = skills_category_health_for_configured_root(
            Some(non_project.path()),
            &crate::settings::skills::mandatory_skill_prompt_bundle(),
            "2026-07-16T12:00:00Z",
        );

        assert_eq!(category.state, SettingsHealthState::ActionRequired);
        assert_eq!(
            category.items[0].diagnostic_code.as_deref(),
            Some("skills.rootInvalid")
        );
    }
}
