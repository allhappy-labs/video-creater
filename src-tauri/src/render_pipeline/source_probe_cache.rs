use super::probe::MediaProbe;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq)]
pub enum SourceProbeCacheLookup {
    Hit(MediaProbe),
    Miss(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SourceProbeCacheRecord {
    schema_version: u32,
    source_path: String,
    source_size_bytes: u64,
    source_modified_unix_ms: u128,
    probe: MediaProbe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceProbeSignature {
    path: String,
    size_bytes: u64,
    modified_unix_ms: u128,
}

pub fn validate_source_probe_cache_hit(
    cache_path: &Path,
    source_path: &Path,
) -> Result<SourceProbeCacheLookup, std::io::Error> {
    if !cache_path.is_file() {
        return Ok(SourceProbeCacheLookup::Miss("metadata missing".to_string()));
    }

    let cache_json = std::fs::read_to_string(cache_path)?;
    let record: SourceProbeCacheRecord = match serde_json::from_str(&cache_json) {
        Ok(record) => record,
        Err(_) => {
            return Ok(SourceProbeCacheLookup::Miss(
                "metadata unreadable".to_string(),
            ));
        }
    };
    if record.schema_version != 1 {
        return Ok(SourceProbeCacheLookup::Miss(
            "metadata schema mismatch".to_string(),
        ));
    }

    let signature = source_probe_signature(source_path)?;
    if record.source_path != signature.path
        || record.source_size_bytes != signature.size_bytes
        || record.source_modified_unix_ms != signature.modified_unix_ms
    {
        return Ok(SourceProbeCacheLookup::Miss(
            "source signature mismatch".to_string(),
        ));
    }

    Ok(SourceProbeCacheLookup::Hit(record.probe))
}

pub fn write_source_probe_cache(
    cache_path: &Path,
    source_path: &Path,
    probe: &MediaProbe,
) -> Result<(), std::io::Error> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let signature = source_probe_signature(source_path)?;
    let record = SourceProbeCacheRecord {
        schema_version: 1,
        source_path: signature.path,
        source_size_bytes: signature.size_bytes,
        source_modified_unix_ms: signature.modified_unix_ms,
        probe: probe.clone(),
    };
    let json = serde_json::to_string_pretty(&record).map_err(std::io::Error::other)?;
    std::fs::write(cache_path, json)
}

fn source_probe_signature(source_path: &Path) -> Result<SourceProbeSignature, std::io::Error> {
    let metadata = std::fs::metadata(source_path)?;
    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    Ok(SourceProbeSignature {
        path: source_path.display().to_string(),
        size_bytes: metadata.len(),
        modified_unix_ms,
    })
}
