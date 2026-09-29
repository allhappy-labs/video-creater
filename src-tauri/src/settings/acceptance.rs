use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::provider_credentials::PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV;

pub const ACCEPTANCE_ROOT_ENV: &str = "VIDEO_CREATER_SETTINGS_ACCEPTANCE_ROOT";
pub const ACCEPTANCE_TOKEN_ENV: &str = "VIDEO_CREATER_SETTINGS_ACCEPTANCE_TOKEN";
pub const ACCEPTANCE_STAGE_ENV: &str = "VIDEO_CREATER_SETTINGS_ACCEPTANCE_STAGE";
pub const ACCEPTANCE_MARKER_FILE: &str = ".video-creater-settings-acceptance";
const APP_SUPPORT_ENV: &str = "VIDEO_CREATER_APP_SUPPORT_DIR";
const ACCEPTANCE_ROOT_PREFIX: &str = "video-creater-settings-acceptance-";
/// Canonical system temporary directory acceptance roots must live under.
/// A fixed path is used rather than `TMPDIR`, which acceptance itself isolates.
#[cfg(target_os = "macos")]
const ACCEPTANCE_TEMP_BOUNDARY: &str = "/private/tmp";
#[cfg(not(target_os = "macos"))]
const ACCEPTANCE_TEMP_BOUNDARY: &str = "/tmp";

#[derive(Debug, Clone)]
pub struct SettingsAcceptanceState {
    root: PathBuf,
    project_root: PathBuf,
    stage: SettingsAcceptanceStage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsAcceptanceStage {
    PreRestart,
    PostRestart,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsAcceptanceContext {
    pub stage: SettingsAcceptanceStage,
    pub project_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsAcceptanceProgress {
    pub stage: SettingsAcceptanceStage,
    pub step: SettingsAcceptanceProgressStep,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsAcceptanceProgressStep {
    SettingsDomSettled,
    InterruptedRecoveryConfirmed,
    ModelDownloadRequested,
    ModelDownloadSettled,
    ModelVerificationRequested,
    ModelVerificationSettled,
    SpeechSeparationConfirmed,
    StorageProjectLoadRequested,
    StorageLoadCommandReceived,
    StorageMutationLeaseAcquired,
    StorageRenderRecoverySettled,
    StorageGenerationRecoverySettled,
    StorageSpeechProjectionSettled,
    StorageProjectSessionRecorded,
    StorageCleanupSettled,
    ProviderKeychainSettled,
    AgentMcpSkillsSettled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsAcceptanceCheckpoint {
    pub stage: SettingsAcceptanceStage,
    pub checks: Vec<SettingsAcceptanceCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_contract: Option<FinalSettingsAcceptanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FinalSettingsAcceptanceReport {
    app_settings: FinalSettingsAppSettings,
    providers: FinalSettingsProviders,
    models: FinalSettingsModels,
    blocked_actions: Vec<FinalSettingsBlockedAction>,
    system_health: FinalSettingsSystemHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalSettingsAppSettings {
    categories: Vec<String>,
    sidebar_health_badges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalSettingsProviders {
    environment_credential_controls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalSettingsModels {
    selection_control: FinalSettingsModelSelectionControl,
    combobox_open: bool,
    filter_query: String,
    filtered_result_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum FinalSettingsModelSelectionControl {
    #[serde(rename = "multiselect-combobox")]
    MultiselectCombobox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalSettingsBlockedAction {
    id: String,
    configure_target: String,
    observed_target: Option<String>,
    configure_target_resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalSettingsSystemHealth {
    optional_provider_counted_as_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsAcceptanceFailure {
    pub stage: SettingsAcceptanceStage,
    pub phase: SettingsAcceptanceCheckId,
    pub diagnostic_code: SettingsAcceptanceFailureCode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SettingsAcceptanceFailureCode {
    #[serde(rename = "settings.acceptance.settingsDom.failed")]
    SettingsDomFailed,
    #[serde(rename = "settings.acceptance.settingsDom.categoryTabs")]
    SettingsDomCategoryTabs,
    #[serde(rename = "settings.acceptance.settingsDom.models")]
    SettingsDomModels,
    #[serde(rename = "settings.acceptance.settingsDom.modelResults")]
    SettingsDomModelResults,
    #[serde(rename = "settings.acceptance.settingsDom.blockedActions")]
    SettingsDomBlockedActions,
    #[serde(rename = "settings.acceptance.settingsDom.integrations")]
    SettingsDomIntegrations,
    #[serde(rename = "settings.acceptance.modelCancel.failed")]
    ModelCancelFailed,
    #[serde(rename = "settings.acceptance.interruptedRecovery.failed")]
    InterruptedRecoveryFailed,
    #[serde(rename = "settings.acceptance.modelReady.failed")]
    ModelReadyFailed,
    #[serde(rename = "settings.acceptance.speechSeparation.failed")]
    SpeechSeparationFailed,
    #[serde(rename = "settings.acceptance.storageCleanup.failed")]
    StorageCleanupFailed,
    #[serde(rename = "settings.acceptance.storageCleanup.renderArtifacts")]
    StorageCleanupRenderArtifacts,
    #[serde(rename = "settings.acceptance.storageCleanup.disposableCache")]
    StorageCleanupDisposableCache,
    #[serde(rename = "settings.acceptance.storageCleanup.confirmationMismatch")]
    StorageCleanupConfirmationMismatch,
    #[serde(rename = "settings.acceptance.storageCleanup.progressPersistence")]
    StorageCleanupProgressPersistence,
    #[serde(rename = "settings.acceptance.storageCleanup.projectSession")]
    StorageCleanupProjectSession,
    #[serde(rename = "settings.acceptance.storageCleanup.healthCommand")]
    StorageCleanupHealthCommand,
    #[serde(rename = "settings.acceptance.storageCleanup.previewCommand")]
    StorageCleanupPreviewCommand,
    #[serde(rename = "settings.acceptance.storageCleanup.startCommand")]
    StorageCleanupStartCommand,
    #[serde(rename = "settings.acceptance.providerKeychain.failed")]
    ProviderKeychainFailed,
    #[serde(rename = "settings.acceptance.agentMcpSkills.failed")]
    AgentMcpSkillsFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsAcceptanceCheck {
    pub id: SettingsAcceptanceCheckId,
    pub status: SettingsAcceptanceCheckStatus,
    pub diagnostic_code: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum SettingsAcceptanceCheckId {
    SettingsDom,
    ModelCancel,
    InterruptedRecovery,
    ModelReady,
    SpeechSeparation,
    StorageCleanup,
    ProviderKeychain,
    AgentMcpSkills,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsAcceptanceCheckStatus {
    Passed,
    Failed,
    Blocked,
}

impl SettingsAcceptanceState {
    pub fn from_environment() -> Result<Option<Self>, String> {
        let environment = env::vars().collect::<BTreeMap<_, _>>();
        Self::from_environment_map(&environment)
    }

    fn from_environment_map(
        environment: &BTreeMap<String, String>,
    ) -> Result<Option<Self>, String> {
        let root = environment.get(ACCEPTANCE_ROOT_ENV);
        let token = environment.get(ACCEPTANCE_TOKEN_ENV);
        let stage = environment.get(ACCEPTANCE_STAGE_ENV);
        let keychain_service = environment.get(PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV);
        if root.is_none() && token.is_none() && stage.is_none() && keychain_service.is_none() {
            return Ok(None);
        }
        let root = root.ok_or_else(|| format!("{ACCEPTANCE_ROOT_ENV} is required"))?;
        let token = token.ok_or_else(|| format!("{ACCEPTANCE_TOKEN_ENV} is required"))?;
        let stage = stage.ok_or_else(|| format!("{ACCEPTANCE_STAGE_ENV} is required"))?;
        validate_token(token)?;
        let expected_keychain_service = format!(
            "com.olhapi.video-creater.settings-acceptance.{}",
            &token[..32]
        );
        if keychain_service != Some(&expected_keychain_service) {
            return Err("settings acceptance requires its isolated Keychain service".to_string());
        }
        let requested_root = PathBuf::from(root);
        if !requested_root.is_absolute() {
            return Err("settings acceptance root must be absolute".to_string());
        }
        let metadata = fs::symlink_metadata(&requested_root)
            .map_err(|error| format!("settings acceptance root is unavailable: {error}"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("settings acceptance root must be a real directory".to_string());
        }
        let canonical_root = requested_root
            .canonicalize()
            .map_err(|error| format!("settings acceptance root cannot be resolved: {error}"))?;
        if canonical_root != requested_root
            || !canonical_root.starts_with(ACCEPTANCE_TEMP_BOUNDARY)
            || !canonical_root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(ACCEPTANCE_ROOT_PREFIX))
        {
            return Err(
                "settings acceptance root is outside the reviewed temporary boundary".to_string(),
            );
        }
        validate_marker(&canonical_root, token)?;
        for variable in ["HOME", "TMPDIR", APP_SUPPORT_ENV] {
            validate_isolated_directory(&canonical_root, environment, variable)?;
        }
        let project_root = validate_acceptance_child_directory(
            &canonical_root,
            &canonical_root.join("projects/acceptance-project"),
            "acceptance project root",
        )?;
        let stage = match stage.as_str() {
            "pre_restart" => SettingsAcceptanceStage::PreRestart,
            "post_restart" => SettingsAcceptanceStage::PostRestart,
            _ => return Err("settings acceptance stage is not allowlisted".to_string()),
        };
        Ok(Some(Self {
            root: canonical_root,
            project_root,
            stage,
        }))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn stage(&self) -> SettingsAcceptanceStage {
        self.stage
    }

    pub fn context(&self) -> SettingsAcceptanceContext {
        SettingsAcceptanceContext {
            stage: self.stage,
            project_root: self.project_root.clone(),
        }
    }

    pub fn write_progress(&self, progress: SettingsAcceptanceProgress) -> Result<PathBuf, String> {
        if progress.stage != self.stage {
            return Err("settings acceptance progress stage mismatch".to_string());
        }
        let checkpoint_dir = self.root.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).map_err(|error| {
            format!("could not create acceptance checkpoint directory: {error}")
        })?;
        let checkpoint_dir = validate_acceptance_child_directory(
            &self.root,
            &checkpoint_dir,
            "checkpoint directory",
        )?;
        let name = match self.stage {
            SettingsAcceptanceStage::PreRestart => "pre-restart-progress.json",
            SettingsAcceptanceStage::PostRestart => "post-restart-progress.json",
        };
        let destination = checkpoint_dir.join(name);
        let temporary = checkpoint_dir.join(format!(
            ".{name}.{:?}.{}.tmp",
            progress.step,
            std::process::id()
        ));
        let bytes = serde_json::to_vec_pretty(&progress)
            .map_err(|error| format!("could not serialize acceptance progress: {error}"))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create acceptance progress: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("could not write acceptance progress: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync acceptance progress: {error}"))?;
        drop(file);
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("could not publish acceptance progress: {error}"))?;
        Ok(destination)
    }

    pub fn write_checkpoint(
        &self,
        checkpoint: SettingsAcceptanceCheckpoint,
    ) -> Result<PathBuf, String> {
        if checkpoint.stage != self.stage {
            return Err("settings acceptance checkpoint stage mismatch".to_string());
        }
        if checkpoint.checks.is_empty() || checkpoint.checks.len() > 8 {
            return Err("settings acceptance checkpoint check count is invalid".to_string());
        }
        match (checkpoint.stage, checkpoint.product_contract.as_ref()) {
            (SettingsAcceptanceStage::PreRestart, None) => {}
            (SettingsAcceptanceStage::PostRestart, Some(report)) => {
                validate_final_product_contract(report)?;
            }
            (SettingsAcceptanceStage::PreRestart, Some(_)) => {
                return Err("pre-restart checkpoint cannot include a product contract".to_string());
            }
            (SettingsAcceptanceStage::PostRestart, None) => {
                return Err("post-restart checkpoint requires a product contract".to_string());
            }
        }
        let mut ids = BTreeSet::new();
        for check in &checkpoint.checks {
            if !ids.insert(check.id) {
                return Err("settings acceptance checkpoint contains duplicate checks".to_string());
            }
            if !check_allowed_in_stage(check.id, checkpoint.stage) {
                return Err(
                    "settings acceptance checkpoint check is not allowed in this stage".to_string(),
                );
            }
            let expected = diagnostic_code(check.id, check.status);
            if check.diagnostic_code != expected {
                return Err("settings acceptance diagnostic code is not canonical".to_string());
            }
        }
        if ids != expected_check_ids(checkpoint.stage) {
            return Err(
                "settings acceptance checkpoint does not contain the exact stage checks"
                    .to_string(),
            );
        }
        let checkpoint_dir = self.root.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).map_err(|error| {
            format!("could not create acceptance checkpoint directory: {error}")
        })?;
        let checkpoint_dir = validate_acceptance_child_directory(
            &self.root,
            &checkpoint_dir,
            "checkpoint directory",
        )?;
        let name = match self.stage {
            SettingsAcceptanceStage::PreRestart => "pre-restart.json",
            SettingsAcceptanceStage::PostRestart => "post-restart.json",
        };
        let destination = checkpoint_dir.join(name);
        let temporary = checkpoint_dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(&checkpoint)
            .map_err(|error| format!("could not serialize acceptance checkpoint: {error}"))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create acceptance checkpoint: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("could not write acceptance checkpoint: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync acceptance checkpoint: {error}"))?;
        drop(file);
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("could not publish acceptance checkpoint: {error}"))?;
        Ok(destination)
    }

    pub fn write_failure(&self, failure: SettingsAcceptanceFailure) -> Result<PathBuf, String> {
        self.validate_failure(&failure)?;
        let checkpoint_dir = self.root.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).map_err(|error| {
            format!("could not create acceptance checkpoint directory: {error}")
        })?;
        let checkpoint_dir = validate_acceptance_child_directory(
            &self.root,
            &checkpoint_dir,
            "checkpoint directory",
        )?;
        let name = match self.stage {
            SettingsAcceptanceStage::PreRestart => "pre-restart-failure.json",
            SettingsAcceptanceStage::PostRestart => "post-restart-failure.json",
        };
        let destination = checkpoint_dir.join(name);
        let temporary = checkpoint_dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(&failure)
            .map_err(|error| format!("could not serialize acceptance failure: {error}"))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create acceptance failure: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("could not write acceptance failure: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync acceptance failure: {error}"))?;
        drop(file);
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("could not publish acceptance failure: {error}"))?;
        Ok(destination)
    }

    pub fn validate_failure(&self, failure: &SettingsAcceptanceFailure) -> Result<(), String> {
        if failure.stage != self.stage {
            return Err("settings acceptance failure stage mismatch".to_string());
        }
        if !check_allowed_in_stage(failure.phase, failure.stage) {
            return Err(
                "settings acceptance failure phase is not allowed in this stage".to_string(),
            );
        }
        if !failure_code_allowed_for_phase(failure.phase, failure.diagnostic_code) {
            return Err("settings acceptance failure diagnostic code is not canonical".to_string());
        }
        Ok(())
    }
}

fn validate_final_product_contract(report: &FinalSettingsAcceptanceReport) -> Result<(), String> {
    let expected_categories = [
        "general",
        "projects",
        "aiModels",
        "integrations",
        "storage",
        "advanced",
    ];
    let blocked_actions = report.blocked_actions.as_slice();
    let openai_action_present = blocked_actions.iter().any(|action| {
        action.id == "missing-openai-provider"
            && action.configure_target == "integrations:openai"
            && action.observed_target.as_deref() == Some("integrations:openai")
            && action.configure_target_resolved
    });
    let all_blocked_actions_resolved = blocked_actions.iter().all(|action| {
        !action.id.is_empty()
            && action.configure_target.starts_with("integrations:")
            && action.observed_target.as_deref() == Some(action.configure_target.as_str())
            && action.configure_target_resolved
    });
    if report.app_settings.categories != expected_categories
        || report.app_settings.sidebar_health_badges != 0
        || report.providers.environment_credential_controls != 0
        || report.models.selection_control
            != FinalSettingsModelSelectionControl::MultiselectCombobox
        || !report.models.combobox_open
        || report.models.filter_query != "gpt image"
        || report.models.filtered_result_count == 0
        || !openai_action_present
        || !all_blocked_actions_resolved
        || report.system_health.optional_provider_counted_as_required
    {
        return Err("settings acceptance product contract is invalid".to_string());
    }
    Ok(())
}

fn failure_code(id: SettingsAcceptanceCheckId) -> SettingsAcceptanceFailureCode {
    match id {
        SettingsAcceptanceCheckId::SettingsDom => SettingsAcceptanceFailureCode::SettingsDomFailed,
        SettingsAcceptanceCheckId::ModelCancel => SettingsAcceptanceFailureCode::ModelCancelFailed,
        SettingsAcceptanceCheckId::InterruptedRecovery => {
            SettingsAcceptanceFailureCode::InterruptedRecoveryFailed
        }
        SettingsAcceptanceCheckId::ModelReady => SettingsAcceptanceFailureCode::ModelReadyFailed,
        SettingsAcceptanceCheckId::SpeechSeparation => {
            SettingsAcceptanceFailureCode::SpeechSeparationFailed
        }
        SettingsAcceptanceCheckId::StorageCleanup => {
            SettingsAcceptanceFailureCode::StorageCleanupFailed
        }
        SettingsAcceptanceCheckId::ProviderKeychain => {
            SettingsAcceptanceFailureCode::ProviderKeychainFailed
        }
        SettingsAcceptanceCheckId::AgentMcpSkills => {
            SettingsAcceptanceFailureCode::AgentMcpSkillsFailed
        }
    }
}

fn failure_code_allowed_for_phase(
    phase: SettingsAcceptanceCheckId,
    code: SettingsAcceptanceFailureCode,
) -> bool {
    matches!(
        (phase, code),
        (
            SettingsAcceptanceCheckId::SettingsDom,
            SettingsAcceptanceFailureCode::SettingsDomFailed
                | SettingsAcceptanceFailureCode::SettingsDomCategoryTabs
                | SettingsAcceptanceFailureCode::SettingsDomModels
                | SettingsAcceptanceFailureCode::SettingsDomModelResults
                | SettingsAcceptanceFailureCode::SettingsDomBlockedActions
                | SettingsAcceptanceFailureCode::SettingsDomIntegrations
        ) | (
            SettingsAcceptanceCheckId::StorageCleanup,
            SettingsAcceptanceFailureCode::StorageCleanupRenderArtifacts
                | SettingsAcceptanceFailureCode::StorageCleanupDisposableCache
        )
    ) || code == failure_code(phase)
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "settings acceptance token must be 64 lowercase hexadecimal characters".to_string(),
        );
    }
    Ok(())
}

fn validate_marker(root: &Path, token: &str) -> Result<(), String> {
    let marker = root.join(ACCEPTANCE_MARKER_FILE);
    let metadata = fs::symlink_metadata(&marker)
        .map_err(|error| format!("settings acceptance marker is unavailable: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("settings acceptance marker must be a real file".to_string());
    }
    let marker_token = fs::read_to_string(marker)
        .map_err(|error| format!("settings acceptance marker cannot be read: {error}"))?;
    if marker_token != token {
        return Err("settings acceptance marker token mismatch".to_string());
    }
    Ok(())
}

fn validate_isolated_directory(
    root: &Path,
    environment: &BTreeMap<String, String>,
    variable: &str,
) -> Result<(), String> {
    let path = environment
        .get(variable)
        .ok_or_else(|| format!("{variable} is required for settings acceptance"))?;
    let requested = PathBuf::from(path);
    let metadata = fs::symlink_metadata(&requested)
        .map_err(|error| format!("{variable} is unavailable: {error}"))?;
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("{variable} cannot be resolved: {error}"))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || canonical != requested
        || canonical == root
        || !canonical.starts_with(root)
    {
        return Err(format!("{variable} escapes the settings acceptance root"));
    }
    Ok(())
}

fn validate_acceptance_child_directory(
    root: &Path,
    requested: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(requested)
        .map_err(|error| format!("{label} is unavailable: {error}"))?;
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("{label} cannot be resolved: {error}"))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || canonical != requested
        || canonical == root
        || !canonical.starts_with(root)
    {
        return Err(format!("{label} escapes the settings acceptance root"));
    }
    Ok(canonical)
}

fn check_allowed_in_stage(id: SettingsAcceptanceCheckId, stage: SettingsAcceptanceStage) -> bool {
    match stage {
        SettingsAcceptanceStage::PreRestart => matches!(
            id,
            SettingsAcceptanceCheckId::SettingsDom | SettingsAcceptanceCheckId::ModelCancel
        ),
        SettingsAcceptanceStage::PostRestart => {
            !matches!(id, SettingsAcceptanceCheckId::ModelCancel)
        }
    }
}

fn expected_check_ids(stage: SettingsAcceptanceStage) -> BTreeSet<SettingsAcceptanceCheckId> {
    match stage {
        SettingsAcceptanceStage::PreRestart => [
            SettingsAcceptanceCheckId::SettingsDom,
            SettingsAcceptanceCheckId::ModelCancel,
        ]
        .into_iter()
        .collect(),
        SettingsAcceptanceStage::PostRestart => [
            SettingsAcceptanceCheckId::SettingsDom,
            SettingsAcceptanceCheckId::InterruptedRecovery,
            SettingsAcceptanceCheckId::ModelReady,
            SettingsAcceptanceCheckId::SpeechSeparation,
            SettingsAcceptanceCheckId::StorageCleanup,
            SettingsAcceptanceCheckId::ProviderKeychain,
            SettingsAcceptanceCheckId::AgentMcpSkills,
        ]
        .into_iter()
        .collect(),
    }
}

fn diagnostic_code(id: SettingsAcceptanceCheckId, status: SettingsAcceptanceCheckStatus) -> String {
    let id = match id {
        SettingsAcceptanceCheckId::SettingsDom => "settingsDom",
        SettingsAcceptanceCheckId::ModelCancel => "modelCancel",
        SettingsAcceptanceCheckId::InterruptedRecovery => "interruptedRecovery",
        SettingsAcceptanceCheckId::ModelReady => "modelReady",
        SettingsAcceptanceCheckId::SpeechSeparation => "speechSeparation",
        SettingsAcceptanceCheckId::StorageCleanup => "storageCleanup",
        SettingsAcceptanceCheckId::ProviderKeychain => "providerKeychain",
        SettingsAcceptanceCheckId::AgentMcpSkills => "agentMcpSkills",
    };
    let status = match status {
        SettingsAcceptanceCheckStatus::Passed => "passed",
        SettingsAcceptanceCheckStatus::Failed => "failed",
        SettingsAcceptanceCheckStatus::Blocked => "blocked",
    };
    format!("settings.acceptance.{id}.{status}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn gated_environment(root: &std::path::Path, token: &str) -> BTreeMap<String, String> {
        let home = root.join("home");
        let tmp = root.join("tmp");
        let app_support = root.join("app-support");
        let project = root.join("projects/acceptance-project");
        for path in [&home, &tmp, &app_support, &project] {
            fs::create_dir_all(path).expect("acceptance subdirectory");
        }
        fs::write(root.join(ACCEPTANCE_MARKER_FILE), token).expect("acceptance marker");
        BTreeMap::from([
            (ACCEPTANCE_ROOT_ENV.to_string(), root.display().to_string()),
            (ACCEPTANCE_TOKEN_ENV.to_string(), token.to_string()),
            (ACCEPTANCE_STAGE_ENV.to_string(), "pre_restart".to_string()),
            ("HOME".to_string(), home.display().to_string()),
            ("TMPDIR".to_string(), tmp.display().to_string()),
            (
                "VIDEO_CREATER_APP_SUPPORT_DIR".to_string(),
                app_support.display().to_string(),
            ),
            (
                "VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE".to_string(),
                format!(
                    "com.olhapi.video-creater.settings-acceptance.{}",
                    &token[..32]
                ),
            ),
        ])
    }

    #[test]
    fn valid_gate_requires_marker_token_and_all_isolated_roots() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        fs::create_dir(&root).expect("acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let state = SettingsAcceptanceState::from_environment_map(&gated_environment(&root, token))
            .expect("valid gate")
            .expect("acceptance enabled");

        assert_eq!(state.stage(), SettingsAcceptanceStage::PreRestart);
        assert_eq!(state.root(), root.canonicalize().expect("canonical root"));
        assert_eq!(
            state.context().project_root,
            root.join("projects/acceptance-project")
        );
    }

    #[test]
    fn normal_launch_is_inert_without_acceptance_environment() {
        assert!(
            SettingsAcceptanceState::from_environment_map(&BTreeMap::new())
                .expect("normal launch")
                .is_none()
        );
    }

    #[test]
    fn gate_rejects_weak_token_marker_mismatch_and_root_escape() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        fs::create_dir(&root).expect("acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let mut environment = gated_environment(&root, token);

        environment.insert(ACCEPTANCE_TOKEN_ENV.to_string(), "weak".to_string());
        assert!(SettingsAcceptanceState::from_environment_map(&environment).is_err());

        environment = gated_environment(&root, token);
        fs::write(root.join(ACCEPTANCE_MARKER_FILE), "different").expect("mismatch marker");
        assert!(SettingsAcceptanceState::from_environment_map(&environment).is_err());

        environment = gated_environment(&root, token);
        environment.insert("HOME".to_string(), ACCEPTANCE_TEMP_BOUNDARY.to_string());
        assert!(SettingsAcceptanceState::from_environment_map(&environment).is_err());

        environment = gated_environment(&root, token);
        environment.remove("VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE");
        assert!(SettingsAcceptanceState::from_environment_map(&environment).is_err());

        let provider_only = BTreeMap::from([(
            PROVIDER_CREDENTIAL_KEYCHAIN_SERVICE_ENV.to_string(),
            "com.olhapi.video-creater.settings-acceptance.0123456789abcdef0123456789abcdef"
                .to_string(),
        )]);
        assert!(SettingsAcceptanceState::from_environment_map(&provider_only).is_err());
    }

    #[test]
    fn gate_rejects_symlinked_root_and_isolated_directory_escape() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let real_root = parent.path().join("video-creater-settings-acceptance-real");
        fs::create_dir(&real_root).expect("real acceptance root");
        let linked_root = parent
            .path()
            .join("video-creater-settings-acceptance-linked");
        symlink(&real_root, &linked_root).expect("linked acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let linked_environment = gated_environment(&linked_root, token);
        assert!(SettingsAcceptanceState::from_environment_map(&linked_environment).is_err());

        let isolated_root = parent
            .path()
            .join("video-creater-settings-acceptance-isolated");
        fs::create_dir(&isolated_root).expect("isolated acceptance root");
        let mut isolated_environment = gated_environment(&isolated_root, token);
        let home = isolated_root.join("home");
        fs::remove_dir(&home).expect("remove real home");
        symlink(parent.path(), &home).expect("linked home escape");
        isolated_environment.insert("HOME".to_string(), home.display().to_string());
        assert!(SettingsAcceptanceState::from_environment_map(&isolated_environment).is_err());
    }

    #[test]
    fn checkpoint_writer_rejects_wrong_stage_duplicates_and_unknown_diagnostics() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        fs::create_dir(&root).expect("acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let state = SettingsAcceptanceState::from_environment_map(&gated_environment(&root, token))
            .expect("valid gate")
            .expect("acceptance enabled");
        let check = SettingsAcceptanceCheck {
            id: SettingsAcceptanceCheckId::SettingsDom,
            status: SettingsAcceptanceCheckStatus::Passed,
            diagnostic_code: "settings.acceptance.settingsDom.passed".to_string(),
        };

        assert!(state
            .write_checkpoint(SettingsAcceptanceCheckpoint {
                stage: SettingsAcceptanceStage::PostRestart,
                checks: vec![check.clone()],
                product_contract: None,
            })
            .is_err());
        assert!(state
            .write_checkpoint(SettingsAcceptanceCheckpoint {
                stage: SettingsAcceptanceStage::PreRestart,
                checks: vec![check.clone(), check],
                product_contract: None,
            })
            .is_err());
        assert!(state
            .write_checkpoint(SettingsAcceptanceCheckpoint {
                stage: SettingsAcceptanceStage::PreRestart,
                checks: vec![SettingsAcceptanceCheck {
                    id: SettingsAcceptanceCheckId::SettingsDom,
                    status: SettingsAcceptanceCheckStatus::Passed,
                    diagnostic_code: "settings.acceptance.settingsDom.unknown".to_string(),
                }],
                product_contract: None,
            })
            .is_err());
        assert!(state
            .write_checkpoint(SettingsAcceptanceCheckpoint {
                stage: SettingsAcceptanceStage::PreRestart,
                checks: vec![SettingsAcceptanceCheck {
                    id: SettingsAcceptanceCheckId::SettingsDom,
                    status: SettingsAcceptanceCheckStatus::Passed,
                    diagnostic_code: "settings.acceptance.settingsDom.passed".to_string(),
                }],
                product_contract: None,
            })
            .is_err());
    }

    #[test]
    fn checkpoint_writer_rejects_symlinked_checkpoint_directory() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        let outside = parent.path().join("outside-checkpoints");
        fs::create_dir(&root).expect("acceptance root");
        fs::create_dir(&outside).expect("outside checkpoint root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let state = SettingsAcceptanceState::from_environment_map(&gated_environment(&root, token))
            .expect("valid gate")
            .expect("acceptance enabled");
        symlink(&outside, root.join("checkpoints")).expect("linked checkpoint directory");

        let error = state
            .write_checkpoint(SettingsAcceptanceCheckpoint {
                stage: SettingsAcceptanceStage::PreRestart,
                checks: vec![
                    SettingsAcceptanceCheck {
                        id: SettingsAcceptanceCheckId::SettingsDom,
                        status: SettingsAcceptanceCheckStatus::Passed,
                        diagnostic_code: "settings.acceptance.settingsDom.passed".to_string(),
                    },
                    SettingsAcceptanceCheck {
                        id: SettingsAcceptanceCheckId::ModelCancel,
                        status: SettingsAcceptanceCheckStatus::Passed,
                        diagnostic_code: "settings.acceptance.modelCancel.passed".to_string(),
                    },
                ],
                product_contract: None,
            })
            .expect_err("symlinked checkpoint directory must be rejected");

        assert!(error.contains("escapes"));
        assert!(!outside.join("pre-restart.json").exists());
    }

    #[test]
    fn failure_writer_persists_bounded_diagnostic_evidence() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        fs::create_dir(&root).expect("acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let state = SettingsAcceptanceState::from_environment_map(&gated_environment(&root, token))
            .expect("valid gate")
            .expect("acceptance enabled");

        let path = state
            .write_failure(SettingsAcceptanceFailure {
                stage: SettingsAcceptanceStage::PreRestart,
                phase: SettingsAcceptanceCheckId::SettingsDom,
                diagnostic_code: SettingsAcceptanceFailureCode::SettingsDomFailed,
            })
            .expect("failure evidence");
        let failure: SettingsAcceptanceFailure =
            serde_json::from_slice(&fs::read(path).expect("failure bytes")).expect("failure JSON");
        assert_eq!(failure.phase, SettingsAcceptanceCheckId::SettingsDom);
        assert_eq!(
            failure.diagnostic_code,
            SettingsAcceptanceFailureCode::SettingsDomFailed
        );

        let echoed_secret =
            serde_json::from_value::<SettingsAcceptanceFailure>(serde_json::json!({
                "stage": "pre_restart",
                "phase": "settingsDom",
                "diagnosticCode": "settings.acceptance.settingsDom.failed",
                "message": "Bearer sk-native-acceptance-secret-value: acceptance-canary"
            }));
        assert!(echoed_secret.is_err());
        let category_tabs_failure =
            serde_json::from_value::<SettingsAcceptanceFailure>(serde_json::json!({
                "stage": "pre_restart",
                "phase": "settingsDom",
                "diagnosticCode": "settings.acceptance.settingsDom.categoryTabs"
            }))
            .expect("bounded category-tabs code must deserialize");
        assert!(state.validate_failure(&category_tabs_failure).is_ok());
        let model_results_failure =
            serde_json::from_value::<SettingsAcceptanceFailure>(serde_json::json!({
                "stage": "pre_restart",
                "phase": "settingsDom",
                "diagnosticCode": "settings.acceptance.settingsDom.modelResults"
            }))
            .expect("bounded model-results code must deserialize");
        assert!(state.validate_failure(&model_results_failure).is_ok());
        assert!(state
            .write_failure(SettingsAcceptanceFailure {
                stage: SettingsAcceptanceStage::PreRestart,
                phase: SettingsAcceptanceCheckId::SettingsDom,
                diagnostic_code: SettingsAcceptanceFailureCode::ModelCancelFailed,
            })
            .is_err());
    }

    #[test]
    fn progress_writer_persists_only_the_typed_acceptance_boundary() {
        let parent = tempfile::tempdir_in(ACCEPTANCE_TEMP_BOUNDARY).expect("temp parent");
        let root = parent.path().join("video-creater-settings-acceptance-run");
        fs::create_dir(&root).expect("acceptance root");
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let mut environment = gated_environment(&root, token);
        environment.insert(ACCEPTANCE_STAGE_ENV.to_string(), "post_restart".to_string());
        let state = SettingsAcceptanceState::from_environment_map(&environment)
            .expect("valid gate")
            .expect("acceptance enabled");

        let path = state
            .write_progress(SettingsAcceptanceProgress {
                stage: SettingsAcceptanceStage::PostRestart,
                step: SettingsAcceptanceProgressStep::StorageLoadCommandReceived,
            })
            .expect("progress evidence");
        let progress: SettingsAcceptanceProgress =
            serde_json::from_slice(&fs::read(path).expect("progress bytes"))
                .expect("progress JSON");
        assert_eq!(progress.stage, SettingsAcceptanceStage::PostRestart);
        assert_eq!(
            progress.step,
            SettingsAcceptanceProgressStep::StorageLoadCommandReceived
        );

        assert!(state
            .write_progress(SettingsAcceptanceProgress {
                stage: SettingsAcceptanceStage::PreRestart,
                step: SettingsAcceptanceProgressStep::StorageLoadCommandReceived,
            })
            .is_err());
    }

    #[test]
    fn checkpoint_round_trips_the_typed_final_product_contract() {
        let value = serde_json::json!({
            "stage": "post_restart",
            "checks": [],
            "productContract": {
                "appSettings": {
                    "categories": ["general", "projects", "aiModels", "integrations", "storage", "advanced"],
                    "sidebarHealthBadges": 0
                },
                "providers": { "environmentCredentialControls": 0 },
                "models": {
                    "selectionControl": "multiselect-combobox",
                    "comboboxOpen": true,
                    "filterQuery": "gpt image",
                    "filteredResultCount": 1
                },
                "blockedActions": [{
                    "id": "missing-openai-provider",
                    "configureTarget": "integrations:openai",
                    "observedTarget": "integrations:openai",
                    "configureTargetResolved": true
                }],
                "systemHealth": { "optionalProviderCountedAsRequired": false }
            }
        });
        let checkpoint: SettingsAcceptanceCheckpoint =
            serde_json::from_value(value.clone()).expect("typed checkpoint");
        let encoded = serde_json::to_value(checkpoint).expect("encoded checkpoint");

        assert_eq!(encoded["productContract"], value["productContract"]);

        let mut with_extra_resolved = value["productContract"].clone();
        with_extra_resolved["blockedActions"]
            .as_array_mut()
            .expect("blocked actions")
            .push(serde_json::json!({
                "id": "missing-google-provider",
                "configureTarget": "integrations:google",
                "observedTarget": "integrations:google",
                "configureTargetResolved": true
            }));
        let report: FinalSettingsAcceptanceReport =
            serde_json::from_value(with_extra_resolved).expect("extra resolved action report");
        assert!(validate_final_product_contract(&report).is_ok());

        let mut unresolved = value["productContract"].clone();
        unresolved["blockedActions"][0]["observedTarget"] = serde_json::Value::Null;
        unresolved["blockedActions"][0]["configureTargetResolved"] = serde_json::json!(false);
        let report: FinalSettingsAcceptanceReport =
            serde_json::from_value(unresolved).expect("unresolved action report");
        assert!(validate_final_product_contract(&report).is_err());
    }
}
