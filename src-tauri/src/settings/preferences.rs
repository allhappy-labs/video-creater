use crate::project::model::CaptionRenderMode;
use crate::provider_credentials::is_supported_provider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tempfile::NamedTempFile;
use thiserror::Error;

const APP_PREFERENCES_SCHEMA_VERSION: u32 = 2;
const MIN_PROJECT_DIMENSION: u32 = 2;
const MAX_PROJECT_DIMENSION: u32 = 16_384;
const MIN_PROJECT_FPS: f64 = 1.0;
const MAX_PROJECT_FPS: f64 = 120.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectLocationPreference {
    Ask,
    SuggestedParent { parent_path: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GenerationExecutionBackend {
    InProcess,
    Temporal,
}

/// Which conversation-turn agent backend the app should use. `Automatic` resolves
/// at turn time to whichever backend is ready, so an install with only one agent
/// available needs no configuration.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentBackendPreference {
    #[default]
    Automatic,
    Codex,
    Claude,
}

/// The Claude model alias passed to `--model`. Aliases resolve server-side to the
/// current generation, so no dated model id is stored.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClaudeModelPreference {
    #[default]
    Sonnet,
    Haiku,
    Opus,
}

impl ClaudeModelPreference {
    pub fn alias(self) -> &'static str {
        match self {
            Self::Sonnet => "sonnet",
            Self::Haiku => "haiku",
            Self::Opus => "opus",
        }
    }

    /// The `--fallback-model` alias: the next cheaper model, so an overloaded first choice
    /// degrades instead of failing the turn. Haiku is already the cheapest, so it has none.
    pub fn fallback_alias(self) -> Option<&'static str> {
        match self {
            Self::Opus => Some(Self::Sonnet.alias()),
            Self::Sonnet => Some(Self::Haiku.alias()),
            Self::Haiku => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewProjectDefaults {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub loudness_lufs: f64,
    pub captions: CaptionRenderMode,
}

impl Default for NewProjectDefaults {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30.0,
            loudness_lufs: -14.0,
            captions: CaptionRenderMode::BurnIn,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppPreferencesV2 {
    pub schema_version: u32,
    pub project_location: ProjectLocationPreference,
    pub require_provider_upload_confirmation: bool,
    pub render_completion_notifications: bool,
    pub new_project_defaults: NewProjectDefaults,
    pub enabled_generation_model_ids: Vec<String>,
    pub generation_execution_backend: GenerationExecutionBackend,
    #[serde(default)]
    pub agent_backend: AgentBackendPreference,
    #[serde(default)]
    pub claude_model: ClaudeModelPreference,
    /// An explicit `claude` path. Empty means "resolve it from `PATH` and the
    /// well-known install locations"; `Option` is avoided here so that a patch
    /// clearing the override is distinguishable from a patch that omits it.
    #[serde(default)]
    pub claude_executable_path: String,
}

impl Default for AppPreferencesV2 {
    fn default() -> Self {
        Self {
            schema_version: APP_PREFERENCES_SCHEMA_VERSION,
            project_location: ProjectLocationPreference::Ask,
            require_provider_upload_confirmation: true,
            render_completion_notifications: false,
            new_project_defaults: NewProjectDefaults::default(),
            enabled_generation_model_ids: Vec::new(),
            generation_execution_backend: GenerationExecutionBackend::InProcess,
            agent_backend: AgentBackendPreference::Automatic,
            claude_model: ClaudeModelPreference::Sonnet,
            claude_executable_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAppPreferencesV1 {
    pub provider_credential_env_var: Option<String>,
    #[serde(default)]
    pub disabled_generation_model_ids: Vec<String>,
    pub require_provider_upload_confirmation: Option<bool>,
    pub render_completion_notifications: Option<bool>,
    pub project_location: Option<ProjectLocationPreference>,
    pub generation_execution_backend: Option<GenerationExecutionBackend>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppPreferencesPatch {
    pub project_location: Option<ProjectLocationPreference>,
    pub require_provider_upload_confirmation: Option<bool>,
    pub render_completion_notifications: Option<bool>,
    pub new_project_defaults: Option<NewProjectDefaults>,
    pub enabled_generation_model_ids: Option<Vec<String>>,
    pub generation_execution_backend: Option<GenerationExecutionBackend>,
    pub agent_backend: Option<AgentBackendPreference>,
    pub claude_model: Option<ClaudeModelPreference>,
    pub claude_executable_path: Option<String>,
}

impl AppPreferencesV2 {
    /// The user's explicit `claude` path, or `None` when the app should resolve it.
    pub fn claude_executable_override(&self) -> Option<&str> {
        let trimmed = self.claude_executable_path.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }
}

#[derive(Debug, Error)]
pub enum AppPreferencesError {
    #[error("app preferences schema version {0} is unsupported")]
    UnsupportedSchema(u32),
    #[error("project dimensions must be even values between {MIN_PROJECT_DIMENSION} and {MAX_PROJECT_DIMENSION}")]
    InvalidDimensions,
    #[error("project fps must be finite and between {MIN_PROJECT_FPS} and {MAX_PROJECT_FPS}")]
    InvalidFps,
    #[error("project loudness LUFS must be finite")]
    InvalidLoudness,
    #[error("generation model ID {0:?} must use a canonical supported-provider:model-id value")]
    InvalidGenerationModelId(String),
    #[error("suggested project parent path {0:?} must be an absolute normalized path")]
    InvalidProjectPath(String),
    #[error("Claude executable path {0:?} must be a single-line path without control characters")]
    InvalidClaudeExecutablePath(String),
    #[error("failed to create app preferences directory {path}: {source}")]
    CreateDirectory {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to read app preferences {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse app preferences {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("failed to serialize app preferences: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write app preferences {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to sync app preferences {path}: {source}")]
    Sync {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to replace app preferences {from} with {to}: {source}")]
    Persist {
        from: String,
        to: String,
        source: std::io::Error,
    },
}

impl AppPreferencesError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedSchema(_) => "settings.preferences.unsupportedSchema",
            Self::InvalidDimensions => "settings.preferences.invalidDimensions",
            Self::InvalidFps => "settings.preferences.invalidFps",
            Self::InvalidLoudness => "settings.preferences.invalidLoudness",
            Self::InvalidGenerationModelId(_) => "settings.preferences.invalidGenerationModelId",
            Self::InvalidProjectPath(_) => "settings.preferences.invalidProjectPath",
            Self::InvalidClaudeExecutablePath(_) => {
                "settings.preferences.invalidClaudeExecutablePath"
            }
            Self::CreateDirectory { .. }
            | Self::Write { .. }
            | Self::Sync { .. }
            | Self::Persist { .. }
            | Self::Serialize(_) => "settings.preferences.writeFailed",
            Self::Read { .. } | Self::Parse { .. } => "settings.preferences.readFailed",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferencesCommandError {
    pub code: String,
    pub message: String,
    pub detail: String,
}

impl AppPreferencesCommandError {
    pub fn lock() -> Self {
        Self {
            code: "settings.preferences.unavailable".to_string(),
            message: "App preferences are temporarily unavailable.".to_string(),
            detail: "app preferences store lock failed".to_string(),
        }
    }
}

impl From<AppPreferencesError> for AppPreferencesCommandError {
    fn from(error: AppPreferencesError) -> Self {
        let code = error.code().to_string();
        let message = match error {
            AppPreferencesError::InvalidDimensions
            | AppPreferencesError::InvalidFps
            | AppPreferencesError::InvalidLoudness
            | AppPreferencesError::InvalidGenerationModelId(_)
            | AppPreferencesError::InvalidProjectPath(_)
            | AppPreferencesError::InvalidClaudeExecutablePath(_)
            | AppPreferencesError::UnsupportedSchema(_) => {
                "App preferences contain an invalid value."
            }
            _ => "App preferences could not be persisted.",
        }
        .to_string();
        Self {
            code,
            message,
            detail: error.to_string(),
        }
    }
}

pub struct AppPreferencesStore {
    path: PathBuf,
}

pub struct AppPreferencesState(pub Mutex<AppPreferencesStore>);

impl AppPreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load_or_migrate(
        &self,
        legacy: Option<LegacyAppPreferencesV1>,
    ) -> Result<AppPreferencesV2, AppPreferencesError> {
        if self.path.exists() {
            return self.load_existing();
        }

        let mut preferences = legacy.map(Self::migrate_v1).unwrap_or_default();
        validate_and_normalize(&mut preferences)?;
        self.write_atomic(&preferences)?;
        Ok(preferences)
    }

    /// The saved preferences, or the defaults when none are saved yet. Never writes, so a legacy
    /// migration the editor has not run yet is left for `load_or_migrate`.
    pub fn load_saved_or_default(&self) -> Result<AppPreferencesV2, AppPreferencesError> {
        if self.path.exists() {
            return self.load_existing();
        }
        Ok(AppPreferencesV2::default())
    }

    pub fn update(
        &self,
        patch: AppPreferencesPatch,
    ) -> Result<AppPreferencesV2, AppPreferencesError> {
        let mut preferences = if self.path.exists() {
            self.load_existing()?
        } else {
            AppPreferencesV2::default()
        };
        apply_patch(&mut preferences, patch);
        validate_and_normalize(&mut preferences)?;
        self.write_atomic(&preferences)?;
        Ok(preferences)
    }

    fn load_existing(&self) -> Result<AppPreferencesV2, AppPreferencesError> {
        let json = fs::read_to_string(&self.path).map_err(|source| AppPreferencesError::Read {
            path: self.path.display().to_string(),
            source,
        })?;
        let mut preferences =
            serde_json::from_str::<AppPreferencesV2>(&json).map_err(|source| {
                AppPreferencesError::Parse {
                    path: self.path.display().to_string(),
                    source,
                }
            })?;
        validate_and_normalize(&mut preferences)?;
        Ok(preferences)
    }

    fn migrate_v1(legacy: LegacyAppPreferencesV1) -> AppPreferencesV2 {
        let LegacyAppPreferencesV1 {
            provider_credential_env_var: _,
            disabled_generation_model_ids: _,
            require_provider_upload_confirmation,
            render_completion_notifications,
            project_location,
            generation_execution_backend,
        } = legacy;
        let defaults = AppPreferencesV2::default();
        AppPreferencesV2 {
            project_location: project_location.unwrap_or(defaults.project_location),
            require_provider_upload_confirmation: require_provider_upload_confirmation
                .unwrap_or(defaults.require_provider_upload_confirmation),
            render_completion_notifications: render_completion_notifications
                .unwrap_or(defaults.render_completion_notifications),
            generation_execution_backend: generation_execution_backend
                .unwrap_or(defaults.generation_execution_backend),
            ..defaults
        }
    }

    fn write_atomic(&self, preferences: &AppPreferencesV2) -> Result<(), AppPreferencesError> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| AppPreferencesError::CreateDirectory {
            path: parent.display().to_string(),
            source,
        })?;
        let json =
            serde_json::to_vec_pretty(preferences).map_err(AppPreferencesError::Serialize)?;
        let mut temp_file =
            NamedTempFile::with_prefix_in(".preferences.json.", parent).map_err(|source| {
                AppPreferencesError::Write {
                    path: parent.display().to_string(),
                    source,
                }
            })?;
        let temp_path = temp_file.path().to_path_buf();
        temp_file
            .write_all(&json)
            .map_err(|source| AppPreferencesError::Write {
                path: temp_path.display().to_string(),
                source,
            })?;
        temp_file
            .as_file()
            .sync_all()
            .map_err(|source| AppPreferencesError::Sync {
                path: temp_path.display().to_string(),
                source,
            })?;
        temp_file
            .persist(&self.path)
            .map_err(|error| AppPreferencesError::Persist {
                from: temp_path.display().to_string(),
                to: self.path.display().to_string(),
                source: error.error,
            })?;
        Ok(())
    }
}

fn apply_patch(preferences: &mut AppPreferencesV2, patch: AppPreferencesPatch) {
    if let Some(project_location) = patch.project_location {
        preferences.project_location = project_location;
    }
    if let Some(require_confirmation) = patch.require_provider_upload_confirmation {
        preferences.require_provider_upload_confirmation = require_confirmation;
    }
    if let Some(notifications) = patch.render_completion_notifications {
        preferences.render_completion_notifications = notifications;
    }
    if let Some(defaults) = patch.new_project_defaults {
        preferences.new_project_defaults = defaults;
    }
    if let Some(model_ids) = patch.enabled_generation_model_ids {
        preferences.enabled_generation_model_ids = model_ids;
    }
    if let Some(backend) = patch.generation_execution_backend {
        preferences.generation_execution_backend = backend;
    }
    if let Some(backend) = patch.agent_backend {
        preferences.agent_backend = backend;
    }
    if let Some(model) = patch.claude_model {
        preferences.claude_model = model;
    }
    if let Some(path) = patch.claude_executable_path {
        preferences.claude_executable_path = path;
    }
}

fn validate_and_normalize(preferences: &mut AppPreferencesV2) -> Result<(), AppPreferencesError> {
    if preferences.schema_version != APP_PREFERENCES_SCHEMA_VERSION {
        return Err(AppPreferencesError::UnsupportedSchema(
            preferences.schema_version,
        ));
    }
    let defaults = &preferences.new_project_defaults;
    if !(MIN_PROJECT_DIMENSION..=MAX_PROJECT_DIMENSION).contains(&defaults.width)
        || !(MIN_PROJECT_DIMENSION..=MAX_PROJECT_DIMENSION).contains(&defaults.height)
        || !defaults.width.is_multiple_of(2)
        || !defaults.height.is_multiple_of(2)
    {
        return Err(AppPreferencesError::InvalidDimensions);
    }
    if !defaults.fps.is_finite() || !(MIN_PROJECT_FPS..=MAX_PROJECT_FPS).contains(&defaults.fps) {
        return Err(AppPreferencesError::InvalidFps);
    }
    if !defaults.loudness_lufs.is_finite() {
        return Err(AppPreferencesError::InvalidLoudness);
    }

    if let ProjectLocationPreference::SuggestedParent { parent_path } =
        &mut preferences.project_location
    {
        *parent_path = normalized_absolute_path(parent_path)
            .ok_or_else(|| AppPreferencesError::InvalidProjectPath(parent_path.clone()))?;
    }

    if preferences
        .claude_executable_path
        .chars()
        .any(char::is_control)
    {
        return Err(AppPreferencesError::InvalidClaudeExecutablePath(
            preferences.claude_executable_path.clone(),
        ));
    }
    preferences.claude_executable_path = preferences.claude_executable_path.trim().to_string();

    let mut canonical_model_ids = BTreeSet::new();
    for model_id in &preferences.enabled_generation_model_ids {
        let canonical = model_id.trim();
        let Some((provider, id)) = canonical.split_once(':') else {
            return Err(AppPreferencesError::InvalidGenerationModelId(
                model_id.clone(),
            ));
        };
        if provider.is_empty()
            || id.is_empty()
            || provider != provider.to_ascii_lowercase()
            || !provider.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '-')
            })
            || !is_supported_provider(provider)
            || id.chars().any(char::is_whitespace)
            || id.chars().any(char::is_control)
        {
            return Err(AppPreferencesError::InvalidGenerationModelId(
                model_id.clone(),
            ));
        }
        canonical_model_ids.insert(canonical.to_string());
    }
    preferences.enabled_generation_model_ids = canonical_model_ids.into_iter().collect();
    Ok(())
}

fn normalized_absolute_path(raw: &str) -> Option<String> {
    let path = Path::new(raw.trim());
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized.to_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::CaptionRenderMode;

    #[test]
    fn missing_store_uses_safe_v2_defaults() {
        let root = tempfile::tempdir().unwrap();
        let store = AppPreferencesStore::new(root.path().join("preferences.json"));
        let preferences = store.load_or_migrate(None).unwrap();
        assert_eq!(preferences.schema_version, 2);
        assert!(preferences.require_provider_upload_confirmation);
        assert!(!preferences.render_completion_notifications);
        assert!(preferences.enabled_generation_model_ids.is_empty());
        assert_eq!(preferences.new_project_defaults.width, 1920);
        assert_eq!(preferences.new_project_defaults.height, 1080);
        assert_eq!(preferences.new_project_defaults.fps, 30.0);
    }

    #[test]
    fn v1_migration_drops_environment_and_negative_model_fields() {
        let root = tempfile::tempdir().unwrap();
        let store = AppPreferencesStore::new(root.path().join("preferences.json"));
        let migrated = store
            .load_or_migrate(Some(LegacyAppPreferencesV1 {
                provider_credential_env_var: Some("FAL_KEY".into()),
                disabled_generation_model_ids: vec![],
                require_provider_upload_confirmation: Some(false),
                render_completion_notifications: Some(true),
                project_location: Some(ProjectLocationPreference::Ask),
                generation_execution_backend: Some(GenerationExecutionBackend::InProcess),
            }))
            .unwrap();
        assert!(!migrated.require_provider_upload_confirmation);
        assert!(migrated.render_completion_notifications);
        assert!(migrated.enabled_generation_model_ids.is_empty());
        let serialized = serde_json::to_string(&migrated).unwrap();
        assert!(!serialized.contains("providerCredentialEnvVar"));
        assert!(!serialized.contains("disabledGenerationModelIds"));
    }

    #[test]
    fn default_preferences_pick_the_automatic_agent_backend() {
        let preferences = AppPreferencesV2::default();
        assert_eq!(preferences.agent_backend, AgentBackendPreference::Automatic);
        assert_eq!(preferences.claude_model, ClaudeModelPreference::Sonnet);
        assert_eq!(preferences.claude_executable_override(), None);
    }

    #[test]
    fn stored_preferences_without_the_agent_keys_default_to_automatic() {
        let mut stored =
            serde_json::to_value(AppPreferencesV2::default()).expect("serialize defaults");
        let object = stored.as_object_mut().expect("preferences object");
        object.remove("agentBackend");
        object.remove("claudeModel");
        object.remove("claudeExecutablePath");
        let preferences =
            serde_json::from_value::<AppPreferencesV2>(stored).expect("deserialize defaults");
        assert_eq!(preferences.agent_backend, AgentBackendPreference::Automatic);
        assert_eq!(preferences.claude_model, ClaudeModelPreference::Sonnet);
        assert_eq!(preferences.claude_executable_override(), None);
    }

    #[test]
    fn each_claude_model_falls_back_to_the_next_cheaper_one() {
        assert_eq!(ClaudeModelPreference::Opus.fallback_alias(), Some("sonnet"));
        assert_eq!(
            ClaudeModelPreference::Sonnet.fallback_alias(),
            Some("haiku")
        );
        // Haiku is the cheapest alias, so there is nothing cheaper to fall back to.
        assert_eq!(ClaudeModelPreference::Haiku.fallback_alias(), None);
    }

    #[test]
    fn the_claude_agent_backend_round_trips_through_camel_case_json() {
        let mut stored =
            serde_json::to_value(AppPreferencesV2::default()).expect("serialize defaults");
        stored["agentBackend"] = serde_json::json!("claude");
        stored["claudeModel"] = serde_json::json!("opus");
        let preferences =
            serde_json::from_value::<AppPreferencesV2>(stored).expect("deserialize claude backend");
        assert_eq!(preferences.agent_backend, AgentBackendPreference::Claude);
        assert_eq!(preferences.claude_model, ClaudeModelPreference::Opus);
        let reserialized = serde_json::to_value(&preferences).expect("reserialize");
        assert_eq!(reserialized["agentBackend"], serde_json::json!("claude"));
        assert_eq!(reserialized["claudeModel"], serde_json::json!("opus"));
    }

    #[test]
    fn an_unknown_agent_backend_is_a_deserialize_error() {
        let mut stored =
            serde_json::to_value(AppPreferencesV2::default()).expect("serialize defaults");
        stored["agentBackend"] = serde_json::json!("gemini");
        let error = serde_json::from_value::<AppPreferencesV2>(stored).unwrap_err();
        assert!(
            error.to_string().contains("gemini"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn an_agent_backend_patch_round_trips_through_the_store() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        let store = AppPreferencesStore::new(path.clone());
        let updated = store
            .update(AppPreferencesPatch {
                agent_backend: Some(AgentBackendPreference::Claude),
                claude_model: Some(ClaudeModelPreference::Haiku),
                claude_executable_path: Some("  /opt/claude/bin/claude  ".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(updated.agent_backend, AgentBackendPreference::Claude);
        assert_eq!(updated.claude_model, ClaudeModelPreference::Haiku);
        assert_eq!(
            updated.claude_executable_override(),
            Some("/opt/claude/bin/claude")
        );

        let reloaded = AppPreferencesStore::new(path)
            .load_saved_or_default()
            .unwrap();
        assert_eq!(reloaded.agent_backend, AgentBackendPreference::Claude);
        assert_eq!(reloaded.claude_model, ClaudeModelPreference::Haiku);
        assert_eq!(
            reloaded.claude_executable_override(),
            Some("/opt/claude/bin/claude")
        );
    }

    #[test]
    fn a_blank_claude_executable_path_clears_the_override() {
        let root = tempfile::tempdir().unwrap();
        let store = AppPreferencesStore::new(root.path().join("preferences.json"));
        store
            .update(AppPreferencesPatch {
                claude_executable_path: Some("/opt/claude/bin/claude".to_string()),
                ..Default::default()
            })
            .unwrap();
        let cleared = store
            .update(AppPreferencesPatch {
                claude_executable_path: Some("   ".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(cleared.claude_executable_override(), None);
        assert_eq!(cleared.claude_executable_path, "");
    }

    #[test]
    fn a_claude_executable_path_with_control_characters_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let store = AppPreferencesStore::new(root.path().join("preferences.json"));
        let error = store
            .update(AppPreferencesPatch {
                claude_executable_path: Some("/opt/claude\n--evil".to_string()),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(
            error.code(),
            "settings.preferences.invalidClaudeExecutablePath"
        );
    }

    #[test]
    fn writes_are_atomic_and_invalid_dimensions_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let store = AppPreferencesStore::new(root.path().join("preferences.json"));
        let error = store
            .update(AppPreferencesPatch {
                new_project_defaults: Some(NewProjectDefaults {
                    width: 0,
                    height: 1080,
                    fps: 30.0,
                    loudness_lufs: -14.0,
                    captions: CaptionRenderMode::BurnIn,
                }),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(error.code(), "settings.preferences.invalidDimensions");
        assert!(!root.path().join("preferences.json").exists());
    }
}
