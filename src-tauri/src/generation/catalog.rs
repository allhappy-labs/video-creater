use crate::generation::download::{read_response_bounded, CATALOG_RESPONSE_LIMIT};
use crate::generation::elevenlabs::{
    ELEVENLABS_MUSIC_MODEL_ID, ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID,
};
use crate::generation::fal::{
    FAL_AURA_SR_MODEL_ID, FAL_FLUX_SCHNELL_MODEL_ID, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
    FAL_KREA_2_TURBO_MODEL_ID, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
    FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
    FAL_SEED_AUDIO_MODEL_ID, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
    FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use crate::generation::google::{
    GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_LYRIA_3_PRO_MODEL_ID, GOOGLE_PROVIDER,
    GOOGLE_VEO_31_FAST_MODEL_ID,
};
use crate::generation::minimax::{MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER};
use crate::generation::openai::{
    OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
    OPENAI_PROVIDER,
};
use crate::generation::replicate::{
    REPLICATE_FLUX_11_PRO_MODEL_ID, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
    REPLICATE_SEEDANCE_20_FAST_MODEL_ID, REPLICATE_SEEDANCE_20_MODEL_ID,
};
use crate::generation::xai::{
    XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const EXECUTION_ADAPTER_CONTRACT_VERSION: u32 = 1;
pub const CATALOG_URL_ENV: &str = "VIDEO_CREATER_GENERATION_CATALOG_URL";
pub const CATALOG_CACHE_ENV: &str = "VIDEO_CREATER_GENERATION_CATALOG_CACHE";
pub const CATALOG_SIGNING_KEY_ENV: &str = "VIDEO_CREATER_GENERATION_CATALOG_SIGNING_KEY";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationCatalogDocument {
    pub schema_version: u32,
    pub execution_adapter_contract_version: u32,
    pub catalog_version: String,
    pub capabilities_version: String,
    pub generated_at: String,
    pub expires_at: String,
    pub models: Vec<Value>,
    #[serde(default)]
    pub disabled_model_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationExecutionAdapter {
    pub id: &'static str,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationCatalogCacheEnvelope {
    pub schema_version: u32,
    pub document: GenerationCatalogDocument,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenerationCatalogProvenance {
    pub source: String,
    pub catalog_version: String,
    pub capabilities_version: String,
    pub stale: bool,
    pub cache_status: String,
    pub hash_verified: bool,
    pub signature_verified: bool,
    pub remote_configured: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGenerationCatalog {
    pub models: Vec<Value>,
    pub provenance: GenerationCatalogProvenance,
}

#[derive(Debug, Clone)]
pub struct GenerationCatalogConfig {
    pub remote_url: Option<String>,
    pub cache_path: PathBuf,
    pub signing_key: Option<String>,
}

impl GenerationCatalogConfig {
    pub fn from_env() -> Self {
        Self {
            remote_url: nonblank_env(CATALOG_URL_ENV),
            cache_path: nonblank_env(CATALOG_CACHE_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(default_cache_path),
            signing_key: nonblank_env(CATALOG_SIGNING_KEY_ENV),
        }
    }
}

pub fn resolve_generation_catalog(builtin_models: Vec<Value>) -> ResolvedGenerationCatalog {
    let config = GenerationCatalogConfig::from_env();
    resolve_generation_catalog_with(builtin_models, &config, Utc::now(), |url| {
        fetch_remote_catalog(url)
    })
}

pub fn resolve_generation_catalog_with(
    builtin_models: Vec<Value>,
    config: &GenerationCatalogConfig,
    now: DateTime<Utc>,
    fetch: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> ResolvedGenerationCatalog {
    let remote_configured = config.remote_url.is_some();
    if let (Some(url), Some(_)) = (&config.remote_url, &config.signing_key) {
        if let Ok(bytes) = fetch(url) {
            if let Ok((document, signature_verified)) =
                decode_verified_envelope(&bytes, config.signing_key.as_deref())
            {
                if let Ok(models) = validated_enabled_models(&document) {
                    let envelope: GenerationCatalogCacheEnvelope =
                        serde_json::from_slice(&bytes).expect("verified envelope decodes");
                    let _ = write_cache_atomically(&config.cache_path, &envelope);
                    return resolved(
                        models,
                        &document,
                        CatalogResolutionStatus {
                            source: "remote",
                            cache_status: "refreshed",
                            stale: is_stale(&document, now),
                            hash_verified: true,
                            signature_verified,
                            remote_configured: true,
                        },
                    );
                }
            }
        }
    }

    if !remote_configured || config.signing_key.is_some() {
        if let Ok(bytes) = fs::read(&config.cache_path) {
            if let Ok((document, signature_verified)) =
                decode_verified_envelope(&bytes, config.signing_key.as_deref())
            {
                if let Ok(models) = validated_enabled_models(&document) {
                    let stale = is_stale(&document, now);
                    return resolved(
                        models,
                        &document,
                        CatalogResolutionStatus {
                            source: "cache",
                            cache_status: if stale { "stale" } else { "fresh" },
                            stale,
                            hash_verified: true,
                            signature_verified,
                            remote_configured,
                        },
                    );
                }
            }
        }
    }

    ResolvedGenerationCatalog {
        models: builtin_models,
        provenance: GenerationCatalogProvenance {
            source: "builtin".to_string(),
            catalog_version: "builtin-v1".to_string(),
            capabilities_version: "builtin-v1".to_string(),
            stale: false,
            cache_status: if config.cache_path.exists() {
                "corrupt".to_string()
            } else {
                "missing".to_string()
            },
            hash_verified: false,
            signature_verified: false,
            remote_configured,
        },
    }
}

pub fn signed_catalog_envelope(
    document: GenerationCatalogDocument,
    signing_key: Option<&str>,
) -> GenerationCatalogCacheEnvelope {
    let document_bytes = serde_json::to_vec(&document).expect("catalog document serialization");
    let sha256 = hex_digest(&document_bytes);
    let signature = signing_key.map(|key| hmac_sha256_hex(key.as_bytes(), sha256.as_bytes()));
    GenerationCatalogCacheEnvelope {
        schema_version: CATALOG_SCHEMA_VERSION,
        document,
        sha256,
        signature,
    }
}

fn decode_verified_envelope(
    bytes: &[u8],
    signing_key: Option<&str>,
) -> Result<(GenerationCatalogDocument, bool), String> {
    let envelope: GenerationCatalogCacheEnvelope =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if envelope.schema_version != CATALOG_SCHEMA_VERSION
        || envelope.document.schema_version != CATALOG_SCHEMA_VERSION
    {
        return Err("unsupported catalog schema version".to_string());
    }
    let document_bytes =
        serde_json::to_vec(&envelope.document).map_err(|error| error.to_string())?;
    let expected_hash = hex_digest(&document_bytes);
    if !constant_time_eq(expected_hash.as_bytes(), envelope.sha256.as_bytes()) {
        return Err("catalog hash mismatch".to_string());
    }
    let signature_verified = if let Some(key) = signing_key {
        let expected = hmac_sha256_hex(key.as_bytes(), envelope.sha256.as_bytes());
        let actual = envelope
            .signature
            .as_deref()
            .ok_or_else(|| "catalog signature is missing".to_string())?;
        if !constant_time_eq(expected.as_bytes(), actual.as_bytes()) {
            return Err("catalog signature mismatch".to_string());
        }
        true
    } else {
        false
    };
    Ok((envelope.document, signature_verified))
}

/// Returns the exact packaged adapter contract for a model that this native
/// binary can execute. Remote catalog rows are configuration, not executable
/// code, so accepting a row outside this registry would expose a model that
/// the provider workflow can never dispatch.
pub fn packaged_generation_execution_adapter(
    provider: &str,
    model_id: &str,
    kind: &str,
) -> Option<GenerationExecutionAdapter> {
    let adapter_id = match (provider, model_id, kind) {
        (
            FAL_PROVIDER,
            FAL_WAN_TEXT_TO_VIDEO_MODEL_ID
            | FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
            | FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID
            | FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
            | FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID
            | FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID
            | FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
            "video",
        )
        | (
            FAL_PROVIDER,
            FAL_FLUX_SCHNELL_MODEL_ID
            | FAL_KREA_2_TURBO_MODEL_ID
            | FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID
            | FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
            "image",
        )
        | (
            FAL_PROVIDER,
            FAL_SEED_AUDIO_MODEL_ID
            | FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID
            | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
            | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            "audio",
        )
        | (FAL_PROVIDER, FAL_AURA_SR_MODEL_ID | FAL_VIDEO_UPSCALER_MODEL_ID, "upscale") => {
            "video-creater.fal-queue"
        }
        (
            REPLICATE_PROVIDER,
            REPLICATE_FLUX_SCHNELL_MODEL_ID
            | REPLICATE_FLUX_DEV_MODEL_ID
            | REPLICATE_FLUX_11_PRO_MODEL_ID
            | REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
            "image",
        )
        | (
            REPLICATE_PROVIDER,
            REPLICATE_SEEDANCE_20_MODEL_ID | REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
            "video",
        ) => "video-creater.replicate-predictions",
        (
            OPENAI_PROVIDER,
            OPENAI_GPT_IMAGE_2_MODEL_ID | OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
            "image",
        )
        | (OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID, "audio") => {
            "video-creater.openai-media"
        }
        (XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID, "image")
        | (XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID, "video") => "video-creater.xai-media",
        (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID, "video")
        | (GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID | GOOGLE_LYRIA_3_PRO_MODEL_ID, "audio") => {
            "video-creater.google-generative-media"
        }
        (ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID | ELEVENLABS_MUSIC_MODEL_ID, "audio") => {
            "video-creater.elevenlabs-media"
        }
        (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID, "audio") => "video-creater.minimax-music",
        ("mock", "mock-upscale-v1", "upscale") => "video-creater.mock-local",
        _ => return None,
    };
    Some(GenerationExecutionAdapter {
        id: adapter_id,
        version: 1,
    })
}

fn validate_declared_execution_adapter(
    model: &Value,
    expected: GenerationExecutionAdapter,
) -> Result<(), String> {
    let declared = model
        .get("executionAdapter")
        .and_then(Value::as_object)
        .ok_or_else(|| "catalog model executionAdapter is missing or invalid".to_string())?;
    if declared.len() != 2 || !declared.contains_key("id") || !declared.contains_key("version") {
        return Err("catalog model executionAdapter has invalid fields".to_string());
    }
    let id = declared
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let version = declared
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok());
    if id != expected.id || version != Some(expected.version) {
        return Err("catalog model executionAdapter is not packaged by this build".to_string());
    }
    Ok(())
}

fn validated_enabled_models(document: &GenerationCatalogDocument) -> Result<Vec<Value>, String> {
    if document.execution_adapter_contract_version != EXECUTION_ADAPTER_CONTRACT_VERSION
        || document.catalog_version.trim().is_empty()
        || document.capabilities_version.trim().is_empty()
        || DateTime::parse_from_rfc3339(&document.generated_at).is_err()
        || DateTime::parse_from_rfc3339(&document.expires_at).is_err()
    {
        return Err("catalog metadata is invalid".to_string());
    }
    let disabled = document
        .disabled_model_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    let mut identities = HashSet::new();
    let mut models = Vec::new();
    for model in &document.models {
        let provider = model
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let id = model.get("id").and_then(Value::as_str).unwrap_or("").trim();
        let kind = model
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if provider.is_empty()
            || id.is_empty()
            || !matches!(kind, "video" | "image" | "audio" | "upscale")
            || !identities.insert(format!("{provider}:{id}"))
        {
            return Err("catalog contains an invalid or duplicate model".to_string());
        }
        let identity = format!("{provider}:{id}");
        if disabled.contains(id) || disabled.contains(identity.as_str()) {
            continue;
        }
        let packaged_adapter = packaged_generation_execution_adapter(provider, id, kind)
            .ok_or_else(|| "catalog model has no packaged execution adapter".to_string())?;
        validate_declared_execution_adapter(model, packaged_adapter)?;
        models.push(model.clone());
    }
    if models.is_empty() {
        return Err("catalog contains no enabled models".to_string());
    }
    Ok(models)
}

struct CatalogResolutionStatus<'a> {
    source: &'a str,
    cache_status: &'a str,
    stale: bool,
    hash_verified: bool,
    signature_verified: bool,
    remote_configured: bool,
}

fn resolved(
    models: Vec<Value>,
    document: &GenerationCatalogDocument,
    status: CatalogResolutionStatus<'_>,
) -> ResolvedGenerationCatalog {
    ResolvedGenerationCatalog {
        models,
        provenance: GenerationCatalogProvenance {
            source: status.source.to_string(),
            catalog_version: document.catalog_version.clone(),
            capabilities_version: document.capabilities_version.clone(),
            stale: status.stale,
            cache_status: status.cache_status.to_string(),
            hash_verified: status.hash_verified,
            signature_verified: status.signature_verified,
            remote_configured: status.remote_configured,
        },
    }
}

fn is_stale(document: &GenerationCatalogDocument, now: DateTime<Utc>) -> bool {
    DateTime::parse_from_rfc3339(&document.expires_at)
        .map(|expires| expires.with_timezone(&Utc) <= now)
        .unwrap_or(true)
}

fn fetch_remote_catalog(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .send()
        .map_err(|error| error.to_string())?;
    read_response_bounded(response, CATALOG_RESPONSE_LIMIT, || false)
        .map_err(|error| error.to_string())
}

fn write_cache_atomically(
    path: &Path,
    envelope: &GenerationCatalogCacheEnvelope,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "cache path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(".generation-catalog-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &temp,
        serde_json::to_vec_pretty(envelope).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

fn default_cache_path() -> PathBuf {
    if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(root).join("video-creater/generation-catalog-v1.json");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Caches/video-creater/generation-catalog-v1.json")
}

fn nonblank_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = if key.len() > BLOCK {
        Sha256::digest(key).to_vec()
    } else {
        key.to_vec()
    };
    normalized.resize(BLOCK, 0);
    let mut inner = vec![0x36; BLOCK];
    let mut outer = vec![0x5c; BLOCK];
    for (index, byte) in normalized.iter().enumerate() {
        inner[index] ^= byte;
        outer[index] ^= byte;
    }
    inner.extend_from_slice(message);
    outer.extend_from_slice(&Sha256::digest(&inner));
    hex_digest(&outer)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn document(expires_at: &str) -> GenerationCatalogDocument {
        GenerationCatalogDocument {
            schema_version: 1,
            execution_adapter_contract_version: EXECUTION_ADAPTER_CONTRACT_VERSION,
            catalog_version: "remote-7".to_string(),
            capabilities_version: "caps-3".to_string(),
            generated_at: "2026-07-10T00:00:00Z".to_string(),
            expires_at: expires_at.to_string(),
            models: vec![
                json!({
                    "provider": FAL_PROVIDER,
                    "id": FAL_FLUX_SCHNELL_MODEL_ID,
                    "kind": "image",
                    "executionAdapter": {
                        "id": "video-creater.fal-queue",
                        "version": 1
                    }
                }),
                json!({
                    "provider": FAL_PROVIDER,
                    "id": FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
                    "kind": "video",
                    "executionAdapter": {
                        "id": "video-creater.fal-queue",
                        "version": 1
                    }
                }),
            ],
            disabled_model_ids: vec![format!("{FAL_PROVIDER}:{FAL_WAN_TEXT_TO_VIDEO_MODEL_ID}")],
        }
    }

    fn config(dir: &tempfile::TempDir) -> GenerationCatalogConfig {
        GenerationCatalogConfig {
            remote_url: Some("https://catalog.invalid/models.json".to_string()),
            cache_path: dir.path().join("catalog.json"),
            signing_key: Some("fixture-signing-key".to_string()),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 0, 0, 0).unwrap()
    }

    #[test]
    fn fresh_remote_catalog_is_verified_cached_and_filters_disabled_models() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let bytes = serde_json::to_vec(&signed_catalog_envelope(
            document("2026-07-12T00:00:00Z"),
            config.signing_key.as_deref(),
        ))
        .unwrap();
        let resolved =
            resolve_generation_catalog_with(vec![json!({"id":"builtin"})], &config, now(), |_| {
                Ok(bytes)
            });
        assert_eq!(resolved.provenance.source, "remote");
        assert!(resolved.provenance.hash_verified && resolved.provenance.signature_verified);
        assert_eq!(resolved.models.len(), 1);
        assert_eq!(resolved.models[0]["id"], FAL_FLUX_SCHNELL_MODEL_ID);
        assert!(config.cache_path.is_file());
    }

    #[test]
    fn stale_verified_cache_is_retained_with_stale_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let envelope = signed_catalog_envelope(
            document("2026-07-10T00:00:00Z"),
            config.signing_key.as_deref(),
        );
        fs::write(&config.cache_path, serde_json::to_vec(&envelope).unwrap()).unwrap();
        let resolved =
            resolve_generation_catalog_with(vec![], &config, now(), |_| Err("offline".to_string()));
        assert_eq!(resolved.provenance.source, "cache");
        assert!(resolved.provenance.stale);
        assert_eq!(resolved.provenance.cache_status, "stale");
    }

    #[test]
    fn disabled_models_are_removed_from_verified_catalogs() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let bytes = serde_json::to_vec(&signed_catalog_envelope(
            document("2026-07-12T00:00:00Z"),
            config.signing_key.as_deref(),
        ))
        .unwrap();
        let resolved = resolve_generation_catalog_with(vec![], &config, now(), |_| Ok(bytes));
        assert_eq!(
            resolved
                .models
                .iter()
                .map(|model| model["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![FAL_FLUX_SCHNELL_MODEL_ID]
        );
    }

    #[test]
    fn disabled_unavailable_models_are_not_exposed_or_required_to_be_packaged() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let mut remote = document("2026-07-12T00:00:00Z");
        remote.models.push(json!({
            "provider": FAL_PROVIDER,
            "id": "fal-ai/future-disabled-model",
            "kind": "image"
        }));
        remote
            .disabled_model_ids
            .push(format!("{FAL_PROVIDER}:fal-ai/future-disabled-model"));
        let bytes = serde_json::to_vec(&signed_catalog_envelope(
            remote,
            config.signing_key.as_deref(),
        ))
        .unwrap();

        let resolved = resolve_generation_catalog_with(vec![], &config, now(), |_| Ok(bytes));

        assert_eq!(resolved.provenance.source, "remote");
        assert_eq!(resolved.models.len(), 1);
        assert_eq!(resolved.models[0]["id"], FAL_FLUX_SCHNELL_MODEL_ID);
    }

    #[test]
    fn signed_remote_catalog_accepts_an_exact_packaged_adapter_contract() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let bytes = serde_json::to_vec(&signed_catalog_envelope(
            document("2026-07-12T00:00:00Z"),
            config.signing_key.as_deref(),
        ))
        .unwrap();

        let resolved = resolve_generation_catalog_with(vec![], &config, now(), |_| Ok(bytes));

        assert_eq!(resolved.provenance.source, "remote");
        assert_eq!(resolved.models.len(), 1);
        assert_eq!(resolved.models[0]["id"], FAL_FLUX_SCHNELL_MODEL_ID);
        assert_eq!(
            resolved.models[0]["executionAdapter"],
            json!({"id":"video-creater.fal-queue","version":1})
        );
    }

    #[test]
    fn signed_remote_catalog_rejects_a_model_without_a_packaged_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let mut remote = document("2026-07-12T00:00:00Z");
        remote.models = vec![json!({
            "provider": FAL_PROVIDER,
            "id": "fal-ai/future-model-not-in-this-build",
            "kind": "image",
            "executionAdapter": {"id":"video-creater.fal-queue","version":1}
        })];
        remote.disabled_model_ids.clear();
        let bytes = serde_json::to_vec(&signed_catalog_envelope(
            remote,
            config.signing_key.as_deref(),
        ))
        .unwrap();
        let builtin = vec![json!({"provider":"mock","id":"builtin","kind":"image"})];

        let resolved =
            resolve_generation_catalog_with(builtin.clone(), &config, now(), |_| Ok(bytes));

        assert_eq!(resolved.models, builtin);
        assert_eq!(resolved.provenance.source, "builtin");
        assert!(!config.cache_path.exists());
    }

    #[test]
    fn signed_remote_catalog_rejects_an_adapter_identity_or_version_mismatch() {
        for execution_adapter in [
            json!({"id":"video-creater.replicate-predictions","version":1}),
            json!({"id":"video-creater.fal-queue","version":2}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let config = config(&dir);
            let mut remote = document("2026-07-12T00:00:00Z");
            remote.models[0]["executionAdapter"] = execution_adapter;
            let bytes = serde_json::to_vec(&signed_catalog_envelope(
                remote,
                config.signing_key.as_deref(),
            ))
            .unwrap();
            let builtin = vec![json!({"provider":"mock","id":"builtin","kind":"image"})];

            let resolved =
                resolve_generation_catalog_with(builtin.clone(), &config, now(), |_| Ok(bytes));

            assert_eq!(resolved.models, builtin);
            assert_eq!(resolved.provenance.source, "builtin");
            assert!(!config.cache_path.exists());
        }
    }

    #[test]
    fn signed_remote_catalog_rejects_an_unsupported_contract_version_or_model_kind() {
        let cases = [
            (EXECUTION_ADAPTER_CONTRACT_VERSION + 1, "image"),
            (EXECUTION_ADAPTER_CONTRACT_VERSION, "video"),
        ];
        for (contract_version, kind) in cases {
            let dir = tempfile::tempdir().unwrap();
            let config = config(&dir);
            let mut remote = document("2026-07-12T00:00:00Z");
            remote.execution_adapter_contract_version = contract_version;
            remote.models[0]["kind"] = json!(kind);
            let bytes = serde_json::to_vec(&signed_catalog_envelope(
                remote,
                config.signing_key.as_deref(),
            ))
            .unwrap();
            let builtin = vec![json!({"provider":"mock","id":"builtin","kind":"image"})];

            let resolved =
                resolve_generation_catalog_with(builtin.clone(), &config, now(), |_| Ok(bytes));

            assert_eq!(resolved.models, builtin);
            assert_eq!(resolved.provenance.source, "builtin");
            assert!(!config.cache_path.exists());
        }
    }

    #[test]
    fn corrupt_cache_falls_back_to_builtin_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        fs::write(&config.cache_path, b"{broken").unwrap();
        let builtin = vec![json!({"provider":"mock","id":"builtin","kind":"image"})];
        let resolved = resolve_generation_catalog_with(builtin.clone(), &config, now(), |_| {
            Err("offline".to_string())
        });
        assert_eq!(resolved.models, builtin);
        assert_eq!(resolved.provenance.source, "builtin");
        assert_eq!(resolved.provenance.cache_status, "corrupt");
    }

    #[test]
    fn missing_remote_and_cache_use_offline_builtin_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(&dir);
        config.remote_url = None;
        config.signing_key = None;
        let builtin = vec![json!({"provider":"mock","id":"builtin","kind":"image"})];
        let resolved = resolve_generation_catalog_with(builtin.clone(), &config, now(), |_| {
            panic!("network must not run")
        });
        assert_eq!(resolved.models, builtin);
        assert_eq!(resolved.provenance.cache_status, "missing");
        assert!(!resolved.provenance.remote_configured);
    }
}
