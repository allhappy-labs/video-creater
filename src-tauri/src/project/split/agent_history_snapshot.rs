//! Project snapshots stored in the agent Undo history.
//!
//! Each history entry keeps the whole pre-apply project (and legacy entries a
//! post-apply one), so snapshots are stored as raw deflate of the project JSON,
//! base64-encoded: `{ "encoding": "deflate-base64", "jsonBytes": N, "data": "…" }`.
//! Older history files store plain `VideoProject` objects; they still read, and
//! are compressed when the history is written again.
//!
//! History keeps at most `MAX_AGENT_EDIT_HISTORY_ENTRIES` entries and at most
//! `MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES` of stored snapshot data, dropping
//! the oldest entries first. The newest entry is always kept.

use super::{SplitAgentEditHistoryEntry, SplitProjectError};
use crate::project::model::VideoProject;
use base64::Engine;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub(super) const MAX_AGENT_EDIT_HISTORY_ENTRIES: usize = 20;
/// Cap on the stored (base64) snapshot bytes the agent Undo history retains.
pub const MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES: usize = 32 * 1024 * 1024;
const ENCODING: &str = "deflate-base64";

/// A project snapshot held compressed: raw deflate of its JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredSnapshot", into = "StoredSnapshot")]
pub struct AgentProjectSnapshot {
    compressed: Vec<u8>,
    json_bytes: usize,
}

impl AgentProjectSnapshot {
    pub fn from_project(project: &VideoProject) -> Result<Self, SplitProjectError> {
        let json =
            serde_json::to_vec(project).map_err(|error| snapshot_error(error.to_string()))?;
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&json)
            .map_err(|error| snapshot_error(error.to_string()))?;
        let compressed = encoder
            .finish()
            .map_err(|error| snapshot_error(error.to_string()))?;
        Ok(Self {
            compressed,
            json_bytes: json.len(),
        })
    }

    /// The snapshot's project. Corrupt data, or data whose JSON length doesn't
    /// match `jsonBytes`, is a JSON error.
    pub fn project(&self) -> Result<VideoProject, SplitProjectError> {
        let mut json = Vec::with_capacity(self.json_bytes);
        DeflateDecoder::new(self.compressed.as_slice())
            .take(self.json_bytes as u64 + 1)
            .read_to_end(&mut json)
            .map_err(|error| snapshot_error(format!("snapshot data doesn't inflate: {error}")))?;
        if json.len() != self.json_bytes {
            return Err(snapshot_error(format!(
                "snapshot holds {} JSON bytes, but jsonBytes is {}",
                json.len(),
                self.json_bytes
            )));
        }
        serde_json::from_slice(&json).map_err(|error| snapshot_error(error.to_string()))
    }

    /// Bytes the snapshot takes in the history file (its base64 `data`).
    pub fn stored_bytes(&self) -> usize {
        self.compressed.len().div_ceil(3) * 4
    }
}

fn snapshot_error(message: String) -> SplitProjectError {
    SplitProjectError::Json {
        path: "agent edit history snapshot".to_string(),
        message,
    }
}

/// The serialized forms: compressed, or a plain project from older files.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum StoredSnapshot {
    Compressed(CompressedSnapshot),
    Legacy(Box<VideoProject>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompressedSnapshot {
    encoding: String,
    json_bytes: usize,
    data: String,
}

impl TryFrom<StoredSnapshot> for AgentProjectSnapshot {
    type Error = SplitProjectError;

    fn try_from(stored: StoredSnapshot) -> Result<Self, Self::Error> {
        match stored {
            StoredSnapshot::Legacy(project) => Self::from_project(&project),
            StoredSnapshot::Compressed(snapshot) => {
                if snapshot.encoding != ENCODING {
                    return Err(snapshot_error(format!(
                        "unsupported snapshot encoding {}",
                        snapshot.encoding
                    )));
                }
                let compressed = base64::engine::general_purpose::STANDARD
                    .decode(snapshot.data)
                    .map_err(|error| {
                        snapshot_error(format!("snapshot data isn't base64: {error}"))
                    })?;
                Ok(Self {
                    compressed,
                    json_bytes: snapshot.json_bytes,
                })
            }
        }
    }
}

impl From<AgentProjectSnapshot> for StoredSnapshot {
    fn from(snapshot: AgentProjectSnapshot) -> Self {
        Self::Compressed(CompressedSnapshot {
            encoding: ENCODING.to_string(),
            json_bytes: snapshot.json_bytes,
            data: base64::engine::general_purpose::STANDARD.encode(snapshot.compressed),
        })
    }
}

fn entry_stored_bytes(entry: &SplitAgentEditHistoryEntry) -> usize {
    entry.before.stored_bytes()
        + entry
            .after
            .as_ref()
            .map_or(0, AgentProjectSnapshot::stored_bytes)
}

/// Drops the oldest entries beyond `MAX_AGENT_EDIT_HISTORY_ENTRIES`, then while
/// the stored snapshot bytes exceed `max_bytes`, keeping the newest entry.
pub(super) fn retain_agent_edit_history_budget(
    entries: &mut Vec<SplitAgentEditHistoryEntry>,
    max_bytes: usize,
) {
    if entries.len() > MAX_AGENT_EDIT_HISTORY_ENTRIES {
        let excess = entries.len() - MAX_AGENT_EDIT_HISTORY_ENTRIES;
        entries.drain(0..excess);
    }
    let mut total = entries.iter().map(entry_stored_bytes).sum::<usize>();
    let mut dropped = 0;
    while total > max_bytes && entries.len() - dropped > 1 {
        total -= entry_stored_bytes(&entries[dropped]);
        dropped += 1;
    }
    entries.drain(0..dropped);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use serde_json::json;

    fn entry(id: &str, project: &VideoProject) -> SplitAgentEditHistoryEntry {
        SplitAgentEditHistoryEntry {
            id: id.to_string(),
            action_count: 1,
            before: AgentProjectSnapshot::from_project(project).expect("snapshot"),
            after: None,
            action_ids: Vec::new(),
            after_content_revision: None,
            after_content_hash: None,
            after_content_hash_version: None,
            added_bookkeeping_ids: Default::default(),
            added_generated_asset_ids: Vec::new(),
            background_generated_asset_ids: Vec::new(),
            session_id: None,
            turn_id: None,
        }
    }

    /// A project whose snapshot is larger than 1 KiB even compressed.
    fn incompressible_project(seed: u64) -> VideoProject {
        let mut state = seed;
        let mut project = sample_project();
        project.name = (0..4096)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                char::from(b'a' + ((state >> 33) % 26) as u8)
            })
            .collect();
        project
    }

    fn decode(value: serde_json::Value) -> Result<VideoProject, SplitProjectError> {
        let stored = serde_json::from_value::<StoredSnapshot>(value).expect("stored form");
        AgentProjectSnapshot::try_from(stored)?.project()
    }

    #[test]
    fn a_snapshot_round_trips_through_its_compressed_form() {
        let project = sample_project();
        let snapshot = AgentProjectSnapshot::from_project(&project).expect("snapshot");

        let value = serde_json::to_value(&snapshot).expect("serialize");
        assert_eq!(value["encoding"], json!(ENCODING));
        assert_eq!(
            value["jsonBytes"],
            json!(serde_json::to_vec(&project).expect("json").len())
        );
        assert_eq!(
            value["data"].as_str().map(str::len),
            Some(snapshot.stored_bytes())
        );
        let read = serde_json::from_value::<AgentProjectSnapshot>(value).expect("deserialize");
        assert_eq!(read, snapshot);
        assert_eq!(read.project().expect("project"), project);
    }

    #[test]
    fn a_legacy_plain_project_still_reads() {
        let project = sample_project();

        let read = serde_json::from_value::<AgentProjectSnapshot>(
            serde_json::to_value(&project).expect("plain project"),
        )
        .expect("legacy snapshot");

        assert_eq!(read.project().expect("project"), project);
        assert_eq!(
            serde_json::to_value(&read).expect("serialize")["encoding"],
            json!(ENCODING)
        );
    }

    #[test]
    fn a_corrupt_or_mismatched_snapshot_is_a_json_error() {
        let valid = serde_json::to_value(
            AgentProjectSnapshot::from_project(&sample_project()).expect("snapshot"),
        )
        .expect("serialize");
        let json_bytes = valid["jsonBytes"].as_u64().expect("json bytes");
        let not_base64 =
            json!({ "encoding": ENCODING, "jsonBytes": json_bytes, "data": "not base64!" });
        let not_deflate = json!({
            "encoding": ENCODING,
            "jsonBytes": json_bytes,
            "data": base64::engine::general_purpose::STANDARD.encode([0xff_u8; 64]),
        });
        let mut mismatched = valid.clone();
        mismatched["jsonBytes"] = json!(json_bytes + 1);
        let mut shorter = valid;
        shorter["jsonBytes"] = json!(json_bytes - 1);

        for value in [not_base64, not_deflate, mismatched, shorter] {
            let error = decode(value.clone()).expect_err("corrupt snapshot");
            assert!(
                matches!(error, SplitProjectError::Json { .. }),
                "{value}: {error:?}"
            );
        }
    }

    #[test]
    fn the_budget_drops_oldest_entries_but_keeps_the_newest() {
        let mut large = (0..3)
            .map(|index| entry(&format!("large-{index}"), &incompressible_project(index)))
            .collect::<Vec<_>>();
        assert!(large[2].before.stored_bytes() > 1024);

        retain_agent_edit_history_budget(&mut large, 1024);

        assert_eq!(
            large
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["large-2"]
        );

        let project = sample_project();
        let mut small = (0..25)
            .map(|index| entry(&format!("small-{index}"), &project))
            .collect::<Vec<_>>();

        retain_agent_edit_history_budget(&mut small, MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES);

        assert_eq!(small.len(), MAX_AGENT_EDIT_HISTORY_ENTRIES);
        assert_eq!(small[0].id, "small-5");
        assert_eq!(small[19].id, "small-24");
    }
}
