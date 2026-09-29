use super::acquisition::{
    HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    ModelDownloadProgressSnapshot,
};
use super::model::{
    catalog_entry_runtime_id, catalog_entry_runtime_label, required_files_dir_for_entry,
    runtime_model_subdir_name, safe_model_dir_name, transcription_model_catalog,
    InstalledModelManifest, InstalledModelSource, ModelArtifactSource, ModelInstallStatus,
    TranscriptionModelArtifactFormat, TranscriptionModelCatalogEntry, TranscriptionModelFile,
    FLUID_AUDIO_COREML_RUNTIME_ID, SHERPA_ONNX_RUNTIME_ID,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

pub const MODEL_MANIFEST_FILE_NAME: &str = "video-creater-model-manifest.json";
pub const COREML_INSPECTION_FILE_NAME: &str = "video-creater-coreml-inspection.json";
pub const VIDEO_CREATER_APP_IDENTIFIER: &str = "com.olhapi.video-creater";
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

pub fn default_global_transcription_model_root() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "linux")]
    {
        let xdg_data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        linux_global_transcription_model_root(xdg_data_home.as_deref(), &home)
    }
    #[cfg(not(target_os = "linux"))]
    {
        global_transcription_model_root_for_home(&home)
    }
}

/// Matches Tauri's Linux `app_data_dir()` (`$XDG_DATA_HOME` or `~/.local/share`, plus the bundle
/// identifier), so workflow workers and the desktop app share one model root.
pub fn linux_global_transcription_model_root(xdg_data_home: Option<&Path>, home: &Path) -> PathBuf {
    xdg_data_home
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join(".local").join("share"))
        .join(VIDEO_CREATER_APP_IDENTIFIER)
        .join("models")
}

pub fn global_transcription_model_root_for_home(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join(VIDEO_CREATER_APP_IDENTIFIER)
        .join("models")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelStatus {
    pub model_id: String,
    pub display_name: String,
    pub is_active: bool,
    pub install_status: ModelInstallStatus,
    pub local_path: String,
    pub approximate_size_bytes: u64,
    pub installed_bytes: u64,
    pub downloaded_files: u32,
    pub total_files: u32,
    pub source_repo_id: Option<String>,
    pub source_revision: Option<String>,
    pub source_license: Option<String>,
    pub artifact_format: String,
    pub runtime_id: String,
    pub runtime_label: String,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreMlInspectionSummary {
    schema_version: u32,
    model_id: String,
    runtime_id: &'static str,
    model_root: String,
    runtime_model_dir: String,
    bundles: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ModelStoreError {
    #[error("unsupported transcription model {0}")]
    UnsupportedModel(String),
    #[error("unsupported transcription runtime {runtime_id} for model {model_id}")]
    UnsupportedRuntime {
        model_id: String,
        runtime_id: String,
    },
    #[error("failed to check model path {path}: {source}")]
    CheckPath {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize model manifest: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write model manifest {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to remove model directory {path}: {source}")]
    Remove {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to create model import directory {path}: {source}")]
    CreateImportDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to read model import directory {path}: {source}")]
    ReadImportDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to copy model file from {source_path} to {target_path}: {source}")]
    CopyImportFile {
        source_path: String,
        target_path: String,
        source: std::io::Error,
    },
    #[error("failed to replace imported model directory {path}: {source}")]
    ReplaceImportDir {
        path: String,
        source: std::io::Error,
    },
    #[error("missing imported model file {0}")]
    MissingImportFile(String),
    #[error("failed to download model: {0}")]
    Download(String),
    #[error(transparent)]
    Acquisition(#[from] ModelAcquisitionError),
    #[error("downloaded model manifest is invalid: {0}")]
    ManifestInvalid(String),
    #[error("downloaded model hash mismatch: {0}")]
    HashMismatch(String),
    #[error("downloaded Core ML bundle is invalid: {0}")]
    CoreMlInvalid(String),
    #[error("insufficient storage for model download: {0}")]
    StorageInsufficient(String),
    #[error("a download is already active for model {0}")]
    DownloadAlreadyActive(String),
    #[error("model download progress could not be persisted: {0}")]
    ProgressPersistence(String),
    #[error("model download completed but terminal state could not be persisted: {0}")]
    TerminalPersistence(String),
}

#[derive(Debug, Clone)]
struct ActiveDownload {
    token: DownloadToken,
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct ModelDownloadStatusProgress {
    downloaded_files: u32,
    total_files: u32,
    downloaded_bytes: u64,
}

impl From<&ActiveDownload> for ModelDownloadStatusProgress {
    fn from(download: &ActiveDownload) -> Self {
        Self {
            downloaded_files: download.downloaded_files,
            total_files: download.total_files,
            downloaded_bytes: download.downloaded_bytes,
        }
    }
}

#[derive(Debug, Clone)]
struct ModelStatusError {
    code: String,
    detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadToken(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartDownload {
    Started {
        token: DownloadToken,
        status: TranscriptionModelStatus,
    },
    AlreadyActive(TranscriptionModelStatus),
}

pub struct PendingModelPublish<'a> {
    store: &'a TranscriptionModelStore,
    entry: &'a TranscriptionModelCatalogEntry,
    token: DownloadToken,
    staged_model_dir: PathBuf,
}

impl PendingModelPublish<'_> {
    pub fn publish(self) -> Result<bool, ModelStoreError> {
        let mut active_downloads = self.store.lock_active_downloads()?;
        let is_owner = active_downloads
            .get(self.entry.id)
            .map(|download| download.token == self.token)
            .unwrap_or(false);
        if !is_owner {
            return Ok(false);
        }
        let model_dir = self.store.model_dir_for_entry(self.entry);
        let backup_dir = self.store.import_backup_dir_for_entry(self.entry);
        replace_model_dir_with_staged(&model_dir, &self.staged_model_dir, &backup_dir)?;
        self.store.clear_known_invalid(self.entry.id)?;
        active_downloads.remove(self.entry.id);
        Ok(true)
    }
}

struct StoreDownloadProgress<'a> {
    store: &'a TranscriptionModelStore,
    model_id: &'a str,
    token: DownloadToken,
    observer:
        &'a (dyn Fn(ModelDownloadProgressSnapshot) -> Result<(), ModelStoreError> + Send + Sync),
}

impl ModelDownloadProgress for StoreDownloadProgress<'_> {
    fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
        self.store
            .download_is_cancelled_or_stale(self.model_id, self.token)
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))
    }

    fn file_completed(&self, downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
        let _ = downloaded_files;
        Ok(())
    }

    fn progress_updated(
        &self,
        snapshot: ModelDownloadProgressSnapshot,
    ) -> Result<(), ModelAcquisitionError> {
        self.store
            .update_download_progress_snapshot_if_owner(self.model_id, self.token, &snapshot)
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
        (self.observer)(snapshot)
            .map_err(|error| ModelAcquisitionError::Progress(error.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptionModelStore {
    root: PathBuf,
    active_model_id: Arc<Mutex<String>>,
    active_downloads: Arc<Mutex<BTreeMap<String, ActiveDownload>>>,
    last_errors: Arc<Mutex<BTreeMap<String, ModelStatusError>>>,
    known_invalid_models: Arc<Mutex<BTreeMap<String, RequiredArtifactInventory>>>,
    next_download_token: Arc<AtomicU64>,
}

impl TranscriptionModelStore {
    pub fn new(root: PathBuf) -> Self {
        let default_model_id = transcription_model_catalog()
            .first()
            .map(|entry| entry.id.to_string())
            .unwrap_or_default();

        Self {
            root,
            active_model_id: Arc::new(Mutex::new(default_model_id)),
            active_downloads: Arc::new(Mutex::new(BTreeMap::new())),
            last_errors: Arc::new(Mutex::new(BTreeMap::new())),
            known_invalid_models: Arc::new(Mutex::new(BTreeMap::new())),
            next_download_token: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn list(&self) -> Result<Vec<TranscriptionModelStatus>, ModelStoreError> {
        transcription_model_catalog()
            .into_iter()
            .map(|entry| self.status_from_entry(&entry))
            .collect()
    }

    pub fn active_model_id(&self) -> Result<String, ModelStoreError> {
        self.active_model_id
            .lock()
            .map(|active_model_id| active_model_id.clone())
            .map_err(|_| ModelStoreError::Download("active model lock failed".to_string()))
    }

    pub fn active_status(&self) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let active_model_id = self.active_model_id()?;
        self.status(&active_model_id)
    }

    pub fn set_active_model(
        &self,
        model_id: &str,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        {
            let mut active_model_id = self
                .active_model_id
                .lock()
                .map_err(|_| ModelStoreError::Download("active model lock failed".to_string()))?;
            *active_model_id = entry.id.to_string();
        }

        self.status(entry.id)
    }

    pub fn status(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;

        self.status_from_entry(&entry)
    }

    pub fn mark_download_started(
        &self,
        model_id: &str,
        total_files: u32,
    ) -> Result<StartDownload, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let token;
        {
            let mut active_downloads = self.lock_active_downloads()?;
            if let Some(download) = active_downloads.get(entry.id) {
                return Ok(StartDownload::AlreadyActive(
                    self.build_status_with_progress(
                        &entry,
                        ModelInstallStatus::Downloading,
                        None,
                        ModelDownloadStatusProgress::from(download),
                    ),
                ));
            }

            token = self.next_download_token();
            active_downloads.insert(
                entry.id.to_string(),
                ActiveDownload {
                    token,
                    downloaded_files: 0,
                    total_files,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                },
            );
        }

        Ok(StartDownload::Started {
            token,
            status: self.build_status_with_progress(
                &entry,
                ModelInstallStatus::Downloading,
                None,
                ModelDownloadStatusProgress {
                    downloaded_files: 0,
                    total_files,
                    downloaded_bytes: 0,
                },
            ),
        })
    }

    pub fn cancel_download(
        &self,
        model_id: &str,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let had_active_download = self.cancel_download_if_active(entry.id)?;

        if had_active_download {
            self.record_last_error(
                entry.id,
                &ModelStoreError::Acquisition(ModelAcquisitionError::Cancelled),
            )?;
            return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
        }

        self.status_from_entry(&entry)
    }

    pub fn cancel_download_if_active(&self, model_id: &str) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        self.remove_active_download(entry.id)
    }

    pub fn download_is_active(&self, model_id: &str) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        Ok(self.lock_active_downloads()?.contains_key(entry.id))
    }

    pub fn download(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        match self.download_with_observer(model_id, |_| Ok(())) {
            Err(ModelStoreError::Acquisition(ModelAcquisitionError::Cancelled)) => {
                let entry = self.catalog_entry(model_id)?;
                Ok(self.build_status(&entry, ModelInstallStatus::Failed, None))
            }
            result => result,
        }
    }

    pub fn download_with_observer<F>(
        &self,
        model_id: &str,
        observer: F,
    ) -> Result<TranscriptionModelStatus, ModelStoreError>
    where
        F: Fn(ModelDownloadProgressSnapshot) -> Result<(), ModelStoreError> + Send + Sync,
    {
        let entry = self.catalog_entry(model_id)?;
        let total_files = entry
            .artifact_sources
            .first()
            .map(|source| source.include_files.len())
            .unwrap_or(entry.required_files.len()) as u32;
        let token = match self.mark_download_started(entry.id, total_files)? {
            StartDownload::Started { token, .. } => token,
            StartDownload::AlreadyActive(_) => {
                return Err(ModelStoreError::DownloadAlreadyActive(entry.id.to_string()))
            }
        };
        self.download_reserved_with_observer_and_publisher(entry.id, token, observer, |pending| {
            pending.publish()
        })
    }

    pub fn download_reserved_with_observer_and_publisher<F, P>(
        &self,
        model_id: &str,
        token: DownloadToken,
        observer: F,
        publisher: P,
    ) -> Result<TranscriptionModelStatus, ModelStoreError>
    where
        F: Fn(ModelDownloadProgressSnapshot) -> Result<(), ModelStoreError> + Send + Sync,
        P: FnOnce(PendingModelPublish<'_>) -> Result<bool, ModelStoreError>,
    {
        let entry = self.catalog_entry(model_id)?;
        self.clear_last_error(entry.id)?;
        let download_result =
            self.download_with_default_provider(&entry, token, &observer, publisher);

        match download_result {
            Ok(true) => {
                self.clear_last_error(entry.id)?;
                self.status_from_entry(&entry)
            }
            Ok(false) => {
                self.remove_active_download_if_owner(entry.id, token)?;
                let error = ModelStoreError::Acquisition(ModelAcquisitionError::Cancelled);
                self.record_last_error(entry.id, &error)?;
                Err(error)
            }
            Err(error) => {
                self.remove_active_download_if_owner(entry.id, token)?;
                self.record_last_error(entry.id, &error)?;
                Err(error)
            }
        }
    }

    #[doc(hidden)]
    pub fn download_from_local_source_for_test(
        &self,
        model_id: &str,
        source_dir: &Path,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let source =
            entry.artifact_sources.first().cloned().ok_or_else(|| {
                ModelStoreError::Download("model has no artifact source".to_string())
            })?;
        let total_files = source.include_files.len() as u32;
        let token = match self.mark_download_started(entry.id, total_files)? {
            StartDownload::Started { token, .. } => token,
            StartDownload::AlreadyActive(status) => return Ok(status),
        };
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|error| ModelStoreError::Download(error.to_string()))?;
        let provider = HuggingFaceHubProvider::with_base_urls(
            client,
            "http://127.0.0.1/unused",
            source_dir.to_string_lossy().to_string(),
        );
        let result = self.download_with_provider(
            &entry,
            &source,
            &provider,
            token,
            &|_snapshot| Ok(()),
            |pending| pending.publish(),
        );
        if result.as_ref().is_err() || matches!(result, Ok(false)) {
            self.remove_active_download_if_owner(entry.id, token)?;
        }
        if !result? {
            return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
        }
        self.status_from_entry(&entry)
    }

    pub fn import_model(
        &self,
        model_id: &str,
        source_dir: &Path,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        if !source_dir.is_dir() {
            return Err(ModelStoreError::MissingImportFile(
                source_dir.display().to_string(),
            ));
        }

        let model_dir = self.model_dir_for_entry(&entry);
        let runtime_model_dir = self.runtime_files_dir_for_entry(&entry, &model_dir);
        if paths_refer_to_same_dir(source_dir, &model_dir)
            || paths_refer_to_same_dir(source_dir, &runtime_model_dir)
        {
            return self.verify(entry.id);
        }

        let staging_root = self.create_import_staging_root(&entry)?;
        let _staging_cleanup = StagingDirCleanup {
            path: staging_root.clone(),
        };
        let import_model_dir = staging_root.join(safe_model_dir_name(entry.id));
        fs::create_dir_all(&import_model_dir).map_err(|source| {
            ModelStoreError::CreateImportDir {
                path: import_model_dir.display().to_string(),
                source,
            }
        })?;

        let import_runtime_model_dir = self.runtime_files_dir_for_entry(&entry, &import_model_dir);
        copy_directory_contents(source_dir, &import_runtime_model_dir)?;
        validate_required_import_files(&entry, &import_runtime_model_dir)?;
        if self
            .write_verified_staged_manifest(&entry, &staging_root)?
            .is_none()
        {
            return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
        }

        let backup_dir = self.import_backup_dir_for_entry(&entry);
        replace_model_dir_with_staged(&model_dir, &import_model_dir, &backup_dir)?;
        self.clear_known_invalid(entry.id)?;
        self.write_coreml_inspection_summary_if_needed(&entry, &model_dir)?;

        self.status_from_entry(&entry)
    }

    pub fn verify(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let model_dir = self.model_dir_for_entry(&entry);

        let manifest_path = model_dir.join(MODEL_MANIFEST_FILE_NAME);
        let existing_manifest = if path_exists(&manifest_path)? {
            let Ok(json) = fs::read_to_string(&manifest_path) else {
                return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
            };
            let Ok(manifest) = serde_json::from_str::<InstalledModelManifest>(&json) else {
                return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
            };
            Some(manifest)
        } else {
            None
        };
        let mut manifest = match existing_manifest {
            Some(manifest) if manifest_metadata_is_trusted(&entry, &manifest) => {
                let inventory = audit_required_artifacts(&entry, &self.root, Some(&manifest), true);
                if !inventory.complete {
                    self.mark_known_invalid(entry.id, inventory)?;
                    return Ok(self.build_status_with_verified_inventory(
                        &entry,
                        ModelInstallStatus::Failed,
                        Some(manifest),
                        inventory,
                    ));
                }
                manifest
            }
            _ => {
                let Some(manifest) = self.reconstruct_manifest_from_catalog_files(&entry)? else {
                    return self.status_from_entry(&entry);
                };
                let inventory = audit_required_artifacts(&entry, &self.root, Some(&manifest), true);
                if !manifest_metadata_is_trusted(&entry, &manifest) || !inventory.complete {
                    self.mark_known_invalid(entry.id, inventory)?;
                    return Ok(self.build_status_with_verified_inventory(
                        &entry,
                        ModelInstallStatus::Failed,
                        Some(manifest),
                        inventory,
                    ));
                }
                manifest
            }
        };
        manifest.verified_at = Some(Utc::now().to_rfc3339());

        self.write_manifest_to_model_dir(&model_dir, &manifest)?;
        self.clear_known_invalid(entry.id)?;
        self.write_coreml_inspection_summary_if_needed(&entry, &model_dir)?;

        self.status_from_entry(&entry)
    }

    fn ready_manifest_for_entry_with_source(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        source: Option<&ModelArtifactSource>,
        installed_files: Vec<TranscriptionModelFile>,
    ) -> InstalledModelManifest {
        let now = Utc::now().to_rfc3339();
        InstalledModelManifest {
            schema_version: 1,
            model_id: entry.id.to_string(),
            revision: entry.revision.to_string(),
            modality: Some(entry.modality),
            artifact_format: Some(entry.artifact_format),
            source: source.map(|source| InstalledModelSource {
                provider: source.provider,
                repo_id: source.repo_id.clone(),
                revision: source.revision.clone(),
                license: source.license.clone(),
            }),
            installed_files,
            installed_at: now.clone(),
            verified_at: Some(now),
        }
    }

    fn write_manifest_to_model_dir(
        &self,
        model_dir: &Path,
        manifest: &InstalledModelManifest,
    ) -> Result<(), ModelStoreError> {
        let manifest_path = model_dir.join(MODEL_MANIFEST_FILE_NAME);
        let json = serde_json::to_string_pretty(&manifest).map_err(ModelStoreError::Serialize)?;
        let temporary_path = model_dir.join(format!(
            ".{MODEL_MANIFEST_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let write_result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary_path, &manifest_path)
        })();
        if let Err(source) = write_result {
            let _ = fs::remove_file(&temporary_path);
            return Err(ModelStoreError::Write {
                path: manifest_path.display().to_string(),
                source,
            });
        }
        Ok(())
    }

    fn reconstruct_manifest_from_catalog_files(
        &self,
        entry: &TranscriptionModelCatalogEntry,
    ) -> Result<Option<InstalledModelManifest>, ModelStoreError> {
        let inventory = audit_required_artifacts(entry, &self.root, None, false);
        if !inventory.complete {
            return Ok(None);
        }
        let model_dir = self.model_dir_for_entry(entry);
        let runtime_dir = self.runtime_files_dir_for_entry(entry, &model_dir);
        let mut installed_files = Vec::with_capacity(entry.required_files.len());
        for required in &entry.required_files {
            let path = runtime_dir.join(&required.path);
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| ModelStoreError::Write {
                    path: path.display().to_string(),
                    source,
                })?;
            let sha256 =
                sha256_file_for_manifest(&path).map_err(|source| ModelStoreError::Write {
                    path: path.display().to_string(),
                    source,
                })?;
            installed_files.push(TranscriptionModelFile {
                path: required.path.clone(),
                size_bytes: Some(metadata.len()),
                sha256: Some(sha256),
            });
        }
        Ok(Some(self.ready_manifest_for_entry_with_source(
            entry,
            None,
            installed_files,
        )))
    }

    fn write_coreml_inspection_summary_if_needed(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        model_dir: &Path,
    ) -> Result<(), ModelStoreError> {
        if !is_core_ml_catalog(entry) {
            return Ok(());
        }

        let summary = CoreMlInspectionSummary {
            schema_version: 1,
            model_id: entry.id.to_string(),
            runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID,
            model_root: model_dir.display().to_string(),
            runtime_model_dir: self
                .runtime_files_dir_for_entry(entry, model_dir)
                .display()
                .to_string(),
            bundles: core_ml_bundle_names(entry),
        };
        let json = serde_json::to_string_pretty(&summary).map_err(ModelStoreError::Serialize)?;
        for report_dir in [
            model_dir.to_path_buf(),
            self.runtime_files_dir_for_entry(entry, model_dir),
        ] {
            fs::create_dir_all(&report_dir).map_err(|source| ModelStoreError::Write {
                path: report_dir.display().to_string(),
                source,
            })?;
            let report_path = report_dir.join(COREML_INSPECTION_FILE_NAME);
            fs::write(&report_path, &json).map_err(|source| ModelStoreError::Write {
                path: report_path.display().to_string(),
                source,
            })?;
        }

        Ok(())
    }

    fn write_verified_staged_manifest(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        staging_root: &Path,
    ) -> Result<Option<InstalledModelManifest>, ModelStoreError> {
        let staging_store = Self::new(staging_root.to_path_buf());
        let Some(manifest) = staging_store.reconstruct_manifest_from_catalog_files(entry)? else {
            return Ok(None);
        };
        staging_store.write_manifest_to_model_dir(
            &staging_root.join(safe_model_dir_name(entry.id)),
            &manifest,
        )?;
        staging_store.write_coreml_inspection_summary_if_needed(
            entry,
            &staging_root.join(safe_model_dir_name(entry.id)),
        )?;
        Ok(Some(manifest))
    }

    fn write_verified_staged_manifest_with_source(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        staging_root: &Path,
        source: Option<&ModelArtifactSource>,
        installed_files: Vec<TranscriptionModelFile>,
    ) -> Result<Option<InstalledModelManifest>, ModelStoreError> {
        let manifest = self.ready_manifest_for_entry_with_source(entry, source, installed_files);
        if !manifest_metadata_is_trusted(entry, &manifest)
            || !audit_required_artifacts(entry, staging_root, Some(&manifest), true).complete
        {
            return Ok(None);
        }

        self.write_manifest_to_model_dir(
            &staging_root.join(safe_model_dir_name(entry.id)),
            &manifest,
        )?;
        self.write_coreml_inspection_summary_if_needed(
            entry,
            &staging_root.join(safe_model_dir_name(entry.id)),
        )?;
        Ok(Some(manifest))
    }

    pub fn remove(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let model_dir = self.model_dir_for_entry(&entry);
        self.remove_active_download(entry.id)?;

        if path_exists(&model_dir)? {
            fs::remove_dir_all(&model_dir).map_err(|source| ModelStoreError::Remove {
                path: model_dir.display().to_string(),
                source,
            })?;
        }

        Ok(self.build_status(&entry, ModelInstallStatus::Missing, None))
    }

    pub fn model_dir(&self, model_id: &str) -> Result<PathBuf, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;

        Ok(self.model_dir_for_entry(&entry))
    }

    pub fn runtime_model_dir(
        &self,
        model_id: &str,
        runtime_id: &str,
    ) -> Result<PathBuf, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let Some(runtime_subdir) = runtime_model_subdir_name(runtime_id) else {
            return Err(ModelStoreError::UnsupportedRuntime {
                model_id: entry.id.to_string(),
                runtime_id: runtime_id.to_string(),
            });
        };
        if runtime_id == FLUID_AUDIO_COREML_RUNTIME_ID && !is_core_ml_catalog(&entry) {
            return Err(ModelStoreError::UnsupportedRuntime {
                model_id: entry.id.to_string(),
                runtime_id: runtime_id.to_string(),
            });
        }
        if runtime_id == SHERPA_ONNX_RUNTIME_ID
            && entry.artifact_format != TranscriptionModelArtifactFormat::SherpaOnnxTransducer
        {
            return Err(ModelStoreError::UnsupportedRuntime {
                model_id: entry.id.to_string(),
                runtime_id: runtime_id.to_string(),
            });
        }

        Ok(self.model_dir_for_entry(&entry).join(runtime_subdir))
    }

    fn catalog_entry(
        &self,
        model_id: &str,
    ) -> Result<TranscriptionModelCatalogEntry, ModelStoreError> {
        transcription_model_catalog()
            .into_iter()
            .find(|entry| entry.id == model_id)
            .ok_or_else(|| ModelStoreError::UnsupportedModel(model_id.to_string()))
    }

    fn status_from_entry(
        &self,
        entry: &TranscriptionModelCatalogEntry,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        if let Some(status) = self.active_download_status(entry)? {
            return Ok(status);
        }

        let model_dir = self.model_dir_for_entry(entry);
        if !path_exists(&model_dir)? {
            return Ok(self.build_status(entry, ModelInstallStatus::Missing, None));
        }

        let manifest_path = model_dir.join(MODEL_MANIFEST_FILE_NAME);
        if !path_exists(&manifest_path)? {
            return Ok(self.build_status(entry, ModelInstallStatus::Failed, None));
        }

        let Ok(json) = fs::read_to_string(&manifest_path) else {
            return Ok(self.build_status(entry, ModelInstallStatus::Failed, None));
        };
        let Ok(manifest) = serde_json::from_str::<InstalledModelManifest>(&json) else {
            return Ok(self.build_status(entry, ModelInstallStatus::Failed, None));
        };
        if !manifest_metadata_is_trusted(entry, &manifest) {
            return Ok(self.build_status(entry, ModelInstallStatus::Failed, None));
        }
        if let Some(inventory) = self.known_invalid_inventory(entry.id)? {
            return Ok(self.build_status_with_verified_inventory(
                entry,
                ModelInstallStatus::Failed,
                Some(manifest),
                inventory,
            ));
        }
        let inventory = audit_required_artifacts(entry, &self.root, Some(&manifest), false);
        let install_status = if !inventory.complete {
            ModelInstallStatus::Failed
        } else if manifest.verified_at.is_some() {
            ModelInstallStatus::Ready
        } else {
            ModelInstallStatus::Verifying
        };

        Ok(self.build_status(entry, install_status, Some(manifest)))
    }

    fn active_download_status(
        &self,
        entry: &TranscriptionModelCatalogEntry,
    ) -> Result<Option<TranscriptionModelStatus>, ModelStoreError> {
        let active_downloads = self.lock_active_downloads()?;

        Ok(active_downloads.get(entry.id).map(|download| {
            self.build_status_with_progress(
                entry,
                ModelInstallStatus::Downloading,
                None,
                ModelDownloadStatusProgress::from(download),
            )
        }))
    }

    fn known_invalid_inventory(
        &self,
        model_id: &str,
    ) -> Result<Option<RequiredArtifactInventory>, ModelStoreError> {
        self.known_invalid_models
            .lock()
            .map(|models| models.get(model_id).copied())
            .map_err(|_| {
                ModelStoreError::Download("model integrity status lock failed".to_string())
            })
    }

    fn mark_known_invalid(
        &self,
        model_id: &str,
        inventory: RequiredArtifactInventory,
    ) -> Result<(), ModelStoreError> {
        self.known_invalid_models
            .lock()
            .map(|mut models| {
                models.insert(model_id.to_string(), inventory);
            })
            .map_err(|_| {
                ModelStoreError::Download("model integrity status lock failed".to_string())
            })
    }

    fn clear_known_invalid(&self, model_id: &str) -> Result<(), ModelStoreError> {
        self.known_invalid_models
            .lock()
            .map(|mut models| {
                models.remove(model_id);
            })
            .map_err(|_| {
                ModelStoreError::Download("model integrity status lock failed".to_string())
            })
    }

    fn build_status(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        install_status: ModelInstallStatus,
        manifest: Option<InstalledModelManifest>,
    ) -> TranscriptionModelStatus {
        self.build_status_with_progress(
            entry,
            install_status,
            manifest,
            ModelDownloadStatusProgress {
                downloaded_files: 0,
                total_files: entry.required_files.len() as u32,
                downloaded_bytes: 0,
            },
        )
    }

    fn build_status_with_verified_inventory(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        install_status: ModelInstallStatus,
        manifest: Option<InstalledModelManifest>,
        inventory: RequiredArtifactInventory,
    ) -> TranscriptionModelStatus {
        let mut status = self.build_status(entry, install_status, manifest);
        status.installed_bytes = inventory.installed_bytes;
        status.downloaded_files = inventory.installed_files;
        status.total_files = unique_required_file_count(entry);
        status
    }

    fn build_status_with_progress(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        install_status: ModelInstallStatus,
        manifest: Option<InstalledModelManifest>,
        progress: ModelDownloadStatusProgress,
    ) -> TranscriptionModelStatus {
        let ModelDownloadStatusProgress {
            downloaded_files,
            total_files,
            downloaded_bytes,
        } = progress;
        let model_dir = self.model_dir_for_entry(entry);
        let is_active = self
            .active_model_id
            .lock()
            .map(|active_model_id| active_model_id.as_str() == entry.id)
            .unwrap_or(false);

        let source = manifest
            .as_ref()
            .and_then(|manifest| manifest.source.as_ref())
            .map(|source| {
                (
                    source.repo_id.clone(),
                    source.revision.clone(),
                    source.license.clone(),
                )
            })
            .or_else(|| {
                entry.artifact_sources.first().map(|source| {
                    (
                        source.repo_id.clone(),
                        source.revision.clone(),
                        source.license.clone(),
                    )
                })
            });
        let is_downloading = install_status == ModelInstallStatus::Downloading;
        let inventory = if is_downloading {
            RequiredArtifactInventory::default()
        } else {
            audit_required_artifacts(entry, &self.root, manifest.as_ref(), false)
        };
        let installed_bytes = if is_downloading {
            downloaded_bytes
        } else {
            inventory.installed_bytes
        };
        let last_error = self
            .last_errors
            .lock()
            .ok()
            .and_then(|errors| errors.get(entry.id).cloned());

        let downloaded_files = if is_downloading {
            downloaded_files
        } else {
            inventory.installed_files
        };
        let total_files = if is_downloading {
            total_files
        } else {
            unique_required_file_count(entry)
        };

        TranscriptionModelStatus {
            model_id: entry.id.to_string(),
            display_name: entry.display_name.to_string(),
            is_active,
            install_status,
            local_path: model_dir.display().to_string(),
            approximate_size_bytes: entry.approximate_size_bytes,
            installed_bytes,
            downloaded_files,
            total_files,
            source_repo_id: source.as_ref().map(|source| source.0.clone()),
            source_revision: source.as_ref().map(|source| source.1.clone()),
            source_license: source.and_then(|source| source.2),
            artifact_format: artifact_format_id(entry),
            runtime_id: catalog_entry_runtime_id(entry).to_string(),
            runtime_label: catalog_entry_runtime_label(entry).to_string(),
            last_error_code: last_error.as_ref().map(|error| error.code.clone()),
            last_error_detail: last_error.map(|error| error.detail),
            verified_at: manifest.and_then(|manifest| manifest.verified_at),
        }
    }

    fn model_dir_for_entry(&self, entry: &TranscriptionModelCatalogEntry) -> PathBuf {
        self.root.join(safe_model_dir_name(entry.id))
    }

    fn runtime_files_dir_for_entry(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        model_dir: &Path,
    ) -> PathBuf {
        required_files_dir_for_entry(entry, model_dir)
    }

    fn create_import_staging_root(
        &self,
        entry: &TranscriptionModelCatalogEntry,
    ) -> Result<PathBuf, ModelStoreError> {
        fs::create_dir_all(&self.root).map_err(|source| ModelStoreError::CreateImportDir {
            path: self.root.display().to_string(),
            source,
        })?;

        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let token = self.next_download_token();
        let staging_root = self.root.join(format!(
            ".{}.importing.{}.{}.{}",
            safe_model_dir_name(entry.id),
            std::process::id(),
            token.0,
            timestamp
        ));
        fs::create_dir(&staging_root).map_err(|source| ModelStoreError::CreateImportDir {
            path: staging_root.display().to_string(),
            source,
        })?;

        Ok(staging_root)
    }

    fn import_backup_dir_for_entry(&self, entry: &TranscriptionModelCatalogEntry) -> PathBuf {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let token = self.next_download_token();
        self.root.join(format!(
            ".{}.backup.{}.{}.{}",
            safe_model_dir_name(entry.id),
            std::process::id(),
            token.0,
            timestamp
        ))
    }

    fn download_with_default_provider(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        token: DownloadToken,
        observer: &(dyn Fn(ModelDownloadProgressSnapshot) -> Result<(), ModelStoreError>
              + Send
              + Sync),
        publisher: impl FnOnce(PendingModelPublish<'_>) -> Result<bool, ModelStoreError>,
    ) -> Result<bool, ModelStoreError> {
        let source = entry
            .artifact_sources
            .first()
            .ok_or_else(|| ModelStoreError::Download("model has no artifact source".to_string()))?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .timeout(DOWNLOAD_REQUEST_TIMEOUT)
            .build()
            .map_err(|source| {
                ModelStoreError::Download(format!("failed to create download client: {source}"))
            })?;
        let provider = HuggingFaceHubProvider::new(client);
        provider.inspect_remote(source)?;

        self.download_with_provider(entry, source, &provider, token, observer, publisher)
    }

    fn download_with_provider(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        source: &ModelArtifactSource,
        provider: &dyn ModelArtifactProvider,
        token: DownloadToken,
        observer: &(dyn Fn(ModelDownloadProgressSnapshot) -> Result<(), ModelStoreError>
              + Send
              + Sync),
        publisher: impl FnOnce(PendingModelPublish<'_>) -> Result<bool, ModelStoreError>,
    ) -> Result<bool, ModelStoreError> {
        let staging_root = self.create_import_staging_root(entry)?;
        let _staging_cleanup = StagingDirCleanup {
            path: staging_root.clone(),
        };
        let staged_model_dir = staging_root.join(safe_model_dir_name(entry.id));
        let staged_runtime_dir = self.runtime_files_dir_for_entry(entry, &staged_model_dir);
        fs::create_dir_all(&staged_runtime_dir).map_err(|error| {
            if io_error_is_storage_insufficient(&error) {
                ModelStoreError::StorageInsufficient(error.to_string())
            } else {
                ModelStoreError::Download(format!(
                    "failed to create staged model directory {}: {error}",
                    staged_runtime_dir.display()
                ))
            }
        })?;

        let artifact = match provider.download(
            source,
            &staged_runtime_dir,
            &StoreDownloadProgress {
                store: self,
                model_id: entry.id,
                token,
                observer,
            },
        ) {
            Ok(artifact) => artifact,
            Err(ModelAcquisitionError::Cancelled) => return Ok(false),
            Err(ModelAcquisitionError::Progress(error)) => {
                return Err(ModelStoreError::ProgressPersistence(error))
            }
            Err(error) => return Err(ModelStoreError::Acquisition(error)),
        };

        if self.download_is_cancelled_or_stale(entry.id, token)? {
            return Ok(false);
        }
        validate_downloaded_artifact(entry, &artifact)?;
        validate_required_import_files(entry, &staged_runtime_dir)
            .map_err(|error| ModelStoreError::CoreMlInvalid(error.to_string()))?;
        let artifact_source = artifact.source;
        let installed_files = artifact
            .files
            .into_iter()
            .map(|file| TranscriptionModelFile {
                path: file.relative_path,
                size_bytes: Some(file.size_bytes),
                sha256: Some(file.sha256),
            })
            .collect();
        if self
            .write_verified_staged_manifest_with_source(
                entry,
                &staging_root,
                Some(&artifact_source),
                installed_files,
            )?
            .is_none()
        {
            return Err(ModelStoreError::ManifestInvalid(
                "verified manifest did not validate against the staged artifact".to_string(),
            ));
        }

        if self.download_is_cancelled_or_stale(entry.id, token)? {
            return Ok(false);
        }
        publisher(PendingModelPublish {
            store: self,
            entry,
            token,
            staged_model_dir,
        })
    }

    pub fn download_is_cancelled_or_stale(
        &self,
        model_id: &str,
        token: DownloadToken,
    ) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let active_downloads = self.lock_active_downloads()?;

        Ok(match active_downloads.get(entry.id) {
            Some(download) => download.token != token,
            None => true,
        })
    }

    pub fn update_download_progress_if_owner(
        &self,
        model_id: &str,
        token: DownloadToken,
        downloaded_files: u32,
    ) -> Result<bool, ModelStoreError> {
        let current = {
            let entry = self.catalog_entry(model_id)?;
            let active_downloads = self.lock_active_downloads()?;
            active_downloads.get(entry.id).cloned()
        };
        let Some(current) = current else {
            return Ok(false);
        };
        self.update_download_progress_snapshot_if_owner(
            model_id,
            token,
            &ModelDownloadProgressSnapshot {
                downloaded_files,
                total_files: current.total_files,
                downloaded_bytes: current.downloaded_bytes,
                total_bytes: current.total_bytes,
            },
        )
    }

    pub fn update_download_progress_snapshot_if_owner(
        &self,
        model_id: &str,
        token: DownloadToken,
        snapshot: &ModelDownloadProgressSnapshot,
    ) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let mut active_downloads = self.lock_active_downloads()?;
        if let Some(download) = active_downloads
            .get_mut(entry.id)
            .filter(|download| download.token == token)
        {
            download.downloaded_files = snapshot.downloaded_files;
            download.total_files = snapshot.total_files;
            download.downloaded_bytes = snapshot.downloaded_bytes;
            download.total_bytes = snapshot.total_bytes;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn remove_active_download_if_owner(
        &self,
        model_id: &str,
        token: DownloadToken,
    ) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let mut active_downloads = self.lock_active_downloads()?;

        let is_owner = active_downloads
            .get(entry.id)
            .map(|download| download.token == token)
            .unwrap_or(false);
        if is_owner {
            active_downloads.remove(entry.id);
            return Ok(true);
        }

        Ok(false)
    }

    fn remove_active_download(&self, model_id: &str) -> Result<bool, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let mut active_downloads = self.lock_active_downloads()?;

        Ok(active_downloads.remove(entry.id).is_some())
    }

    fn lock_active_downloads(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, ActiveDownload>>, ModelStoreError> {
        self.active_downloads
            .lock()
            .map_err(|_| ModelStoreError::Download("active download lock failed".to_string()))
    }

    fn next_download_token(&self) -> DownloadToken {
        DownloadToken(self.next_download_token.fetch_add(1, Ordering::Relaxed))
    }

    fn record_last_error(
        &self,
        model_id: &str,
        error: &ModelStoreError,
    ) -> Result<(), ModelStoreError> {
        self.last_errors
            .lock()
            .map_err(|_| ModelStoreError::Download("model error lock failed".to_string()))?
            .insert(
                model_id.to_string(),
                ModelStatusError {
                    code: error.stable_code().to_string(),
                    detail: error.to_string(),
                },
            );
        Ok(())
    }

    fn clear_last_error(&self, model_id: &str) -> Result<(), ModelStoreError> {
        self.last_errors
            .lock()
            .map_err(|_| ModelStoreError::Download("model error lock failed".to_string()))?
            .remove(model_id);
        Ok(())
    }
}

impl ModelStoreError {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::Acquisition(ModelAcquisitionError::Cancelled) => "model.download.cancelled",
            Self::Acquisition(ModelAcquisitionError::StorageInsufficient(_)) => {
                "model.storage.insufficient"
            }
            Self::Acquisition(ModelAcquisitionError::Progress(_))
            | Self::ProgressPersistence(_) => "model.download.failed",
            Self::TerminalPersistence(_) => "model.download.failed",
            Self::Acquisition(
                ModelAcquisitionError::UnsupportedProvider(_)
                | ModelAcquisitionError::Inspect(_)
                | ModelAcquisitionError::SourceRepoMismatch { .. }
                | ModelAcquisitionError::SourceRevisionMismatch { .. },
            ) => "model.source.unavailable",
            Self::Acquisition(ModelAcquisitionError::SourceFileMissing(_))
            | Self::ManifestInvalid(_) => "model.manifest.invalid",
            Self::HashMismatch(_) => "model.hash.mismatch",
            Self::CoreMlInvalid(_) => "model.coreml.invalid",
            // Linux models are ONNX files, so a missing import file is not a Core ML problem there.
            Self::MissingImportFile(_) if !cfg!(target_os = "macos") => "model.import.incomplete",
            Self::MissingImportFile(_) => "model.coreml.invalid",
            Self::StorageInsufficient(_) => "model.storage.insufficient",
            Self::DownloadAlreadyActive(_) => "model.download.active",
            Self::CheckPath { source, .. }
            | Self::Write { source, .. }
            | Self::Remove { source, .. }
            | Self::CreateImportDir { source, .. }
            | Self::ReadImportDir { source, .. }
            | Self::CopyImportFile { source, .. }
            | Self::ReplaceImportDir { source, .. }
                if io_error_is_storage_insufficient(source) =>
            {
                "model.storage.insufficient"
            }
            _ => "model.download.failed",
        }
    }
}

fn io_error_is_storage_insufficient(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::StorageFull
        || error
            .raw_os_error()
            .is_some_and(|code| code == 28 || code == 122)
}

fn artifact_format_id(entry: &TranscriptionModelCatalogEntry) -> String {
    match entry.artifact_format {
        super::model::TranscriptionModelArtifactFormat::CoreMlBundle => {
            "core_ml_bundle".to_string()
        }
        super::model::TranscriptionModelArtifactFormat::SherpaOnnxTransducer => {
            "sherpa_onnx_transducer".to_string()
        }
    }
}

fn path_exists(path: &Path) -> Result<bool, ModelStoreError> {
    path.try_exists()
        .map_err(|source| ModelStoreError::CheckPath {
            path: path.display().to_string(),
            source,
        })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RequiredArtifactInventory {
    installed_files: u32,
    installed_bytes: u64,
    complete: bool,
}

fn unique_required_file_count(entry: &TranscriptionModelCatalogEntry) -> u32 {
    entry
        .required_files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u32
}

fn manifest_metadata_is_trusted(
    entry: &TranscriptionModelCatalogEntry,
    manifest: &InstalledModelManifest,
) -> bool {
    if manifest.schema_version != 1
        || manifest.model_id != entry.id
        || manifest.revision != entry.revision
        || manifest.modality != Some(entry.modality)
        || manifest.artifact_format != Some(entry.artifact_format)
        || manifest.installed_at.trim().is_empty()
    {
        return false;
    }
    if let Some(source) = manifest.source.as_ref() {
        let matches_catalog = entry.artifact_sources.iter().any(|candidate| {
            source.provider == candidate.provider
                && source.repo_id == candidate.repo_id
                && source.revision == candidate.revision
                && source.license == candidate.license
        });
        if !matches_catalog {
            return false;
        }
    }

    let mut required = BTreeMap::new();
    for file in &entry.required_files {
        if required.insert(file.path.as_str(), file).is_some() {
            return false;
        }
    }
    if manifest.installed_files.len() != required.len() {
        return false;
    }
    let mut installed = BTreeMap::new();
    for file in &manifest.installed_files {
        let Some(required_file) = required.get(file.path.as_str()) else {
            return false;
        };
        if installed.insert(file.path.as_str(), file).is_some() {
            return false;
        }
        let Some(size_bytes) = file.size_bytes.filter(|size| *size > 0) else {
            return false;
        };
        let Some(sha256) = file.sha256.as_deref().filter(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) else {
            return false;
        };
        if required_file
            .size_bytes
            .is_some_and(|required_size| required_size != size_bytes)
            || required_file
                .sha256
                .as_deref()
                .is_some_and(|required_hash| required_hash != sha256)
        {
            return false;
        }
    }
    installed.len() == required.len()
}

fn audit_required_artifacts(
    entry: &TranscriptionModelCatalogEntry,
    model_root: &Path,
    manifest: Option<&InstalledModelManifest>,
    verify_hashes: bool,
) -> RequiredArtifactInventory {
    let model_dir = model_root.join(safe_model_dir_name(entry.id));
    let runtime_dir = required_files_dir_for_entry(entry, &model_dir);
    let Ok(root_canonical) = model_root.canonicalize() else {
        return RequiredArtifactInventory::default();
    };
    let Ok(model_metadata) = fs::symlink_metadata(&model_dir) else {
        return RequiredArtifactInventory::default();
    };
    let Ok(model_canonical) = model_dir.canonicalize() else {
        return RequiredArtifactInventory::default();
    };
    let Ok(runtime_metadata) = fs::symlink_metadata(&runtime_dir) else {
        return RequiredArtifactInventory::default();
    };
    let Ok(runtime_canonical) = runtime_dir.canonicalize() else {
        return RequiredArtifactInventory::default();
    };
    if !model_metadata.is_dir()
        || model_metadata.file_type().is_symlink()
        || !runtime_metadata.is_dir()
        || runtime_metadata.file_type().is_symlink()
        || !model_canonical.starts_with(&root_canonical)
        || !runtime_canonical.starts_with(&model_canonical)
    {
        return RequiredArtifactInventory::default();
    }

    let installed_by_path = manifest.map(|manifest| {
        manifest
            .installed_files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<BTreeMap<_, _>>()
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut inventory = RequiredArtifactInventory::default();
    for required in &entry.required_files {
        if !seen.insert(required.path.as_str()) {
            return RequiredArtifactInventory::default();
        }
        let expected = installed_by_path
            .as_ref()
            .and_then(|installed| installed.get(required.path.as_str()).copied())
            .unwrap_or(required);
        let path = runtime_dir.join(&required.path);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() == 0
            || !canonical.starts_with(&runtime_canonical)
            || expected
                .size_bytes
                .is_some_and(|expected_size| expected_size != metadata.len())
        {
            continue;
        }
        if verify_hashes {
            let Some(expected_sha256) = expected.sha256.as_deref() else {
                continue;
            };
            let Ok(actual_sha256) = sha256_file_for_manifest(&path) else {
                continue;
            };
            if actual_sha256 != expected_sha256 {
                continue;
            }
        }
        inventory.installed_files += 1;
        inventory.installed_bytes = inventory.installed_bytes.saturating_add(metadata.len());
    }
    inventory.complete = inventory.installed_files == seen.len() as u32;
    inventory
}

fn sha256_file_for_manifest(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_downloaded_artifact(
    entry: &TranscriptionModelCatalogEntry,
    artifact: &super::acquisition::DownloadedModelArtifact,
) -> Result<(), ModelStoreError> {
    for required in &entry.required_files {
        let downloaded = artifact
            .files
            .iter()
            .find(|file| file.relative_path == required.path)
            .ok_or_else(|| ModelStoreError::CoreMlInvalid(required.path.clone()))?;
        if let Some(expected_size) = required.size_bytes {
            if downloaded.size_bytes != expected_size {
                return Err(ModelStoreError::ManifestInvalid(format!(
                    "{} has {} bytes; expected {}",
                    required.path, downloaded.size_bytes, expected_size
                )));
            }
        }
        if let Some(expected_sha256) = required.sha256.as_deref() {
            if downloaded.sha256 != expected_sha256 {
                return Err(ModelStoreError::HashMismatch(required.path.clone()));
            }
        }
    }
    Ok(())
}

fn validate_required_import_files(
    entry: &TranscriptionModelCatalogEntry,
    model_dir: &Path,
) -> Result<(), ModelStoreError> {
    for required in &entry.required_files {
        let source_path = model_dir.join(&required.path);
        if !path_exists(&source_path)? || !source_path.is_file() {
            return Err(ModelStoreError::MissingImportFile(required.path.clone()));
        }
    }

    Ok(())
}

fn copy_directory_contents(source_dir: &Path, target_dir: &Path) -> Result<(), ModelStoreError> {
    fs::create_dir_all(target_dir).map_err(|source| ModelStoreError::CreateImportDir {
        path: target_dir.display().to_string(),
        source,
    })?;

    let entries = fs::read_dir(source_dir).map_err(|source| ModelStoreError::ReadImportDir {
        path: source_dir.display().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ModelStoreError::ReadImportDir {
            path: source_dir.display().to_string(),
            source,
        })?;
        let source_path = entry.path();
        if paths_refer_to_same_dir(&source_path, target_dir) {
            continue;
        }

        let target_path = target_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|source| ModelStoreError::ReadImportDir {
                path: source_path.display().to_string(),
                source,
            })?;
        if file_type.is_dir() {
            copy_directory_contents(&source_path, &target_path)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|source| ModelStoreError::CreateImportDir {
                    path: parent.display().to_string(),
                    source,
                })?;
            }
            fs::copy(&source_path, &target_path).map_err(|source| {
                ModelStoreError::CopyImportFile {
                    source_path: source_path.display().to_string(),
                    target_path: target_path.display().to_string(),
                    source,
                }
            })?;
        }
    }

    Ok(())
}

fn replace_model_dir_with_staged(
    model_dir: &Path,
    staged_model_dir: &Path,
    backup_dir: &Path,
) -> Result<(), ModelStoreError> {
    if path_exists(backup_dir)? {
        fs::remove_dir_all(backup_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
            path: backup_dir.display().to_string(),
            source,
        })?;
    }

    if !path_exists(model_dir)? {
        fs::rename(staged_model_dir, model_dir).map_err(|source| {
            ModelStoreError::ReplaceImportDir {
                path: format!("{} to {}", staged_model_dir.display(), model_dir.display()),
                source,
            }
        })?;
        return Ok(());
    }

    fs::rename(model_dir, backup_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
        path: format!("{} to {}", model_dir.display(), backup_dir.display()),
        source,
    })?;

    match fs::rename(staged_model_dir, model_dir) {
        Ok(()) => {
            fs::remove_dir_all(backup_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
                path: backup_dir.display().to_string(),
                source,
            })?;
            Ok(())
        }
        Err(source) => {
            let replace_error = ModelStoreError::ReplaceImportDir {
                path: format!("{} to {}", staged_model_dir.display(), model_dir.display()),
                source,
            };
            if let Err(restore_source) = fs::rename(backup_dir, model_dir) {
                return Err(ModelStoreError::ReplaceImportDir {
                    path: format!(
                        "{} to {} after failed replace",
                        backup_dir.display(),
                        model_dir.display()
                    ),
                    source: restore_source,
                });
            }
            Err(replace_error)
        }
    }
}

fn paths_refer_to_same_dir(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

struct StagingDirCleanup {
    path: PathBuf,
}

impl Drop for StagingDirCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn is_core_ml_catalog(entry: &TranscriptionModelCatalogEntry) -> bool {
    entry
        .required_files
        .iter()
        .any(|file| file.path.contains(".mlmodelc/"))
}

fn core_ml_bundle_names(entry: &TranscriptionModelCatalogEntry) -> Vec<String> {
    let mut bundles = Vec::new();
    for file in &entry.required_files {
        let Some((bundle_name, _)) = file.path.split_once('/') else {
            continue;
        };
        if bundle_name.ends_with(".mlmodelc")
            && !bundles.iter().any(|existing| existing == bundle_name)
        {
            bundles.push(bundle_name.to_string());
        }
    }

    bundles
}

#[cfg(test)]
fn part_file_path_for_token(
    target_path: &Path,
    token: DownloadToken,
) -> Result<PathBuf, ModelStoreError> {
    let file_name = target_path.file_name().ok_or_else(|| {
        ModelStoreError::Download(format!(
            "model file path has no file name: {}",
            target_path.display()
        ))
    })?;
    let part_file_name = format!("{}.{}.part", file_name.to_string_lossy(), token.0);

    Ok(target_path.with_file_name(part_file_name))
}

#[cfg(test)]
fn replace_with_part_file_if_owner(
    store: &TranscriptionModelStore,
    model_id: &str,
    token: DownloadToken,
    part_path: &Path,
    target_path: &Path,
) -> Result<bool, ModelStoreError> {
    let entry = store.catalog_entry(model_id)?;
    let active_downloads = store.lock_active_downloads()?;
    let is_owner = active_downloads
        .get(entry.id)
        .map(|download| download.token == token)
        .unwrap_or(false);
    if !is_owner {
        drop(active_downloads);
        let _ = fs::remove_file(part_path);
        return Ok(false);
    }

    if path_exists(target_path)? {
        fs::remove_file(target_path).map_err(|source| {
            ModelStoreError::Download(format!(
                "failed to remove existing model file {}: {source}",
                target_path.display()
            ))
        })?;
    }

    fs::rename(part_path, target_path)
        .map_err(|source| {
            ModelStoreError::Download(format!(
                "failed to move downloaded file {} to {}: {source}",
                part_path.display(),
                target_path.display()
            ))
        })
        .map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::{
        part_file_path_for_token, replace_model_dir_with_staged, replace_with_part_file_if_owner,
        validate_downloaded_artifact, DownloadToken, ModelStoreError, StartDownload,
        TranscriptionModelStore,
    };
    use crate::transcription::acquisition::{
        DownloadedModelArtifact, DownloadedModelFile, HuggingFaceHubProvider,
        ModelDownloadProgressSnapshot,
    };
    use crate::transcription::model::{ModelInstallStatus, FLUID_AUDIO_COREML_RUNTIME_ID};
    use std::fs;
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    fn fixture_catalog_entry() -> crate::transcription::model::TranscriptionModelCatalogEntry {
        let entry = crate::transcription::model::parakeet_v3_coreml_catalog_entry();
        crate::transcription::model::override_catalog_for_current_test_thread(vec![entry.clone()]);
        entry
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_catalog_exposes_pinned_sherpa_onnx_artifacts_and_rejects_forged_bytes() {
        use crate::transcription::model::{
            parakeet_v3_catalog_entry, TranscriptionModelArtifactFormat, SHERPA_ONNX_RUNTIME_ID,
        };
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = parakeet_v3_catalog_entry();
        assert_eq!(
            entry.artifact_format,
            TranscriptionModelArtifactFormat::SherpaOnnxTransducer
        );

        let status = store.status(entry.id).expect("model status");
        assert_eq!(status.install_status, ModelInstallStatus::Missing);
        assert_eq!(
            status.source_repo_id.as_deref(),
            Some("csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8")
        );
        assert_eq!(
            status.source_revision.as_deref(),
            Some("2bda32ec70b097a55adaa07d9a7173915b43cc78")
        );
        assert_eq!(status.source_license.as_deref(), Some("cc-by-4.0"));
        assert_eq!(status.artifact_format, "sherpa_onnx_transducer");
        assert_eq!(status.runtime_id, SHERPA_ONNX_RUNTIME_ID);
        assert_eq!(status.runtime_label, "ONNX");
        assert_eq!(status.total_files, 4);
        assert!(entry
            .required_files
            .iter()
            .all(|file| file.size_bytes.is_some_and(|size| size > 0)
                && file.sha256.as_deref().is_some_and(|hash| hash.len() == 64)));
        assert_eq!(
            store
                .runtime_model_dir(entry.id, SHERPA_ONNX_RUNTIME_ID)
                .expect("sherpa runtime dir"),
            root.path()
                .join("nvidia__parakeet-tdt-0.6b-v3")
                .join("sherpa-onnx")
        );
        assert!(matches!(
            store.runtime_model_dir(entry.id, FLUID_AUDIO_COREML_RUNTIME_ID),
            Err(ModelStoreError::UnsupportedRuntime { .. })
        ));

        // Correctly named files whose bytes do not match the pinned hashes never become ready.
        let runtime_dir = store
            .runtime_model_dir(entry.id, SHERPA_ONNX_RUNTIME_ID)
            .expect("sherpa runtime dir");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        for required in &entry.required_files {
            fs::write(runtime_dir.join(&required.path), b"forged").expect("forged file");
        }
        let verified = store.verify(entry.id).expect("verify forged install");
        assert_ne!(verified.install_status, ModelInstallStatus::Ready);
        assert!(!store
            .model_dir(entry.id)
            .expect("model dir")
            .join(super::MODEL_MANIFEST_FILE_NAME)
            .exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_model_root_follows_tauri_app_data_dir() {
        use std::path::Path;
        assert_eq!(
            super::linux_global_transcription_model_root(None, Path::new("/home/editor")),
            Path::new("/home/editor/.local/share/com.olhapi.video-creater/models")
        );
        assert_eq!(
            super::linux_global_transcription_model_root(
                Some(Path::new("/data/xdg")),
                Path::new("/home/editor")
            ),
            Path::new("/data/xdg/com.olhapi.video-creater/models")
        );
        assert_eq!(
            super::linux_global_transcription_model_root(
                Some(Path::new("relative")),
                Path::new("/home/editor")
            ),
            Path::new("/home/editor/.local/share/com.olhapi.video-creater/models")
        );
    }

    #[test]
    fn token_scoped_part_file_path_differs_for_same_target() {
        let root = tempfile::tempdir().expect("temp root");
        let target_path = root.path().join("model.safetensors");

        let first = part_file_path_for_token(&target_path, DownloadToken(1)).expect("first part");
        let second = part_file_path_for_token(&target_path, DownloadToken(2)).expect("second part");

        assert_ne!(first, second);
        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("model.safetensors.1.part")
        );
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("model.safetensors.2.part")
        );
    }

    #[test]
    fn replace_imported_model_dir_restores_backup_when_install_rename_fails() {
        let root = tempfile::tempdir().expect("temp root");
        let model_dir = root.path().join("model");
        let backup_dir = root.path().join("model.backup");
        let missing_import_dir = root.path().join("missing-import");
        fs::create_dir_all(&model_dir).expect("model dir");
        fs::write(model_dir.join("marker.txt"), b"known good").expect("marker");

        let error = replace_model_dir_with_staged(&model_dir, &missing_import_dir, &backup_dir)
            .expect_err("missing staged dir should fail");

        assert!(error.to_string().contains("missing-import"));
        assert_eq!(
            fs::read(model_dir.join("marker.txt")).expect("restored marker"),
            b"known good"
        );
        assert!(!backup_dir.exists());
    }

    #[test]
    fn stale_token_cannot_replace_existing_target() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();
        let first_start = store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("start download");
        let StartDownload::Started {
            token: stale_token, ..
        } = first_start
        else {
            panic!("first start should own download");
        };
        store.cancel_download(entry.id).expect("cancel");
        store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("restart download");
        let part_path = root.path().join("model.safetensors.1.part");
        let target_path = root.path().join("model.safetensors");
        fs::write(&part_path, b"stale").expect("write part");
        fs::write(&target_path, b"current").expect("write target");

        let replaced = replace_with_part_file_if_owner(
            &store,
            entry.id,
            stale_token,
            &part_path,
            &target_path,
        )
        .expect("replace check");

        assert!(!replaced);
        assert!(!part_path.exists());
        assert_eq!(fs::read(&target_path).expect("read target"), b"current");
    }

    #[test]
    fn owner_token_replaces_existing_target() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();
        let start = store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("start download");
        let StartDownload::Started { token, .. } = start else {
            panic!("first start should own download");
        };
        let part_path = root.path().join("model.safetensors.1.part");
        let target_path = root.path().join("model.safetensors");
        fs::write(&part_path, b"new").expect("write part");
        fs::write(&target_path, b"old").expect("write target");

        let replaced =
            replace_with_part_file_if_owner(&store, entry.id, token, &part_path, &target_path)
                .expect("replace");

        assert!(replaced);
        assert!(!part_path.exists());
        assert_eq!(fs::read(&target_path).expect("read target"), b"new");
    }

    #[test]
    fn owner_token_moves_when_target_is_absent() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();
        let start = store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("start download");
        let StartDownload::Started { token, .. } = start else {
            panic!("first start should own download");
        };
        let part_path = root.path().join("tokenizer.json.1.part");
        let target_path = root.path().join("tokenizer.json");
        fs::write(&part_path, b"new").expect("write part");

        let replaced =
            replace_with_part_file_if_owner(&store, entry.id, token, &part_path, &target_path)
                .expect("replace");

        assert!(replaced);
        assert!(!part_path.exists());
        assert_eq!(fs::read(&target_path).expect("read target"), b"new");
    }

    #[test]
    fn status_exposes_catalog_provenance_before_install() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();

        let status = store.status(entry.id).expect("model status");

        assert_eq!(status.install_status, ModelInstallStatus::Missing);
        assert_eq!(status.installed_bytes, 0);
        assert_eq!(
            status.source_repo_id.as_deref(),
            Some("FluidInference/parakeet-tdt-0.6b-v3-coreml")
        );
        assert_eq!(
            status.source_revision.as_deref(),
            Some("aed02740059203c4a87495924f685de3722ae9ce")
        );
        assert_eq!(status.source_license.as_deref(), Some("cc-by-4.0"));
        assert_eq!(status.artifact_format, "core_ml_bundle");
        assert_eq!(status.runtime_id, FLUID_AUDIO_COREML_RUNTIME_ID);
        assert_eq!(status.last_error_code, None);
    }

    #[test]
    fn verify_preserves_download_manifest_integrity_and_reports_installed_files() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();
        let model_dir = store.model_dir_for_entry(&entry);
        let runtime_dir = store.runtime_files_dir_for_entry(&entry, &model_dir);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        for required in &entry.required_files {
            let path = runtime_dir.join(&required.path);
            fs::create_dir_all(path.parent().expect("required file parent")).expect("parent");
            fs::write(path, b"fixture").expect("required file");
        }
        let installed_files = entry
            .required_files
            .iter()
            .cloned()
            .map(|mut file| {
                file.size_bytes = Some(7);
                file.sha256 = Some(
                    "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d".to_string(),
                );
                file
            })
            .collect::<Vec<_>>();
        let manifest = store.ready_manifest_for_entry_with_source(
            &entry,
            entry.artifact_sources.first(),
            installed_files,
        );
        store
            .write_manifest_to_model_dir(&model_dir, &manifest)
            .expect("write downloaded manifest");

        let status = store.verify(entry.id).expect("verify downloaded model");
        let verified_manifest =
            serde_json::from_slice::<crate::transcription::model::InstalledModelManifest>(
                &fs::read(model_dir.join(super::MODEL_MANIFEST_FILE_NAME))
                    .expect("read verified manifest"),
            )
            .expect("parse verified manifest");

        assert_eq!(status.install_status, ModelInstallStatus::Ready);
        assert_eq!(status.downloaded_files, entry.required_files.len() as u32);
        assert_eq!(status.downloaded_files, status.total_files);
        assert_eq!(verified_manifest.source, manifest.source);
        assert_eq!(verified_manifest.installed_files, manifest.installed_files);
    }

    fn materialize_catalog_fixture(
        store: &TranscriptionModelStore,
    ) -> (
        crate::transcription::model::TranscriptionModelCatalogEntry,
        std::path::PathBuf,
        crate::transcription::model::InstalledModelManifest,
    ) {
        let entry = fixture_catalog_entry();
        let model_dir = store.model_dir_for_entry(&entry);
        let runtime_dir = store.runtime_files_dir_for_entry(&entry, &model_dir);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        for required in &entry.required_files {
            let path = runtime_dir.join(&required.path);
            fs::create_dir_all(path.parent().expect("required file parent")).expect("parent");
            fs::write(path, b"fixture").expect("required file");
        }
        let installed_files = entry
            .required_files
            .iter()
            .cloned()
            .map(|mut file| {
                file.size_bytes = Some(7);
                file.sha256 = Some(
                    "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d".to_string(),
                );
                file
            })
            .collect::<Vec<_>>();
        let manifest = store.ready_manifest_for_entry_with_source(
            &entry,
            entry.artifact_sources.first(),
            installed_files,
        );
        (entry, model_dir, manifest)
    }

    #[test]
    fn status_and_verify_count_only_present_required_artifacts() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let (entry, model_dir, manifest) = materialize_catalog_fixture(&store);
        store
            .write_manifest_to_model_dir(&model_dir, &manifest)
            .expect("manifest");
        let missing = store
            .runtime_files_dir_for_entry(&entry, &model_dir)
            .join(&entry.required_files[0].path);
        fs::remove_file(missing).expect("remove required file");

        let before = store.status(entry.id).expect("partial status");
        let immediate = store.verify(entry.id).expect("partial verify");
        let after = store.status(entry.id).expect("status after verify");

        for status in [&before, &immediate, &after] {
            assert_eq!(status.install_status, ModelInstallStatus::Failed);
            assert_eq!(
                status.downloaded_files,
                entry.required_files.len() as u32 - 1
            );
            assert_eq!(
                status.installed_bytes,
                (entry.required_files.len() as u64 - 1) * 7
            );
        }
        assert_eq!(before, immediate);
        assert_eq!(immediate, after);
    }

    #[test]
    fn missing_manifest_partial_install_reports_actual_required_files() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let (entry, model_dir, _) = materialize_catalog_fixture(&store);
        let runtime_dir = store.runtime_files_dir_for_entry(&entry, &model_dir);
        fs::remove_file(runtime_dir.join(&entry.required_files[0].path)).expect("remove file");

        let status = store.status(entry.id).expect("partial status");
        let verified = store.verify(entry.id).expect("partial verify");
        let after = store.status(entry.id).expect("partial status after verify");

        for observed in [&status, &verified, &after] {
            assert_eq!(observed.install_status, ModelInstallStatus::Failed);
            assert_eq!(
                observed.downloaded_files,
                entry.required_files.len() as u32 - 1
            );
            assert_eq!(
                observed.installed_bytes,
                (entry.required_files.len() as u64 - 1) * 7
            );
        }
        assert_eq!(status, verified);
        assert_eq!(verified, after);
    }

    #[test]
    fn trusted_manifest_hash_corruption_fails_without_rewriting_metadata() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let (entry, model_dir, manifest) = materialize_catalog_fixture(&store);
        store
            .write_manifest_to_model_dir(&model_dir, &manifest)
            .expect("manifest");
        let manifest_path = model_dir.join(super::MODEL_MANIFEST_FILE_NAME);
        let before_manifest = fs::read(&manifest_path).expect("manifest bytes");
        let runtime_dir = store.runtime_files_dir_for_entry(&entry, &model_dir);
        fs::write(runtime_dir.join(&entry.required_files[0].path), b"corrupt")
            .expect("corrupt file");

        let immediate = store.verify(entry.id).expect("verify corrupt model");
        let after = store.status(entry.id).expect("status after verify");

        for status in [&immediate, &after] {
            assert_eq!(status.install_status, ModelInstallStatus::Failed);
            assert_eq!(
                status.downloaded_files,
                entry.required_files.len() as u32 - 1
            );
            assert_eq!(
                status.installed_bytes,
                (entry.required_files.len() as u64 - 1) * 7
            );
        }
        assert_eq!(immediate, after);
        assert_eq!(
            fs::read(manifest_path).expect("manifest after"),
            before_manifest
        );
    }

    #[test]
    fn verify_discards_each_forged_provenance_field_and_reconstructs_complete_filesystem() {
        #[derive(Clone, Copy)]
        enum Tamper {
            Provider,
            Repo,
            Revision,
            License,
        }

        for tamper in [
            Tamper::Provider,
            Tamper::Repo,
            Tamper::Revision,
            Tamper::License,
        ] {
            let root = tempfile::tempdir().expect("temp root");
            let store = TranscriptionModelStore::new(root.path().to_path_buf());
            let (entry, model_dir, mut manifest) = materialize_catalog_fixture(&store);
            let source = manifest.source.as_mut().expect("source");
            match tamper {
                Tamper::Provider => {
                    source.provider =
                        crate::transcription::model::ModelArtifactProviderKind::LocalImport
                }
                Tamper::Repo => source.repo_id = "attacker/forged".to_string(),
                Tamper::Revision => source.revision = "forged-revision".to_string(),
                Tamper::License => source.license = Some("forged-license".to_string()),
            }
            store
                .write_manifest_to_model_dir(&model_dir, &manifest)
                .expect("forged manifest");

            assert_eq!(
                store
                    .status(entry.id)
                    .expect("forged status")
                    .install_status,
                ModelInstallStatus::Failed
            );
            let verified = store.verify(entry.id).expect("recover forged manifest");
            let after = store.status(entry.id).expect("status after recovery");
            let persisted =
                serde_json::from_slice::<crate::transcription::model::InstalledModelManifest>(
                    &fs::read(model_dir.join(super::MODEL_MANIFEST_FILE_NAME))
                        .expect("reconstructed manifest"),
                )
                .expect("parse reconstructed manifest");

            assert_eq!(verified.install_status, ModelInstallStatus::Ready);
            assert_eq!(verified, after);
            assert_eq!(verified.downloaded_files, entry.required_files.len() as u32);
            assert_eq!(persisted.source, None);
            assert_eq!(persisted.installed_files.len(), entry.required_files.len());
            assert!(persisted
                .installed_files
                .iter()
                .all(|file| file.size_bytes.is_some() && file.sha256.is_some()));
        }
    }

    #[test]
    fn verify_rejects_missing_duplicate_and_extra_manifest_paths_before_clean_recovery() {
        #[derive(Clone, Copy)]
        enum InvalidSet {
            Missing,
            Duplicate,
            Extra,
        }

        for invalid_set in [
            InvalidSet::Missing,
            InvalidSet::Duplicate,
            InvalidSet::Extra,
        ] {
            let root = tempfile::tempdir().expect("temp root");
            let store = TranscriptionModelStore::new(root.path().to_path_buf());
            let (entry, model_dir, mut manifest) = materialize_catalog_fixture(&store);
            match invalid_set {
                InvalidSet::Missing => {
                    manifest.installed_files.pop();
                }
                InvalidSet::Duplicate => manifest
                    .installed_files
                    .push(manifest.installed_files[0].clone()),
                InvalidSet::Extra => manifest.installed_files.push(
                    crate::transcription::model::TranscriptionModelFile {
                        path: "unexpected.bin".to_string(),
                        size_bytes: Some(7),
                        sha256: Some(
                            "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d"
                                .to_string(),
                        ),
                    },
                ),
            }
            store
                .write_manifest_to_model_dir(&model_dir, &manifest)
                .expect("invalid manifest");

            assert_eq!(
                store
                    .status(entry.id)
                    .expect("invalid status")
                    .install_status,
                ModelInstallStatus::Failed
            );
            let verified = store.verify(entry.id).expect("clean recovery");
            let after = store.status(entry.id).expect("status after recovery");
            let persisted =
                serde_json::from_slice::<crate::transcription::model::InstalledModelManifest>(
                    &fs::read(model_dir.join(super::MODEL_MANIFEST_FILE_NAME))
                        .expect("reconstructed manifest"),
                )
                .expect("parse reconstructed manifest");

            assert_eq!(verified.install_status, ModelInstallStatus::Ready);
            assert_eq!(verified, after);
            assert_eq!(persisted.installed_files.len(), entry.required_files.len());
            assert!(persisted
                .installed_files
                .iter()
                .all(|file| file.path != "unexpected.bin"));
        }
    }

    #[test]
    fn verify_reconstructs_stale_manifest_only_from_complete_catalog_files() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let (entry, model_dir, mut manifest) = materialize_catalog_fixture(&store);
        manifest.revision = "stale-revision".to_string();
        manifest.installed_files[0].size_bytes = None;
        manifest.installed_files[0].sha256 = None;
        store
            .write_manifest_to_model_dir(&model_dir, &manifest)
            .expect("stale manifest");

        let verified = store.verify(entry.id).expect("recover stale manifest");
        let after = store.status(entry.id).expect("status after recovery");

        assert_eq!(verified.install_status, ModelInstallStatus::Ready);
        assert_eq!(verified, after);
        assert_eq!(verified.downloaded_files, entry.required_files.len() as u32);
    }

    #[test]
    fn download_failures_expose_stable_operation_codes() {
        use crate::transcription::acquisition::ModelAcquisitionError;

        assert_eq!(
            ModelStoreError::Acquisition(ModelAcquisitionError::Inspect("offline".to_string()))
                .stable_code(),
            "model.source.unavailable"
        );
        assert_eq!(
            ModelStoreError::Acquisition(ModelAcquisitionError::Cancelled).stable_code(),
            "model.download.cancelled"
        );
        assert_eq!(
            ModelStoreError::ManifestInvalid("bad JSON".to_string()).stable_code(),
            "model.manifest.invalid"
        );
        assert_eq!(
            ModelStoreError::HashMismatch("config.json".to_string()).stable_code(),
            "model.hash.mismatch"
        );
        assert_eq!(
            ModelStoreError::CoreMlInvalid("Encoder.mlmodelc".to_string()).stable_code(),
            "model.coreml.invalid"
        );
        assert_eq!(
            ModelStoreError::MissingImportFile("encoder.int8.onnx".to_string()).stable_code(),
            if cfg!(target_os = "macos") {
                "model.coreml.invalid"
            } else {
                "model.import.incomplete"
            }
        );
        assert_eq!(
            ModelStoreError::StorageInsufficient("disk full".to_string()).stable_code(),
            "model.storage.insufficient"
        );
        assert_eq!(
            ModelStoreError::Download("other".to_string()).stable_code(),
            "model.download.failed"
        );
    }

    #[test]
    fn downloaded_artifact_validation_distinguishes_layout_hash_and_manifest_failures() {
        let mut entry = fixture_catalog_entry();
        entry.required_files = vec![entry.required_files[0].clone()];
        entry.required_files[0].size_bytes = Some(4);
        entry.required_files[0].sha256 = Some("expected".to_string());
        let artifact = DownloadedModelArtifact {
            source: entry.artifact_sources[0].clone(),
            files: Vec::new(),
        };
        assert!(matches!(
            validate_downloaded_artifact(&entry, &artifact),
            Err(ModelStoreError::CoreMlInvalid(_))
        ));

        let file = DownloadedModelFile {
            relative_path: entry.required_files[0].path.clone(),
            absolute_path: std::path::PathBuf::from("config.json"),
            size_bytes: 3,
            sha256: "wrong".to_string(),
        };
        let artifact = DownloadedModelArtifact {
            source: entry.artifact_sources[0].clone(),
            files: vec![file.clone()],
        };
        assert!(matches!(
            validate_downloaded_artifact(&entry, &artifact),
            Err(ModelStoreError::ManifestInvalid(_))
        ));

        entry.required_files[0].size_bytes = Some(3);
        assert!(matches!(
            validate_downloaded_artifact(&entry, &artifact),
            Err(ModelStoreError::HashMismatch(_))
        ));
    }

    #[test]
    fn already_active_download_is_not_reported_as_success() {
        let root = tempfile::tempdir().expect("temp root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let entry = fixture_catalog_entry();
        store
            .mark_download_started(entry.id, entry.required_files.len() as u32)
            .expect("reserve first download");

        let error = store
            .download_with_observer(entry.id, |_| Ok(()))
            .expect_err("second owner must not succeed");

        assert!(matches!(error, ModelStoreError::DownloadAlreadyActive(_)));
        assert_eq!(error.stable_code(), "model.download.active");
    }

    #[test]
    fn http_cancellation_cannot_publish_staged_model() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
            Err(error) => panic!("bind model fixture server: {error}"),
        };
        let address = listener.local_addr().expect("fixture address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 512];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).expect("read request");
                assert_ne!(read, 0, "request ended before headers");
                request.extend_from_slice(&chunk[..read]);
            }
            let body = vec![b'm'; 192 * 1024];
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(headers.as_bytes()).expect("write headers");
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        });
        let root = tempfile::tempdir().expect("model root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let mut entry = fixture_catalog_entry();
        entry.required_files = vec![entry.required_files[0].clone()];
        let mut source = entry.artifact_sources[0].clone();
        source.include_files = vec![entry.required_files[0].path.clone()];
        let start = store
            .mark_download_started(entry.id, 1)
            .expect("start download");
        let StartDownload::Started { token, .. } = start else {
            panic!("download should start");
        };
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("fixture client"),
            "http://127.0.0.1/unused",
            format!("http://{address}"),
        );
        let cancelled = AtomicBool::new(false);
        let result = store
            .download_with_provider(
                &entry,
                &source,
                &provider,
                token,
                &|snapshot: ModelDownloadProgressSnapshot| {
                    if snapshot.downloaded_bytes > 0 && !cancelled.swap(true, Ordering::SeqCst) {
                        store.cancel_download(entry.id).expect("cancel download");
                    }
                    Ok(())
                },
                |pending| pending.publish(),
            )
            .expect("cancelled result");
        server.join().expect("fixture server");

        assert!(!result);
        assert!(cancelled.load(Ordering::SeqCst));
        assert!(!store.model_dir(entry.id).expect("model dir").exists());
    }

    #[test]
    fn cancellation_after_staged_verification_prevents_publication() {
        let root = tempfile::tempdir().expect("model root");
        let source_root = tempfile::tempdir().expect("source root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let mut entry = fixture_catalog_entry();
        entry.required_files = vec![entry.required_files[0].clone()];
        let mut source = entry.artifact_sources[0].clone();
        source.include_files = vec![entry.required_files[0].path.clone()];
        fs::write(
            source_root.path().join(&entry.required_files[0].path),
            b"valid",
        )
        .expect("fixture file");
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::new(),
            "http://127.0.0.1/unused",
            source_root.path().to_string_lossy().to_string(),
        );
        let StartDownload::Started { token, .. } = store
            .mark_download_started(entry.id, 1)
            .expect("reserve download")
        else {
            panic!("download should reserve");
        };

        let published = store
            .download_with_provider(&entry, &source, &provider, token, &|_| Ok(()), |pending| {
                assert!(store
                    .cancel_download_if_active(entry.id)
                    .expect("cancel at publish barrier"));
                pending.publish()
            })
            .expect("cancelled publication");

        assert!(!published);
        assert!(!store.model_dir(entry.id).expect("model dir").exists());
    }

    #[test]
    fn progress_persistence_failure_aborts_before_publication() {
        let root = tempfile::tempdir().expect("model root");
        let source_root = tempfile::tempdir().expect("source root");
        let store = TranscriptionModelStore::new(root.path().to_path_buf());
        let mut entry = fixture_catalog_entry();
        entry.required_files = vec![entry.required_files[0].clone()];
        let mut source = entry.artifact_sources[0].clone();
        source.include_files = vec![entry.required_files[0].path.clone()];
        fs::write(
            source_root.path().join(&entry.required_files[0].path),
            b"valid",
        )
        .expect("fixture file");
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::new(),
            "http://127.0.0.1/unused",
            source_root.path().to_string_lossy().to_string(),
        );
        let StartDownload::Started { token, .. } = store
            .mark_download_started(entry.id, 1)
            .expect("reserve download")
        else {
            panic!("download should reserve");
        };

        let error = store
            .download_with_provider(
                &entry,
                &source,
                &provider,
                token,
                &|_| {
                    Err(ModelStoreError::ProgressPersistence(
                        "journal unavailable".to_string(),
                    ))
                },
                |pending| pending.publish(),
            )
            .expect_err("progress persistence must abort");

        assert!(matches!(error, ModelStoreError::ProgressPersistence(_)));
        assert!(!store.model_dir(entry.id).expect("model dir").exists());
    }
}
