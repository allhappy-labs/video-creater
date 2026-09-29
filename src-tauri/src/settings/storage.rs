use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::UNIX_EPOCH;
use thiserror::Error;

const OPEN_PROJECT_REASON: &str = "Open a project to inspect";
const CLEANUP_QUARANTINE_PREFIX: &str = ".video-creater-cleanup-";

/// Audited app-owned producers for Tauri's `app_cache_dir` cleanup scope.
/// Project-local filmstrip, visual-search, speech, and render caches use project sidecars instead.
pub const DISPOSABLE_APP_CACHE_WRITER_IDS: &[&str] = &[];

static STORAGE_MUTATION_COORDINATOR: Mutex<()> = Mutex::new(());

/// Process-wide ownership proof for app-managed mutations that may overlap storage cleanup.
///
/// This lock is intentionally non-reentrant. Public writers acquire it once for their full
/// lifetime and pass the lease to internal helpers instead of acquiring it again.
pub struct StorageMutationLease {
    _guard: MutexGuard<'static, ()>,
}

pub fn acquire_storage_mutation_lease() -> Result<StorageMutationLease, String> {
    STORAGE_MUTATION_COORDINATOR
        .lock()
        .map(|guard| StorageMutationLease { _guard: guard })
        .map_err(|_| "storage/project mutation coordinator lock is poisoned".to_string())
}

#[derive(Debug, Clone, Copy)]
struct WalkLimits {
    max_depth: usize,
    max_entries: usize,
}

const DEFAULT_WALK_LIMITS: WalkLimits = WalkLimits {
    max_depth: 64,
    max_entries: 100_000,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StorageScope {
    GlobalModels,
    DisposableAppCache,
    ProjectMedia,
    ProjectTranscripts,
    ProjectRenderArtifacts,
    ProjectWorkflowArtifacts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageInventoryItem {
    pub id: String,
    pub scope: StorageScope,
    pub path: Option<String>,
    pub bytes: u64,
    pub free_bytes: Option<u64>,
    pub removable: bool,
    pub unavailable_reason: Option<String>,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StorageCleanupTarget {
    DisposableAppCache,
    ProjectRenderArtifacts { artifact_ids: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupPreviewItem {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupPreview {
    pub target: StorageCleanupTarget,
    pub items: Vec<StorageCleanupPreviewItem>,
    pub total_bytes: u64,
    pub preview_nonce: String,
    pub confirmation_token: String,
    pub project_generation: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupProgress {
    pub path: String,
    pub completed_items: u64,
    pub total_items: u64,
    pub removed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupReport {
    pub removed_paths: Vec<String>,
    pub removed_bytes: u64,
    pub removed_count: u64,
}

impl StorageCleanupReport {
    fn empty() -> Self {
        Self {
            removed_paths: Vec::new(),
            removed_bytes: 0,
            removed_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCleanupRunFailure {
    pub error: Box<StorageCleanupError>,
    pub partial_report: StorageCleanupReport,
    pub quarantined_paths: Vec<String>,
    pub persistence_boundary_unknown: bool,
}

impl StorageCleanupRunFailure {
    pub fn code(&self) -> &'static str {
        self.error.code()
    }
}

impl std::fmt::Display for StorageCleanupRunFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} Removed {} item(s) ({} bytes) before failure.",
            self.error, self.partial_report.removed_count, self.partial_report.removed_bytes
        )?;
        if self.persistence_boundary_unknown {
            write!(
                formatter,
                " Operation journal persistence boundary is unknown."
            )?;
        }
        if !self.quarantined_paths.is_empty() {
            write!(
                formatter,
                " Preserved quarantined paths: {}.",
                self.quarantined_paths.join(", ")
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for StorageCleanupRunFailure {}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageCleanupError {
    #[error("Open a project before cleaning project render artifacts.")]
    ActiveProjectRequired,
    #[error("Cleanup root {0} must be an absolute path without parent traversal.")]
    InvalidRoot(String),
    #[error("Cleanup artifact ID {0:?} is not a direct render artifact directory name.")]
    InvalidArtifactId(String),
    #[error("Cleanup artifact ID {0:?} is duplicated.")]
    DuplicateArtifactId(String),
    #[error("Cleanup target {0} is unavailable: {1}")]
    TargetUnavailable(String, String),
    #[error("Cleanup target {0} is not an artifact directory.")]
    TargetNotArtifactDirectory(String),
    #[error("Cleanup path {path} escapes its allowlisted root {root}.")]
    ScopeEscaped { path: String, root: String },
    #[error("Cleanup path {0} cannot be represented exactly as UTF-8.")]
    NonUtf8Path(String),
    #[error("Cleanup preview could not inspect {0}: {1}")]
    Inspect(String, String),
    #[error("Cleanup confirmation does not match the current exact target paths and contents.")]
    ConfirmationMismatch,
    #[error("Cleanup could not remove {0}: {1}")]
    Remove(String, String),
    #[error("Cleanup progress could not be reported: {0}")]
    Progress(String),
    #[error("The active project changed after cleanup was previewed.")]
    ProjectSessionMismatch,
    #[error("Cleanup resolves multiple requested paths to the same filesystem identity: {0}")]
    DuplicateResolvedIdentity(String),
    #[error("Cleanup quarantine could not safely stage {0}: {1}")]
    Quarantine(String, String),
    #[error("Cleanup quarantine identity no longer matches the confirmed item at {0}: {1}")]
    QuarantineIdentityMismatch(String, String),
}

impl StorageCleanupError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ActiveProjectRequired => "settings.storage.cleanupProjectRequired",
            Self::InvalidRoot(_) => "settings.storage.cleanupInvalidRoot",
            Self::InvalidArtifactId(_) | Self::DuplicateArtifactId(_) => {
                "settings.storage.cleanupInvalidArtifactId"
            }
            Self::TargetUnavailable(_, _) => "settings.storage.cleanupTargetUnavailable",
            Self::TargetNotArtifactDirectory(_) => {
                "settings.storage.cleanupTargetNotArtifactDirectory"
            }
            Self::ScopeEscaped { .. } => "settings.storage.cleanupScopeEscaped",
            Self::NonUtf8Path(_) => "settings.storage.cleanupNonUtf8Path",
            Self::Inspect(_, _) => "settings.storage.cleanupInspectionFailed",
            Self::ConfirmationMismatch => "settings.storage.cleanupConfirmationMismatch",
            Self::Remove(_, _) => "settings.storage.cleanupRemoveFailed",
            Self::Progress(_) => "settings.storage.cleanupProgressFailed",
            Self::ProjectSessionMismatch => "settings.storage.cleanupProjectSessionMismatch",
            Self::DuplicateResolvedIdentity(_) => {
                "settings.storage.cleanupDuplicateResolvedIdentity"
            }
            Self::Quarantine(_, _) => "settings.storage.cleanupQuarantineFailed",
            Self::QuarantineIdentityMismatch(_, _) => {
                "settings.storage.cleanupConfirmationMismatch"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum CleanupEntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct CleanupFileIdentity {
    device: u64,
    inode: u64,
    entry_type: CleanupEntryType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedCleanupItem {
    path: PathBuf,
    bytes: u64,
    fingerprint: [u8; 32],
    identity: CleanupFileIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedCleanupPlan {
    scope_root: PathBuf,
    items: Vec<ResolvedCleanupItem>,
}

pub fn collect_storage_inventory(
    global_model_root: &Path,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
) -> Vec<StorageInventoryItem> {
    collect_storage_inventory_with_limits(
        global_model_root,
        app_cache_root,
        active_project_dir,
        DEFAULT_WALK_LIMITS,
    )
}

fn collect_storage_inventory_with_limits(
    global_model_root: &Path,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    limits: WalkLimits,
) -> Vec<StorageInventoryItem> {
    let mut inventory = vec![
        measured_item(
            "storage.globalModels",
            StorageScope::GlobalModels,
            global_model_root,
            &[global_model_root],
            false,
            None,
            limits,
        ),
        measured_item(
            "storage.disposableAppCache",
            StorageScope::DisposableAppCache,
            app_cache_root,
            &[app_cache_root],
            true,
            None,
            limits,
        ),
    ];

    let Some(project_dir) = active_project_dir else {
        inventory.extend(project_unavailable_items());
        return inventory;
    };
    let canonical_project_root = match fs::canonicalize(project_dir) {
        Ok(path) if path.is_dir() => path,
        _ => {
            inventory.extend(project_error_items(
                project_dir,
                "The active project folder is unavailable.",
            ));
            return inventory;
        }
    };
    let media = project_dir.join("media");
    let transcripts = project_dir.join("transcripts");
    let renders = project_dir.join("renders");
    let jobs = project_dir.join("jobs");
    let logs = project_dir.join("logs");
    inventory.extend([
        measured_item(
            "storage.projectMedia",
            StorageScope::ProjectMedia,
            &media,
            &[&media],
            false,
            Some(&canonical_project_root),
            limits,
        ),
        measured_item(
            "storage.projectTranscripts",
            StorageScope::ProjectTranscripts,
            &transcripts,
            &[&transcripts],
            false,
            Some(&canonical_project_root),
            limits,
        ),
        measured_item(
            "storage.projectRenderArtifacts",
            StorageScope::ProjectRenderArtifacts,
            &renders,
            &[&renders],
            true,
            Some(&canonical_project_root),
            limits,
        ),
        measured_item(
            "storage.projectWorkflowArtifacts",
            StorageScope::ProjectWorkflowArtifacts,
            project_dir,
            &[&jobs, &logs],
            false,
            Some(&canonical_project_root),
            limits,
        ),
    ]);
    inventory
}

pub fn preview_storage_cleanup(
    target: &StorageCleanupTarget,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
) -> Result<StorageCleanupPreview, StorageCleanupError> {
    preview_storage_cleanup_for_generation(target, app_cache_root, active_project_dir, None)
}

pub fn preview_storage_cleanup_for_generation(
    target: &StorageCleanupTarget,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    project_generation: Option<u64>,
) -> Result<StorageCleanupPreview, StorageCleanupError> {
    let preview_nonce = uuid::Uuid::new_v4().to_string();
    preview_storage_cleanup_for_generation_with_nonce(
        target,
        app_cache_root,
        active_project_dir,
        project_generation,
        &preview_nonce,
    )
}

pub fn preview_storage_cleanup_for_generation_with_nonce(
    target: &StorageCleanupTarget,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    project_generation: Option<u64>,
    preview_nonce: &str,
) -> Result<StorageCleanupPreview, StorageCleanupError> {
    let plan = resolve_cleanup_plan(target, app_cache_root, active_project_dir)?;
    let project_generation = match target {
        StorageCleanupTarget::DisposableAppCache => None,
        StorageCleanupTarget::ProjectRenderArtifacts { .. } => project_generation,
    };
    let confirmation_token =
        cleanup_confirmation_token(target, &plan, project_generation, preview_nonce);
    let items = plan
        .items
        .iter()
        .map(|item| {
            Ok(StorageCleanupPreviewItem {
                path: exact_path_string(&item.path)?,
                bytes: item.bytes,
            })
        })
        .collect::<Result<Vec<_>, StorageCleanupError>>()?;
    let total_bytes = items
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    Ok(StorageCleanupPreview {
        target: target.clone(),
        items,
        total_bytes,
        preview_nonce: preview_nonce.to_string(),
        confirmation_token,
        project_generation,
    })
}

pub fn run_storage_cleanup<F>(
    target: &StorageCleanupTarget,
    confirmation_token: &str,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    progress: F,
) -> Result<StorageCleanupReport, StorageCleanupRunFailure>
where
    F: FnMut(&StorageCleanupProgress) -> Result<(), StorageCleanupError>,
{
    run_storage_cleanup_with_hook_for_generation(
        target,
        confirmation_token,
        app_cache_root,
        active_project_dir,
        None,
        |_| Ok(()),
        progress,
    )
}

pub fn run_storage_cleanup_for_generation<F>(
    target: &StorageCleanupTarget,
    confirmation_token: &str,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    project_generation: Option<u64>,
    progress: F,
) -> Result<StorageCleanupReport, StorageCleanupRunFailure>
where
    F: FnMut(&StorageCleanupProgress) -> Result<(), StorageCleanupError>,
{
    run_storage_cleanup_with_hook_for_generation(
        target,
        confirmation_token,
        app_cache_root,
        active_project_dir,
        project_generation,
        |_| Ok(()),
        progress,
    )
}

pub fn run_storage_cleanup_with_hook<B, F>(
    target: &StorageCleanupTarget,
    confirmation_token: &str,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    before_quarantine: B,
    progress: F,
) -> Result<StorageCleanupReport, StorageCleanupRunFailure>
where
    B: FnMut(&Path) -> Result<(), StorageCleanupError>,
    F: FnMut(&StorageCleanupProgress) -> Result<(), StorageCleanupError>,
{
    run_storage_cleanup_with_hook_for_generation(
        target,
        confirmation_token,
        app_cache_root,
        active_project_dir,
        None,
        before_quarantine,
        progress,
    )
}

pub fn run_storage_cleanup_with_hook_for_generation<B, F>(
    target: &StorageCleanupTarget,
    confirmation_token: &str,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    project_generation: Option<u64>,
    mut before_quarantine: B,
    mut progress: F,
) -> Result<StorageCleanupReport, StorageCleanupRunFailure>
where
    B: FnMut(&Path) -> Result<(), StorageCleanupError>,
    F: FnMut(&StorageCleanupProgress) -> Result<(), StorageCleanupError>,
{
    let mut report = StorageCleanupReport::empty();
    let plan = resolve_cleanup_plan(target, app_cache_root, active_project_dir)
        .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?;
    let project_generation = match target {
        StorageCleanupTarget::DisposableAppCache => None,
        StorageCleanupTarget::ProjectRenderArtifacts { .. } => project_generation,
    };
    let Some(preview_nonce) = cleanup_confirmation_token_nonce(confirmation_token) else {
        return Err(cleanup_run_failure(
            StorageCleanupError::ConfirmationMismatch,
            report,
            Vec::new(),
        ));
    };
    if confirmation_token
        != cleanup_confirmation_token(target, &plan, project_generation, preview_nonce)
    {
        return Err(cleanup_run_failure(
            StorageCleanupError::ConfirmationMismatch,
            report,
            Vec::new(),
        ));
    }

    let total_items = plan.items.len() as u64;
    let quarantine_root = if plan.items.is_empty() {
        None
    } else {
        Some(CleanupQuarantineGuard::new(
            create_cleanup_quarantine(&plan.scope_root)
                .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?,
        ))
    };
    for expected in &plan.items {
        revalidate_cleanup_item(
            target,
            app_cache_root,
            active_project_dir,
            &plan.scope_root,
            expected,
        )
        .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?;
        before_quarantine(&expected.path)
            .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?;
        let quarantine_root = quarantine_root
            .as_ref()
            .expect("non-empty cleanup plan has a quarantine root")
            .path();
        let quarantined = quarantine_cleanup_item(&plan.scope_root, quarantine_root, expected)
            .map_err(|failure| {
                cleanup_run_failure(failure.error, report.clone(), failure.quarantined_paths)
            })?;
        remove_verified_quarantined_cleanup_item(
            &plan.scope_root,
            quarantine_root,
            expected,
            &quarantined,
        )
        .map_err(|failure| {
            cleanup_run_failure(failure.error, report.clone(), failure.quarantined_paths)
        })?;
        let path = exact_path_string(&expected.path)
            .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?;
        report.removed_bytes = report.removed_bytes.saturating_add(expected.bytes);
        report.removed_count = report.removed_count.saturating_add(1);
        report.removed_paths.push(path.clone());
        progress(&StorageCleanupProgress {
            path,
            completed_items: report.removed_count,
            total_items,
            removed_bytes: report.removed_bytes,
        })
        .map_err(|error| cleanup_run_failure(error, report.clone(), Vec::new()))?;
    }

    Ok(report)
}

fn cleanup_run_failure(
    error: StorageCleanupError,
    partial_report: StorageCleanupReport,
    quarantined_paths: Vec<String>,
) -> StorageCleanupRunFailure {
    let persistence_boundary_unknown = matches!(error, StorageCleanupError::Progress(_));
    StorageCleanupRunFailure {
        error: Box::new(error),
        partial_report,
        quarantined_paths,
        persistence_boundary_unknown,
    }
}

#[derive(Debug)]
struct QuarantineFailure {
    error: StorageCleanupError,
    quarantined_paths: Vec<String>,
}

struct CleanupQuarantineGuard {
    path: PathBuf,
}

impl CleanupQuarantineGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CleanupQuarantineGuard {
    fn drop(&mut self) {
        remove_empty_quarantine_root(&self.path);
    }
}

fn resolve_cleanup_plan(
    target: &StorageCleanupTarget,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
) -> Result<ResolvedCleanupPlan, StorageCleanupError> {
    match target {
        StorageCleanupTarget::DisposableAppCache => {
            validate_configured_root(app_cache_root)?;
            let Some(scope_root) = canonical_existing_directory(app_cache_root, true)? else {
                return Ok(ResolvedCleanupPlan {
                    scope_root: app_cache_root.to_path_buf(),
                    items: Vec::new(),
                });
            };
            ensure_no_cleanup_quarantines(&scope_root)?;
            let entries = fs::read_dir(&scope_root).map_err(|error| {
                StorageCleanupError::Inspect(scope_root.display().to_string(), error.to_string())
            })?;
            let mut paths = entries
                .map(|entry| {
                    entry.map(|entry| entry.path()).map_err(|error| {
                        StorageCleanupError::Inspect(
                            scope_root.display().to_string(),
                            error.to_string(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, StorageCleanupError>>()?;
            paths.retain(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with(CLEANUP_QUARANTINE_PREFIX))
            });
            paths.sort();
            let items = paths
                .into_iter()
                .map(|path| scan_cleanup_item(&path, &scope_root))
                .collect::<Result<Vec<_>, StorageCleanupError>>()?;
            ensure_unique_resolved_items(&items)?;
            Ok(ResolvedCleanupPlan { scope_root, items })
        }
        StorageCleanupTarget::ProjectRenderArtifacts { artifact_ids } => {
            let project_dir =
                active_project_dir.ok_or(StorageCleanupError::ActiveProjectRequired)?;
            validate_configured_root(project_dir)?;
            let project_root = canonical_existing_directory(project_dir, false)?
                .ok_or(StorageCleanupError::ActiveProjectRequired)?;
            let renders = project_dir.join("renders");
            let scope_root = canonical_existing_directory(&renders, false)?.ok_or_else(|| {
                StorageCleanupError::TargetUnavailable(
                    renders.display().to_string(),
                    "render artifact root does not exist".to_string(),
                )
            })?;
            ensure_contained(&scope_root, &project_root)?;
            ensure_no_cleanup_quarantines(&scope_root)?;
            let artifact_ids = validated_artifact_ids(artifact_ids)?;
            let mut items = Vec::with_capacity(artifact_ids.len());
            for artifact_id in artifact_ids {
                let path = scope_root.join(&artifact_id);
                let metadata = fs::symlink_metadata(&path).map_err(|error| {
                    StorageCleanupError::TargetUnavailable(
                        path.display().to_string(),
                        error.to_string(),
                    )
                })?;
                if metadata.file_type().is_symlink() {
                    return Err(scope_escaped(&path, &scope_root));
                }
                if !metadata.is_dir() {
                    return Err(StorageCleanupError::TargetNotArtifactDirectory(
                        path.display().to_string(),
                    ));
                }
                items.push(scan_cleanup_item(&path, &scope_root)?);
            }
            ensure_unique_resolved_items(&items)?;
            Ok(ResolvedCleanupPlan { scope_root, items })
        }
    }
}

fn validated_artifact_ids(artifact_ids: &[String]) -> Result<Vec<String>, StorageCleanupError> {
    if artifact_ids.is_empty() {
        return Err(StorageCleanupError::InvalidArtifactId(String::new()));
    }
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(artifact_ids.len());
    for artifact_id in artifact_ids {
        if !is_valid_artifact_id(artifact_id) {
            return Err(StorageCleanupError::InvalidArtifactId(artifact_id.clone()));
        }
        if !seen.insert(artifact_id.clone()) {
            return Err(StorageCleanupError::DuplicateArtifactId(
                artifact_id.clone(),
            ));
        }
        validated.push(artifact_id.clone());
    }
    validated.sort();
    Ok(validated)
}

fn is_valid_artifact_id(artifact_id: &str) -> bool {
    let mut components = Path::new(artifact_id).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !artifact_id.contains('/')
        && !artifact_id.contains('\\')
        && artifact_id != "index.json"
        && !artifact_id.starts_with(CLEANUP_QUARANTINE_PREFIX)
}

fn ensure_unique_resolved_items(items: &[ResolvedCleanupItem]) -> Result<(), StorageCleanupError> {
    let mut paths = HashSet::new();
    let mut identities = HashSet::new();
    for item in items {
        if !paths.insert(item.path.clone()) || !identities.insert(item.identity) {
            return Err(StorageCleanupError::DuplicateResolvedIdentity(
                item.path.display().to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_configured_root(path: &Path) -> Result<(), StorageCleanupError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(StorageCleanupError::InvalidRoot(path.display().to_string()));
    }
    Ok(())
}

fn canonical_existing_directory(
    path: &Path,
    allow_missing: bool,
) -> Result<Option<PathBuf>, StorageCleanupError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => {
            return Err(StorageCleanupError::TargetUnavailable(
                path.display().to_string(),
                error.to_string(),
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(scope_escaped(path, path));
    }
    if !metadata.is_dir() {
        return Err(StorageCleanupError::TargetUnavailable(
            path.display().to_string(),
            "expected a directory".to_string(),
        ));
    }
    fs::canonicalize(path).map(Some).map_err(|error| {
        StorageCleanupError::TargetUnavailable(path.display().to_string(), error.to_string())
    })
}

fn scan_cleanup_item(
    path: &Path,
    allowed_root: &Path,
) -> Result<ResolvedCleanupItem, StorageCleanupError> {
    let root_metadata = fs::symlink_metadata(path).map_err(|error| {
        StorageCleanupError::Inspect(path.display().to_string(), error.to_string())
    })?;
    if root_metadata.file_type().is_symlink() {
        return Err(scope_escaped(path, allowed_root));
    }
    let canonical_path = fs::canonicalize(path).map_err(|error| {
        StorageCleanupError::Inspect(path.display().to_string(), error.to_string())
    })?;
    ensure_contained(&canonical_path, allowed_root)?;
    if canonical_path.parent() != Some(allowed_root) {
        return Err(scope_escaped(&canonical_path, allowed_root));
    }
    let identity = cleanup_file_identity(&root_metadata)?;

    let mut hasher = Sha256::new();
    let mut total_bytes = 0_u64;
    let mut pending = vec![canonical_path.clone()];
    while let Some(current) = pending.pop() {
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            StorageCleanupError::Inspect(current.display().to_string(), error.to_string())
        })?;
        let relative = current.strip_prefix(&canonical_path).map_err(|_| {
            StorageCleanupError::Inspect(
                current.display().to_string(),
                "cleanup fingerprint path escaped its item root".to_string(),
            )
        })?;
        update_digest_field(&mut hasher, relative.to_string_lossy().as_bytes());
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        update_digest_field(&mut hasher, &modified_nanos.to_le_bytes());
        if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(&current).map_err(|error| {
                StorageCleanupError::Inspect(current.display().to_string(), error.to_string())
            })?;
            ensure_contained(&target, allowed_root)?;
            update_digest_field(&mut hasher, b"symlink");
            update_digest_field(&mut hasher, exact_path_string(&target)?.as_bytes());
        } else if metadata.is_file() {
            let canonical = fs::canonicalize(&current).map_err(|error| {
                StorageCleanupError::Inspect(current.display().to_string(), error.to_string())
            })?;
            ensure_contained(&canonical, allowed_root)?;
            update_digest_field(&mut hasher, b"file");
            update_digest_field(&mut hasher, &metadata.len().to_le_bytes());
            total_bytes = total_bytes.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            let canonical = fs::canonicalize(&current).map_err(|error| {
                StorageCleanupError::Inspect(current.display().to_string(), error.to_string())
            })?;
            ensure_contained(&canonical, allowed_root)?;
            update_digest_field(&mut hasher, b"directory");
            let entries = fs::read_dir(&current).map_err(|error| {
                StorageCleanupError::Inspect(current.display().to_string(), error.to_string())
            })?;
            let mut children = entries
                .map(|entry| {
                    entry.map(|entry| entry.path()).map_err(|error| {
                        StorageCleanupError::Inspect(
                            current.display().to_string(),
                            error.to_string(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, StorageCleanupError>>()?;
            children.sort_by(|left, right| right.cmp(left));
            pending.extend(children);
        } else {
            return Err(StorageCleanupError::Inspect(
                current.display().to_string(),
                "unsupported filesystem entry type".to_string(),
            ));
        }
    }

    Ok(ResolvedCleanupItem {
        path: canonical_path,
        bytes: total_bytes,
        fingerprint: hasher.finalize().into(),
        identity,
    })
}

#[cfg(unix)]
fn cleanup_file_identity(
    metadata: &fs::Metadata,
) -> Result<CleanupFileIdentity, StorageCleanupError> {
    let entry_type = if metadata.is_file() {
        CleanupEntryType::File
    } else if metadata.is_dir() {
        CleanupEntryType::Directory
    } else {
        return Err(StorageCleanupError::Inspect(
            "filesystem identity".to_string(),
            "cleanup supports only regular files and directories".to_string(),
        ));
    };
    Ok(CleanupFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        entry_type,
    })
}

#[cfg(not(unix))]
fn cleanup_file_identity(
    _metadata: &fs::Metadata,
) -> Result<CleanupFileIdentity, StorageCleanupError> {
    Err(StorageCleanupError::Inspect(
        "filesystem identity".to_string(),
        "stable cleanup identity is unavailable on this platform".to_string(),
    ))
}

fn cleanup_confirmation_token(
    target: &StorageCleanupTarget,
    plan: &ResolvedCleanupPlan,
    project_generation: Option<u64>,
    preview_nonce: &str,
) -> String {
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, b"video-creater-storage-cleanup-v1");
    update_digest_field(&mut hasher, preview_nonce.as_bytes());
    match target {
        StorageCleanupTarget::DisposableAppCache => {
            update_digest_field(&mut hasher, b"disposable-app-cache");
        }
        StorageCleanupTarget::ProjectRenderArtifacts { artifact_ids } => {
            update_digest_field(&mut hasher, b"project-render-artifacts");
            match project_generation {
                Some(generation) => update_digest_field(&mut hasher, &generation.to_le_bytes()),
                None => update_digest_field(&mut hasher, b"project-generation-unbound"),
            }
            let mut artifact_ids = artifact_ids.clone();
            artifact_ids.sort();
            for artifact_id in artifact_ids {
                update_digest_field(&mut hasher, artifact_id.as_bytes());
            }
        }
    }
    update_digest_field(&mut hasher, plan.scope_root.to_string_lossy().as_bytes());
    for item in &plan.items {
        update_digest_field(&mut hasher, item.path.to_string_lossy().as_bytes());
        update_digest_field(&mut hasher, &item.bytes.to_le_bytes());
        update_digest_field(&mut hasher, &item.fingerprint);
        update_digest_field(&mut hasher, &item.identity.device.to_le_bytes());
        update_digest_field(&mut hasher, &item.identity.inode.to_le_bytes());
        update_digest_field(
            &mut hasher,
            match item.identity.entry_type {
                CleanupEntryType::File => b"file",
                CleanupEntryType::Directory => b"directory",
            },
        );
    }
    format!("storage-cleanup-v1:{preview_nonce}:{:x}", hasher.finalize())
}

fn cleanup_confirmation_token_nonce(value: &str) -> Option<&str> {
    let value = value.strip_prefix("storage-cleanup-v1:")?;
    let (preview_nonce, digest) = value.split_once(':')?;
    if uuid::Uuid::parse_str(preview_nonce).is_err()
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(preview_nonce)
}

pub fn is_storage_cleanup_confirmation_token(value: &str) -> bool {
    cleanup_confirmation_token_nonce(value).is_some()
}

fn update_digest_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn revalidate_cleanup_item(
    target: &StorageCleanupTarget,
    app_cache_root: &Path,
    active_project_dir: Option<&Path>,
    expected_scope_root: &Path,
    expected: &ResolvedCleanupItem,
) -> Result<(), StorageCleanupError> {
    let current_scope_root = match target {
        StorageCleanupTarget::DisposableAppCache => {
            canonical_existing_directory(app_cache_root, false)?.ok_or_else(|| {
                StorageCleanupError::TargetUnavailable(
                    app_cache_root.display().to_string(),
                    "cache root disappeared".to_string(),
                )
            })?
        }
        StorageCleanupTarget::ProjectRenderArtifacts { artifact_ids } => {
            let project_dir =
                active_project_dir.ok_or(StorageCleanupError::ActiveProjectRequired)?;
            let project_root = canonical_existing_directory(project_dir, false)?
                .ok_or(StorageCleanupError::ActiveProjectRequired)?;
            let renders = canonical_existing_directory(&project_dir.join("renders"), false)?
                .ok_or_else(|| {
                    StorageCleanupError::TargetUnavailable(
                        project_dir.join("renders").display().to_string(),
                        "render artifact root disappeared".to_string(),
                    )
                })?;
            ensure_contained(&renders, &project_root)?;
            let valid_ids = validated_artifact_ids(artifact_ids)?;
            let file_name = expected
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| StorageCleanupError::InvalidArtifactId(String::new()))?;
            if !valid_ids.iter().any(|id| id == file_name) {
                return Err(StorageCleanupError::InvalidArtifactId(
                    file_name.to_string(),
                ));
            }
            renders
        }
    };
    if current_scope_root != expected_scope_root {
        return Err(scope_escaped(&current_scope_root, expected_scope_root));
    }
    if expected.path.parent() != Some(current_scope_root.as_path()) {
        return Err(scope_escaped(&expected.path, &current_scope_root));
    }
    let current = scan_cleanup_item(&expected.path, &current_scope_root)?;
    if current != *expected {
        return Err(StorageCleanupError::ConfirmationMismatch);
    }
    Ok(())
}

fn ensure_no_cleanup_quarantines(scope_root: &Path) -> Result<(), StorageCleanupError> {
    for entry in fs::read_dir(scope_root).map_err(|error| {
        StorageCleanupError::Inspect(scope_root.display().to_string(), error.to_string())
    })? {
        let entry = entry.map_err(|error| {
            StorageCleanupError::Inspect(scope_root.display().to_string(), error.to_string())
        })?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(CLEANUP_QUARANTINE_PREFIX))
        {
            return Err(StorageCleanupError::TargetUnavailable(
                entry.path().display().to_string(),
                "a prior cleanup preserved a private quarantine; recover it before retrying"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn create_cleanup_quarantine(scope_root: &Path) -> Result<PathBuf, StorageCleanupError> {
    let quarantine_root = scope_root.join(format!(
        "{CLEANUP_QUARANTINE_PREFIX}{}",
        uuid::Uuid::new_v4()
    ));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&quarantine_root).map_err(|error| {
            StorageCleanupError::Quarantine(
                quarantine_root.display().to_string(),
                error.to_string(),
            )
        })?;
    }
    #[cfg(not(unix))]
    fs::create_dir(&quarantine_root).map_err(|error| {
        StorageCleanupError::Quarantine(quarantine_root.display().to_string(), error.to_string())
    })?;
    ensure_cleanup_quarantine_directory(scope_root, &quarantine_root)?;
    Ok(quarantine_root)
}

fn quarantine_cleanup_item(
    scope_root: &Path,
    quarantine_root: &Path,
    expected: &ResolvedCleanupItem,
) -> Result<PathBuf, QuarantineFailure> {
    ensure_cleanup_quarantine_directory(scope_root, quarantine_root).map_err(|error| {
        QuarantineFailure {
            error,
            quarantined_paths: Vec::new(),
        }
    })?;
    let source_name = expected.path.file_name().ok_or_else(|| QuarantineFailure {
        error: StorageCleanupError::Quarantine(
            expected.path.display().to_string(),
            "cleanup item has no direct-child name".to_string(),
        ),
        quarantined_paths: Vec::new(),
    })?;
    let quarantine_name = format!("item-{}", uuid::Uuid::new_v4());
    atomic_rename_between_directories(
        scope_root,
        source_name,
        quarantine_root,
        std::ffi::OsStr::new(&quarantine_name),
    )
    .map_err(|error| QuarantineFailure {
        error: StorageCleanupError::Quarantine(
            expected.path.display().to_string(),
            error.to_string(),
        ),
        quarantined_paths: Vec::new(),
    })?;
    let quarantined = quarantine_root.join(&quarantine_name);
    let actual = scan_cleanup_item(&quarantined, quarantine_root);
    match actual {
        Ok(actual) if cleanup_item_matches(&actual, expected) => {}
        Ok(_) => {
            return Err(restore_quarantine_mismatch(
                scope_root,
                quarantine_root,
                expected,
                &quarantined,
                "the renamed identity differed from preview",
            ));
        }
        Err(error) => {
            return Err(restore_quarantine_mismatch(
                scope_root,
                quarantine_root,
                expected,
                &quarantined,
                &format!("the renamed item could not be verified: {error}"),
            ));
        }
    }
    Ok(quarantined)
}

fn cleanup_item_matches(actual: &ResolvedCleanupItem, expected: &ResolvedCleanupItem) -> bool {
    actual.identity == expected.identity
        && actual.bytes == expected.bytes
        && actual.fingerprint == expected.fingerprint
}

fn remove_verified_quarantined_cleanup_item(
    scope_root: &Path,
    quarantine_root: &Path,
    expected: &ResolvedCleanupItem,
    quarantined: &Path,
) -> Result<(), QuarantineFailure> {
    let actual = scan_cleanup_item(quarantined, quarantine_root);
    match actual {
        Ok(actual) if cleanup_item_matches(&actual, expected) => {}
        Ok(_) => {
            return Err(restore_quarantine_mismatch(
                scope_root,
                quarantine_root,
                expected,
                quarantined,
                "the quarantined identity changed immediately before deletion",
            ));
        }
        Err(error) => {
            return Err(restore_quarantine_mismatch(
                scope_root,
                quarantine_root,
                expected,
                quarantined,
                &format!("the quarantined item could not be revalidated: {error}"),
            ));
        }
    }
    remove_cleanup_item(quarantined).map_err(|error| QuarantineFailure {
        error,
        quarantined_paths: vec![quarantined.display().to_string()],
    })?;
    Ok(())
}

fn restore_quarantine_mismatch(
    scope_root: &Path,
    quarantine_root: &Path,
    expected: &ResolvedCleanupItem,
    quarantined: &Path,
    reason: &str,
) -> QuarantineFailure {
    let quarantine_name = quarantined.file_name();
    let source_name = expected.path.file_name();
    let restore = match (quarantine_name, source_name) {
        (Some(quarantine_name), Some(source_name)) => atomic_restore_without_replace(
            quarantine_root,
            quarantine_name,
            scope_root,
            source_name,
        ),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cleanup restore path is not a direct child",
        )),
    };
    let (detail, quarantined_paths) = match restore {
        Ok(()) => {
            remove_empty_quarantine_root(quarantine_root);
            (
                format!("{reason} and was restored without replacement"),
                Vec::new(),
            )
        }
        Err(error) => (
            format!("{reason}; safe restore failed: {error}"),
            vec![quarantined.display().to_string()],
        ),
    };
    QuarantineFailure {
        error: StorageCleanupError::QuarantineIdentityMismatch(
            expected.path.display().to_string(),
            detail,
        ),
        quarantined_paths,
    }
}

fn ensure_cleanup_quarantine_directory(
    scope_root: &Path,
    quarantine_root: &Path,
) -> Result<(), StorageCleanupError> {
    let metadata = fs::symlink_metadata(quarantine_root).map_err(|error| {
        StorageCleanupError::Quarantine(quarantine_root.display().to_string(), error.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(scope_escaped(quarantine_root, scope_root));
    }
    #[cfg(unix)]
    if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o077 != 0 {
        return Err(StorageCleanupError::Quarantine(
            quarantine_root.display().to_string(),
            "private cleanup quarantine permissions are broader than 0700".to_string(),
        ));
    }
    let canonical = fs::canonicalize(quarantine_root).map_err(|error| {
        StorageCleanupError::Quarantine(quarantine_root.display().to_string(), error.to_string())
    })?;
    if canonical.parent() != Some(scope_root) {
        return Err(scope_escaped(&canonical, scope_root));
    }
    Ok(())
}

#[cfg(unix)]
fn open_directory_no_follow(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(unix)]
fn direct_name_cstring(name: &std::ffi::OsStr) -> std::io::Result<CString> {
    if Path::new(name).components().count() != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cleanup rename name must be one direct path component",
        ));
    }
    CString::new(name.as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cleanup rename name contains NUL",
        )
    })
}

#[cfg(unix)]
fn atomic_rename_between_directories(
    source_dir: &Path,
    source_name: &std::ffi::OsStr,
    destination_dir: &Path,
    destination_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    let source_dir = open_directory_no_follow(source_dir)?;
    let destination_dir = open_directory_no_follow(destination_dir)?;
    let source_name = direct_name_cstring(source_name)?;
    let destination_name = direct_name_cstring(destination_name)?;
    let result = unsafe {
        libc::renameat(
            source_dir.as_raw_fd(),
            source_name.as_ptr(),
            destination_dir.as_raw_fd(),
            destination_name.as_ptr(),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn atomic_rename_between_directories(
    source_dir: &Path,
    source_name: &std::ffi::OsStr,
    destination_dir: &Path,
    destination_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    fs::rename(
        source_dir.join(source_name),
        destination_dir.join(destination_name),
    )
}

#[cfg(target_os = "macos")]
fn atomic_restore_without_replace(
    source_dir: &Path,
    source_name: &std::ffi::OsStr,
    destination_dir: &Path,
    destination_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    let source_dir = open_directory_no_follow(source_dir)?;
    let destination_dir = open_directory_no_follow(destination_dir)?;
    let source_name = direct_name_cstring(source_name)?;
    let destination_name = direct_name_cstring(destination_name)?;
    let result = unsafe {
        libc::renameatx_np(
            source_dir.as_raw_fd(),
            source_name.as_ptr(),
            destination_dir.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn atomic_restore_without_replace(
    source_dir: &Path,
    source_name: &std::ffi::OsStr,
    destination_dir: &Path,
    destination_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    let source_dir = open_directory_no_follow(source_dir)?;
    let destination_dir = open_directory_no_follow(destination_dir)?;
    let source_name = direct_name_cstring(source_name)?;
    let destination_name = direct_name_cstring(destination_name)?;
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            source_dir.as_raw_fd(),
            source_name.as_ptr(),
            destination_dir.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn atomic_restore_without_replace(
    _source_dir: &Path,
    _source_name: &std::ffi::OsStr,
    _destination_dir: &Path,
    _destination_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace restore is unavailable on this platform",
    ))
}

fn remove_empty_quarantine_root(quarantine_root: &Path) {
    let is_empty = fs::read_dir(quarantine_root)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_none();
    if is_empty {
        let _ = fs::remove_dir(quarantine_root);
    }
}

fn remove_cleanup_item(path: &Path) -> Result<(), StorageCleanupError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        StorageCleanupError::Remove(path.display().to_string(), error.to_string())
    })?;
    if metadata.file_type().is_symlink() {
        return Err(StorageCleanupError::ScopeEscaped {
            path: path.display().to_string(),
            root: path.parent().unwrap_or(path).display().to_string(),
        });
    }
    let result = if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result
        .map_err(|error| StorageCleanupError::Remove(path.display().to_string(), error.to_string()))
}

fn ensure_contained(path: &Path, allowed_root: &Path) -> Result<(), StorageCleanupError> {
    if path.starts_with(allowed_root) {
        Ok(())
    } else {
        Err(scope_escaped(path, allowed_root))
    }
}

fn scope_escaped(path: &Path, root: &Path) -> StorageCleanupError {
    StorageCleanupError::ScopeEscaped {
        path: path.display().to_string(),
        root: root.display().to_string(),
    }
}

fn exact_path_string(path: &Path) -> Result<String, StorageCleanupError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| StorageCleanupError::NonUtf8Path(path.display().to_string()))
}

fn measured_item(
    id: &str,
    scope: StorageScope,
    display_path: &Path,
    measured_roots: &[&Path],
    removable: bool,
    allowed_root: Option<&Path>,
    limits: WalkLimits,
) -> StorageInventoryItem {
    let measured = measured_roots.iter().try_fold(0_u64, |total, root| {
        bounded_directory_bytes(root, allowed_root, limits).map(|bytes| total.saturating_add(bytes))
    });
    let (bytes, mut unavailable_reason) = match measured {
        Ok(bytes) => (bytes, None),
        Err(error) => (0, Some(error)),
    };
    let artifact_ids =
        if scope == StorageScope::ProjectRenderArtifacts && unavailable_reason.is_none() {
            match list_render_artifact_ids(display_path, allowed_root) {
                Ok(artifact_ids) => artifact_ids,
                Err(error) => {
                    unavailable_reason = Some(error.to_string());
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
    StorageInventoryItem {
        id: id.to_string(),
        scope,
        path: Some(display_path.display().to_string()),
        bytes,
        free_bytes: unavailable_reason
            .is_none()
            .then(|| volume_free_bytes(display_path))
            .flatten(),
        removable,
        unavailable_reason,
        artifact_ids,
    }
}

fn list_render_artifact_ids(
    render_root: &Path,
    allowed_root: Option<&Path>,
) -> Result<Vec<String>, StorageCleanupError> {
    let metadata = match fs::symlink_metadata(render_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(StorageCleanupError::Inspect(
                render_root.display().to_string(),
                error.to_string(),
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(scope_escaped(render_root, render_root));
    }
    let canonical_render_root = fs::canonicalize(render_root).map_err(|error| {
        StorageCleanupError::Inspect(render_root.display().to_string(), error.to_string())
    })?;
    if let Some(allowed_root) = allowed_root {
        ensure_contained(&canonical_render_root, allowed_root)?;
    }

    let entries = fs::read_dir(&canonical_render_root).map_err(|error| {
        StorageCleanupError::Inspect(
            canonical_render_root.display().to_string(),
            error.to_string(),
        )
    })?;
    let mut artifact_ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            StorageCleanupError::Inspect(
                canonical_render_root.display().to_string(),
                error.to_string(),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            StorageCleanupError::Inspect(entry.path().display().to_string(), error.to_string())
        })?;
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        let Some(artifact_id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if is_valid_artifact_id(&artifact_id) {
            artifact_ids.push(artifact_id);
        }
    }
    artifact_ids.sort();
    artifact_ids.dedup();
    Ok(artifact_ids)
}

fn project_unavailable_items() -> Vec<StorageInventoryItem> {
    [
        ("storage.projectMedia", StorageScope::ProjectMedia, false),
        (
            "storage.projectTranscripts",
            StorageScope::ProjectTranscripts,
            false,
        ),
        (
            "storage.projectRenderArtifacts",
            StorageScope::ProjectRenderArtifacts,
            true,
        ),
        (
            "storage.projectWorkflowArtifacts",
            StorageScope::ProjectWorkflowArtifacts,
            false,
        ),
    ]
    .into_iter()
    .map(|(id, scope, removable)| StorageInventoryItem {
        id: id.to_string(),
        scope,
        path: None,
        bytes: 0,
        free_bytes: None,
        removable,
        unavailable_reason: Some(OPEN_PROJECT_REASON.to_string()),
        artifact_ids: Vec::new(),
    })
    .collect()
}

fn project_error_items(project_dir: &Path, reason: &str) -> Vec<StorageInventoryItem> {
    project_unavailable_items()
        .into_iter()
        .map(|mut item| {
            item.path = Some(project_dir.display().to_string());
            item.unavailable_reason = Some(reason.to_string());
            item
        })
        .collect()
}

fn bounded_directory_bytes(
    root: &Path,
    allowed_root: Option<&Path>,
    limits: WalkLimits,
) -> Result<u64, String> {
    let root_metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("Could not inspect {}: {error}", root.display())),
    };
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("Could not resolve {}: {error}", root.display()))?;
    let allowed_root = allowed_root.unwrap_or(&canonical_root);
    if !canonical_root.starts_with(allowed_root) {
        return Err(format!(
            "Storage scope {} escapes the active project root {}.",
            root.display(),
            allowed_root.display()
        ));
    }
    if root_metadata.is_file() {
        return Ok(root_metadata.len());
    }

    let mut total = 0_u64;
    let mut visited_directories = HashSet::<PathBuf>::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    if limits.max_entries == 0 {
        return Err("Storage inventory exceeded its entry limit (0).".to_string());
    }
    let mut discovered_entries = 1_usize;
    while let Some((path, depth)) = pending.pop() {
        let link_metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
        let (metadata, canonical_path) = if link_metadata.file_type().is_symlink() {
            let canonical = match fs::canonicalize(&path) {
                Ok(canonical) if canonical.starts_with(allowed_root) => canonical,
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("Could not resolve {}: {error}", path.display()));
                }
            };
            let metadata = fs::metadata(&path)
                .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
            (metadata, canonical)
        } else {
            let canonical = fs::canonicalize(&path)
                .map_err(|error| format!("Could not resolve {}: {error}", path.display()))?;
            if !canonical.starts_with(allowed_root) {
                continue;
            }
            (link_metadata, canonical)
        };

        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
            continue;
        }
        if !metadata.is_dir() || !visited_directories.insert(canonical_path) {
            continue;
        }
        let entries = fs::read_dir(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            let next_depth = depth.saturating_add(1);
            if next_depth > limits.max_depth {
                return Err(format!(
                    "Storage inventory exceeded its depth limit ({}).",
                    limits.max_depth
                ));
            }
            discovered_entries = discovered_entries.saturating_add(1);
            if discovered_entries > limits.max_entries {
                return Err(format!(
                    "Storage inventory exceeded its entry limit ({}).",
                    limits.max_entries
                ));
            }
            pending.push((entry.path(), next_depth));
        }
    }
    Ok(total)
}

#[cfg(unix)]
fn volume_free_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let mut probe = path;
    while !probe.exists() {
        probe = probe.parent()?;
    }
    let path = CString::new(probe.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    let available = (stats.f_bavail as u128).saturating_mul(stats.f_bsize as u128);
    Some(available.min(u128::from(u64::MAX)) as u64)
}

#[cfg(not(unix))]
fn volume_free_bytes(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        collect_storage_inventory, collect_storage_inventory_with_limits, preview_storage_cleanup,
        preview_storage_cleanup_for_generation, run_storage_cleanup, run_storage_cleanup_with_hook,
        StorageCleanupTarget, StorageScope, WalkLimits,
    };

    fn item(
        inventory: &[super::StorageInventoryItem],
        scope: StorageScope,
    ) -> &super::StorageInventoryItem {
        inventory
            .iter()
            .find(|item| item.scope == scope)
            .expect("inventory scope")
    }

    #[test]
    fn inventory_reports_six_scoped_rows_with_usage_and_free_space() {
        let root = tempfile::tempdir().expect("inventory root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        for directory in [
            &models,
            &cache,
            &project.join("media"),
            &project.join("transcripts"),
            &project.join("renders"),
            &project.join("jobs"),
            &project.join("logs"),
        ] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        fs::write(models.join("model.bin"), vec![0; 3]).expect("model fixture");
        fs::write(cache.join("preview.bin"), vec![0; 5]).expect("cache fixture");
        fs::write(project.join("media/source.mp4"), vec![0; 7]).expect("media fixture");
        fs::write(project.join("transcripts/source.json"), vec![0; 11])
            .expect("transcript fixture");
        fs::write(project.join("renders/draft.mov"), vec![0; 13]).expect("render fixture");
        fs::write(project.join("jobs/index.json"), vec![0; 17]).expect("job fixture");
        fs::write(project.join("logs/render.log"), vec![0; 19]).expect("log fixture");

        let inventory = collect_storage_inventory(&models, &cache, Some(&project));

        assert_eq!(inventory.len(), 6);
        assert_eq!(item(&inventory, StorageScope::GlobalModels).bytes, 3);
        assert_eq!(item(&inventory, StorageScope::DisposableAppCache).bytes, 5);
        assert_eq!(item(&inventory, StorageScope::ProjectMedia).bytes, 7);
        assert_eq!(item(&inventory, StorageScope::ProjectTranscripts).bytes, 11);
        assert_eq!(
            item(&inventory, StorageScope::ProjectRenderArtifacts).bytes,
            13
        );
        assert_eq!(
            item(&inventory, StorageScope::ProjectWorkflowArtifacts).bytes,
            36
        );
        assert!(inventory.iter().all(|item| item.free_bytes.is_some()));
        assert!(item(&inventory, StorageScope::DisposableAppCache).removable);
        assert!(item(&inventory, StorageScope::ProjectRenderArtifacts).removable);
        assert!(!item(&inventory, StorageScope::GlobalModels).removable);
        assert!(inventory
            .iter()
            .all(|item| item.unavailable_reason.is_none()));
    }

    #[test]
    fn inventory_lists_only_safe_render_artifact_directories() {
        let root = tempfile::tempdir().expect("inventory root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let renders = project.join("renders");
        for directory in [
            &models,
            &cache,
            &renders.join("draft-001"),
            &renders.join("final-002"),
            &renders.join("index.json"),
            &renders.join(".video-creater-cleanup-private"),
        ] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        fs::write(renders.join("loose-frame.png"), b"frame").expect("loose render file");

        let inventory = collect_storage_inventory(&models, &cache, Some(&project));
        assert_eq!(
            item(&inventory, StorageScope::ProjectRenderArtifacts).artifact_ids,
            vec!["draft-001".to_string(), "final-002".to_string()],
        );
    }

    #[cfg(unix)]
    #[test]
    fn inventory_does_not_follow_symlinks_outside_the_scoped_root_or_cycles() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("inventory root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let media = project.join("media");
        let outside = root.path().join("outside");
        fs::create_dir_all(&models).expect("models");
        fs::create_dir_all(&cache).expect("cache");
        fs::create_dir_all(&media).expect("media");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(media.join("inside.bin"), vec![0; 7]).expect("inside fixture");
        fs::write(outside.join("secret.bin"), vec![0; 4_096]).expect("outside fixture");
        symlink(outside.join("secret.bin"), media.join("escape.bin")).expect("escape symlink");
        symlink(&media, media.join("cycle")).expect("cycle symlink");

        let inventory = collect_storage_inventory(&models, &cache, Some(&project));

        let media_item = item(&inventory, StorageScope::ProjectMedia);
        assert_eq!(media_item.bytes, 7);
        assert_eq!(media_item.unavailable_reason, None);
    }

    #[cfg(unix)]
    #[test]
    fn project_media_and_render_roots_cannot_symlink_outside_the_project() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("inventory root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let outside_media = root.path().join("outside-media");
        let outside_renders = root.path().join("outside-renders");
        for directory in [&models, &cache, &project, &outside_media, &outside_renders] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        fs::write(outside_media.join("secret.mov"), vec![0; 4_096]).expect("outside media");
        fs::write(outside_renders.join("secret.mov"), vec![0; 8_192]).expect("outside render");
        symlink(&outside_media, project.join("media")).expect("media root symlink");
        symlink(&outside_renders, project.join("renders")).expect("render root symlink");

        let inventory = collect_storage_inventory(&models, &cache, Some(&project));

        for scope in [
            StorageScope::ProjectMedia,
            StorageScope::ProjectRenderArtifacts,
        ] {
            let item = item(&inventory, scope);
            assert_eq!(item.bytes, 0);
            assert_eq!(item.free_bytes, None);
            assert!(item
                .unavailable_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("escapes the active project root")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn project_job_and_log_roots_cannot_symlink_outside_the_project() {
        use std::os::unix::fs::symlink;

        for escaped_root in ["jobs", "logs"] {
            let root = tempfile::tempdir().expect("inventory root");
            let models = root.path().join("models");
            let cache = root.path().join("cache");
            let project = root.path().join("project");
            let outside = root.path().join("outside");
            fs::create_dir_all(&models).expect("models");
            fs::create_dir_all(&cache).expect("cache");
            fs::create_dir_all(&project).expect("project");
            fs::create_dir_all(&outside).expect("outside");
            fs::create_dir_all(project.join(if escaped_root == "jobs" {
                "logs"
            } else {
                "jobs"
            }))
            .expect("contained workflow root");
            fs::write(outside.join("secret.log"), vec![0; 16_384]).expect("outside workflow");
            symlink(&outside, project.join(escaped_root)).expect("workflow root symlink");

            let inventory = collect_storage_inventory(&models, &cache, Some(&project));
            let workflow = item(&inventory, StorageScope::ProjectWorkflowArtifacts);

            assert_eq!(workflow.bytes, 0, "escaped {escaped_root} bytes");
            assert_eq!(workflow.free_bytes, None);
            assert!(workflow
                .unavailable_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("escapes the active project root")));
        }
    }

    #[test]
    fn wide_directory_hits_the_entry_budget_before_children_are_queued() {
        let root = tempfile::tempdir().expect("inventory root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        fs::create_dir_all(&models).expect("models");
        fs::create_dir_all(&cache).expect("cache");
        for index in 0..10 {
            fs::write(models.join(format!("model-{index}.bin")), [index as u8])
                .expect("model fixture");
        }

        let inventory = collect_storage_inventory_with_limits(
            &models,
            &cache,
            None,
            WalkLimits {
                max_depth: 64,
                max_entries: 3,
            },
        );

        let models = item(&inventory, StorageScope::GlobalModels);
        assert_eq!(models.bytes, 0);
        assert_eq!(models.free_bytes, None);
        assert_eq!(
            models.unavailable_reason.as_deref(),
            Some("Storage inventory exceeded its entry limit (3).")
        );
    }

    #[test]
    fn project_scopes_require_an_active_project() {
        let root = tempfile::tempdir().expect("inventory root");
        let inventory = collect_storage_inventory(
            &root.path().join("models"),
            &root.path().join("cache"),
            None,
        );

        for scope in [
            StorageScope::ProjectMedia,
            StorageScope::ProjectTranscripts,
            StorageScope::ProjectRenderArtifacts,
            StorageScope::ProjectWorkflowArtifacts,
        ] {
            let item = item(&inventory, scope);
            assert_eq!(item.path, None);
            assert_eq!(item.bytes, 0);
            assert_eq!(item.free_bytes, None);
            assert_eq!(
                item.unavailable_reason.as_deref(),
                Some("Open a project to inspect")
            );
        }
    }

    #[test]
    fn cleanup_preview_allowlists_exact_cache_children_and_render_artifact_ids() {
        let root = tempfile::tempdir().expect("cleanup root");
        let models = root.path().join("models");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let selected_render = project.join("renders/render-draft-1");
        for directory in [
            &cache.join("frames"),
            &models,
            &project.join("media"),
            &project.join("transcripts"),
            &project.join("generated"),
            &selected_render,
            &project.join("renders/accepted-output"),
        ] {
            fs::create_dir_all(directory).expect("cleanup fixture directory");
        }
        fs::write(cache.join("frames/frame.png"), vec![0; 3]).expect("cache fixture");
        fs::write(cache.join("loose.tmp"), vec![0; 5]).expect("cache file fixture");
        fs::write(models.join("model.bin"), vec![0; 6]).expect("model fixture");
        fs::write(project.join("project.json"), vec![0; 7]).expect("manifest fixture");
        fs::write(project.join("timeline.json"), vec![0; 11]).expect("timeline fixture");
        fs::write(project.join("media/source.mov"), vec![0; 13]).expect("media fixture");
        fs::write(project.join("transcripts/source.json"), vec![0; 17])
            .expect("transcript fixture");
        fs::write(project.join("generated/clip.mp4"), vec![0; 19]).expect("generated fixture");
        fs::write(project.join("renders/index.json"), vec![0; 23]).expect("render index fixture");
        fs::write(selected_render.join("output.mp4"), vec![0; 29])
            .expect("selected render fixture");
        fs::write(
            project.join("renders/accepted-output/output.mp4"),
            vec![0; 31],
        )
        .expect("accepted render fixture");

        let cache_preview = preview_storage_cleanup(
            &StorageCleanupTarget::DisposableAppCache,
            &cache,
            Some(&project),
        )
        .expect("cache preview");
        assert_eq!(cache_preview.total_bytes, 8);
        assert_eq!(cache_preview.items.len(), 2);
        assert_eq!(
            cache_preview
                .items
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                cache
                    .join("frames")
                    .canonicalize()
                    .expect("canonical cache directory")
                    .to_str()
                    .expect("UTF-8 cache directory"),
                cache
                    .join("loose.tmp")
                    .canonicalize()
                    .expect("canonical cache file")
                    .to_str()
                    .expect("UTF-8 cache file"),
            ]
        );

        let render_preview = preview_storage_cleanup(
            &StorageCleanupTarget::ProjectRenderArtifacts {
                artifact_ids: vec!["render-draft-1".to_string()],
            },
            &cache,
            Some(&project),
        )
        .expect("render preview");
        assert_eq!(render_preview.total_bytes, 29);
        assert_eq!(render_preview.items.len(), 1);
        assert_eq!(
            render_preview.items[0].path,
            selected_render
                .canonicalize()
                .expect("canonical selected render")
                .display()
                .to_string()
        );
        for protected in [
            models.join("model.bin"),
            project.join("project.json"),
            project.join("timeline.json"),
            project.join("media/source.mov"),
            project.join("transcripts/source.json"),
            project.join("generated/clip.mp4"),
            project.join("renders/index.json"),
            project.join("renders/accepted-output"),
        ] {
            assert!(!render_preview
                .items
                .iter()
                .any(|item| item.path == protected.display().to_string()));
        }
        assert_ne!(
            cache_preview.confirmation_token,
            render_preview.confirmation_token
        );
    }

    #[test]
    fn render_cleanup_refuses_non_artifact_ids_and_non_directory_outputs() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(project.join("renders")).expect("renders root");
        fs::write(project.join("renders/index.json"), b"{}").expect("render index");
        fs::write(project.join("renders/accepted.mp4"), b"accepted").expect("accepted output");

        for artifact_id in [
            "",
            ".",
            "..",
            "../media",
            "nested/render",
            "nested\\render",
            "index.json",
            "accepted.mp4",
        ] {
            let error = preview_storage_cleanup(
                &StorageCleanupTarget::ProjectRenderArtifacts {
                    artifact_ids: vec![artifact_id.to_string()],
                },
                &cache,
                Some(&project),
            )
            .expect_err("unsafe or non-directory artifact target must fail");
            assert!(
                matches!(
                    error.code(),
                    "settings.storage.cleanupInvalidArtifactId"
                        | "settings.storage.cleanupTargetNotArtifactDirectory"
                ),
                "unexpected cleanup error for {artifact_id:?}: {error}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_preview_refuses_cache_and_render_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let outside = root.path().join("outside");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(project.join("renders/render-draft-1")).expect("render root");
        fs::create_dir_all(&outside).expect("outside root");
        fs::write(outside.join("keep.bin"), b"keep").expect("outside fixture");
        symlink(outside.join("keep.bin"), cache.join("escape.bin")).expect("cache escape");

        let cache_error = preview_storage_cleanup(
            &StorageCleanupTarget::DisposableAppCache,
            &cache,
            Some(&project),
        )
        .expect_err("cache symlink escape must fail");
        assert_eq!(cache_error.code(), "settings.storage.cleanupScopeEscaped");

        fs::remove_file(cache.join("escape.bin")).expect("remove cache escape fixture");
        symlink(
            outside.join("keep.bin"),
            project.join("renders/render-draft-1/escape.bin"),
        )
        .expect("render escape");
        let render_error = preview_storage_cleanup(
            &StorageCleanupTarget::ProjectRenderArtifacts {
                artifact_ids: vec!["render-draft-1".to_string()],
            },
            &cache,
            Some(&project),
        )
        .expect_err("render symlink escape must fail");
        assert_eq!(render_error.code(), "settings.storage.cleanupScopeEscaped");
        assert!(outside.join("keep.bin").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_preview_refuses_duplicate_resolved_hardlink_identities() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        fs::create_dir_all(&cache).expect("cache root");
        fs::write(cache.join("first.tmp"), b"same inode").expect("first cache file");
        fs::hard_link(cache.join("first.tmp"), cache.join("alias.tmp")).expect("hardlink alias");

        let error =
            preview_storage_cleanup(&StorageCleanupTarget::DisposableAppCache, &cache, None)
                .expect_err("duplicate resolved identity must fail");

        assert_eq!(
            error.code(),
            "settings.storage.cleanupDuplicateResolvedIdentity"
        );
        assert!(cache.join("first.tmp").is_file());
        assert!(cache.join("alias.tmp").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_preview_refuses_direct_symlink_to_an_in_scope_sibling() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        fs::create_dir_all(&cache).expect("cache root");
        fs::write(cache.join("real.tmp"), b"keep").expect("real cache file");
        symlink(cache.join("real.tmp"), cache.join("alias.tmp")).expect("cache alias");

        let error =
            preview_storage_cleanup(&StorageCleanupTarget::DisposableAppCache, &cache, None)
                .expect_err("direct cache symlink must fail");

        assert_eq!(error.code(), "settings.storage.cleanupScopeEscaped");
        assert!(cache.join("real.tmp").is_file());
        assert!(cache.join("alias.tmp").symlink_metadata().is_ok());
    }

    #[test]
    fn cleanup_run_requires_current_matching_confirmation_and_preserves_unnamed_paths() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let selected = project.join("renders/render-draft-1");
        let accepted = project.join("renders/accepted-output");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&selected).expect("selected render");
        fs::create_dir_all(&accepted).expect("accepted render");
        fs::write(selected.join("output.mp4"), b"draft").expect("selected output");
        fs::write(accepted.join("output.mp4"), b"accepted").expect("accepted output");
        fs::write(project.join("renders/index.json"), b"index").expect("render index");
        fs::write(project.join("project.json"), b"manifest").expect("manifest");
        let target = StorageCleanupTarget::ProjectRenderArtifacts {
            artifact_ids: vec!["render-draft-1".to_string()],
        };
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");

        let mismatch = run_storage_cleanup(
            &target,
            "not-the-preview-token",
            &cache,
            Some(&project),
            |_| Ok(()),
        )
        .expect_err("mismatched confirmation must fail");
        assert_eq!(
            mismatch.code(),
            "settings.storage.cleanupConfirmationMismatch"
        );
        assert!(selected.is_dir());

        fs::write(selected.join("added-after-preview.bin"), b"new").expect("stale fixture");
        let stale = run_storage_cleanup(
            &target,
            &preview.confirmation_token,
            &cache,
            Some(&project),
            |_| Ok(()),
        )
        .expect_err("stale confirmation must fail");
        assert_eq!(stale.code(), "settings.storage.cleanupConfirmationMismatch");
        assert!(selected.is_dir());

        let fresh =
            preview_storage_cleanup(&target, &cache, Some(&project)).expect("fresh preview");
        let mut progress = Vec::new();
        let report = run_storage_cleanup(
            &target,
            &fresh.confirmation_token,
            &cache,
            Some(&project),
            |item| {
                progress.push(item.clone());
                Ok(())
            },
        )
        .expect("run cleanup");
        assert_eq!(report.removed_paths, vec![fresh.items[0].path.clone()]);
        assert_eq!(report.removed_bytes, fresh.total_bytes);
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].completed_items, 1);
        assert_eq!(progress[0].total_items, 1);
        assert!(!selected.exists());
        assert!(accepted.join("output.mp4").is_file());
        assert!(project.join("renders/index.json").is_file());
        assert!(project.join("project.json").is_file());
    }

    #[test]
    fn render_cleanup_confirmation_token_is_bound_to_project_generation() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let selected = project.join("renders/render-draft-1");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&selected).expect("selected render");
        fs::write(selected.join("output.mp4"), b"draft").expect("selected output");
        let target = StorageCleanupTarget::ProjectRenderArtifacts {
            artifact_ids: vec!["render-draft-1".to_string()],
        };

        let generation_one =
            preview_storage_cleanup_for_generation(&target, &cache, Some(&project), Some(1))
                .expect("generation one preview");
        let generation_two =
            preview_storage_cleanup_for_generation(&target, &cache, Some(&project), Some(2))
                .expect("generation two preview");

        assert_eq!(generation_one.project_generation, Some(1));
        assert_eq!(generation_two.project_generation, Some(2));
        assert_ne!(
            generation_one.confirmation_token,
            generation_two.confirmation_token
        );
    }

    #[test]
    fn identical_cleanup_previews_receive_distinct_issuance_tokens() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        fs::create_dir_all(&cache).expect("empty cache root");
        let target = StorageCleanupTarget::DisposableAppCache;

        let first = preview_storage_cleanup(&target, &cache, None).expect("first preview");
        let second = preview_storage_cleanup(&target, &cache, None).expect("second preview");

        assert!(first.items.is_empty());
        assert_eq!(first.items, second.items);
        assert_eq!(first.total_bytes, second.total_bytes);
        assert_ne!(first.preview_nonce, second.preview_nonce);
        assert_ne!(first.confirmation_token, second.confirmation_token);
    }

    #[test]
    fn disposable_app_cache_has_no_registered_app_owned_writers() {
        assert!(super::DISPOSABLE_APP_CACHE_WRITER_IDS.is_empty());
    }

    #[test]
    fn render_writer_lease_blocks_cleanup_quarantine_until_release() {
        use std::sync::mpsc;

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let selected = project.join("renders/render-1");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&selected).expect("selected render");
        fs::write(selected.join("output.mp4"), b"writer output").expect("render output");
        let target = StorageCleanupTarget::ProjectRenderArtifacts {
            artifact_ids: vec!["render-1".to_string()],
        };
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");
        let writer_lease = super::acquire_storage_mutation_lease().expect("writer lease");
        let (quarantine_tx, quarantine_rx) = mpsc::sync_channel(1);
        let cleanup_cache = cache.clone();
        let cleanup_project = project.clone();
        let cleanup_target = target.clone();
        let cleanup_token = preview.confirmation_token.clone();
        let cleanup = std::thread::spawn(move || {
            let _cleanup_lease = super::acquire_storage_mutation_lease().expect("cleanup lease");
            run_storage_cleanup_with_hook(
                &cleanup_target,
                &cleanup_token,
                &cleanup_cache,
                Some(&cleanup_project),
                |_| {
                    quarantine_tx.send(()).expect("quarantine boundary");
                    Ok(())
                },
                |_| Ok(()),
            )
            .expect("cleanup result")
        });

        assert!(matches!(
            quarantine_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(selected.join("output.mp4").is_file());
        drop(writer_lease);
        quarantine_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("cleanup proceeds after writer release");
        let report = cleanup.join().expect("cleanup thread");
        assert_eq!(report.removed_count, 1);
        assert!(!selected.exists());
    }

    #[test]
    fn cleanup_lease_blocks_render_writer_until_deletion_accounting_finishes() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        };

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let selected = project.join("renders/render-1");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&selected).expect("selected render");
        fs::write(selected.join("output.mp4"), b"old output").expect("render output");
        let target = StorageCleanupTarget::ProjectRenderArtifacts {
            artifact_ids: vec!["render-1".to_string()],
        };
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");
        let (cleanup_ready_tx, cleanup_ready_rx) = mpsc::sync_channel(1);
        let (continue_tx, continue_rx) = mpsc::sync_channel(1);
        let accounting_finished = Arc::new(AtomicBool::new(false));
        let cleanup_accounting_finished = Arc::clone(&accounting_finished);
        let cleanup_cache = cache.clone();
        let cleanup_project = project.clone();
        let cleanup_target = target.clone();
        let cleanup_token = preview.confirmation_token.clone();
        let cleanup = std::thread::spawn(move || {
            let _cleanup_lease = super::acquire_storage_mutation_lease().expect("cleanup lease");
            let report = run_storage_cleanup_with_hook(
                &cleanup_target,
                &cleanup_token,
                &cleanup_cache,
                Some(&cleanup_project),
                |_| {
                    cleanup_ready_tx.send(()).expect("cleanup ready");
                    continue_rx.recv().expect("continue cleanup");
                    Ok(())
                },
                |_| Ok(()),
            )
            .expect("cleanup result");
            cleanup_accounting_finished.store(true, Ordering::Release);
            report
        });
        cleanup_ready_rx.recv().expect("cleanup holds lease");

        let (writer_finished_tx, writer_finished_rx) = mpsc::sync_channel(1);
        let writer_accounting_finished = Arc::clone(&accounting_finished);
        let writer_selected = selected.clone();
        let writer = std::thread::spawn(move || {
            let _writer_lease = super::acquire_storage_mutation_lease().expect("writer lease");
            assert!(writer_accounting_finished.load(Ordering::Acquire));
            assert!(!writer_selected.exists());
            fs::create_dir_all(&writer_selected).expect("new render directory");
            fs::write(writer_selected.join("output.mp4"), b"new output")
                .expect("new render output");
            writer_finished_tx.send(()).expect("writer finished");
        });

        assert!(matches!(
            writer_finished_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(selected.join("output.mp4").is_file());
        continue_tx.send(()).expect("continue cleanup");
        let report = cleanup.join().expect("cleanup thread");
        writer_finished_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("writer proceeds after cleanup accounting");
        writer.join().expect("writer thread");

        assert_eq!(report.removed_count, 1);
        assert_eq!(
            fs::read(selected.join("output.mp4")).expect("new output"),
            b"new output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_revalidates_each_item_immediately_before_deletion() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        let outside = root.path().join("outside");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&project).expect("project root");
        fs::create_dir_all(&outside).expect("outside root");
        fs::write(cache.join("a-first.tmp"), b"first").expect("first cache item");
        fs::write(cache.join("b-second.tmp"), b"second").expect("second cache item");
        fs::write(outside.join("keep.bin"), b"outside").expect("outside fixture");
        let target = StorageCleanupTarget::DisposableAppCache;
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");
        let second = cache.join("b-second.tmp");

        let error = run_storage_cleanup(
            &target,
            &preview.confirmation_token,
            &cache,
            Some(&project),
            |progress| {
                if progress.completed_items == 1 {
                    fs::remove_file(&second).expect("replace second cache item");
                    symlink(outside.join("keep.bin"), &second).expect("swap second to escape");
                }
                Ok(())
            },
        )
        .expect_err("late symlink swap must stop cleanup");

        assert_eq!(error.code(), "settings.storage.cleanupScopeEscaped");
        assert!(!cache.join("a-first.tmp").exists());
        assert!(second.symlink_metadata().is_ok());
        assert!(outside.join("keep.bin").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_quarantine_identity_check_prevents_rename_swap_deletion() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        fs::create_dir_all(cache.join("a-selected")).expect("selected cache directory");
        fs::create_dir_all(cache.join("b-unselected")).expect("unselected cache directory");
        fs::write(cache.join("a-selected/selected.marker"), b"selected").expect("selected marker");
        fs::write(cache.join("b-unselected/unselected.marker"), b"unselected")
            .expect("unselected marker");
        let target = StorageCleanupTarget::ProjectRenderArtifacts {
            artifact_ids: vec!["a-selected".to_string()],
        };
        let project = root.path().join("project");
        fs::create_dir_all(project.join("renders")).expect("project renders");
        fs::rename(cache.join("a-selected"), project.join("renders/a-selected"))
            .expect("move selected fixture into renders");
        fs::rename(
            cache.join("b-unselected"),
            project.join("renders/b-unselected"),
        )
        .expect("move unselected fixture into renders");
        let selected = project.join("renders/a-selected");
        let unselected = project.join("renders/b-unselected");
        let swap = project.join("renders/swap");
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");

        let error = run_storage_cleanup_with_hook(
            &target,
            &preview.confirmation_token,
            &cache,
            Some(&project),
            |_| {
                fs::rename(&selected, &swap).expect("move selected aside");
                fs::rename(&unselected, &selected).expect("swap unselected into selected path");
                fs::rename(&swap, &unselected).expect("swap selected into unselected path");
                Ok(())
            },
            |_| Ok(()),
        )
        .expect_err("quarantine identity mismatch must stop deletion");

        assert_eq!(error.code(), "settings.storage.cleanupConfirmationMismatch");
        assert_eq!(error.partial_report.removed_count, 0);
        assert!(selected.join("unselected.marker").is_file());
        assert!(unselected.join("selected.marker").is_file());
        assert!(error.quarantined_paths.is_empty(), "item should restore");
        assert!(!project
            .join("renders/.video-creater-cleanup-quarantine")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rechecks_quarantined_fingerprint_immediately_before_deletion() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        fs::create_dir_all(cache.join("selected")).expect("selected cache directory");
        fs::write(cache.join("selected/marker"), b"selected").expect("selected marker");
        let target = StorageCleanupTarget::DisposableAppCache;
        let plan = super::resolve_cleanup_plan(&target, &cache, None).expect("cleanup plan");
        let expected = plan.items[0].clone();
        let quarantine_root =
            super::create_cleanup_quarantine(&plan.scope_root).expect("private quarantine");
        assert!(quarantine_root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(super::CLEANUP_QUARANTINE_PREFIX)));
        assert_eq!(
            fs::metadata(&quarantine_root)
                .expect("quarantine metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let quarantined =
            super::quarantine_cleanup_item(&plan.scope_root, &quarantine_root, &expected)
                .expect("quarantine selected item");
        fs::write(quarantined.join("marker"), b"changed after verification")
            .expect("change quarantined contents");

        let error = super::remove_verified_quarantined_cleanup_item(
            &plan.scope_root,
            &quarantine_root,
            &expected,
            &quarantined,
        )
        .expect_err("changed quarantined fingerprint must not be deleted");

        assert_eq!(
            error.error.code(),
            "settings.storage.cleanupConfirmationMismatch"
        );
        assert!(cache.join("selected/marker").is_file());
        assert!(!quarantine_root.exists());
    }

    #[test]
    fn cleanup_stops_before_the_next_item_when_progress_reporting_fails() {
        let root = tempfile::tempdir().expect("cleanup root");
        let cache = root.path().join("cache");
        let project = root.path().join("project");
        fs::create_dir_all(&cache).expect("cache root");
        fs::create_dir_all(&project).expect("project root");
        fs::write(cache.join("a-first.tmp"), b"first").expect("first cache item");
        fs::write(cache.join("b-second.tmp"), b"second").expect("second cache item");
        let target = StorageCleanupTarget::DisposableAppCache;
        let preview = preview_storage_cleanup(&target, &cache, Some(&project)).expect("preview");

        let error = run_storage_cleanup(
            &target,
            &preview.confirmation_token,
            &cache,
            Some(&project),
            |_| {
                Err(super::StorageCleanupError::Progress(
                    "journal unavailable".to_string(),
                ))
            },
        )
        .expect_err("progress persistence failure must stop cleanup");

        assert_eq!(error.code(), "settings.storage.cleanupProgressFailed");
        assert_eq!(error.partial_report.removed_count, 1);
        assert_eq!(error.partial_report.removed_paths.len(), 1);
        assert_eq!(error.partial_report.removed_bytes, 5);
        assert!(error.persistence_boundary_unknown);
        assert!(!cache.join("a-first.tmp").exists());
        assert!(cache.join("b-second.tmp").is_file());
    }
}
