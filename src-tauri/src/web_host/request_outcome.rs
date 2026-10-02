//! Compact durable receipts, never RPC bodies or authorization credentials.

use super::rpc::{RpcEnvelope, RpcResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

#[cfg(unix)]
#[path = "request_outcome_io.rs"]
mod io;
#[cfg(not(unix))]
#[path = "request_outcome_portable.rs"]
mod io;
use io::JournalDirectory;

const MAX_RECORDS: usize = 16_384;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECORD_BYTES: usize = 4096;
const MAX_CANCELLATION_RECORDS: usize = 1024;
const MAX_CANCELLATION_BYTES: usize = 4 * 1024 * 1024;
pub const RETENTION_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    Pending,
    Interrupted,
    Succeeded,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestOutcome {
    pub request_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_revision: Option<u64>,
    pub status: OutcomeStatus,
    pub issued_at: u64,
    pub acknowledged: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    protocol: u32,
    principal_hash: String,
    fingerprint: String,
    creation_nonce: Option<String>,
    outcome: RequestOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeError {
    Unavailable,
    Capacity,
    Conflict,
    Expired,
    NotFound,
    Pending,
}

pub enum OutcomeAdmission {
    New { creation_nonce: Option<String> },
    Existing(RequestOutcome),
}

struct State {
    records: BTreeMap<String, Record>,
    bytes: usize,
    cancellation_bytes: usize,
    cancellation_records: usize,
    next_maintenance: u64,
}

pub struct RequestOutcomeStore {
    directory: JournalDirectory,
    state: Mutex<State>,
}

impl RequestOutcomeStore {
    /// Lifetime directory lock excludes another cooperating host on the same journal inode.
    pub fn open(path: &Path, now: u64) -> Result<Self, OutcomeError> {
        let directory = JournalDirectory::open(path).map_err(|_| OutcomeError::Unavailable)?;
        let mut records = BTreeMap::new();
        let mut bytes = 0usize;
        let mut cancellation_bytes = 0usize;
        let mut cancellation_records = 0usize;
        for name in directory
            .names((MAX_RECORDS + MAX_CANCELLATION_RECORDS) * 2)
            .map_err(|_| OutcomeError::Unavailable)?
        {
            let Some(key) = name.strip_suffix(".json").filter(|key| hash_shape(key)) else {
                continue;
            };
            let raw = directory
                .read(&name, MAX_RECORD_BYTES)
                .map_err(|_| OutcomeError::Unavailable)?;
            let mut record: Record =
                serde_json::from_slice(&raw).map_err(|_| OutcomeError::Unavailable)?;
            if !valid_record(key, &record) {
                return Err(OutcomeError::Unavailable);
            }
            if record.outcome.status == OutcomeStatus::Pending {
                record.outcome.status = OutcomeStatus::Interrupted;
                directory
                    .write(
                        &name,
                        &serde_json::to_vec(&record).map_err(|_| OutcomeError::Unavailable)?,
                    )
                    .map_err(|_| OutcomeError::Unavailable)?;
            }
            let cost = record_cost(&record)?;
            if cancellation_operation(&record.outcome.operation) {
                cancellation_bytes = cancellation_bytes
                    .checked_add(cost)
                    .ok_or(OutcomeError::Capacity)?;
                cancellation_records += 1;
            } else {
                bytes = bytes.checked_add(cost).ok_or(OutcomeError::Capacity)?;
            }
            records.insert(key.to_owned(), record);
            if records.len() - cancellation_records > MAX_RECORDS
                || bytes > MAX_BYTES
                || cancellation_records > MAX_CANCELLATION_RECORDS
                || cancellation_bytes > MAX_CANCELLATION_BYTES
            {
                return Err(OutcomeError::Capacity);
            }
        }
        let store = Self {
            directory,
            state: Mutex::new(State {
                records,
                bytes,
                cancellation_bytes,
                cancellation_records,
                next_maintenance: now,
            }),
        };
        store.maintain(now)?;
        Ok(store)
    }

    pub fn begin(
        &self,
        principal: &str,
        request: &RpcEnvelope,
        payload: &[u8],
        now: u64,
    ) -> Result<OutcomeAdmission, OutcomeError> {
        self.maintain(now)?;
        if !valid_request_id(&request.request_id)
            || request.project_id.as_deref().is_some_and(|id| !safe_id(id))
        {
            return Err(OutcomeError::Unavailable);
        }
        let key = record_key(principal, &request.request_id);
        let fingerprint = digest(payload);
        let mut state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        if let Some(record) = state.records.get(&key) {
            if record.fingerprint != fingerprint {
                return Err(OutcomeError::Conflict);
            }
            return Ok(OutcomeAdmission::Existing(record.outcome.clone()));
        }
        if request.request_id.starts_with("browser-v2-")
            && request_timestamp(&request.request_id).is_none()
        {
            return Err(OutcomeError::Expired);
        }
        if let Some(created) = request_timestamp(&request.request_id) {
            if created > now.saturating_add(300) || now.saturating_sub(created) > RETENTION_SECONDS
            {
                return Err(OutcomeError::Expired);
            }
        }
        let record = Record {
            protocol: 1,
            principal_hash: digest(principal.as_bytes()),
            fingerprint,
            creation_nonce: (request.operation == "remote_create_project")
                .then(|| uuid::Uuid::new_v4().simple().to_string()),
            outcome: RequestOutcome {
                request_id: request.request_id.clone(),
                operation: request.operation.clone(),
                project_id: request.project_id.clone(),
                canonical_project_id: None,
                catalog_project_id: None,
                content_revision: None,
                status: OutcomeStatus::Pending,
                issued_at: now,
                acknowledged: false,
            },
        };
        let cancellation = cancellation_operation(&request.operation);
        if (cancellation && state.cancellation_records >= MAX_CANCELLATION_RECORDS)
            || (!cancellation && state.records.len() - state.cancellation_records >= MAX_RECORDS)
        {
            return Err(OutcomeError::Capacity);
        }
        self.persist(&mut state, &key, record.clone())?;
        Ok(OutcomeAdmission::New {
            creation_nonce: record.creation_nonce,
        })
    }

    pub fn finish(
        &self,
        principal: &str,
        request_id: &str,
        response: &RpcResponse,
    ) -> Result<RequestOutcome, OutcomeError> {
        let key = record_key(principal, request_id);
        let mut state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        let mut record = state
            .records
            .get(&key)
            .cloned()
            .ok_or(OutcomeError::NotFound)?;
        record.outcome.status = if response.ok {
            OutcomeStatus::Succeeded
        } else {
            OutcomeStatus::Rejected
        };
        if let Some(result) = response.result.as_ref() {
            let project = result.get("project").unwrap_or(result);
            record.outcome.canonical_project_id = project
                .get("id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| canonical_id(id))
                .map(str::to_owned);
            record.outcome.content_revision = project
                .get("contentRevision")
                .and_then(serde_json::Value::as_u64);
            record.outcome.catalog_project_id = result
                .get("catalogProjectId")
                .and_then(serde_json::Value::as_str)
                .filter(|id| safe_id(id))
                .map(str::to_owned);
        }
        self.persist(&mut state, &key, record.clone())?;
        Ok(record.outcome)
    }

    pub fn interrupt(&self, principal: &str, request_id: &str) -> Result<(), OutcomeError> {
        let key = record_key(principal, request_id);
        let mut state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        let mut record = state
            .records
            .get(&key)
            .cloned()
            .ok_or(OutcomeError::NotFound)?;
        if record.outcome.status != OutcomeStatus::Pending {
            return Ok(());
        }
        record.outcome.status = OutcomeStatus::Interrupted;
        self.persist(&mut state, &key, record)
    }

    pub fn lookup(
        &self,
        principal: &str,
        request_id: &str,
    ) -> Result<RequestOutcome, OutcomeError> {
        self.state
            .lock()
            .map_err(|_| OutcomeError::Unavailable)?
            .records
            .get(&record_key(principal, request_id))
            .map(|record| record.outcome.clone())
            .ok_or(OutcomeError::NotFound)
    }

    /// An expiry attestation contains no historical outcome or project metadata.
    /// Browser markers lack principal identity, so even a foreign retained record
    /// prevents treating a missing scoped lookup as a globally absent request.
    pub fn replay_fenced_absent(&self, request_id: &str, now: u64) -> Result<bool, OutcomeError> {
        let state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        Ok(expired_absent(&state, request_id, now))
    }

    pub fn creation_nonce(&self, principal: &str, request_id: &str) -> Option<String> {
        self.state
            .lock()
            .ok()?
            .records
            .get(&record_key(principal, request_id))?
            .creation_nonce
            .clone()
    }

    pub fn outstanding(
        &self,
        principal: &str,
        after: Option<&str>,
    ) -> Result<Vec<RequestOutcome>, OutcomeError> {
        let principal_hash = digest(principal.as_bytes());
        let state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        let mut outcomes: Vec<_> = state
            .records
            .values()
            .filter(|record| {
                record.principal_hash == principal_hash && !record.outcome.acknowledged
            })
            .map(|record| record.outcome.clone())
            .collect();
        outcomes.sort_by(|a, b| a.request_id.cmp(&b.request_id));
        Ok(outcomes
            .into_iter()
            .filter(|outcome| after.is_none_or(|after| outcome.request_id.as_str() > after))
            .take(256)
            .collect())
    }

    /// Explicit reconciliation may acknowledge interruption; it never calls it failed.
    pub fn acknowledge(&self, principal: &str, request_id: &str) -> Result<(), OutcomeError> {
        self.acknowledge_with_clock(principal, request_id, None)
    }

    /// After explicit canonical reconciliation, confirm an absent expired identity's
    /// replay fence. The absence and age checks share the retained-record ACK lock.
    pub fn acknowledge_at(
        &self,
        principal: &str,
        request_id: &str,
        now: u64,
    ) -> Result<(), OutcomeError> {
        self.acknowledge_with_clock(principal, request_id, Some(now))
    }

    fn acknowledge_with_clock(
        &self,
        principal: &str,
        request_id: &str,
        now: Option<u64>,
    ) -> Result<(), OutcomeError> {
        let key = record_key(principal, request_id);
        let mut state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        let Some(mut record) = state.records.get(&key).cloned() else {
            return if now.is_some_and(|now| expired_absent(&state, request_id, now)) {
                Ok(())
            } else {
                Err(OutcomeError::NotFound)
            };
        };
        if record.outcome.status == OutcomeStatus::Pending {
            return Err(OutcomeError::Pending);
        }
        record.outcome.acknowledged = true;
        self.persist(&mut state, &key, record)
    }

    fn persist(&self, state: &mut State, key: &str, record: Record) -> Result<(), OutcomeError> {
        let raw = serde_json::to_vec(&record).map_err(|_| OutcomeError::Unavailable)?;
        let old = state
            .records
            .get(key)
            .map(record_cost)
            .transpose()?
            .unwrap_or(0);
        let cancellation = cancellation_operation(&record.outcome.operation);
        let current_bytes = if cancellation {
            state.cancellation_bytes
        } else {
            state.bytes
        };
        let maximum = if cancellation {
            MAX_CANCELLATION_BYTES
        } else {
            MAX_BYTES
        };
        let bytes = current_bytes
            .saturating_sub(old)
            .checked_add(record_cost(&record)?)
            .ok_or(OutcomeError::Capacity)?;
        if raw.len() > MAX_RECORD_BYTES || bytes > maximum {
            return Err(OutcomeError::Capacity);
        }
        self.directory
            .write(&format!("{key}.json"), &raw)
            .map_err(|_| OutcomeError::Unavailable)?;
        let new = state.records.insert(key.to_owned(), record).is_none();
        if cancellation {
            state.cancellation_bytes = bytes;
            state.cancellation_records += usize::from(new);
        } else {
            state.bytes = bytes;
        }
        Ok(())
    }

    fn maintain(&self, now: u64) -> Result<(), OutcomeError> {
        let mut state = self.state.lock().map_err(|_| OutcomeError::Unavailable)?;
        if now < state.next_maintenance {
            return Ok(());
        }
        let expired: Vec<_> = state
            .records
            .iter()
            .filter(|(_, record)| {
                record.outcome.acknowledged
                    && matches!(
                        record.outcome.status,
                        OutcomeStatus::Succeeded | OutcomeStatus::Rejected
                    )
                    && request_timestamp(&record.outcome.request_id)
                        .is_some_and(|created| now.saturating_sub(created) > RETENTION_SECONDS)
            })
            .map(|(key, record)| (key.clone(), serde_json::to_vec(record).map(|raw| raw.len())))
            .collect();
        for (key, size) in expired {
            self.directory
                .remove(&format!("{key}.json"))
                .map_err(|_| OutcomeError::Unavailable)?;
            let removed = state
                .records
                .remove(&key)
                .ok_or(OutcomeError::Unavailable)?;
            let cost = size.map_err(|_| OutcomeError::Unavailable)?;
            if cancellation_operation(&removed.outcome.operation) {
                state.cancellation_bytes = state.cancellation_bytes.saturating_sub(cost);
                state.cancellation_records = state.cancellation_records.saturating_sub(1);
            } else {
                state.bytes = state.bytes.saturating_sub(cost);
            }
        }
        state.next_maintenance = now.saturating_add(60);
        Ok(())
    }
}

fn cancellation_operation(operation: &str) -> bool {
    super::registry::RpcRegistry::from_inventory()
        .operation(operation)
        .is_some_and(|descriptor| {
            descriptor.mutation == crate::app_service::operation::MutationClass::JobMutation
                && descriptor.name.starts_with("cancel_")
        })
}

fn expired_absent(state: &State, request_id: &str, now: u64) -> bool {
    valid_request_id(request_id)
        && request_timestamp(request_id)
            .is_some_and(|created| now.saturating_sub(created) > RETENTION_SECONDS)
        && !state
            .records
            .values()
            .any(|record| record.outcome.request_id == request_id)
}

fn request_timestamp(id: &str) -> Option<u64> {
    let (time, suffix) = id.strip_prefix("browser-v2-")?.split_once('-')?;
    if suffix.is_empty() || !safe_id(suffix) {
        return None;
    }
    time.parse().ok()
}
fn digest(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn record_key(principal: &str, request_id: &str) -> String {
    digest(format!("{}:{request_id}", digest(principal.as_bytes())).as_bytes())
}
fn hash_shape(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn valid_request_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
}

fn canonical_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}
fn valid_record(key: &str, record: &Record) -> bool {
    record.protocol == 1
        && hash_shape(&record.principal_hash)
        && hash_shape(&record.fingerprint)
        && key
            == digest(format!("{}:{}", record.principal_hash, record.outcome.request_id).as_bytes())
        && valid_request_id(&record.outcome.request_id)
        && !record.outcome.operation.is_empty()
        && record.outcome.operation.len() <= 128
        && record
            .outcome
            .operation
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && [
            &record.outcome.project_id,
            &record.outcome.catalog_project_id,
        ]
        .into_iter()
        .all(|id| id.as_deref().is_none_or(safe_id))
        && record
            .outcome
            .canonical_project_id
            .as_deref()
            .is_none_or(canonical_id)
        && record.creation_nonce.as_deref().is_none_or(|nonce| {
            nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn record_cost(record: &Record) -> Result<usize, OutcomeError> {
    if matches!(
        record.outcome.status,
        OutcomeStatus::Pending | OutcomeStatus::Interrupted
    ) {
        Ok(MAX_RECORD_BYTES)
    } else {
        serde_json::to_vec(record)
            .map(|bytes| bytes.len())
            .map_err(|_| OutcomeError::Unavailable)
    }
}

#[cfg(test)]
#[path = "request_outcome_tests.rs"]
mod tests;
