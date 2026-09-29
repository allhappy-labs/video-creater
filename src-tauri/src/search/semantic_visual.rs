//! Local visual-embedding persistence and retrieval primitives.
//!
//! This module deliberately does not claim or download an encoder. Callers must
//! provide a licensed model identity plus embeddings; absent or stale indexes
//! remain explicitly `NotIndexed`, leaving the existing lexical search intact.

use super::safe_path_segment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const SEMANTIC_VISUAL_SIDECAR_SCHEMA_VERSION: u32 = 1;
pub const SEMANTIC_VISUAL_VECTOR_FORMAT: &str = "i16le-l2-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVisualModelSpec {
    pub id: String,
    pub version: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVisualFrameRow {
    pub time_seconds: f64,
    pub shot_start_seconds: f64,
    pub shot_end_seconds: f64,
    pub thumbnail_relative_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticVisualIndexInput {
    pub media_id: String,
    pub source_fingerprint: String,
    pub sampling_policy: String,
    pub model: SemanticVisualModelSpec,
    pub frames: Vec<SemanticVisualFrameRow>,
    pub vectors: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SemanticVisualVectorStore {
    relative_path: String,
    format: String,
    dimensions: usize,
    vector_count: usize,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SemanticVisualSidecar {
    schema_version: u32,
    status: String,
    media_id: String,
    source_fingerprint: String,
    sampling_policy: String,
    model: SemanticVisualModelSpec,
    index_fingerprint: String,
    vector_store: SemanticVisualVectorStore,
    frames: Vec<SemanticVisualFrameRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticVisualIndexExpectation {
    pub media_id: String,
    pub source_fingerprint: String,
    pub sampling_policy: String,
    pub model: SemanticVisualModelSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVisualHit {
    pub media_id: String,
    pub time_seconds: f64,
    pub shot_start_seconds: f64,
    pub shot_end_seconds: f64,
    pub score: f32,
    pub thumbnail_relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SemanticVisualAvailability {
    Ready,
    NotIndexed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVisualQueryResult {
    pub status: SemanticVisualAvailability,
    pub indexed_media_ids: Vec<String>,
    pub unavailable: Vec<SemanticVisualUnavailableIndex>,
    pub hits: Vec<SemanticVisualHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVisualUnavailableIndex {
    pub media_id: String,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum SemanticVisualError {
    #[error("semantic visual index is invalid: {0}")]
    Invalid(String),
    #[error("semantic visual index IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("semantic visual index JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

struct LoadedSemanticVisualIndex {
    sidecar: SemanticVisualSidecar,
    vectors: Vec<Vec<f32>>,
}

pub fn load_semantic_visual_index_input(
    project_dir: &Path,
    expectation: &SemanticVisualIndexExpectation,
) -> Result<SemanticVisualIndexInput, SemanticVisualError> {
    let loaded = load_current_semantic_visual_index(project_dir, expectation)?;
    Ok(SemanticVisualIndexInput {
        media_id: loaded.sidecar.media_id,
        source_fingerprint: loaded.sidecar.source_fingerprint,
        sampling_policy: loaded.sidecar.sampling_policy,
        model: loaded.sidecar.model,
        frames: loaded.sidecar.frames,
        vectors: loaded.vectors,
    })
}

pub fn write_semantic_visual_index(
    project_dir: &Path,
    input: &SemanticVisualIndexInput,
) -> Result<PathBuf, SemanticVisualError> {
    validate_index_input(input)?;
    let normalized = input
        .vectors
        .iter()
        .map(|vector| normalized_vector(vector))
        .collect::<Result<Vec<_>, _>>()?;
    let vector_bytes = encode_vectors(&normalized);
    let vector_sha256 = sha256_bytes(&vector_bytes);
    let media_dir = semantic_media_dir(project_dir, &input.media_id);
    fs::create_dir_all(&media_dir)?;
    let vector_path = media_dir.join("vectors.i16");
    write_bytes_atomically(&vector_path, &vector_bytes)?;

    let relative_vector_path = format!(
        "search/visual/{}/semantic/vectors.i16",
        safe_path_segment(&input.media_id)
    );
    let index_fingerprint = semantic_index_fingerprint(
        &input.source_fingerprint,
        &input.sampling_policy,
        &input.model,
        &input.frames,
    )?;
    let sidecar = SemanticVisualSidecar {
        schema_version: SEMANTIC_VISUAL_SIDECAR_SCHEMA_VERSION,
        status: "ready".to_string(),
        media_id: input.media_id.clone(),
        source_fingerprint: input.source_fingerprint.clone(),
        sampling_policy: input.sampling_policy.clone(),
        model: input.model.clone(),
        index_fingerprint,
        vector_store: SemanticVisualVectorStore {
            relative_path: relative_vector_path,
            format: SEMANTIC_VISUAL_VECTOR_FORMAT.to_string(),
            dimensions: input.model.dimensions,
            vector_count: input.vectors.len(),
            sha256: vector_sha256,
        },
        frames: input.frames.clone(),
    };
    let sidecar_path = media_dir.join("embeddings.json");
    write_json_atomically(&sidecar_path, &sidecar)?;
    Ok(sidecar_path)
}

pub fn query_semantic_visual_indexes(
    project_dir: &Path,
    expectations: &[SemanticVisualIndexExpectation],
    query_embedding: &[f32],
    limit: usize,
    minimum_score: Option<f32>,
) -> SemanticVisualQueryResult {
    let query = match normalized_vector(query_embedding) {
        Ok(query) => query,
        Err(error) => {
            return SemanticVisualQueryResult {
                status: SemanticVisualAvailability::NotIndexed,
                indexed_media_ids: Vec::new(),
                unavailable: expectations
                    .iter()
                    .map(|expectation| SemanticVisualUnavailableIndex {
                        media_id: expectation.media_id.clone(),
                        reason: error.to_string(),
                    })
                    .collect(),
                hits: Vec::new(),
            };
        }
    };

    let mut loaded = Vec::new();
    let mut unavailable = Vec::new();
    for expectation in expectations {
        match load_current_semantic_visual_index(project_dir, expectation) {
            Ok(index) if query.len() == expectation.model.dimensions => loaded.push(index),
            Ok(_) => unavailable.push(SemanticVisualUnavailableIndex {
                media_id: expectation.media_id.clone(),
                reason: "query embedding dimensions do not match the configured model".to_string(),
            }),
            Err(error) => unavailable.push(SemanticVisualUnavailableIndex {
                media_id: expectation.media_id.clone(),
                reason: error.to_string(),
            }),
        }
    }

    let indexed_media_ids = loaded
        .iter()
        .map(|index| index.sidecar.media_id.clone())
        .collect::<Vec<_>>();
    let mut best_per_shot = BTreeMap::<(String, u64, u64), SemanticVisualHit>::new();
    for index in loaded {
        for (frame, vector) in index.sidecar.frames.iter().zip(index.vectors.iter()) {
            let score = dot_product(&query, vector);
            if minimum_score.is_some_and(|minimum| score < minimum) {
                continue;
            }
            let hit = SemanticVisualHit {
                media_id: index.sidecar.media_id.clone(),
                time_seconds: frame.time_seconds,
                shot_start_seconds: frame.shot_start_seconds,
                shot_end_seconds: frame.shot_end_seconds,
                score,
                thumbnail_relative_path: frame.thumbnail_relative_path.clone(),
            };
            let key = (
                hit.media_id.clone(),
                hit.shot_start_seconds.to_bits(),
                hit.shot_end_seconds.to_bits(),
            );
            let replace = best_per_shot
                .get(&key)
                .is_none_or(|existing| compare_hits(&hit, existing) == Ordering::Less);
            if replace {
                best_per_shot.insert(key, hit);
            }
        }
    }
    let mut hits = best_per_shot.into_values().collect::<Vec<_>>();
    hits.sort_by(compare_hits);
    hits.truncate(limit.clamp(1, 50));

    SemanticVisualQueryResult {
        status: if indexed_media_ids.is_empty() {
            SemanticVisualAvailability::NotIndexed
        } else {
            SemanticVisualAvailability::Ready
        },
        indexed_media_ids,
        unavailable,
        hits,
    }
}

fn load_current_semantic_visual_index(
    project_dir: &Path,
    expectation: &SemanticVisualIndexExpectation,
) -> Result<LoadedSemanticVisualIndex, SemanticVisualError> {
    validate_expectation(expectation)?;
    let sidecar_path =
        semantic_media_dir(project_dir, &expectation.media_id).join("embeddings.json");
    let sidecar_source = fs::read_to_string(&sidecar_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SemanticVisualError::Invalid(
                "not indexed; configure a licensed local encoder and build embeddings".to_string(),
            )
        } else {
            SemanticVisualError::Io(error)
        }
    })?;
    let sidecar: SemanticVisualSidecar = serde_json::from_str(&sidecar_source)?;
    validate_sidecar(&sidecar, expectation)?;
    let vector_path =
        resolve_project_relative_path(project_dir, &sidecar.vector_store.relative_path)?;
    let vector_bytes = fs::read(vector_path)?;
    if sha256_bytes(&vector_bytes) != sidecar.vector_store.sha256 {
        return Err(SemanticVisualError::Invalid(
            "vector checksum does not match the sidecar".to_string(),
        ));
    }
    let vectors = decode_vectors(
        &vector_bytes,
        sidecar.vector_store.vector_count,
        sidecar.vector_store.dimensions,
    )?;
    Ok(LoadedSemanticVisualIndex { sidecar, vectors })
}

fn validate_index_input(input: &SemanticVisualIndexInput) -> Result<(), SemanticVisualError> {
    validate_identity_fields(
        &input.media_id,
        &input.source_fingerprint,
        &input.sampling_policy,
        &input.model,
    )?;
    if input.frames.is_empty() || input.frames.len() != input.vectors.len() {
        return Err(SemanticVisualError::Invalid(
            "frame and vector counts must match and be non-zero".to_string(),
        ));
    }
    for frame in &input.frames {
        validate_frame(frame)?;
    }
    for vector in &input.vectors {
        if vector.len() != input.model.dimensions {
            return Err(SemanticVisualError::Invalid(
                "vector dimensions do not match the model spec".to_string(),
            ));
        }
        normalized_vector(vector)?;
    }
    Ok(())
}

fn validate_expectation(
    expectation: &SemanticVisualIndexExpectation,
) -> Result<(), SemanticVisualError> {
    validate_identity_fields(
        &expectation.media_id,
        &expectation.source_fingerprint,
        &expectation.sampling_policy,
        &expectation.model,
    )
}

fn validate_identity_fields(
    media_id: &str,
    source_fingerprint: &str,
    sampling_policy: &str,
    model: &SemanticVisualModelSpec,
) -> Result<(), SemanticVisualError> {
    if media_id.trim().is_empty()
        || source_fingerprint.trim().is_empty()
        || sampling_policy.trim().is_empty()
        || model.id.trim().is_empty()
        || model.version.trim().is_empty()
    {
        return Err(SemanticVisualError::Invalid(
            "media, source, sampling, and model identity fields must not be blank".to_string(),
        ));
    }
    if model.dimensions == 0 || model.dimensions > 65_536 {
        return Err(SemanticVisualError::Invalid(
            "model dimensions must be between 1 and 65536".to_string(),
        ));
    }
    Ok(())
}

fn validate_frame(frame: &SemanticVisualFrameRow) -> Result<(), SemanticVisualError> {
    if !frame.time_seconds.is_finite()
        || !frame.shot_start_seconds.is_finite()
        || !frame.shot_end_seconds.is_finite()
        || frame.shot_start_seconds < 0.0
        || frame.time_seconds < frame.shot_start_seconds
        || frame.time_seconds > frame.shot_end_seconds
    {
        return Err(SemanticVisualError::Invalid(
            "frame timestamps must be finite and inside a non-negative shot range".to_string(),
        ));
    }
    validate_relative_path(&frame.thumbnail_relative_path)
}

fn validate_sidecar(
    sidecar: &SemanticVisualSidecar,
    expectation: &SemanticVisualIndexExpectation,
) -> Result<(), SemanticVisualError> {
    if sidecar.schema_version != SEMANTIC_VISUAL_SIDECAR_SCHEMA_VERSION
        || sidecar.status != "ready"
        || sidecar.media_id != expectation.media_id
        || sidecar.source_fingerprint != expectation.source_fingerprint
        || sidecar.sampling_policy != expectation.sampling_policy
        || sidecar.model != expectation.model
    {
        return Err(SemanticVisualError::Invalid(
            "embedding sidecar is stale for the expected source, model, or sampling policy"
                .to_string(),
        ));
    }
    if sidecar.vector_store.format != SEMANTIC_VISUAL_VECTOR_FORMAT
        || sidecar.vector_store.dimensions != sidecar.model.dimensions
        || sidecar.vector_store.vector_count != sidecar.frames.len()
        || sidecar.frames.is_empty()
    {
        return Err(SemanticVisualError::Invalid(
            "embedding vector-store metadata is inconsistent".to_string(),
        ));
    }
    validate_relative_path(&sidecar.vector_store.relative_path)?;
    for frame in &sidecar.frames {
        validate_frame(frame)?;
    }
    let expected_fingerprint = semantic_index_fingerprint(
        &sidecar.source_fingerprint,
        &sidecar.sampling_policy,
        &sidecar.model,
        &sidecar.frames,
    )?;
    if sidecar.index_fingerprint != expected_fingerprint {
        return Err(SemanticVisualError::Invalid(
            "embedding index fingerprint does not match its metadata".to_string(),
        ));
    }
    Ok(())
}

fn normalized_vector(vector: &[f32]) -> Result<Vec<f32>, SemanticVisualError> {
    if vector.is_empty() || vector.iter().any(|value| !value.is_finite()) {
        return Err(SemanticVisualError::Invalid(
            "embedding vectors must contain finite values".to_string(),
        ));
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err(SemanticVisualError::Invalid(
            "embedding vectors must have a non-zero norm".to_string(),
        ));
    }
    Ok(vector.iter().map(|value| value / norm).collect())
}

fn encode_vectors(vectors: &[Vec<f32>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vectors.iter().map(Vec::len).sum::<usize>() * 2);
    for vector in vectors {
        for value in vector {
            let quantized = (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            bytes.extend_from_slice(&quantized.to_le_bytes());
        }
    }
    bytes
}

fn decode_vectors(
    bytes: &[u8],
    vector_count: usize,
    dimensions: usize,
) -> Result<Vec<Vec<f32>>, SemanticVisualError> {
    let expected_len = vector_count
        .checked_mul(dimensions)
        .and_then(|values| values.checked_mul(2))
        .ok_or_else(|| SemanticVisualError::Invalid("vector-store size overflow".to_string()))?;
    if bytes.len() != expected_len {
        return Err(SemanticVisualError::Invalid(
            "vector-store byte length does not match its metadata".to_string(),
        ));
    }
    let mut vectors = Vec::with_capacity(vector_count);
    for vector_bytes in bytes.chunks_exact(dimensions * 2) {
        let vector = vector_bytes
            .chunks_exact(2)
            .map(|value| i16::from_le_bytes([value[0], value[1]]) as f32 / i16::MAX as f32)
            .collect::<Vec<_>>();
        vectors.push(normalized_vector(&vector)?);
    }
    Ok(vectors)
}

fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn compare_hits(left: &SemanticVisualHit, right: &SemanticVisualHit) -> Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| left.media_id.cmp(&right.media_id))
        .then_with(|| left.shot_start_seconds.total_cmp(&right.shot_start_seconds))
        .then_with(|| left.time_seconds.total_cmp(&right.time_seconds))
}

fn semantic_index_fingerprint(
    source_fingerprint: &str,
    sampling_policy: &str,
    model: &SemanticVisualModelSpec,
    frames: &[SemanticVisualFrameRow],
) -> Result<String, SemanticVisualError> {
    let metadata = serde_json::to_vec(&(source_fingerprint, sampling_policy, model, frames))?;
    Ok(sha256_bytes(&metadata))
}

fn semantic_media_dir(project_dir: &Path, media_id: &str) -> PathBuf {
    project_dir
        .join("search")
        .join("visual")
        .join(safe_path_segment(media_id))
        .join("semantic")
}

fn validate_relative_path(path: &str) -> Result<(), SemanticVisualError> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(SemanticVisualError::Invalid(
            "artifact references must be non-empty project-relative paths".to_string(),
        ));
    }
    Ok(())
}

fn resolve_project_relative_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, SemanticVisualError> {
    validate_relative_path(relative_path)?;
    Ok(project_dir.join(relative_path))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Result<(), SemanticVisualError> {
    let parent = path
        .parent()
        .ok_or_else(|| SemanticVisualError::Invalid("artifact path has no parent".to_string()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".vectors-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn write_json_atomically(
    path: &Path,
    sidecar: &SemanticVisualSidecar,
) -> Result<(), SemanticVisualError> {
    let parent = path
        .parent()
        .ok_or_else(|| SemanticVisualError::Invalid("sidecar path has no parent".to_string()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".embeddings-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, serde_json::to_vec_pretty(sidecar)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> SemanticVisualModelSpec {
        SemanticVisualModelSpec {
            id: "fixture-encoder".to_string(),
            version: "1".to_string(),
            dimensions: 3,
        }
    }

    fn input() -> SemanticVisualIndexInput {
        SemanticVisualIndexInput {
            media_id: "media-1".to_string(),
            source_fingerprint: "source-sha256".to_string(),
            sampling_policy: "shot-boundaries-v1".to_string(),
            model: model(),
            frames: vec![
                SemanticVisualFrameRow {
                    time_seconds: 0.4,
                    shot_start_seconds: 0.0,
                    shot_end_seconds: 2.0,
                    thumbnail_relative_path: "search/visual/media-1/frame-0.png".to_string(),
                },
                SemanticVisualFrameRow {
                    time_seconds: 1.4,
                    shot_start_seconds: 0.0,
                    shot_end_seconds: 2.0,
                    thumbnail_relative_path: "search/visual/media-1/frame-1.png".to_string(),
                },
                SemanticVisualFrameRow {
                    time_seconds: 3.0,
                    shot_start_seconds: 2.0,
                    shot_end_seconds: 4.0,
                    thumbnail_relative_path: "search/visual/media-1/frame-2.png".to_string(),
                },
            ],
            vectors: vec![
                vec![0.8, 0.2, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
            ],
        }
    }

    fn expectation() -> SemanticVisualIndexExpectation {
        SemanticVisualIndexExpectation {
            media_id: "media-1".to_string(),
            source_fingerprint: "source-sha256".to_string(),
            sampling_policy: "shot-boundaries-v1".to_string(),
            model: model(),
        }
    }

    #[test]
    fn compact_sidecar_round_trip_ranks_cosine_hits_and_deduplicates_each_shot() {
        let temporary = tempfile::tempdir().expect("project dir");
        let sidecar_path =
            write_semantic_visual_index(temporary.path(), &input()).expect("write semantic index");
        let vector_path = sidecar_path
            .parent()
            .expect("semantic dir")
            .join("vectors.i16");
        assert_eq!(
            fs::metadata(vector_path).expect("vectors metadata").len(),
            18
        );
        let sidecar: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&sidecar_path).expect("read sidecar"))
                .expect("parse sidecar");
        assert_eq!(
            sidecar["schemaVersion"],
            SEMANTIC_VISUAL_SIDECAR_SCHEMA_VERSION
        );
        assert_eq!(sidecar["sourceFingerprint"], "source-sha256");
        assert_eq!(sidecar["samplingPolicy"], "shot-boundaries-v1");
        assert_eq!(sidecar["model"]["id"], "fixture-encoder");
        assert_eq!(
            sidecar["vectorStore"]["format"],
            SEMANTIC_VISUAL_VECTOR_FORMAT
        );
        assert_eq!(sidecar["vectorStore"]["dimensions"], 3);
        assert_eq!(sidecar["vectorStore"]["vectorCount"], 3);
        assert_eq!(sidecar["indexFingerprint"].as_str().map(str::len), Some(64));

        let result = query_semantic_visual_indexes(
            temporary.path(),
            &[expectation()],
            &[1.0, 0.0, 0.0],
            10,
            None,
        );

        assert_eq!(result.status, SemanticVisualAvailability::Ready);
        assert_eq!(result.indexed_media_ids, vec!["media-1"]);
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.hits[0].media_id, "media-1");
        assert_eq!(result.hits[0].shot_start_seconds, 0.0);
        assert_eq!(result.hits[0].shot_end_seconds, 2.0);
        assert!((result.hits[0].score - 1.0).abs() < 0.001);
        assert_eq!(
            result.hits[0].thumbnail_relative_path,
            "search/visual/media-1/frame-1.png"
        );
        let payload = serde_json::to_value(&result).expect("serialize query result");
        assert_eq!(payload["hits"][0]["mediaId"], "media-1");
        assert_eq!(payload["hits"][0]["shotStartSeconds"], 0.0);
        assert_eq!(payload["hits"][0]["shotEndSeconds"], 2.0);
        assert_eq!(
            payload["hits"][0]["thumbnailRelativePath"],
            "search/visual/media-1/frame-1.png"
        );
    }

    #[test]
    fn stale_source_model_and_sampling_expectations_are_not_indexed() {
        let temporary = tempfile::tempdir().expect("project dir");
        write_semantic_visual_index(temporary.path(), &input()).expect("write semantic index");

        for stale in [
            SemanticVisualIndexExpectation {
                source_fingerprint: "changed-source".to_string(),
                ..expectation()
            },
            SemanticVisualIndexExpectation {
                sampling_policy: "changed-policy".to_string(),
                ..expectation()
            },
            SemanticVisualIndexExpectation {
                model: SemanticVisualModelSpec {
                    version: "2".to_string(),
                    ..model()
                },
                ..expectation()
            },
        ] {
            let result = query_semantic_visual_indexes(
                temporary.path(),
                &[stale],
                &[1.0, 0.0, 0.0],
                5,
                None,
            );
            assert_eq!(result.status, SemanticVisualAvailability::NotIndexed);
            assert!(result.hits.is_empty());
            assert!(result.unavailable[0].reason.contains("stale"));
        }
    }

    #[test]
    fn missing_or_corrupt_vectors_fail_closed_as_not_indexed() {
        let temporary = tempfile::tempdir().expect("project dir");
        let missing = query_semantic_visual_indexes(
            temporary.path(),
            &[expectation()],
            &[1.0, 0.0, 0.0],
            5,
            None,
        );
        assert_eq!(missing.status, SemanticVisualAvailability::NotIndexed);
        assert!(missing.unavailable[0]
            .reason
            .contains("configure a licensed local encoder"));

        let sidecar_path =
            write_semantic_visual_index(temporary.path(), &input()).expect("write semantic index");
        fs::write(
            sidecar_path
                .parent()
                .expect("semantic dir")
                .join("vectors.i16"),
            [0_u8, 1_u8],
        )
        .expect("corrupt vectors");
        let corrupt = query_semantic_visual_indexes(
            temporary.path(),
            &[expectation()],
            &[1.0, 0.0, 0.0],
            5,
            None,
        );
        assert_eq!(corrupt.status, SemanticVisualAvailability::NotIndexed);
        assert!(corrupt.unavailable[0].reason.contains("checksum"));
    }

    #[test]
    fn write_rejects_invalid_vectors_ranges_and_artifact_paths() {
        let temporary = tempfile::tempdir().expect("project dir");
        let mut invalid = input();
        invalid.vectors[0] = vec![f32::NAN, 0.0, 0.0];
        assert!(write_semantic_visual_index(temporary.path(), &invalid).is_err());

        let mut invalid = input();
        invalid.frames[0].shot_end_seconds = 0.1;
        assert!(write_semantic_visual_index(temporary.path(), &invalid).is_err());

        let mut invalid = input();
        invalid.frames[0].thumbnail_relative_path = "../escape.png".to_string();
        assert!(write_semantic_visual_index(temporary.path(), &invalid).is_err());
    }
}
