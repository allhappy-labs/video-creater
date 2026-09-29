use chrono::{Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

pub const SETTINGS_OPERATION_EVENT: &str = "settings-operation";
pub const MAX_RETAINED_TERMINAL_OPERATIONS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsOperationKind {
    ModelDownload,
    SpeechModelsDownload,
    HealthCheck,
    SkillRepair,
    StorageRefresh,
    StorageCleanup,
    ProviderRefresh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsOperationState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOperationError {
    pub code: String,
    pub message: String,
    pub recovery_action: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOperation {
    pub id: String,
    pub kind: SettingsOperationKind,
    pub target_id: String,
    pub phase: String,
    pub state: SettingsOperationState,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub unit: Option<String>,
    pub cancellable: bool,
    pub message: String,
    pub error: Option<SettingsOperationError>,
    pub started_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsOperationTransition {
    pub state: SettingsOperationState,
    pub phase: Option<String>,
    pub completed_units: Option<u64>,
    pub total_units: Option<u64>,
    pub unit: Option<String>,
    pub cancellable: Option<bool>,
    pub message: Option<String>,
    pub error: Option<SettingsOperationError>,
}

impl SettingsOperationTransition {
    pub fn to(state: SettingsOperationState) -> Self {
        Self {
            state,
            phase: None,
            completed_units: None,
            total_units: None,
            unit: None,
            cancellable: None,
            message: None,
            error: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsOperationRegistryError {
    #[error("settings operation registry lock failed")]
    Lock,
    #[error("settings operation {0} was not found")]
    NotFound(String),
    #[error("settings operation cannot transition from {from} to {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
    #[error("settings operation {0} is not cancellable")]
    NotCancellable(String),
    #[error("failed settings operation requires an error")]
    MissingFailureError,
    #[error("failed settings operation error {0} must not be blank")]
    BlankFailureErrorField(&'static str),
    #[error("settings operation progress exceeds its total units")]
    ProgressExceedsTotal,
    #[error("settings operation journal could not be read: {0}")]
    ReadJournal(String),
    #[error("settings operation journal is invalid: {0}")]
    InvalidJournal(String),
    #[error("settings operation journal could not be written: {0}")]
    WriteJournal(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOperationsCommandError {
    pub code: String,
    pub message: String,
    pub detail: String,
}

impl From<SettingsOperationRegistryError> for SettingsOperationsCommandError {
    fn from(error: SettingsOperationRegistryError) -> Self {
        Self {
            code: "settings.operation.registryUnavailable".to_string(),
            message: "Settings operation history is temporarily unavailable.".to_string(),
            detail: error.to_string(),
        }
    }
}

pub struct SettingsOperationRegistry {
    journal_path: PathBuf,
    operations: Mutex<Vec<SettingsOperation>>,
}

impl SettingsOperationRegistry {
    pub fn load<F>(
        journal_path: PathBuf,
        target_is_complete: F,
    ) -> Result<Self, SettingsOperationRegistryError>
    where
        F: Fn(&SettingsOperation) -> bool,
    {
        let operations = read_journal(&journal_path)?;
        let mut unchanged_operations = Vec::with_capacity(operations.len());
        let mut reconciled_operations = Vec::new();
        let mut changed = false;
        for operation in operations {
            if is_incomplete_state(&operation.state) {
                reconciled_operations.push(reconcile_interrupted(
                    operation.clone(),
                    target_is_complete(&operation),
                ));
                changed = true;
            } else {
                unchanged_operations.push(operation);
            }
        }
        unchanged_operations.extend(reconciled_operations);
        let mut operations = unchanged_operations;
        let operation_count = operations.len();
        retain_bounded_history(&mut operations);
        changed |= operations.len() != operation_count;
        if changed {
            write_journal_atomically(&journal_path, &operations)?;
        }

        Ok(Self {
            journal_path,
            operations: Mutex::new(operations),
        })
    }

    pub fn load_resilient<F>(journal_path: PathBuf, target_is_complete: F) -> Self
    where
        F: Fn(&SettingsOperation) -> bool,
    {
        match Self::load(journal_path.clone(), target_is_complete) {
            Ok(registry) => registry,
            Err(error) => {
                let mut detail = error.to_string();
                match quarantine_journal(&journal_path) {
                    Ok(Some(path)) => {
                        detail.push_str(&format!("; quarantined at {}", path.display()));
                    }
                    Ok(None) => {
                        detail.push_str("; no journal file was available to quarantine");
                    }
                    Err(quarantine_error) => {
                        detail
                            .push_str(&format!("; journal quarantine failed: {quarantine_error}"));
                    }
                }
                let mut operations = vec![journal_recovery_operation(detail)];
                if let Err(write_error) = write_journal_atomically(&journal_path, &operations) {
                    if let Some(detail) = operations[0]
                        .error
                        .as_mut()
                        .and_then(|error| error.detail.as_mut())
                    {
                        detail.push_str(&format!(
                            "; replacement journal write failed: {write_error}"
                        ));
                    }
                }
                Self {
                    journal_path,
                    operations: Mutex::new(operations),
                }
            }
        }
    }

    pub fn start(
        &self,
        kind: SettingsOperationKind,
        target_id: impl Into<String>,
        total_units: Option<u64>,
        unit: Option<String>,
    ) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let now = current_timestamp();
        let operation = SettingsOperation {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            target_id: target_id.into(),
            phase: "queued".to_string(),
            state: SettingsOperationState::Queued,
            completed_units: 0,
            total_units,
            unit,
            cancellable: true,
            message: "Queued.".to_string(),
            error: None,
            started_at: now.clone(),
            updated_at: now,
        };
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let mut next = operations.clone();
        next.push(operation.clone());
        retain_bounded_history(&mut next);
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok(operation)
    }

    pub fn start_or_get_active(
        &self,
        kind: SettingsOperationKind,
        target_id: impl Into<String>,
        total_units: Option<u64>,
        unit: Option<String>,
    ) -> Result<(SettingsOperation, bool), SettingsOperationRegistryError> {
        let target_id = target_id.into();
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        if let Some(operation) = operations.iter().find(|operation| {
            operation.kind == kind
                && operation.target_id == target_id
                && (is_incomplete_state(&operation.state)
                    || operation.error.as_ref().is_some_and(|error| {
                        error.code == "settings.operation.terminalPersistenceFailed"
                    }))
        }) {
            return Ok((operation.clone(), false));
        }

        let now = current_timestamp();
        let operation = SettingsOperation {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            target_id,
            phase: "queued".to_string(),
            state: SettingsOperationState::Queued,
            completed_units: 0,
            total_units,
            unit,
            cancellable: true,
            message: "Queued.".to_string(),
            error: None,
            started_at: now.clone(),
            updated_at: now,
        };
        let mut next = operations.clone();
        next.push(operation.clone());
        retain_bounded_history(&mut next);
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok((operation, true))
    }

    pub fn resolve_or_start_active(
        &self,
        kind: SettingsOperationKind,
        target_id: impl Into<String>,
        total_units: Option<u64>,
        unit: Option<String>,
    ) -> Result<(SettingsOperation, bool), SettingsOperationRegistryError> {
        let (operation, created) = self.start_or_get_active(kind, target_id, total_units, unit)?;
        if created {
            Ok((operation, true))
        } else {
            self.get(&operation.id).map(|operation| (operation, false))
        }
    }

    pub fn resolve_or_start_active_configured<F>(
        &self,
        kind: SettingsOperationKind,
        target_id: impl Into<String>,
        total_units: Option<u64>,
        unit: Option<String>,
        configure: F,
    ) -> Result<(SettingsOperation, bool), SettingsOperationRegistryError>
    where
        F: FnOnce(&mut SettingsOperation),
    {
        let target_id = target_id.into();
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        if let Some(operation) = operations.iter().find(|operation| {
            operation.kind == kind
                && operation.target_id == target_id
                && (is_incomplete_state(&operation.state)
                    || operation.error.as_ref().is_some_and(|error| {
                        error.code == "settings.operation.terminalPersistenceFailed"
                    }))
        }) {
            return Ok((operation.clone(), false));
        }

        let now = current_timestamp();
        let mut operation = SettingsOperation {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            target_id,
            phase: "queued".to_string(),
            state: SettingsOperationState::Queued,
            completed_units: 0,
            total_units,
            unit,
            cancellable: true,
            message: "Queued.".to_string(),
            error: None,
            started_at: now.clone(),
            updated_at: now,
        };
        configure(&mut operation);
        let mut next = operations.clone();
        next.push(operation.clone());
        retain_bounded_history(&mut next);
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok((operation, true))
    }

    pub fn update(
        &self,
        id: &str,
        transition: SettingsOperationTransition,
    ) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let mut next = operations.clone();
        let operation_index = next
            .iter()
            .position(|operation| operation.id == id)
            .ok_or_else(|| SettingsOperationRegistryError::NotFound(id.to_string()))?;
        let operation = &mut next[operation_index];
        validate_state_transition(&operation.state, &transition.state)?;

        let next_completed_units = transition
            .completed_units
            .unwrap_or(operation.completed_units);
        let next_total_units = transition.total_units.or(operation.total_units);
        if next_total_units.is_some_and(|total| next_completed_units > total) {
            return Err(SettingsOperationRegistryError::ProgressExceedsTotal);
        }
        if transition.state == SettingsOperationState::Failed && transition.error.is_none() {
            return Err(SettingsOperationRegistryError::MissingFailureError);
        }
        if let Some(error) = transition.error.as_ref() {
            if error.code.trim().is_empty() {
                return Err(SettingsOperationRegistryError::BlankFailureErrorField(
                    "code",
                ));
            }
            if error.message.trim().is_empty() {
                return Err(SettingsOperationRegistryError::BlankFailureErrorField(
                    "message",
                ));
            }
        }

        operation.state = transition.state;
        if let Some(phase) = transition.phase {
            operation.phase = phase;
        }
        operation.completed_units = next_completed_units;
        operation.total_units = next_total_units;
        if transition.unit.is_some() {
            operation.unit = transition.unit;
        }
        if let Some(cancellable) = transition.cancellable {
            operation.cancellable = cancellable;
        }
        if let Some(message) = transition.message {
            operation.message = message;
        }
        operation.error = if operation.state == SettingsOperationState::Failed {
            transition.error
        } else {
            None
        };
        if is_terminal_state(&operation.state) {
            operation.cancellable = false;
        }
        if operation.state == SettingsOperationState::Cancelling {
            operation.cancellable = false;
        }
        operation.updated_at = next_operation_timestamp(&operation.updated_at);
        let updated = next.remove(operation_index);
        next.push(updated.clone());
        retain_bounded_history(&mut next);
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok(updated)
    }

    pub fn request_cancel(
        &self,
        id: &str,
    ) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let mut next = operations.clone();
        let operation_index = next
            .iter()
            .position(|operation| operation.id == id)
            .ok_or_else(|| SettingsOperationRegistryError::NotFound(id.to_string()))?;
        let operation = &mut next[operation_index];
        if operation.state == SettingsOperationState::Cancelling {
            return Ok(operation.clone());
        }
        validate_state_transition(&operation.state, &SettingsOperationState::Cancelling)?;
        if !operation.cancellable {
            return Err(SettingsOperationRegistryError::NotCancellable(
                id.to_string(),
            ));
        }

        operation.state = SettingsOperationState::Cancelling;
        operation.phase = "cancelling".to_string();
        operation.cancellable = false;
        operation.message = "Cancellation requested.".to_string();
        operation.error = None;
        operation.updated_at = next_operation_timestamp(&operation.updated_at);
        let updated = next.remove(operation_index);
        next.push(updated.clone());
        retain_bounded_history(&mut next);
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok(updated)
    }

    pub fn get(&self, id: &str) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        operations
            .iter()
            .find(|operation| operation.id == id)
            .cloned()
            .ok_or_else(|| SettingsOperationRegistryError::NotFound(id.to_string()))
    }

    pub fn list_active(&self) -> Result<Vec<SettingsOperation>, SettingsOperationRegistryError> {
        Ok(self
            .list_recent()?
            .into_iter()
            .filter(|operation| is_incomplete_state(&operation.state))
            .collect())
    }

    pub fn list_recent(&self) -> Result<Vec<SettingsOperation>, SettingsOperationRegistryError> {
        self.operations
            .lock()
            .map(|operations| operations.clone())
            .map_err(|_| SettingsOperationRegistryError::Lock)
    }

    pub fn invalidate_target(
        &self,
        kind: SettingsOperationKind,
        target_id: &str,
    ) -> Result<bool, SettingsOperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let mut next = operations.clone();
        next.retain(|operation| operation.kind != kind || operation.target_id != target_id);
        if next.len() == operations.len() {
            return Ok(false);
        }
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok(true)
    }

    pub fn invalidate_target_terminal_persistence_warning(
        &self,
        kind: SettingsOperationKind,
        target_id: &str,
    ) -> Result<bool, SettingsOperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let mut next = operations.clone();
        next.retain(|operation| {
            operation.kind != kind
                || operation.target_id != target_id
                || !operation.error.as_ref().is_some_and(|error| {
                    error.code == "settings.operation.terminalPersistenceFailed"
                })
        });
        if next.len() == operations.len() {
            return Ok(false);
        }
        write_journal_atomically(&self.journal_path, &next)?;
        *operations = next;
        Ok(true)
    }

    pub fn project_persistence_failure(
        &self,
        operation_id: &str,
        code: &str,
        message: &str,
        detail: &str,
    ) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let mut operation = self.get(operation_id)?;
        operation.phase = "failed_with_persistence_warning".to_string();
        operation.state = SettingsOperationState::Failed;
        operation.cancellable = false;
        operation.message = message.to_string();
        operation.error = Some(SettingsOperationError {
            code: code.to_string(),
            message: message.to_string(),
            recovery_action: Some("Retry the operation.".to_string()),
            detail: Some(detail.to_string()),
        });
        self.replace_in_memory_without_persisting(operation)
    }

    pub fn replace_in_memory_without_persisting(
        &self,
        mut operation: SettingsOperation,
    ) -> Result<SettingsOperation, SettingsOperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| SettingsOperationRegistryError::Lock)?;
        let operation_index = operations
            .iter_mut()
            .position(|existing| existing.id == operation.id)
            .ok_or_else(|| SettingsOperationRegistryError::NotFound(operation.id.clone()))?;
        operation.updated_at = next_operation_timestamp(&operations[operation_index].updated_at);
        operations.remove(operation_index);
        operations.push(operation.clone());
        retain_bounded_history(&mut operations);
        Ok(operation)
    }
}

pub fn reconcile_interrupted(
    mut operation: SettingsOperation,
    target_is_complete: bool,
) -> SettingsOperation {
    if !is_incomplete_state(&operation.state) {
        return operation;
    }

    operation.updated_at = next_operation_timestamp(&operation.updated_at);
    operation.cancellable = false;
    if target_is_complete {
        operation.phase = "completed".to_string();
        operation.state = SettingsOperationState::Succeeded;
        operation.message = "Completed before the app closed.".to_string();
        operation.error = None;
    } else {
        operation.phase = "interrupted".to_string();
        operation.state = SettingsOperationState::Failed;
        operation.message = "The operation was interrupted.".to_string();
        operation.error = Some(SettingsOperationError {
            code: "settings.operation.interrupted".to_string(),
            message: "The operation was interrupted when Video Creater closed.".to_string(),
            recovery_action: Some("Retry".to_string()),
            detail: None,
        });
    }
    operation
}

fn validate_state_transition(
    from: &SettingsOperationState,
    to: &SettingsOperationState,
) -> Result<(), SettingsOperationRegistryError> {
    let valid = match from {
        SettingsOperationState::Queued => matches!(
            to,
            SettingsOperationState::Queued
                | SettingsOperationState::Running
                | SettingsOperationState::Cancelling
                | SettingsOperationState::Succeeded
                | SettingsOperationState::Failed
                | SettingsOperationState::Cancelled
        ),
        SettingsOperationState::Running => matches!(
            to,
            SettingsOperationState::Running
                | SettingsOperationState::Cancelling
                | SettingsOperationState::Succeeded
                | SettingsOperationState::Failed
                | SettingsOperationState::Cancelled
        ),
        SettingsOperationState::Cancelling => matches!(
            to,
            SettingsOperationState::Cancelling
                | SettingsOperationState::Succeeded
                | SettingsOperationState::Failed
                | SettingsOperationState::Cancelled
        ),
        SettingsOperationState::Succeeded
        | SettingsOperationState::Failed
        | SettingsOperationState::Cancelled => false,
    };
    if valid {
        Ok(())
    } else {
        Err(SettingsOperationRegistryError::InvalidTransition {
            from: state_name(from),
            to: state_name(to),
        })
    }
}

fn state_name(state: &SettingsOperationState) -> &'static str {
    match state {
        SettingsOperationState::Queued => "queued",
        SettingsOperationState::Running => "running",
        SettingsOperationState::Cancelling => "cancelling",
        SettingsOperationState::Succeeded => "succeeded",
        SettingsOperationState::Failed => "failed",
        SettingsOperationState::Cancelled => "cancelled",
    }
}

fn is_incomplete_state(state: &SettingsOperationState) -> bool {
    matches!(
        state,
        SettingsOperationState::Queued
            | SettingsOperationState::Running
            | SettingsOperationState::Cancelling
    )
}

fn is_terminal_state(state: &SettingsOperationState) -> bool {
    matches!(
        state,
        SettingsOperationState::Succeeded
            | SettingsOperationState::Failed
            | SettingsOperationState::Cancelled
    )
}

fn retain_bounded_history(operations: &mut Vec<SettingsOperation>) {
    let terminal_count = operations
        .iter()
        .filter(|operation| is_terminal_state(&operation.state))
        .count();
    let mut terminals_to_remove = terminal_count.saturating_sub(MAX_RETAINED_TERMINAL_OPERATIONS);
    operations.retain(|operation| {
        if terminals_to_remove > 0 && is_terminal_state(&operation.state) {
            terminals_to_remove -= 1;
            false
        } else {
            true
        }
    });
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn next_operation_timestamp(previous: &str) -> String {
    let now = Utc::now();
    let next = chrono::DateTime::parse_from_rfc3339(previous)
        .ok()
        .map(|previous| previous.with_timezone(&Utc) + Duration::milliseconds(1))
        .filter(|minimum| minimum > &now)
        .unwrap_or(now);
    next.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn quarantine_journal(journal_path: &Path) -> Result<Option<PathBuf>, std::io::Error> {
    if !journal_path.exists() {
        return Ok(None);
    }
    let parent = journal_path.parent().unwrap_or_else(|| Path::new("."));
    let quarantine_path = parent.join(format!(
        "operations.corrupt.{}.{}.json",
        Utc::now().format("%Y%m%dT%H%M%S%3fZ"),
        uuid::Uuid::new_v4()
    ));
    fs::rename(journal_path, &quarantine_path)?;
    Ok(Some(quarantine_path))
}

fn journal_recovery_operation(detail: String) -> SettingsOperation {
    let now = current_timestamp();
    SettingsOperation {
        id: uuid::Uuid::new_v4().to_string(),
        kind: SettingsOperationKind::HealthCheck,
        target_id: "settings-operation-journal".to_string(),
        phase: "recovered".to_string(),
        state: SettingsOperationState::Failed,
        completed_units: 0,
        total_units: None,
        unit: None,
        cancellable: false,
        message: "Settings operation history was recovered.".to_string(),
        error: Some(SettingsOperationError {
            code: "settings.operation.journalRecovered".to_string(),
            message: "Settings operation history could not be recovered.".to_string(),
            recovery_action: None,
            detail: Some(detail),
        }),
        started_at: now.clone(),
        updated_at: now,
    }
}

fn read_journal(
    journal_path: &Path,
) -> Result<Vec<SettingsOperation>, SettingsOperationRegistryError> {
    if !journal_path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(journal_path)
        .map_err(|error| SettingsOperationRegistryError::ReadJournal(error.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| SettingsOperationRegistryError::InvalidJournal(error.to_string()))
}

fn write_journal_atomically(
    journal_path: &Path,
    operations: &[SettingsOperation],
) -> Result<(), SettingsOperationRegistryError> {
    let parent = journal_path.parent().ok_or_else(|| {
        SettingsOperationRegistryError::WriteJournal(
            "journal path has no parent directory".to_string(),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| SettingsOperationRegistryError::WriteJournal(error.to_string()))?;
    let file_name = journal_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("operations.json");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(operations)
        .map_err(|error| SettingsOperationRegistryError::WriteJournal(error.to_string()))?;
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary_path, journal_path)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(SettingsOperationRegistryError::WriteJournal(
            error.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        reconcile_interrupted, SettingsOperation, SettingsOperationError, SettingsOperationKind,
        SettingsOperationRegistry, SettingsOperationRegistryError, SettingsOperationState,
        SettingsOperationTransition, SettingsOperationsCommandError,
        MAX_RETAINED_TERMINAL_OPERATIONS,
    };

    fn fixture_operation(state: SettingsOperationState) -> SettingsOperation {
        SettingsOperation {
            id: "operation-1".to_string(),
            kind: SettingsOperationKind::ModelDownload,
            target_id: "model-1".to_string(),
            phase: "download".to_string(),
            state,
            completed_units: 1,
            total_units: Some(2),
            unit: Some("files".to_string()),
            cancellable: true,
            message: "Downloading model.".to_string(),
            error: None,
            started_at: "2026-07-16T12:00:00Z".to_string(),
            updated_at: "2026-07-16T12:00:01Z".to_string(),
        }
    }

    fn failure_error(code: &str, message: &str) -> SettingsOperationError {
        SettingsOperationError {
            code: code.to_string(),
            message: message.to_string(),
            recovery_action: Some("Retry".to_string()),
            detail: None,
        }
    }

    #[test]
    fn concurrent_start_or_get_active_returns_one_canonical_operation() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry = std::sync::Arc::new(
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("operation registry"),
        );
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                registry
                    .start_or_get_active(
                        SettingsOperationKind::ModelDownload,
                        "model-1",
                        Some(100),
                        Some("bytes".to_string()),
                    )
                    .expect("start or get")
            }));
        }
        barrier.wait();
        let first = threads.remove(0).join().expect("first thread");
        let second = threads.remove(0).join().expect("second thread");

        assert_eq!(first.0.id, second.0.id);
        assert_ne!(first.1, second.1);
        assert_eq!(registry.list_active().expect("active operations").len(), 1);
    }

    #[test]
    fn repeat_start_resolves_canonical_terminal_warning() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let running = registry
            .resolve_or_start_active(
                SettingsOperationKind::ModelDownload,
                "model-1",
                Some(100),
                Some("bytes".to_string()),
            )
            .and_then(|(operation, _)| {
                registry.update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Running),
                )
            })
            .expect("running operation");
        let mut warning = running;
        warning.state = SettingsOperationState::Succeeded;
        warning.phase = "ready_with_persistence_warning".to_string();
        warning.cancellable = false;
        warning.error = Some(SettingsOperationError {
            code: "settings.operation.terminalPersistenceFailed".to_string(),
            message: "History persistence failed.".to_string(),
            recovery_action: None,
            detail: Some("injected failure".to_string()),
        });
        let warning = registry
            .replace_in_memory_without_persisting(warning.clone())
            .expect("install warning");

        let (resolved, created) = registry
            .resolve_or_start_active(
                SettingsOperationKind::ModelDownload,
                "model-1",
                Some(100),
                Some("bytes".to_string()),
            )
            .expect("repeat start");

        assert!(!created);
        assert_eq!(resolved, warning);
        assert_eq!(resolved.state, SettingsOperationState::Succeeded);
        assert!(!resolved.cancellable);
    }

    #[test]
    fn invalidating_a_target_discards_stale_warning_and_heals_durable_state() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("operations.json");
        let registry =
            SettingsOperationRegistry::load(journal.clone(), |_| false).expect("registry");
        let running = registry
            .start(
                SettingsOperationKind::SpeechModelsDownload,
                "speech-models",
                Some(100),
                Some("bytes".to_string()),
            )
            .and_then(|operation| {
                registry.update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Running),
                )
            })
            .expect("running operation");
        let mut warning = running;
        warning.state = SettingsOperationState::Succeeded;
        warning.cancellable = false;
        warning.error = Some(SettingsOperationError {
            code: "settings.operation.terminalPersistenceFailed".to_string(),
            message: "History persistence failed.".to_string(),
            recovery_action: None,
            detail: Some("injected failure".to_string()),
        });
        registry
            .replace_in_memory_without_persisting(warning)
            .expect("install stale warning");

        assert!(registry
            .invalidate_target(SettingsOperationKind::SpeechModelsDownload, "speech-models",)
            .expect("invalidate target"));
        let (replacement, created) = registry
            .resolve_or_start_active(
                SettingsOperationKind::SpeechModelsDownload,
                "speech-models",
                Some(100),
                Some("bytes".to_string()),
            )
            .expect("replacement operation");

        assert!(created);
        assert_ne!(replacement.id, "operation-1");
        let durable: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(journal).expect("durable journal"))
                .expect("durable operations");
        assert_eq!(durable.len(), 1);
        assert_eq!(durable[0].id, replacement.id);
    }

    #[test]
    fn persistence_failure_projection_is_terminal_and_does_not_block_retry() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let queued = registry
            .start(
                SettingsOperationKind::SpeechModelsDownload,
                "speech-models",
                Some(100),
                Some("bytes".to_string()),
            )
            .expect("queued operation");

        let visible = registry
            .project_persistence_failure(
                &queued.id,
                "settings.operation.progressPersistenceFailed",
                "Speech model progress could not be saved.",
                "injected write failure",
            )
            .expect("terminal failure projection");
        let (retry, created) = registry
            .resolve_or_start_active(
                SettingsOperationKind::SpeechModelsDownload,
                "speech-models",
                Some(100),
                Some("bytes".to_string()),
            )
            .expect("retry operation");

        assert_eq!(visible.state, SettingsOperationState::Failed);
        assert!(!visible.cancellable);
        assert_eq!(
            visible.error.as_ref().map(|error| error.code.as_str()),
            Some("settings.operation.progressPersistenceFailed")
        );
        assert!(created);
        assert_ne!(retry.id, queued.id);
        assert!(registry.list_active().expect("active").contains(&retry));
    }

    #[test]
    fn in_memory_terminal_warning_hides_stale_durable_running_state() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("operations.json");
        let registry =
            SettingsOperationRegistry::load(journal.clone(), |_| false).expect("registry");
        let operation = registry
            .start(
                SettingsOperationKind::ModelDownload,
                "model-1",
                Some(100),
                Some("bytes".to_string()),
            )
            .and_then(|operation| {
                registry.update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Running),
                )
            })
            .expect("running operation");
        let mut projection = operation.clone();
        projection.state = SettingsOperationState::Succeeded;
        projection.phase = "ready_with_persistence_warning".to_string();
        projection.cancellable = false;
        projection.message =
            "Model is ready, but operation history could not be persisted.".to_string();
        projection.error = Some(SettingsOperationError {
            code: "settings.operation.terminalPersistenceFailed".to_string(),
            message: "The model is ready, but operation history could not be saved.".to_string(),
            recovery_action: Some("Restart Video Creater to reconcile history.".to_string()),
            detail: Some("injected journal write failure".to_string()),
        });

        let projection = registry
            .replace_in_memory_without_persisting(projection.clone())
            .expect("in-memory warning");

        let visible = registry.list_recent().expect("visible operations");
        assert_eq!(visible[0], projection);
        assert!(registry
            .list_active()
            .expect("active operations")
            .is_empty());
        assert_eq!(
            registry.get(&operation.id).expect("visible operation"),
            projection
        );
        let durable: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(&journal).expect("durable journal"))
                .expect("durable operations");
        assert_eq!(durable[0].state, SettingsOperationState::Running);

        let restarted =
            SettingsOperationRegistry::load(journal, |_| true).expect("restart reconcile");
        let reconciled = restarted.list_recent().expect("reconciled operations");
        assert_eq!(reconciled[0].state, SettingsOperationState::Succeeded);
        assert!(!reconciled[0].cancellable);
    }

    #[test]
    fn bounded_terminal_warnings_never_reveal_stale_running_operations() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        for index in 0..(MAX_RETAINED_TERMINAL_OPERATIONS + 10) {
            let running = registry
                .start(
                    SettingsOperationKind::ModelDownload,
                    format!("model-{index}"),
                    Some(100),
                    Some("bytes".to_string()),
                )
                .and_then(|operation| {
                    registry.update(
                        &operation.id,
                        SettingsOperationTransition::to(SettingsOperationState::Running),
                    )
                })
                .expect("running operation");
            let mut warning = running;
            warning.state = SettingsOperationState::Succeeded;
            warning.phase = "ready_with_persistence_warning".to_string();
            warning.cancellable = false;
            warning.error = Some(SettingsOperationError {
                code: "settings.operation.terminalPersistenceFailed".to_string(),
                message: "The model is ready, but operation history could not be saved."
                    .to_string(),
                recovery_action: None,
                detail: Some(format!("failure {index}")),
            });
            registry
                .replace_in_memory_without_persisting(warning)
                .expect("install warning");
        }

        assert!(registry
            .list_active()
            .expect("active operations")
            .is_empty());
        let recent = registry.list_recent().expect("recent operations");
        assert_eq!(recent.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert!(recent.iter().all(|operation| {
            operation.state == SettingsOperationState::Succeeded && !operation.cancellable
        }));
    }

    #[test]
    fn running_operation_cannot_transition_back_to_queued() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("operations.json");
        let registry =
            SettingsOperationRegistry::load(journal, |_| false).expect("operation registry");
        let operation = registry
            .start(
                SettingsOperationKind::ModelDownload,
                "model-1",
                Some(2),
                Some("files".to_string()),
            )
            .expect("queued operation");
        registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("running operation");

        let error = registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Queued),
            )
            .expect_err("running to queued must fail");

        assert!(error.to_string().contains("running"));
        assert!(error.to_string().contains("queued"));
    }

    #[test]
    fn startup_reconciles_running_operation_as_interrupted() {
        let mut operation = fixture_operation(SettingsOperationState::Running);
        operation.updated_at = "2099-07-16T12:00:01.000Z".to_string();
        let previous_updated_at = operation.updated_at.clone();
        let recovered = reconcile_interrupted(operation, false);
        assert_eq!(recovered.state, SettingsOperationState::Failed);
        assert!(recovered.updated_at > previous_updated_at);
        assert_eq!(
            recovered.error.unwrap().code,
            "settings.operation.interrupted"
        );
    }

    #[test]
    fn every_registry_mutation_advances_the_operation_version() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let queued = registry
            .start(SettingsOperationKind::ModelDownload, "model-1", None, None)
            .expect("queued operation");
        let running = registry
            .update(
                &queued.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("running operation");
        let progressed = registry
            .update(
                &queued.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("progress operation");
        let cancelling = registry
            .request_cancel(&queued.id)
            .expect("cancelling operation");
        let mut warning = cancelling.clone();
        warning.state = SettingsOperationState::Succeeded;
        warning.phase = "ready_with_persistence_warning".to_string();
        warning.cancellable = false;
        let warning = registry
            .replace_in_memory_without_persisting(warning)
            .expect("terminal warning");

        assert!(running.updated_at > queued.updated_at);
        assert!(progressed.updated_at > running.updated_at);
        assert!(cancelling.updated_at > progressed.updated_at);
        assert!(warning.updated_at > cancelling.updated_at);
    }

    #[test]
    fn request_cancel_marks_running_cancellable_operation_as_cancelling() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        let registry = SettingsOperationRegistry::load(journal.clone(), |_| false)
            .expect("operation registry");
        let operation = registry
            .start(SettingsOperationKind::HealthCheck, "all", None, None)
            .expect("queued operation");
        registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("running operation");

        let cancelling = registry
            .request_cancel(&operation.id)
            .expect("request cancellation");

        assert_eq!(cancelling.state, SettingsOperationState::Cancelling);
        assert_eq!(cancelling.phase, "cancelling");
        assert!(!cancelling.cancellable);
        let persisted: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(journal).expect("read cancellation journal"))
                .expect("deserialize cancellation journal");
        assert_eq!(persisted[0].state, SettingsOperationState::Cancelling);
    }

    #[test]
    fn non_cancellable_operation_rejects_cancel() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        let registry =
            SettingsOperationRegistry::load(journal, |_| false).expect("operation registry");
        let operation = registry
            .start(
                SettingsOperationKind::SpeechModelsDownload,
                "speech-models",
                None,
                None,
            )
            .expect("queued operation");
        let mut running = SettingsOperationTransition::to(SettingsOperationState::Running);
        running.cancellable = Some(false);
        registry
            .update(&operation.id, running)
            .expect("non-cancellable running operation");

        let error = registry
            .request_cancel(&operation.id)
            .expect_err("non-cancellable operation must reject cancel");

        assert!(error.to_string().contains("not cancellable"));
    }

    #[test]
    fn terminal_operation_rejects_further_transition() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        let registry =
            SettingsOperationRegistry::load(journal, |_| false).expect("operation registry");
        let operation = registry
            .start(SettingsOperationKind::StorageRefresh, "storage", None, None)
            .expect("queued operation");
        registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Succeeded),
            )
            .expect("succeeded operation");

        let error = registry
            .update(
                &operation.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect_err("terminal operation must reject transitions");

        assert!(error.to_string().contains("succeeded"));
        assert!(error.to_string().contains("running"));
    }

    #[test]
    fn startup_marks_interrupted_operation_succeeded_when_target_is_complete() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        std::fs::create_dir_all(journal.parent().expect("journal parent"))
            .expect("create journal parent");
        std::fs::write(
            &journal,
            serde_json::to_vec(&vec![fixture_operation(SettingsOperationState::Running)])
                .expect("serialize journal fixture"),
        )
        .expect("write journal fixture");

        let registry =
            SettingsOperationRegistry::load(journal, |operation| operation.target_id == "model-1")
                .expect("reconciled operation registry");
        let recovered = registry.get("operation-1").expect("recovered operation");

        assert_eq!(recovered.state, SettingsOperationState::Succeeded);
        assert!(recovered.error.is_none());
    }

    #[test]
    fn journal_round_trip_preserves_registered_operations_without_temp_files() {
        let root = tempfile::tempdir().expect("operation journal root");
        let settings_dir = root.path().join("settings");
        let journal = settings_dir.join("operations.json");
        let registry =
            SettingsOperationRegistry::load(journal.clone(), |_| false).expect("registry");
        let started = registry
            .start(
                SettingsOperationKind::SkillRepair,
                "video-creater-video-pipeline",
                Some(4),
                Some("checks".to_string()),
            )
            .expect("start operation");

        let reloaded =
            SettingsOperationRegistry::load(journal, |_| false).expect("reload registry");
        let active = reloaded.list_active().expect("list active operations");
        let operations = reloaded.list_recent().expect("list recent operations");

        assert!(active.is_empty());
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].id, started.id);
        assert_eq!(operations[0].state, SettingsOperationState::Failed);
        assert!(std::fs::read_dir(settings_dir)
            .expect("read settings directory")
            .all(|entry| !entry
                .expect("settings directory entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }

    #[test]
    fn list_active_excludes_terminal_operations_while_recent_keeps_them_visible() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let active = registry
            .start(SettingsOperationKind::HealthCheck, "active", None, None)
            .expect("active operation");
        let terminal = registry
            .start(
                SettingsOperationKind::StorageCleanup,
                "terminal",
                None,
                None,
            )
            .expect("terminal operation");
        let mut failed = SettingsOperationTransition::to(SettingsOperationState::Failed);
        failed.error = Some(failure_error(
            "settings.storage.cleanupFailed",
            "Storage cleanup failed.",
        ));
        registry
            .update(&terminal.id, failed)
            .expect("failed operation");

        let active_operations = registry.list_active().expect("active operations");
        let recent_operations = registry.list_recent().expect("recent operations");

        assert_eq!(active_operations.len(), 1);
        assert_eq!(active_operations[0].id, active.id);
        assert_eq!(recent_operations.len(), 2);
        assert!(recent_operations
            .iter()
            .any(|operation| operation.id == terminal.id));
    }

    #[test]
    fn journal_retains_only_the_newest_bounded_terminal_history() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("operations.json");
        let registry =
            SettingsOperationRegistry::load(journal.clone(), |_| false).expect("registry");
        for index in 0..=MAX_RETAINED_TERMINAL_OPERATIONS {
            let operation = registry
                .start(
                    SettingsOperationKind::StorageRefresh,
                    format!("storage-{index}"),
                    None,
                    None,
                )
                .expect("queued operation");
            registry
                .update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Succeeded),
                )
                .expect("succeeded operation");
        }

        let recent = registry.list_recent().expect("recent operations");
        let persisted: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(journal).expect("read retained journal"))
                .expect("deserialize retained journal");

        assert_eq!(recent.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert_eq!(persisted.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert_eq!(recent[0].target_id, "storage-1");
        assert_eq!(
            recent.last().expect("newest terminal").target_id,
            format!("storage-{MAX_RETAINED_TERMINAL_OPERATIONS}")
        );
    }

    #[test]
    fn retention_keeps_an_old_operation_that_completed_most_recently() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let long_running = registry
            .start(
                SettingsOperationKind::HealthCheck,
                "long-running",
                None,
                None,
            )
            .expect("long-running operation");
        registry
            .update(
                &long_running.id,
                SettingsOperationTransition::to(SettingsOperationState::Running),
            )
            .expect("running operation");
        for index in 0..MAX_RETAINED_TERMINAL_OPERATIONS {
            let operation = registry
                .start(
                    SettingsOperationKind::StorageRefresh,
                    format!("storage-{index}"),
                    None,
                    None,
                )
                .expect("queued operation");
            registry
                .update(
                    &operation.id,
                    SettingsOperationTransition::to(SettingsOperationState::Succeeded),
                )
                .expect("succeeded operation");
        }

        registry
            .update(
                &long_running.id,
                SettingsOperationTransition::to(SettingsOperationState::Succeeded),
            )
            .expect("recently completed long operation");
        let recent = registry.list_recent().expect("recent operations");

        assert_eq!(recent.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert!(recent
            .iter()
            .any(|operation| operation.id == long_running.id));
        assert!(!recent
            .iter()
            .any(|operation| operation.target_id == "storage-0"));
    }

    #[test]
    fn startup_retention_keeps_an_old_operation_reconciled_most_recently() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("operations.json");
        let mut operations = vec![fixture_operation(SettingsOperationState::Running)];
        operations.extend((0..MAX_RETAINED_TERMINAL_OPERATIONS).map(|index| {
            let mut operation = fixture_operation(SettingsOperationState::Succeeded);
            operation.id = format!("terminal-{index}");
            operation.target_id = format!("storage-{index}");
            operation.started_at = format!("2026-07-16T12:00:{index:02}Z");
            operation.updated_at = format!("2026-07-16T12:00:{index:02}Z");
            operation.cancellable = false;
            operation
        }));
        std::fs::write(
            &journal,
            serde_json::to_vec(&operations).expect("serialize startup journal"),
        )
        .expect("write startup journal");

        let registry =
            SettingsOperationRegistry::load(journal.clone(), |_| false).expect("load registry");
        let recent = registry.list_recent().expect("recent startup operations");
        let persisted: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(journal).expect("read reconciled journal"))
                .expect("deserialize reconciled journal");

        assert_eq!(recent.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert_eq!(persisted.len(), MAX_RETAINED_TERMINAL_OPERATIONS);
        assert_eq!(
            recent.last().expect("most recent recovered").id,
            "operation-1"
        );
        assert_eq!(
            recent.last().expect("most recent recovered").state,
            SettingsOperationState::Failed
        );
        assert_eq!(
            recent
                .last()
                .and_then(|operation| operation.error.as_ref())
                .expect("interrupted recovery error")
                .code,
            "settings.operation.interrupted"
        );
        assert!(!recent.iter().any(|operation| operation.id == "terminal-0"));
    }

    #[test]
    fn failed_transition_rejects_blank_error_code_and_message() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let operation = registry
            .start(SettingsOperationKind::HealthCheck, "all", None, None)
            .expect("queued operation");
        let mut blank_code = SettingsOperationTransition::to(SettingsOperationState::Failed);
        blank_code.error = Some(failure_error("  ", "Health check failed."));

        let code_error = registry
            .update(&operation.id, blank_code)
            .expect_err("blank error code must fail");

        assert!(code_error.to_string().contains("code"));

        let mut blank_message = SettingsOperationTransition::to(SettingsOperationState::Failed);
        blank_message.error = Some(failure_error("settings.health.failed", "\n\t"));
        let message_error = registry
            .update(&operation.id, blank_message)
            .expect_err("blank error message must fail");

        assert!(message_error.to_string().contains("message"));
    }

    #[test]
    fn direct_cancelling_transition_forces_operation_non_cancellable() {
        let root = tempfile::tempdir().expect("operation journal root");
        let registry =
            SettingsOperationRegistry::load(root.path().join("operations.json"), |_| false)
                .expect("registry");
        let operation = registry
            .start(SettingsOperationKind::ModelDownload, "model-1", None, None)
            .expect("queued operation");
        let mut cancelling = SettingsOperationTransition::to(SettingsOperationState::Cancelling);
        cancelling.cancellable = Some(true);

        let updated = registry
            .update(&operation.id, cancelling)
            .expect("cancelling operation");

        assert!(!updated.cancellable);
    }

    #[test]
    fn resilient_startup_quarantines_corrupt_journal_and_surfaces_recovery_record() {
        let root = tempfile::tempdir().expect("operation journal root");
        let journal = root.path().join("settings/operations.json");
        std::fs::create_dir_all(journal.parent().expect("journal parent"))
            .expect("create journal parent");
        std::fs::write(&journal, b"{ not valid json").expect("write corrupt journal");

        let registry = SettingsOperationRegistry::load_resilient(journal.clone(), |_| false);
        let recent = registry.list_recent().expect("recovery operations");

        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].state, SettingsOperationState::Failed);
        let error = recent[0].error.as_ref().expect("journal recovery error");
        assert_eq!(error.code, "settings.operation.journalRecovered");
        assert!(!error.message.trim().is_empty());
        assert!(error
            .detail
            .as_deref()
            .expect("journal recovery detail")
            .contains("invalid"));
        assert!(journal.is_file());
        let persisted: Vec<SettingsOperation> =
            serde_json::from_slice(&std::fs::read(&journal).expect("read replacement journal"))
                .expect("replacement journal is valid");
        assert_eq!(
            persisted[0].error.as_ref().expect("persisted error").code,
            error.code
        );
        assert!(std::fs::read_dir(journal.parent().expect("journal parent"))
            .expect("read journal directory")
            .any(|entry| entry
                .expect("journal directory entry")
                .file_name()
                .to_string_lossy()
                .contains("operations.corrupt.")));
    }

    #[test]
    fn command_error_serializes_stable_code_message_and_detail() {
        let error = SettingsOperationsCommandError::from(SettingsOperationRegistryError::Lock);

        let json = serde_json::to_value(error).expect("serialize operations command error");

        assert_eq!(json["code"], "settings.operation.registryUnavailable");
        assert_eq!(
            json["message"],
            "Settings operation history is temporarily unavailable."
        );
        assert_eq!(json["detail"], "settings operation registry lock failed");
    }
}
