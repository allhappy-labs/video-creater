use super::*;
use crate::app_service::events::{EventSink, ServiceEvent};
use crate::provider_credentials::{self, ProviderCredentialError};
use crate::settings::health;
use crate::settings::operations::{
    SettingsOperation, SettingsOperationError, SettingsOperationKind, SettingsOperationRegistry,
    SettingsOperationState, SettingsOperationTransition,
};
use crate::settings::preferences::{AppPreferencesError, AppPreferencesPatch, AppPreferencesStore};
use crate::settings::providers::aggregate_provider_health;
use crate::speech_models::ProductionSpeechModelStore;
use crate::transcription::acquisition::ModelAcquisitionError;
use crate::transcription::model::transcription_model_catalog_entry;
use crate::transcription::model::ModelInstallStatus;
use crate::transcription::runtime::{
    InstalledModel, ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability,
    RuntimeSelection,
};
use crate::transcription::store::{
    default_global_transcription_model_root, ModelStoreError, StartDownload,
    TranscriptionModelStore,
};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[cfg(not(target_os = "linux"))]
use crate::transcription::{
    fluidaudio::FluidAudioCoreMlRuntime as PlatformRuntime,
    model::FLUID_AUDIO_COREML_RUNTIME_ID as RUNTIME_ID,
};
#[cfg(target_os = "linux")]
use crate::transcription::{
    model::SHERPA_ONNX_RUNTIME_ID as RUNTIME_ID, sherpa_onnx::SherpaOnnxRuntime as PlatformRuntime,
};

pub(super) struct HostSettings {
    root: PathBuf,
    preferences: Mutex<AppPreferencesStore>,
    models: Mutex<TranscriptionModelStore>,
    speech: ProductionSpeechModelStore,
    registry: Mutex<Option<Arc<SettingsOperationRegistry>>>,
    coordinator: Mutex<()>,
    events: Mutex<Option<Arc<dyn EventSink>>>,
}

impl Default for HostSettings {
    fn default() -> Self {
        let models = default_global_transcription_model_root();
        Self::new(models.parent().unwrap_or(Path::new(".")).to_path_buf())
    }
}

impl HostSettings {
    pub(super) fn generation_backend_in_process(&self) -> Result<bool, ServiceError> {
        self.preferences
            .lock()
            .map_err(internal)?
            .load_saved_or_default()
            .map_err(preferences_error)
            .map(|preferences| {
                preferences.generation_execution_backend
                    == crate::settings::preferences::GenerationExecutionBackend::InProcess
            })
    }

    fn new(root: PathBuf) -> Self {
        Self {
            preferences: Mutex::new(AppPreferencesStore::new(
                root.join("settings/preferences.json"),
            )),
            models: Mutex::new(TranscriptionModelStore::new(root.join("models"))),
            speech: ProductionSpeechModelStore::new(root.join("models")),
            registry: Mutex::new(None),
            coordinator: Mutex::new(()),
            events: Mutex::new(None),
            root,
        }
    }

    pub(super) fn set_event_sink(&self, sink: Arc<dyn EventSink>) {
        *self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sink);
    }

    fn publish(&self, operation: &SettingsOperation) {
        let sink = self.events.lock().ok().and_then(|sink| sink.clone());
        if let Some(sink) = sink {
            if let Ok(payload) = serde_json::to_value(operation) {
                let _ = sink.publish(ServiceEvent::new("settings-operation", None, payload));
            }
        }
    }

    fn registry(&self) -> Result<Arc<SettingsOperationRegistry>, ServiceError> {
        let mut registry = self.registry.lock().map_err(internal)?;
        if let Some(registry) = registry.as_ref() {
            return Ok(Arc::clone(registry));
        }
        let loaded = SettingsOperationRegistry::load(
            self.root.join("settings/operations.json"),
            |operation| match operation.kind {
                SettingsOperationKind::ModelDownload => self
                    .models
                    .lock()
                    .ok()
                    .and_then(|models| models.status(&operation.target_id).ok())
                    .is_some_and(|status| status.install_status == ModelInstallStatus::Ready),
                SettingsOperationKind::SpeechModelsDownload => self.speech.status().ready,
                _ => false,
            },
        )
        .map_err(internal)?;
        let loaded = Arc::new(loaded);
        *registry = Some(Arc::clone(&loaded));
        Ok(loaded)
    }

    fn update_operation(
        &self,
        registry: &SettingsOperationRegistry,
        id: &str,
        transition: SettingsOperationTransition,
    ) -> Result<SettingsOperation, ServiceError> {
        let operation = registry.update(id, transition).map_err(internal)?;
        self.publish(&operation);
        Ok(operation)
    }
}

impl HostDispatcher {
    /// Configure application-owned settings/model storage; RPC payloads cannot choose these paths.
    pub fn with_settings_root(mut self, root: PathBuf) -> Self {
        self.settings = Arc::new(HostSettings::new(root));
        self
    }

    pub(super) fn dispatch_settings(
        &self,
        request: &RpcEnvelope,
    ) -> Option<Result<Value, ServiceError>> {
        let payload = &request.payload;
        Some(match request.operation.as_str() {
            "get_app_preferences" => self
                .settings
                .preferences
                .lock()
                .map_err(internal)
                .and_then(|store| store.load_saved_or_default().map_err(preferences_error))
                .and_then(value),
            "update_app_preferences" => decode::<AppPreferencesPatch>(payload, "patch")
                .and_then(|patch| {
                    self.settings
                        .preferences
                        .lock()
                        .map_err(internal)?
                        .update(patch)
                        .map_err(preferences_error)
                })
                .and_then(value),
            "list_transcription_models" => self
                .settings
                .models
                .lock()
                .map_err(internal)
                .and_then(|models| models.list().map_err(model_error))
                .and_then(value),
            "get_active_transcription_model" => self
                .settings
                .models
                .lock()
                .map_err(internal)
                .and_then(|models| models.active_status().map_err(model_error))
                .and_then(value),
            "set_active_transcription_model" => text(payload, "modelId")
                .and_then(|id| {
                    self.settings
                        .models
                        .lock()
                        .map_err(internal)?
                        .set_active_model(id)
                        .map_err(model_error)
                })
                .and_then(value),
            "get_transcription_runtime_status" => self.runtime_selection(payload).and_then(value),
            "get_production_speech_model_status" => value(self.settings.speech.status()),
            "verify_production_speech_models" => self
                .settings
                .speech
                .verify()
                .map_err(internal)
                .and_then(value),
            "remove_production_speech_models" => self.remove_speech_models().and_then(value),
            "verify_transcription_model" => text(payload, "modelId")
                .and_then(|id| {
                    let model = self.settings.models.lock().map_err(internal)?.clone();
                    model.verify(id).map_err(model_error)
                })
                .and_then(value),
            "remove_transcription_model" => {
                self.remove_transcription_model(payload).and_then(value)
            }
            "download_transcription_model" => self.start_model_download(payload).and_then(value),
            "cancel_model_download" => self.cancel_model_download(payload).and_then(value),
            "download_production_speech_models" => self.start_speech_download().and_then(value),
            "list_settings_operations" => self
                .settings
                .registry()
                .and_then(|registry| registry.list_recent().map_err(internal))
                .and_then(value),
            "cancel_settings_operation" => self.cancel_settings_operation(payload).and_then(value),
            "list_provider_credential_statuses" => {
                value(provider_credentials::list_provider_credential_statuses())
            }
            "set_provider_credential" => (|| {
                let provider = text(payload, "provider")?;
                let credential = text(payload, "credential")?;
                value(
                    provider_credentials::set_provider_credential(provider.trim(), credential)
                        .map_err(credential_error)?,
                )
            })(),
            "delete_provider_credential" => text(payload, "provider")
                .and_then(|provider| {
                    provider_credentials::delete_provider_credential(provider.trim())
                        .map_err(credential_error)
                })
                .and_then(value),
            "mcp_client_configuration" => self.settings_project_root(request).and_then(|root| {
                value(crate::settings::agent::mcp_client_configuration(
                    root.as_deref(),
                ))
            }),
            "check_render_system"
            | "run_agent_component_self_test"
            | "refresh_storage_inventory" => self.start_settings_check(request).and_then(value),
            "get_update_health" => Ok(
                json!({"state":"unavailable", "installedVersion":env!("CARGO_PKG_VERSION"), "summary":"Updates are not configured for this host build"}),
            ),
            "import_transcription_model" => Err(ServiceError::unavailable(
                "remote model import by host filesystem path",
            )),
            "get_notification_capability" | "request_notification_permission" => {
                Err(ServiceError::unavailable("native desktop notifications"))
            }
            "reveal_storage_inventory_item" => {
                Err(ServiceError::unavailable("native desktop file reveal"))
            }
            "preview_storage_cleanup" | "run_storage_cleanup" => {
                Err(ServiceError::unavailable("remote storage cleanup"))
            }
            "repair_bundled_skills" => {
                Err(ServiceError::unavailable("remote project skill repair"))
            }
            "refresh_provider_health" => Err(ServiceError::unavailable(
                "remote provider account validation",
            )),
            "get_provider_health" => self.provider_health().and_then(value),
            "get_settings_health_snapshot"
            | "get_system_health_snapshot"
            | "refresh_system_health_section"
            | "get_agent_settings_health"
            | "get_skills_settings_health"
            | "get_storage_health"
            | "get_render_system_health" => self.settings_health(request),
            _ => return None,
        })
    }

    fn start_settings_check(
        &self,
        request: &RpcEnvelope,
    ) -> Result<SettingsOperation, ServiceError> {
        let root = self.settings_project_root(request)?;
        let (kind, target) = match request.operation.as_str() {
            "check_render_system" => (
                SettingsOperationKind::HealthCheck,
                "render-system".to_string(),
            ),
            "run_agent_component_self_test" => {
                let target = text(&request.payload, "componentId")?;
                if ![
                    "agent.codex",
                    "agent.claude",
                    "agent.mcpServer",
                    "agent.proposalValidator",
                ]
                .contains(&target)
                {
                    return Err(ServiceError::invalid_input("unknown agent component"));
                }
                (SettingsOperationKind::HealthCheck, target.to_string())
            }
            "refresh_storage_inventory" => (
                SettingsOperationKind::StorageRefresh,
                match root.as_ref() {
                    Some(root) => format!(
                        "storage:{}",
                        self.projects()
                            .map_err(internal)?
                            .id_for_path(root)
                            .map_err(internal)?
                    ),
                    None => "storage".into(),
                },
            ),
            _ => return Err(ServiceError::invalid_input("unknown settings check")),
        };
        let registry = self.settings.registry()?;
        let (operation, created) = registry
            .resolve_or_start_active_configured(kind, target, None, None, |operation| {
                operation.cancellable = false;
                operation.message = "Queued settings check.".into();
            })
            .map_err(internal)?;
        if !created {
            return Ok(operation);
        }
        self.settings.publish(&operation);
        let settings = Arc::clone(&self.settings);
        let operation_id = operation.id.clone();
        let operation_name = request.operation.clone();
        let target = operation.target_id.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
                running.phase = Some("checking".into());
                running.message = Some("Checking settings.".into());
                settings.update_operation(&registry, &operation_id, running)?;
                let mut terminal =
                    SettingsOperationTransition::to(SettingsOperationState::Succeeded);
                terminal.phase = Some("checked".into());
                terminal.message = Some("Settings check completed.".into());
                match operation_name.as_str() {
                    "check_render_system" => {
                        let health = crate::settings::render_system::get_render_system_health();
                        terminal.completed_units = Some(health.items.len() as u64);
                        terminal.total_units = Some(health.items.len() as u64);
                        terminal.unit = Some("components".into());
                        if !health.composition_ready {
                            terminal.state = SettingsOperationState::Failed;
                            terminal.error = Some(SettingsOperationError {
                                code: "settings.renderHealth.notReady".into(),
                                message: "Required render system is not ready.".into(),
                                recovery_action: Some("Review render-system diagnostics.".into()),
                                detail: None,
                            });
                            terminal.message = Some("Required render system is not ready.".into());
                        }
                    }
                    "run_agent_component_self_test" => {
                        let preferences = settings
                            .preferences
                            .lock()
                            .map_err(internal)?
                            .load_saved_or_default()
                            .map_err(preferences_error)?;
                        let result = crate::settings::agent::run_agent_component_self_test(
                            &target,
                            preferences.claude_executable_override(),
                        );
                        terminal.state = result.state;
                        terminal.phase = Some(result.phase);
                        terminal.message = Some(result.message);
                        terminal.error = result.error;
                        terminal.completed_units = Some(result.completed_units);
                        terminal.total_units = result.total_units;
                        terminal.unit = result.unit;
                    }
                    "refresh_storage_inventory" => {
                        let inventory = crate::settings::storage::collect_storage_inventory(
                            &settings.root.join("models"),
                            &settings.root.join("cache"),
                            root.as_deref(),
                        );
                        terminal.completed_units = Some(inventory.len() as u64);
                        terminal.total_units = Some(inventory.len() as u64);
                        terminal.unit = Some("items".into());
                        terminal.message = Some("Storage inventory refreshed.".into());
                    }
                    _ => return Err(ServiceError::invalid_input("unknown settings check")),
                }
                settings.update_operation(&registry, &operation_id, terminal)?;
                Ok(())
            })();
            if let Err(error) = result {
                finish_failed_operation(&settings, &registry, &operation_id, error);
            }
        });
        Ok(operation)
    }

    fn settings_project_root(
        &self,
        request: &RpcEnvelope,
    ) -> Result<Option<PathBuf>, ServiceError> {
        let mut project_id = request.project_id.as_deref();
        for field in ["projectRoot", "activeProjectDir"] {
            match request.payload.get(field) {
                None | Some(Value::Null) => {}
                Some(Value::String(id)) if id.trim().is_empty() => {}
                Some(Value::String(id)) => {
                    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(ServiceError::invalid_input(
                            "settings project root must be an opaque project ID",
                        ));
                    }
                    if project_id.is_some_and(|existing| existing != id) {
                        return Err(ServiceError::invalid_input(
                            "settings project scopes differ",
                        ));
                    }
                    project_id = Some(id);
                }
                _ => {
                    return Err(ServiceError::invalid_input(
                        "settings project root is invalid",
                    ))
                }
            }
        }
        project_id
            .map(|id| {
                self.projects()
                    .map_err(internal)?
                    .resolve(id)
                    .map_err(|_| ServiceError::not_found("project"))
            })
            .transpose()
    }

    fn provider_health(
        &self,
    ) -> Result<Vec<crate::settings::providers::ProviderHealth>, ServiceError> {
        aggregate_provider_health(
            &provider_credentials::list_provider_credential_statuses(),
            &[],
            &[],
            None,
        )
        .map_err(internal)
    }

    fn settings_health(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let root = self.settings_project_root(request)?;
        let preferences = self
            .settings
            .preferences
            .lock()
            .map_err(internal)?
            .load_saved_or_default()
            .map_err(preferences_error)?;
        let claude = preferences.claude_executable_override();
        match request.operation.as_str() {
            "get_agent_settings_health" => value(health::get_agent_settings_health(claude)),
            "get_skills_settings_health" => {
                value(health::get_skills_settings_health(root.as_deref()))
            }
            "get_render_system_health" => {
                value(crate::settings::render_system::get_render_system_health())
            }
            operation => {
                let inventory = crate::settings::storage::collect_storage_inventory(
                    &self.settings.root.join("models"),
                    &self.settings.root.join("cache"),
                    root.as_deref(),
                );
                match operation {
                    "get_storage_health" => value(health::storage_category_health(
                        &inventory,
                        &chrono::Utc::now().to_rfc3339(),
                    )),
                    "get_settings_health_snapshot" => value(
                        health::build_settings_health_snapshot(
                            &self.settings.models,
                            root.as_deref(),
                            &self.provider_health()?,
                            Some(&inventory),
                            claude,
                        )
                        .map_err(internal)?,
                    ),
                    "get_system_health_snapshot" => value(health::build_system_health_snapshot(
                        &self.settings.models,
                        root.as_deref(),
                        &inventory,
                        claude,
                    )),
                    "refresh_system_health_section" => value(
                        health::build_system_health_section(
                            text(&request.payload, "sectionId")?,
                            &self.settings.models,
                            root.as_deref(),
                            &inventory,
                            claude,
                        )
                        .map_err(|_| {
                            ServiceError::invalid_input("unknown system health section")
                        })?,
                    ),
                    _ => Err(ServiceError::invalid_input(
                        "unknown settings health request",
                    )),
                }
            }
        }
    }

    fn runtime_selection(&self, payload: &Value) -> Result<RuntimeSelection, ServiceError> {
        let models = self.settings.models.lock().map_err(internal)?;
        let id = match payload.get("modelId") {
            None | Some(Value::Null) => models.active_model_id().map_err(model_error)?,
            Some(Value::String(id)) => id.clone(),
            _ => return Err(ServiceError::invalid_input("model ID is invalid")),
        };
        let entry = transcription_model_catalog_entry(&id)
            .ok_or_else(|| ServiceError::invalid_input("unsupported transcription model"))?;
        let model = InstalledModel {
            model_id: id,
            model_dir: models
                .runtime_model_dir(entry.id, RUNTIME_ID)
                .map_err(model_error)?,
            artifact_format: ModelArtifactFormat::from(entry.artifact_format),
            modality: ModelModality::Transcription,
            runtime_family: RUNTIME_ID.into(),
        };
        let runtime = PlatformRuntime::new(PlatformRuntime::default_helper_path());
        if !runtime.supports_installed_model(&model) {
            return Ok(RuntimeSelection::Unavailable);
        }
        Ok(match runtime.probe_installed_model(&model) {
            RuntimeCapability::Ready => RuntimeSelection::Native,
            RuntimeCapability::UnsupportedPlatform => RuntimeSelection::UnsupportedPlatform,
            RuntimeCapability::Unavailable => RuntimeSelection::Unavailable,
        })
    }

    fn remove_transcription_model(
        &self,
        payload: &Value,
    ) -> Result<crate::transcription::store::TranscriptionModelStatus, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let id = text(payload, "modelId")?;
        let models = self.settings.models.lock().map_err(internal)?;
        if models.download_is_active(id).map_err(model_error)? {
            return Err(ServiceError::busy("model download"));
        }
        let status = models.remove(id).map_err(model_error)?;
        drop(models);
        self.settings
            .registry()?
            .invalidate_target(SettingsOperationKind::ModelDownload, id)
            .map_err(internal)?;
        Ok(status)
    }

    fn remove_speech_models(
        &self,
    ) -> Result<crate::speech_models::ProductionSpeechModelStatus, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let registry = self.settings.registry()?;
        if registry
            .list_active()
            .map_err(internal)?
            .iter()
            .any(|operation| operation.kind == SettingsOperationKind::SpeechModelsDownload)
        {
            return Err(ServiceError::busy("speech model download"));
        }
        let status = self.settings.speech.remove().map_err(internal)?;
        registry
            .invalidate_target(
                SettingsOperationKind::SpeechModelsDownload,
                &status.model_set_id,
            )
            .map_err(internal)?;
        Ok(status)
    }

    fn start_model_download(&self, payload: &Value) -> Result<SettingsOperation, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let id = text(payload, "modelId")?.to_string();
        let model = self.settings.models.lock().map_err(internal)?.clone();
        let status = model.status(&id).map_err(model_error)?;
        let registry = self.settings.registry()?;
        let (operation, created) = registry
            .resolve_or_start_active(
                SettingsOperationKind::ModelDownload,
                id.clone(),
                Some(status.approximate_size_bytes),
                Some("bytes".into()),
            )
            .map_err(internal)?;
        if !created {
            return Ok(operation);
        }
        let token = match model
            .mark_download_started(&id, status.total_files)
            .map_err(model_error)?
        {
            StartDownload::Started { token, .. } => token,
            StartDownload::AlreadyActive(_) => return Err(ServiceError::busy("model download")),
        };
        self.settings.publish(&operation);
        let settings = Arc::clone(&self.settings);
        let operation_id = operation.id.clone();
        std::thread::spawn(move || {
            let result = (|| {
                begin_download(&settings, &registry, &operation_id)?;
                model
                    .download_reserved_with_observer_and_publisher(
                        &id,
                        token,
                        |progress| {
                            if registry
                                .get(&operation_id)
                                .map_err(|error| {
                                    ModelStoreError::ProgressPersistence(error.to_string())
                                })?
                                .state
                                == SettingsOperationState::Cancelling
                            {
                                return Err(ModelStoreError::Acquisition(
                                    ModelAcquisitionError::Cancelled,
                                ));
                            }
                            let mut transition =
                                SettingsOperationTransition::to(SettingsOperationState::Running);
                            transition.completed_units = Some(progress.downloaded_bytes);
                            transition.total_units = Some(progress.total_bytes);
                            settings
                                .update_operation(&registry, &operation_id, transition)
                                .map_err(|error| {
                                    ModelStoreError::ProgressPersistence(error.to_string())
                                })?;
                            Ok(())
                        },
                        |publish| {
                            let _coordinator = settings.coordinator.lock().map_err(|_| {
                                ModelStoreError::ProgressPersistence(
                                    "model coordinator unavailable".into(),
                                )
                            })?;
                            if registry
                                .get(&operation_id)
                                .map_err(|error| {
                                    ModelStoreError::ProgressPersistence(error.to_string())
                                })?
                                .state
                                == SettingsOperationState::Cancelling
                            {
                                return Ok(false);
                            }
                            let published = publish.publish()?;
                            if published {
                                let mut complete = SettingsOperationTransition::to(
                                    SettingsOperationState::Succeeded,
                                );
                                complete.phase = Some("ready".into());
                                complete.message =
                                    Some("Model download completed and verified.".into());
                                settings
                                    .update_operation(&registry, &operation_id, complete)
                                    .map_err(|error| {
                                        ModelStoreError::ProgressPersistence(error.to_string())
                                    })?;
                            }
                            Ok(published)
                        },
                    )
                    .map_err(internal)?;
                Ok(())
            })();
            if let Err(error) = result {
                let _ = model.cancel_download_if_active(&id);
                finish_failed_operation(&settings, &registry, &operation_id, error);
            }
        });
        Ok(operation)
    }

    fn cancel_model_download(
        &self,
        payload: &Value,
    ) -> Result<crate::transcription::store::TranscriptionModelStatus, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let id = text(payload, "modelId")?;
        self.settings
            .models
            .lock()
            .map_err(internal)?
            .status(id)
            .map_err(model_error)?;
        let registry = self.settings.registry()?;
        if let Some(operation) =
            registry
                .list_active()
                .map_err(internal)?
                .into_iter()
                .find(|operation| {
                    operation.kind == SettingsOperationKind::ModelDownload
                        && operation.target_id == id
                })
        {
            let operation = registry.request_cancel(&operation.id).map_err(internal)?;
            self.settings.publish(&operation);
        }
        self.settings
            .models
            .lock()
            .map_err(internal)?
            .cancel_download(id)
            .map_err(model_error)
    }

    fn cancel_settings_operation(
        &self,
        payload: &Value,
    ) -> Result<SettingsOperation, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let registry = self.settings.registry()?;
        let operation = registry
            .request_cancel(text(payload, "operationId")?)
            .map_err(operation_error)?;
        if operation.kind == SettingsOperationKind::ModelDownload {
            self.settings
                .models
                .lock()
                .map_err(internal)?
                .cancel_download_if_active(&operation.target_id)
                .map_err(model_error)?;
        }
        self.settings.publish(&operation);
        Ok(operation)
    }

    fn start_speech_download(&self) -> Result<SettingsOperation, ServiceError> {
        let _coordinator = self.settings.coordinator.lock().map_err(internal)?;
        let status = self.settings.speech.status();
        let registry = self.settings.registry()?;
        let (operation, created) = registry
            .resolve_or_start_active(
                SettingsOperationKind::SpeechModelsDownload,
                status.model_set_id,
                Some(status.total_bytes),
                Some("bytes".into()),
            )
            .map_err(internal)?;
        if !created {
            return Ok(operation);
        }
        self.settings.publish(&operation);
        let settings = Arc::clone(&self.settings);
        let operation_id = operation.id.clone();
        std::thread::spawn(move || {
            let result = (|| {
                begin_download(&settings, &registry, &operation_id)?;
                settings
                    .speech
                    .download_and_verify_with_cancellation(
                        |progress| {
                            if registry
                                .get(&operation_id)
                                .map_err(|error| {
                                    crate::speech_models::SpeechModelError::ProgressPersistence(
                                        error.to_string(),
                                    )
                                })?
                                .state
                                == SettingsOperationState::Cancelling
                            {
                                return Err(crate::speech_models::SpeechModelError::Cancelled);
                            }
                            let mut transition =
                                SettingsOperationTransition::to(SettingsOperationState::Running);
                            transition.completed_units = Some(progress.downloaded_bytes);
                            transition.total_units = Some(progress.total_bytes);
                            settings
                                .update_operation(&registry, &operation_id, transition)
                                .map_err(|error| {
                                    crate::speech_models::SpeechModelError::ProgressPersistence(
                                        error.to_string(),
                                    )
                                })?;
                            Ok(())
                        },
                        || {
                            if registry
                                .get(&operation_id)
                                .map_err(|error| {
                                    crate::speech_models::SpeechModelError::ProgressPersistence(
                                        error.to_string(),
                                    )
                                })?
                                .state
                                == SettingsOperationState::Cancelling
                            {
                                return Err(crate::speech_models::SpeechModelError::Cancelled);
                            }
                            Ok(())
                        },
                    )
                    .map_err(internal)?;
                let _coordinator = settings.coordinator.lock().map_err(internal)?;
                if registry.get(&operation_id).map_err(internal)?.state
                    == SettingsOperationState::Cancelling
                {
                    return Err(ServiceError::cancelled());
                }
                let mut complete =
                    SettingsOperationTransition::to(SettingsOperationState::Succeeded);
                complete.phase = Some("ready".into());
                complete.message = Some("Speech models downloaded and verified.".into());
                settings.update_operation(&registry, &operation_id, complete)?;
                Ok(())
            })();
            if let Err(error) = result {
                finish_failed_operation(&settings, &registry, &operation_id, error);
            }
        });
        Ok(operation)
    }
}

fn begin_download(
    settings: &HostSettings,
    registry: &SettingsOperationRegistry,
    id: &str,
) -> Result<(), ServiceError> {
    let _coordinator = settings.coordinator.lock().map_err(internal)?;
    if registry.get(id).map_err(internal)?.state == SettingsOperationState::Cancelling {
        return Err(ServiceError::cancelled());
    }
    let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
    running.phase = Some("downloading".into());
    running.message = Some("Downloading verified model files.".into());
    settings.update_operation(registry, id, running)?;
    Ok(())
}

fn finish_failed_operation(
    settings: &HostSettings,
    registry: &SettingsOperationRegistry,
    id: &str,
    error: ServiceError,
) {
    let _coordinator = match settings.coordinator.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let cancelled = registry
        .get(id)
        .is_ok_and(|operation| operation.state == SettingsOperationState::Cancelling);
    let mut terminal = SettingsOperationTransition::to(if cancelled {
        SettingsOperationState::Cancelled
    } else {
        SettingsOperationState::Failed
    });
    terminal.phase = Some(if cancelled { "cancelled" } else { "failed" }.into());
    terminal.message = Some(
        if cancelled {
            "Settings operation cancelled."
        } else {
            "Settings operation failed."
        }
        .into(),
    );
    if !cancelled {
        terminal.error = Some(SettingsOperationError {
            code: "settings.operation.failed".into(),
            message: "Settings operation failed.".into(),
            recovery_action: Some("Review diagnostics and retry the operation.".into()),
            detail: Some(error.to_string()),
        });
    }
    if settings.update_operation(registry, id, terminal).is_err() {
        if let Ok(operation) = registry.project_persistence_failure(
            id,
            "settings.operation.terminalPersistenceFailed",
            "Settings operation history could not be persisted.",
            "Retry after storage is available.",
        ) {
            settings.publish(&operation);
        }
    }
}

fn decode<T: DeserializeOwned>(payload: &Value, field: &str) -> Result<T, ServiceError> {
    serde_json::from_value(
        payload
            .get(field)
            .cloned()
            .ok_or_else(|| ServiceError::invalid_input(format!("{field} is required")))?,
    )
    .map_err(|_| ServiceError::invalid_input(format!("{field} is invalid")))
}
fn text<'a>(payload: &'a Value, field: &str) -> Result<&'a str, ServiceError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ServiceError::invalid_input(format!("{field} is required")))
}
fn value<T: serde::Serialize>(value: T) -> Result<Value, ServiceError> {
    serde_json::to_value(value).map_err(internal)
}
fn internal(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::internal(error.to_string())
}
fn operation_error(
    error: crate::settings::operations::SettingsOperationRegistryError,
) -> ServiceError {
    use crate::settings::operations::SettingsOperationRegistryError;
    match error {
        SettingsOperationRegistryError::NotFound(_) => {
            ServiceError::not_found("settings operation")
        }
        SettingsOperationRegistryError::NotCancellable(_) => {
            ServiceError::invalid_input("settings operation is not cancellable")
        }
        SettingsOperationRegistryError::InvalidTransition { .. } => {
            ServiceError::busy("settings operation")
        }
        _ => internal(error),
    }
}
fn preferences_error(error: AppPreferencesError) -> ServiceError {
    match error {
        AppPreferencesError::UnsupportedSchema(_)
        | AppPreferencesError::InvalidDimensions
        | AppPreferencesError::InvalidFps
        | AppPreferencesError::InvalidLoudness
        | AppPreferencesError::InvalidGenerationModelId(_)
        | AppPreferencesError::InvalidProjectPath(_)
        | AppPreferencesError::InvalidClaudeExecutablePath(_) => {
            ServiceError::invalid_input("app preferences contain an invalid value")
        }
        _ => internal(error),
    }
}
fn model_error(error: ModelStoreError) -> ServiceError {
    match error {
        ModelStoreError::UnsupportedModel(_) | ModelStoreError::UnsupportedRuntime { .. } => {
            ServiceError::invalid_input("unsupported transcription model or runtime")
        }
        _ => internal(error),
    }
}
fn credential_error(error: ProviderCredentialError) -> ServiceError {
    match error {
        ProviderCredentialError::UnsupportedProvider(_)
        | ProviderCredentialError::BlankCredential
        | ProviderCredentialError::CredentialTooLarge => {
            ServiceError::invalid_input("provider or credential is invalid")
        }
        ProviderCredentialError::StoreUnavailable(_) => {
            ServiceError::unavailable("secure provider credential storage")
        }
        _ => ServiceError::internal("secure provider credential operation failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_service::events::FakeEventSink;

    #[test]
    fn cancellation_before_worker_start_persists_terminal_state_and_events() {
        let root = tempfile::tempdir().unwrap();
        let dispatcher = HostDispatcher::default().with_settings_root(root.path().to_path_buf());
        let sink = Arc::new(FakeEventSink::default());
        dispatcher.set_event_sink(sink.clone());
        let model = dispatcher.settings.models.lock().unwrap().clone();
        let status = model.list().unwrap().remove(0);
        model
            .mark_download_started(&status.model_id, status.total_files)
            .unwrap();
        let registry = dispatcher.settings.registry().unwrap();
        let (operation, _) = registry
            .resolve_or_start_active(
                SettingsOperationKind::ModelDownload,
                &status.model_id,
                None,
                None,
            )
            .unwrap();
        dispatcher
            .cancel_settings_operation(&json!({"operationId":operation.id}))
            .unwrap();
        assert!(!model.download_is_active(&status.model_id).unwrap());
        let cancelled = begin_download(&dispatcher.settings, &registry, &operation.id).unwrap_err();
        assert_eq!(
            cancelled.code(),
            crate::app_service::error::ServiceErrorCode::Cancelled
        );
        finish_failed_operation(&dispatcher.settings, &registry, &operation.id, cancelled);
        assert_eq!(
            registry.get(&operation.id).unwrap().state,
            SettingsOperationState::Cancelled
        );
        let restored =
            SettingsOperationRegistry::load(root.path().join("settings/operations.json"), |_| {
                false
            })
            .unwrap();
        assert_eq!(
            restored.get(&operation.id).unwrap().state,
            SettingsOperationState::Cancelled
        );
        assert_eq!(
            sink.recorded()
                .iter()
                .map(|event| event.payload["state"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["cancelling", "cancelled"]
        );
        assert!(!root.path().join("models").exists());
    }

    #[test]
    fn failed_worker_persists_failure_and_emits_no_success() {
        let root = tempfile::tempdir().unwrap();
        let settings = HostSettings::new(root.path().to_path_buf());
        let sink = Arc::new(FakeEventSink::default());
        settings.set_event_sink(sink.clone());
        let registry = settings.registry().unwrap();
        let (operation, _) = registry
            .resolve_or_start_active(
                SettingsOperationKind::SpeechModelsDownload,
                "fixture-speech",
                None,
                None,
            )
            .unwrap();
        begin_download(&settings, &registry, &operation.id).unwrap();
        finish_failed_operation(
            &settings,
            &registry,
            &operation.id,
            ServiceError::internal("isolated transfer failed"),
        );
        let result = registry.get(&operation.id).unwrap();
        assert_eq!(result.state, SettingsOperationState::Failed);
        assert!(result.error.is_some());
        assert_eq!(
            sink.recorded()
                .iter()
                .map(|event| event.payload["state"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["running", "failed"]
        );
        let restored =
            SettingsOperationRegistry::load(root.path().join("settings/operations.json"), |_| {
                false
            })
            .unwrap();
        assert_eq!(restored.get(&operation.id).unwrap(), result);
        assert!(!root.path().join("models").exists());
    }
}
