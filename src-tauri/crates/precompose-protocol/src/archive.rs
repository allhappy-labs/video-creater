//! ZIP safety shared by the isolated renderer and native media import.
use crate::{WorkerBudgets, WorkerErrorCode};
use std::io::{Cursor, Read};
use zip::ZipArchive;

#[derive(Debug)]
pub struct ArchiveFailure {
    pub code: WorkerErrorCode,
    pub message: String,
    pub field: Option<String>,
}
impl ArchiveFailure {
    fn new(code: WorkerErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
        }
    }
    fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}
impl std::fmt::Display for ArchiveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ArchiveFailure {}

pub fn validate_dotlottie_archive(
    source: &[u8],
    animation_id: Option<&str>,
    budgets: &WorkerBudgets,
) -> Result<Vec<String>, ArchiveFailure> {
    if source.len() as u64 > budgets.max_source_bytes {
        return Err(ArchiveFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "dotLottie source exceeds maxSourceBytes",
        ));
    }
    let mut archive = ZipArchive::new(Cursor::new(source)).map_err(|error| {
        ArchiveFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid dotLottie ZIP: {error}"),
        )
    })?;
    if archive.len() > budgets.max_archive_entries as usize {
        return Err(ArchiveFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "dotLottie archive exceeds maxArchiveEntries",
        ));
    }
    let mut expanded_bytes = 0_u64;
    let mut archive_animation_ids = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            ArchiveFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unable to inspect dotLottie entry: {error}"),
            )
        })?;
        if entry.enclosed_name().is_none() {
            return Err(ArchiveFailure::new(
                WorkerErrorCode::SourceInvalid,
                "dotLottie archive contains an unsafe path",
            ));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(ArchiveFailure::new(
                WorkerErrorCode::SourceInvalid,
                "dotLottie archive contains a symlink",
            ));
        }
        expanded_bytes = expanded_bytes.checked_add(entry.size()).ok_or_else(|| {
            ArchiveFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "expanded archive size overflow",
            )
        })?;
        if expanded_bytes > budgets.max_expanded_archive_bytes {
            return Err(ArchiveFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "dotLottie archive exceeds maxExpandedArchiveBytes",
            ));
        }
        if entry.size() > 0
            && (entry.compressed_size() == 0
                || entry.size()
                    > entry
                        .compressed_size()
                        .saturating_mul(u64::from(budgets.max_compression_ratio)))
        {
            return Err(ArchiveFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "dotLottie entry exceeds maxCompressionRatio",
            ));
        }
        if entry.name().ends_with(".json") {
            let mut json_bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .by_ref()
                .take(budgets.max_expanded_archive_bytes.saturating_add(1))
                .read_to_end(&mut json_bytes)
                .map_err(|error| {
                    ArchiveFailure::new(
                        WorkerErrorCode::SourceInvalid,
                        format!("unable to validate dotLottie JSON entry: {error}"),
                    )
                })?;
            if json_bytes.len() as u64 > entry.size() {
                return Err(ArchiveFailure::new(
                    WorkerErrorCode::BudgetExceeded,
                    "dotLottie entry exceeds declared expanded size",
                ));
            }
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&json_bytes) {
                reject_external_assets(&json)?;
                if entry.name() == "manifest.json" {
                    let ids = json
                        .get("animations")
                        .and_then(serde_json::Value::as_array)
                        .map(|animations| {
                            animations
                                .iter()
                                .filter_map(|animation| {
                                    animation
                                        .get("id")
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::to_string)
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    archive_animation_ids = Some(ids);
                }
            }
        }
    }
    let animation_ids = archive_animation_ids.ok_or_else(|| {
        ArchiveFailure::new(
            WorkerErrorCode::SourceInvalid,
            "dotLottie archive is missing a valid manifest.json",
        )
    })?;
    if animation_ids.len() > 1 && animation_id.is_none() {
        return Err(ArchiveFailure::new(
            WorkerErrorCode::SourceInvalid,
            "dotLottie archives with multiple animations require an explicit animationId",
        )
        .field("source.animationId"));
    }
    if let Some(selected) = animation_id {
        if !animation_ids
            .iter()
            .any(|animation_id| animation_id == selected)
        {
            return Err(ArchiveFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("dotLottie animationId `{selected}` is not present in manifest.json"),
            )
            .field("source.animationId"));
        }
    }
    Ok(animation_ids)
}

fn reject_external_assets(json: &serde_json::Value) -> Result<(), ArchiveFailure> {
    if let Some(assets) = json.get("assets").and_then(serde_json::Value::as_array) {
        for asset in assets {
            if let Some(path) = asset.get("p").and_then(serde_json::Value::as_str) {
                if !path.is_empty() && !path.starts_with("data:") {
                    return Err(ArchiveFailure::new(
                        WorkerErrorCode::SourceInvalid,
                        "external Lottie assets are forbidden; inline them as data URIs",
                    )
                    .field("source.path"));
                }
            }
        }
    }
    Ok(())
}
