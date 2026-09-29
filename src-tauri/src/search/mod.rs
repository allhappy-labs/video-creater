use crate::project::model::{
    MediaAnalysisMoment, MediaAsset, MediaKind, TimelineItem, TimelineSource, Transcript,
    TranscriptWord, VideoProject,
};
use semantic_runtime::configured_semantic_encoder_status;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod semantic_runtime;
pub mod semantic_visual;
pub mod visual_cache;
pub mod visual_caption;

const SEARCH_INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SearchIndexError {
    #[error("search query must not be blank")]
    BlankQuery,
    #[error("invalid search scope: {0}")]
    InvalidScope(String),
    #[error("search index IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("search index JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    Visual,
    Spoken,
    Both,
    Metadata,
    Generated,
}

impl SearchScope {
    pub fn parse(value: &str) -> Result<Self, SearchIndexError> {
        match value.trim().to_lowercase().as_str() {
            "visual" => Ok(Self::Visual),
            "spoken" => Ok(Self::Spoken),
            "both" => Ok(Self::Both),
            "metadata" => Ok(Self::Metadata),
            "generated" => Ok(Self::Generated),
            other => Err(SearchIndexError::InvalidScope(other.to_string())),
        }
    }

    fn includes_spoken(self) -> bool {
        matches!(self, Self::Spoken | Self::Both)
    }

    fn includes_visual(self) -> bool {
        matches!(self, Self::Visual | Self::Both)
    }

    fn includes_metadata(self) -> bool {
        matches!(self, Self::Metadata | Self::Both)
    }

    fn includes_generated(self) -> bool {
        matches!(self, Self::Generated | Self::Both)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSearchQuery {
    pub query: String,
    pub limit: usize,
    pub scope: SearchScope,
    pub media_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchIndex {
    pub schema_version: u32,
    pub project_id: String,
    pub project_updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_content_digest: Option<String>,
    pub visual_status: String,
    #[serde(default)]
    pub visual_indexed_asset_count: usize,
    #[serde(default)]
    pub visual_indexed_media_ids: Vec<String>,
    pub spoken_status: String,
    pub media_count: usize,
    pub transcript_count: usize,
    pub generated_asset_count: usize,
    pub timeline_item_count: usize,
    pub spoken: Vec<SpokenSearchEntry>,
    pub metadata: Vec<MetadataSearchEntry>,
    pub generated: Vec<GeneratedSearchEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpokenSearchEntry {
    pub media_id: String,
    pub timeline_item_id: Option<String>,
    pub transcript_id: String,
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub word_count: usize,
    pub words: Vec<SpokenSearchWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpokenSearchWord {
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSearchEntry {
    pub text: String,
    pub media_id: Option<String>,
    pub timeline_item_id: Option<String>,
    pub track_id: Option<String>,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub reason: String,
    pub media: Option<Value>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSearchEntry {
    pub asset_id: String,
    pub media_ids: Vec<String>,
    pub text: String,
    pub generated_asset: Value,
}

pub fn project_search_index_path(project_dir: &Path) -> PathBuf {
    project_dir.join("search").join("index.json")
}

pub fn rebuild_project_search_index(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<ProjectSearchIndex, SearchIndexError> {
    let index = build_project_search_index(project);
    let index_path = project_search_index_path(project_dir);
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&index)?;
    fs::write(index_path, json)?;
    write_project_search_sidecars(project_dir, project)?;
    Ok(index)
}

pub fn load_project_search_index(
    project_dir: &Path,
) -> Result<ProjectSearchIndex, SearchIndexError> {
    let index_json = fs::read_to_string(project_search_index_path(project_dir))?;
    Ok(serde_json::from_str(&index_json)?)
}

pub fn query_project_search(
    project: &VideoProject,
    query: ProjectSearchQuery,
) -> Result<Value, SearchIndexError> {
    let index = build_project_search_index(project);
    query_project_search_index_with_status(
        &index,
        query,
        SearchIndexSource::Memory {
            reason: "notPersisted",
            stored_error: None,
            stored_schema_version: None,
            stored_project_updated_at: None,
        },
        None,
    )
}

pub fn query_project_search_for_project_dir(
    project_dir: &Path,
    project: &VideoProject,
    query: ProjectSearchQuery,
) -> Result<Value, SearchIndexError> {
    match load_project_search_index(project_dir) {
        Ok(index) if search_index_matches_project(&index, project) => {
            let index = search_index_with_visual_sidecar_status(project_dir, index);
            query_project_search_index_with_status(
                &index,
                query,
                SearchIndexSource::Stored,
                Some(project_dir),
            )
        }
        Ok(index) => query_project_search_index_with_status(
            &build_project_search_index(project),
            query,
            SearchIndexSource::Memory {
                reason: "stale",
                stored_error: None,
                stored_schema_version: Some(index.schema_version),
                stored_project_updated_at: Some(index.project_updated_at),
            },
            None,
        ),
        Err(error) => query_project_search_index_with_status(
            &build_project_search_index(project),
            query,
            SearchIndexSource::Memory {
                reason: if matches!(error, SearchIndexError::Io(ref io_error) if io_error.kind() == std::io::ErrorKind::NotFound)
                {
                    "missing"
                } else {
                    "unavailable"
                },
                stored_error: Some(error.to_string()),
                stored_schema_version: None,
                stored_project_updated_at: None,
            },
            None,
        ),
    }
}

pub fn query_project_search_for_project_dir_with_semantic_encoder(
    project_dir: &Path,
    project: &VideoProject,
    query: ProjectSearchQuery,
    expectations: &[semantic_visual::SemanticVisualIndexExpectation],
    encoder: &dyn semantic_runtime::LocalSemanticEncoder,
) -> Result<Value, SearchIndexError> {
    let query_text = query.query.clone();
    let limit = query.limit.clamp(1, 50);
    let includes_visual = query.scope.includes_visual();
    let mut payload = query_project_search_for_project_dir(project_dir, project, query)?;
    if !includes_visual {
        return Ok(payload);
    }
    let semantic = semantic_runtime::query_semantic_visual_with_text(
        project_dir,
        expectations,
        &query_text,
        encoder,
        limit,
    )
    .map_err(|error| SearchIndexError::Io(std::io::Error::other(error.to_string())))?;
    let semantic_hits = semantic
        .hits
        .iter()
        .map(|hit| {
            json!({
                "mediaId": hit.media_id,
                "startSeconds": hit.shot_start_seconds,
                "endSeconds": hit.shot_end_seconds,
                "timeSeconds": hit.time_seconds,
                "score": hit.score,
                "reason": "semantic visual similarity",
                "thumbnailRelativePath": hit.thumbnail_relative_path
            })
        })
        .collect::<Vec<_>>();
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "semanticEncoder".to_string(),
            json!({
                "status": "installed",
                "model": encoder.model(),
                "manifestConfigured": false,
                "licenseReviewed": true,
                "hashVerified": true,
                "message": "Injected local semantic encoder produced this result."
            }),
        );
        object.insert(
            "semanticVisual".to_string(),
            serde_json::to_value(&semantic).unwrap_or(Value::Null),
        );
        if !semantic_hits.is_empty() {
            object.insert("visualStatus".to_string(), json!("ready"));
        }
        if let Some(groups) = object.get_mut("groups").and_then(Value::as_object_mut) {
            groups.insert("visual".to_string(), Value::Array(semantic_hits.clone()));
        }
        let returned =
            if let Some(results) = object.get_mut("results").and_then(Value::as_array_mut) {
                results.extend(
                    semantic_hits
                        .iter()
                        .take(limit)
                        .map(|result| json!({"kind":"visual", "result": result})),
                );
                results.truncate(limit);
                Some(results.len())
            } else {
                None
            };
        if let Some(returned) = returned {
            object.insert("returned".to_string(), json!(returned));
        }
    }
    Ok(payload)
}

pub fn query_project_search_index(
    index: &ProjectSearchIndex,
    query: ProjectSearchQuery,
) -> Result<Value, SearchIndexError> {
    query_project_search_index_with_status(index, query, SearchIndexSource::Stored, None)
}

enum SearchIndexSource<'a> {
    Stored,
    Memory {
        reason: &'a str,
        stored_error: Option<String>,
        stored_schema_version: Option<u32>,
        stored_project_updated_at: Option<String>,
    },
}

fn query_project_search_index_with_status(
    index: &ProjectSearchIndex,
    query: ProjectSearchQuery,
    source: SearchIndexSource<'_>,
    project_dir: Option<&Path>,
) -> Result<Value, SearchIndexError> {
    let normalized_query = query.query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return Err(SearchIndexError::BlankQuery);
    }

    let limit = query.limit.clamp(1, 50);
    let media_filter = query
        .media_id
        .as_deref()
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty());
    let query_tokens = search_tokens(&normalized_query);
    let mut spoken = Vec::new();
    let mut visual = Vec::new();
    let mut metadata = Vec::new();
    let mut generated = Vec::new();
    let mut results = Vec::new();

    if query.scope.includes_spoken() {
        for entry in &index.spoken {
            if spoken.len() >= limit {
                break;
            }
            if media_filter.is_some_and(|filter| filter != entry.media_id.as_str()) {
                continue;
            }

            let entry_text = entry.text.to_lowercase();
            let exact_phrase_match = entry_text.contains(&normalized_query);
            let token_matched_words = if exact_phrase_match {
                Vec::new()
            } else {
                entry
                    .words
                    .iter()
                    .filter(|word| word_matches_query_tokens(&word.text, &query_tokens))
                    .collect::<Vec<_>>()
            };
            if !exact_phrase_match && token_matched_words.is_empty() {
                continue;
            }
            let (start_seconds, end_seconds) = if exact_phrase_match {
                (entry.start_seconds, entry.end_seconds)
            } else {
                (
                    token_matched_words
                        .iter()
                        .map(|word| word.start_seconds)
                        .fold(f64::INFINITY, f64::min),
                    token_matched_words
                        .iter()
                        .map(|word| word.end_seconds)
                        .fold(0.0, f64::max),
                )
            };

            spoken.push(json!({
                "mediaId": entry.media_id,
                "timelineItemId": entry.timeline_item_id,
                "startSeconds": round_seconds(start_seconds),
                "endSeconds": round_seconds(end_seconds),
                "score": spoken_search_score(exact_phrase_match, token_matched_words.len(), query_tokens.len()),
                "reason": if exact_phrase_match {
                    "matched transcript words"
                } else {
                    "matched transcript token overlap"
                }
            }));
        }
    }

    if query.scope.includes_visual() {
        if let Some(project_dir) = project_dir {
            visual = visual_search_results_from_sidecars(
                project_dir,
                index,
                &normalized_query,
                &query_tokens,
                limit,
                media_filter,
            );
        }
    }

    if query.scope.includes_metadata() {
        for entry in &index.metadata {
            if metadata.len() >= limit {
                break;
            }
            if media_filter.is_some_and(|filter| entry.media_id.as_deref() != Some(filter)) {
                continue;
            }
            if !entry.text.to_lowercase().contains(&normalized_query) {
                continue;
            }

            metadata.push(json!({
                "mediaId": entry.media_id,
                "timelineItemId": entry.timeline_item_id,
                "trackId": entry.track_id,
                "startSeconds": entry.start_seconds,
                "endSeconds": entry.end_seconds,
                "score": if entry.timeline_item_id.is_some() { 0.65 } else { 0.7 },
                "reason": entry.reason,
                "media": entry.media,
                "label": entry.label
            }));
        }
    }

    if query.scope.includes_generated() {
        for entry in &index.generated {
            if generated.len() >= limit {
                break;
            }
            if media_filter
                .is_some_and(|filter| !entry.media_ids.iter().any(|media_id| media_id == filter))
            {
                continue;
            }
            if !entry.text.to_lowercase().contains(&normalized_query) {
                continue;
            }

            generated.push(json!({
                "assetId": entry.asset_id,
                "mediaIds": entry.media_ids,
                "score": 0.8,
                "reason": "matched generated asset prompt or output id",
                "generatedAsset": entry.generated_asset
            }));
        }
    }

    push_ranked_group_results(
        &mut results,
        &[
            ("visual", visual.as_slice()),
            ("spoken", spoken.as_slice()),
            ("metadata", metadata.as_slice()),
            ("generated", generated.as_slice()),
        ],
        limit,
    );

    let visual_payload = palmier_visual_status_payload(index, media_filter, &visual);
    let palmier_spoken = palmier_spoken_results_payload(index, &spoken);
    let returned = results.len();
    let mut payload = json!({
        "query": query.query,
        "limit": limit,
        "indexStatus": search_index_status_payload(index, source),
        "visualStatus": index.visual_status,
        "semanticEncoder": configured_semantic_encoder_status(),
        "spokenStatus": index.spoken_status,
        "visual": visual_payload.clone(),
        "groups": {
            "spoken": spoken,
            "visual": visual,
            "metadata": metadata,
            "generated": generated
        },
        "results": results,
        "returned": returned
    });

    if let Some(object) = payload.as_object_mut() {
        if query.scope.includes_visual() {
            if let Some(visual) = visual_payload.as_object() {
                for key in ["status", "indexableAssets", "indexedAssets", "moments"] {
                    if let Some(value) = visual.get(key) {
                        object.insert(key.to_string(), value.clone());
                    }
                }
            }
        }
        if query.scope.includes_spoken() {
            object.insert("spoken".to_string(), palmier_spoken);
        }
    }

    Ok(payload)
}

fn visual_search_results_from_sidecars(
    project_dir: &Path,
    index: &ProjectSearchIndex,
    normalized_query: &str,
    query_tokens: &[String],
    limit: usize,
    media_filter: Option<&str>,
) -> Vec<Value> {
    if query_tokens.is_empty() {
        return Vec::new();
    }

    let mut hits = Vec::new();
    for media_id in &index.visual_indexed_media_ids {
        if media_filter.is_some_and(|filter| filter != media_id.as_str()) {
            continue;
        }
        let sidecar = match read_visual_sidecar(project_dir, media_id) {
            Some(sidecar) => sidecar,
            None => continue,
        };
        if !visual_sidecar_is_ready(&sidecar) {
            continue;
        }
        let media_kind = sidecar
            .get("mediaKind")
            .and_then(Value::as_str)
            .unwrap_or("video");
        let Some(frames) = sidecar.get("frames").and_then(Value::as_array) else {
            continue;
        };

        for frame in frames {
            let searchable_text = visual_frame_search_text(frame);
            if searchable_text.is_empty() {
                continue;
            }
            let searchable_lower = searchable_text.to_lowercase();
            let exact_phrase_match = searchable_lower.contains(normalized_query);
            let matched_token_count = if exact_phrase_match {
                query_tokens.len()
            } else {
                query_tokens
                    .iter()
                    .filter(|token| searchable_lower.contains(token.as_str()))
                    .count()
            };
            if !exact_phrase_match && matched_token_count == 0 {
                continue;
            }

            let time_seconds = number_field(frame, "timeSeconds").unwrap_or(0.0);
            let start_seconds = number_field(frame, "shotStartSeconds").unwrap_or(time_seconds);
            let end_seconds = number_field(frame, "shotEndSeconds").unwrap_or(time_seconds);
            let score = number_field(frame, "score").unwrap_or_else(|| {
                visual_search_score(exact_phrase_match, matched_token_count, query_tokens.len())
            });
            let frame_path = frame
                .get("relativeFramePath")
                .and_then(Value::as_str)
                .map(str::to_string);

            hits.push(json!({
                "mediaId": media_id,
                "startSeconds": round_seconds(start_seconds),
                "endSeconds": round_seconds(end_seconds),
                "timeSeconds": round_seconds(time_seconds),
                "score": round_seconds(score),
                "reason": if exact_phrase_match {
                    "matched visual frame description"
                } else {
                    "matched visual frame token overlap"
                },
                "label": frame
                    .get("label")
                    .or_else(|| frame.get("description"))
                    .or_else(|| frame.get("text"))
                    .and_then(Value::as_str),
                "framePath": frame_path,
                "mediaKind": media_kind
            }));
        }
    }

    hits.sort_by(|left, right| {
        right["score"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&left["score"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    hits
}

fn read_visual_sidecar(project_dir: &Path, media_id: &str) -> Option<Value> {
    let sidecar_path = project_dir
        .join("search")
        .join("visual")
        .join(safe_path_segment(media_id))
        .join("frames.json");
    let sidecar = fs::read_to_string(sidecar_path).ok()?;
    serde_json::from_str(&sidecar).ok()
}

fn visual_sidecar_is_ready(sidecar: &Value) -> bool {
    sidecar
        .get("visualStatus")
        .or_else(|| sidecar.get("status"))
        .and_then(Value::as_str)
        == Some("ready")
}

fn visual_frame_search_text(frame: &Value) -> String {
    let mut parts = Vec::new();
    for key in ["label", "description", "text", "caption"] {
        if let Some(value) = frame.get(key).and_then(Value::as_str) {
            parts.push(value.to_string());
        }
    }
    if let Some(tags) = frame.get("tags").and_then(Value::as_array) {
        parts.extend(tags.iter().filter_map(Value::as_str).map(str::to_string));
    }
    parts.join(" ")
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn visual_search_score(
    exact_phrase_match: bool,
    matched_tokens: usize,
    query_tokens: usize,
) -> f64 {
    if exact_phrase_match {
        0.86
    } else {
        let denominator = query_tokens.max(1) as f64;
        0.58 + ((matched_tokens as f64 / denominator) * 0.22)
    }
}

fn palmier_visual_status_payload(
    index: &ProjectSearchIndex,
    media_filter: Option<&str>,
    visual_hits: &[Value],
) -> Value {
    let indexable_media_ids = index
        .metadata
        .iter()
        .filter(|entry| entry.reason == "matched media metadata")
        .filter(|entry| media_filter.is_none_or(|filter| entry.media_id.as_deref() == Some(filter)))
        .filter_map(|entry| entry.media.as_ref())
        .filter(|media| {
            matches!(
                media.get("kind").and_then(Value::as_str),
                Some("video") | Some("image") | Some("lottie") | Some("generated")
            )
        })
        .filter_map(|media| media.get("id").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    let indexable_assets = indexable_media_ids.len();

    let indexed_assets = if !index.visual_indexed_media_ids.is_empty() {
        index
            .visual_indexed_media_ids
            .iter()
            .filter(|media_id| indexable_media_ids.contains(media_id.as_str()))
            .count()
    } else if index.visual_indexed_asset_count > 0 {
        index.visual_indexed_asset_count.min(indexable_assets)
    } else if index.visual_status == "ready" {
        indexable_assets
    } else {
        0
    };

    json!({
        "status": palmier_visual_status_name(&index.visual_status),
        "indexableAssets": indexable_assets,
        "indexedAssets": indexed_assets,
        "moments": palmier_visual_moments_payload(index, visual_hits)
    })
}

fn palmier_visual_moments_payload(index: &ProjectSearchIndex, visual_hits: &[Value]) -> Value {
    let moments = visual_hits
        .iter()
        .filter_map(|hit| {
            let media_id = hit.get("mediaId")?.as_str()?;
            let mut entry = json!({
                "mediaRef": media_id,
                "name": palmier_media_name(index, media_id),
                "score": hit.get("score").cloned().unwrap_or(Value::Null),
            });
            let object = entry.as_object_mut()?;
            if hit.get("mediaKind").and_then(Value::as_str) == Some("image") {
                object.insert("type".to_string(), json!("image"));
            } else {
                object.insert(
                    "startSeconds".to_string(),
                    hit.get("startSeconds").cloned().unwrap_or(Value::Null),
                );
                object.insert(
                    "endSeconds".to_string(),
                    hit.get("endSeconds").cloned().unwrap_or(Value::Null),
                );
            }
            if let Some(frame_path) = hit.get("framePath").filter(|value| !value.is_null()) {
                object.insert("framePath".to_string(), frame_path.clone());
            }
            Some(entry)
        })
        .collect::<Vec<_>>();

    json!(moments)
}

fn palmier_spoken_results_payload(index: &ProjectSearchIndex, spoken: &[Value]) -> Value {
    let results = spoken
        .iter()
        .filter_map(|hit| {
            let media_id = hit.get("mediaId")?.as_str()?;
            let start_seconds = hit.get("startSeconds")?.as_f64()?;
            let end_seconds = hit.get("endSeconds")?.as_f64()?;
            let entry = index
                .spoken
                .iter()
                .find(|entry| entry.media_id == media_id)?;

            Some(json!({
                "mediaRef": media_id,
                "name": palmier_media_name(index, media_id),
                "startSeconds": start_seconds,
                "endSeconds": end_seconds,
                "text": entry.text.clone()
            }))
        })
        .collect::<Vec<_>>();

    json!(results)
}

fn palmier_media_name(index: &ProjectSearchIndex, media_id: &str) -> String {
    index
        .metadata
        .iter()
        .find(|entry| entry.media_id.as_deref() == Some(media_id))
        .and_then(|entry| entry.media.as_ref())
        .and_then(|media| media.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn palmier_visual_status_name(status: &str) -> &str {
    match status {
        "notInstalled" => "modelNotInstalled",
        other => other,
    }
}

fn search_index_status_payload(index: &ProjectSearchIndex, source: SearchIndexSource<'_>) -> Value {
    match source {
        SearchIndexSource::Stored => json!({
            "stored": true,
            "source": "stored",
            "schemaVersion": index.schema_version,
            "projectUpdatedAt": index.project_updated_at
        }),
        SearchIndexSource::Memory {
            reason,
            stored_error,
            stored_schema_version,
            stored_project_updated_at,
        } => json!({
            "stored": false,
            "source": "memory",
            "reason": reason,
            "schemaVersion": index.schema_version,
            "projectUpdatedAt": index.project_updated_at,
            "storedError": stored_error,
            "storedSchemaVersion": stored_schema_version,
            "storedProjectUpdatedAt": stored_project_updated_at
        }),
    }
}

fn search_index_matches_project(index: &ProjectSearchIndex, project: &VideoProject) -> bool {
    index.schema_version == SEARCH_INDEX_SCHEMA_VERSION
        && index.project_id == project.id
        && index.project_updated_at == project.updated_at
        && index.project_content_digest.as_deref()
            == Some(project_search_content_digest(project).as_str())
        && index.media_count == project.media.len()
        && index.transcript_count == project.transcripts.len()
        && index.generated_asset_count == project.generated_assets.len()
        && index.timeline_item_count
            == project
                .timeline
                .tracks
                .iter()
                .map(|track| track.items.len())
                .sum::<usize>()
}

fn build_project_search_index(project: &VideoProject) -> ProjectSearchIndex {
    let spoken = project
        .transcripts
        .iter()
        .filter_map(|transcript| {
            let text = if transcript.segments.is_empty() {
                transcript
                    .words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                transcript
                    .segments
                    .iter()
                    .map(|segment| segment.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            if text.trim().is_empty() && transcript.words.is_empty() {
                return None;
            }
            let first = transcript.words.first();
            let last = transcript.words.last();
            let words = transcript
                .words
                .iter()
                .map(spoken_search_word)
                .collect::<Vec<_>>();
            Some(SpokenSearchEntry {
                media_id: transcript.media_id.clone(),
                timeline_item_id: timeline_item_id_for_media(project, &transcript.media_id),
                transcript_id: transcript.id.clone(),
                text,
                start_seconds: first.map(|word| word.start_seconds).unwrap_or(0.0),
                end_seconds: last.map(|word| word.end_seconds).unwrap_or(0.0),
                word_count: transcript.words.len(),
                words,
            })
        })
        .collect::<Vec<_>>();

    let mut metadata = project
        .media
        .iter()
        .map(|media| MetadataSearchEntry {
            text: format!(
                "{} {} {} {:?} {} {}",
                media.id,
                media.name.as_deref().unwrap_or(""),
                media.relative_path,
                media.kind,
                media.duration_seconds,
                folder_search_text(project, media.folder_id.as_deref())
            ),
            media_id: Some(media.id.clone()),
            timeline_item_id: None,
            track_id: None,
            start_seconds: None,
            end_seconds: None,
            reason: "matched media metadata".to_string(),
            media: Some(media_payload(media)),
            label: media.name.clone(),
        })
        .collect::<Vec<_>>();

    for track in &project.timeline.tracks {
        for item in &track.items {
            metadata.push(MetadataSearchEntry {
                text: item.label.clone(),
                media_id: media_id_for_timeline_item(item),
                timeline_item_id: Some(item.id.clone()),
                track_id: Some(track.id.clone()),
                start_seconds: Some(item.start_seconds),
                end_seconds: Some(item.start_seconds + item.duration_seconds),
                reason: "matched timeline label".to_string(),
                media: None,
                label: Some(item.label.clone()),
            });
        }
    }

    let generated = project
        .generated_assets
        .iter()
        .map(|asset| {
            let media_ids = asset
                .outputs
                .iter()
                .map(|output| output.media_id.clone())
                .collect::<Vec<_>>();
            GeneratedSearchEntry {
                asset_id: asset.id.clone(),
                media_ids,
                text: format!(
                    "{} {} {} {} {}",
                    asset.id,
                    asset.name.as_deref().unwrap_or(""),
                    asset.prompt,
                    asset
                        .outputs
                        .iter()
                        .map(|output| output.media_id.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    folder_search_text(project, asset.target_folder_id.as_deref())
                ),
                generated_asset: serde_json::to_value(asset).unwrap_or(Value::Null),
            }
        })
        .collect::<Vec<_>>();

    ProjectSearchIndex {
        schema_version: SEARCH_INDEX_SCHEMA_VERSION,
        project_id: project.id.clone(),
        project_updated_at: project.updated_at.clone(),
        project_content_digest: Some(project_search_content_digest(project)),
        visual_status: "notInstalled".to_string(),
        visual_indexed_asset_count: 0,
        visual_indexed_media_ids: Vec::new(),
        spoken_status: if spoken.is_empty() {
            "noTranscripts".to_string()
        } else {
            "ready".to_string()
        },
        media_count: project.media.len(),
        transcript_count: project.transcripts.len(),
        generated_asset_count: project.generated_assets.len(),
        timeline_item_count: project
            .timeline
            .tracks
            .iter()
            .map(|track| track.items.len())
            .sum(),
        spoken,
        metadata,
        generated,
    }
}

fn project_search_content_digest(project: &VideoProject) -> String {
    let bytes = serde_json::to_vec(project)
        .expect("VideoProject serialization is infallible for search fingerprinting");
    format!("{:x}", Sha256::digest(bytes))
}

fn search_index_with_visual_sidecar_status(
    project_dir: &Path,
    mut index: ProjectSearchIndex,
) -> ProjectSearchIndex {
    let visual_media_ids = index
        .metadata
        .iter()
        .filter(|entry| entry.reason == "matched media metadata")
        .filter_map(|entry| {
            let media = entry.media.as_ref()?;
            let kind = media.get("kind").and_then(Value::as_str)?;
            if !matches!(kind, "video" | "image" | "lottie" | "generated") {
                return None;
            }
            media.get("id").and_then(Value::as_str).map(str::to_string)
        })
        .collect::<Vec<_>>();

    if visual_media_ids.is_empty() {
        index.visual_indexed_asset_count = 0;
        return index;
    }

    let mut ready_count = 0usize;
    let mut ready_media_ids = Vec::new();
    let mut indexing_count = 0usize;
    let mut failed_count = 0usize;
    for media_id in &visual_media_ids {
        match read_visual_sidecar_status(project_dir, media_id) {
            Some("ready") => {
                ready_count += 1;
                ready_media_ids.push(media_id.clone());
            }
            Some("indexing") | Some("preparing") | Some("downloadingModel") => indexing_count += 1,
            Some("failed") => failed_count += 1,
            _ => {}
        }
    }

    index.visual_indexed_asset_count = ready_count;
    index.visual_indexed_media_ids = ready_media_ids;
    if ready_count == visual_media_ids.len() {
        index.visual_status = "ready".to_string();
    } else if ready_count > 0 || indexing_count > 0 {
        index.visual_status = "indexing".to_string();
    } else if failed_count > 0 {
        index.visual_status = "failed".to_string();
    }

    index
}

fn read_visual_sidecar_status(project_dir: &Path, media_id: &str) -> Option<&'static str> {
    let sidecar = read_visual_sidecar(project_dir, media_id)?;
    let status = sidecar
        .get("visualStatus")
        .or_else(|| sidecar.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    match status {
        "ready" => Some("ready"),
        "indexing" => Some("indexing"),
        "preparing" => Some("preparing"),
        "downloadingModel" => Some("downloadingModel"),
        "failed" => Some("failed"),
        _ => None,
    }
}

fn folder_search_text(project: &VideoProject, folder_id: Option<&str>) -> String {
    let mut parts = Vec::new();
    let mut visited = Vec::new();
    let mut current = folder_id;

    while let Some(folder_id) = current {
        if visited.iter().any(|visited_id| visited_id == folder_id) {
            break;
        }
        visited.push(folder_id.to_string());
        let Some(folder) = project
            .media_folders
            .iter()
            .find(|folder| folder.id == folder_id)
        else {
            parts.push(folder_id.to_string());
            break;
        };

        parts.push(folder.id.clone());
        parts.push(folder.name.clone());
        current = folder.parent_id.as_deref();
    }

    parts.join(" ")
}

fn write_project_search_sidecars(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<(), SearchIndexError> {
    let search_dir = project_dir.join("search");
    let spoken_dir = search_dir.join("spoken");
    let visual_dir = search_dir.join("visual");
    replace_dir(&spoken_dir)?;
    // Visual analysis is independently cached and may contain extracted frames or
    // model vectors. Rebuilding the text index must not discard a ready cache.
    fs::create_dir_all(&visual_dir)?;

    for transcript in &project.transcripts {
        write_spoken_sidecar(&spoken_dir, transcript)?;
    }

    for media in &project.media {
        if is_visual_media(media) {
            write_visual_frame_sidecar(&visual_dir, media, &project.media_analysis)?;
        }
    }

    Ok(())
}

fn write_spoken_sidecar(
    spoken_dir: &Path,
    transcript: &Transcript,
) -> Result<(), SearchIndexError> {
    fs::create_dir_all(spoken_dir)?;
    let path = spoken_dir.join(format!("{}.json", safe_path_segment(&transcript.media_id)));
    let sidecar = json!({
        "schemaVersion": SEARCH_INDEX_SCHEMA_VERSION,
        "mediaId": transcript.media_id,
        "transcriptId": transcript.id,
        "engine": transcript.engine,
        "rawArtifactPath": transcript.raw_artifact_path,
        "segments": transcript.segments,
        "words": transcript.words
    });
    fs::write(path, serde_json::to_string_pretty(&sidecar)?)?;
    Ok(())
}

fn write_visual_frame_sidecar(
    visual_dir: &Path,
    media: &MediaAsset,
    media_analysis: &[MediaAnalysisMoment],
) -> Result<(), SearchIndexError> {
    let media_dir = visual_dir.join(safe_path_segment(&media.id));
    let sidecar_path = media_dir.join("frames.json");
    let existing_sidecar = fs::read_to_string(&sidecar_path)
        .ok()
        .and_then(|source| serde_json::from_str::<Value>(&source).ok());
    if existing_sidecar
        .as_ref()
        .is_some_and(visual_sidecar_is_ready)
    {
        return Ok(());
    }
    let cached_frame_paths = existing_sidecar
        .as_ref()
        .and_then(|sidecar| sidecar.get("frameCache"))
        .filter(|cache| cache.get("status").and_then(Value::as_str) == Some("ready"))
        .and(existing_sidecar.as_ref())
        .and_then(|sidecar| sidecar.get("frames"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let frame_cache = existing_sidecar
        .as_ref()
        .and_then(|sidecar| sidecar.get("frameCache"))
        .filter(|cache| cache.get("status").and_then(Value::as_str) == Some("ready"))
        .cloned();
    fs::create_dir_all(media_dir.join("frames"))?;
    let media_label = visual_media_label(media);
    let visual_tags = visual_media_tags(media);
    let frames = sampled_visual_frame_times(media)
        .into_iter()
        .enumerate()
        .map(|(index, time_seconds)| {
            let sample_label = visual_sample_label(index, time_seconds, media.duration_seconds);
            let moment = visual_analysis_moment_for_sample(media_analysis, &media.id, time_seconds);
            let description = moment
                .map(|moment| moment.label.trim())
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    visual_frame_seed_description(media, &media_label, &sample_label)
                });
            let mut frame = json!({
                "index": index,
                "timeSeconds": round_seconds(time_seconds),
                "label": media_label.clone(),
                "description": description,
                "tags": visual_tags.clone(),
                "relativeFramePath": cached_frame_paths
                    .get(index)
                    .and_then(|cached| cached.get("relativeFramePath"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!(
                        "search/visual/{}/frames/frame-{index:06}.png",
                        safe_path_segment(&media.id)
                    ))
            });
            if let Some(moment) = moment {
                frame["shotStartSeconds"] = json!(round_seconds(moment.source_in));
                frame["shotEndSeconds"] = json!(round_seconds(moment.source_out));
                frame["visualActionScore"] = json!(round_seconds(moment.visual_action_score));
                frame["audioEnergyScore"] = json!(round_seconds(moment.audio_energy_score));
            }
            frame
        })
        .collect::<Vec<_>>();
    let mut sidecar = json!({
        "schemaVersion": SEARCH_INDEX_SCHEMA_VERSION,
        "mediaId": media.id,
        "mediaKind": media.kind,
        "relativePath": media.relative_path,
        "durationSeconds": media.duration_seconds,
        "status": "notInstalled",
        "visualStatus": "notInstalled",
        "embeddingModel": null,
        "frames": frames
    });
    if let Some(frame_cache) = frame_cache {
        sidecar["frameCache"] = frame_cache;
    }
    fs::write(sidecar_path, serde_json::to_string_pretty(&sidecar)?)?;
    Ok(())
}

fn visual_analysis_moment_for_sample<'a>(
    media_analysis: &'a [MediaAnalysisMoment],
    media_id: &str,
    time_seconds: f64,
) -> Option<&'a MediaAnalysisMoment> {
    media_analysis
        .iter()
        .filter(|moment| moment.media_id == media_id)
        .filter(|moment| {
            moment.source_in.is_finite()
                && moment.source_out.is_finite()
                && moment.source_in <= time_seconds
                && time_seconds <= moment.source_out
        })
        .max_by(|left, right| {
            let left_score = left.visual_action_score.max(left.audio_energy_score);
            let right_score = right.visual_action_score.max(right.audio_energy_score);
            left_score
                .partial_cmp(&right_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn replace_dir(path: &Path) -> Result<(), SearchIndexError> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn is_visual_media(media: &MediaAsset) -> bool {
    matches!(
        media.kind,
        MediaKind::Video | MediaKind::Image | MediaKind::Lottie | MediaKind::Generated
    )
}

fn sampled_visual_frame_times(media: &MediaAsset) -> Vec<f64> {
    match media.kind {
        MediaKind::Image | MediaKind::Lottie => vec![0.0],
        MediaKind::Video | MediaKind::Generated => {
            if media.duration_seconds.is_finite() && media.duration_seconds > 0.0 {
                let midpoint = media.duration_seconds / 2.0;
                vec![0.0, midpoint, media.duration_seconds]
            } else {
                vec![0.0]
            }
        }
        MediaKind::Audio => Vec::new(),
    }
}

fn visual_media_label(media: &MediaAsset) -> String {
    media
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| media_file_name(&media.relative_path))
}

fn visual_media_tags(media: &MediaAsset) -> Vec<String> {
    let mut tags = Vec::new();
    tags.push(media_kind_label(&media.kind).to_string());
    tags.extend(search_tokens(&visual_media_label(media)));
    tags.extend(search_tokens(&media.relative_path));
    tags.sort();
    tags.dedup();
    tags
}

fn visual_frame_seed_description(
    media: &MediaAsset,
    media_label: &str,
    sample_label: &str,
) -> String {
    format!(
        "{media_label} {} visual frame at {sample_label} from {}",
        media_kind_label(&media.kind),
        media.relative_path
    )
}

fn visual_sample_label(index: usize, time_seconds: f64, duration_seconds: f64) -> String {
    if index == 0 {
        return "opening".to_string();
    }
    if duration_seconds.is_finite()
        && duration_seconds > 0.0
        && (time_seconds - duration_seconds).abs() <= 0.001
    {
        return "closing".to_string();
    }
    "middle".to_string()
}

fn media_kind_label(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
        MediaKind::Lottie => "lottie",
        MediaKind::Generated => "generated",
    }
}

fn media_file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn safe_path_segment(value: &str) -> String {
    let segment = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if segment.is_empty() {
        "media".to_string()
    } else {
        segment
    }
}

fn push_ranked_group_results(results: &mut Vec<Value>, groups: &[(&str, &[Value])], limit: usize) {
    let mut ranked = Vec::new();
    let mut sequence = 0usize;
    for (kind, group) in groups {
        for item in *group {
            ranked.push((
                item.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                sequence,
                json!({ "kind": kind, "result": item }),
            ));
            sequence += 1;
        }
    }

    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    results.extend(ranked.into_iter().take(limit).map(|(_, _, result)| result));
}

fn media_payload(media: &crate::project::model::MediaAsset) -> Value {
    json!({
        "id": media.id,
        "name": media.name,
        "relativePath": media.relative_path,
        "kind": media.kind,
        "durationSeconds": media.duration_seconds,
        "width": media.width,
        "height": media.height,
        "fps": media.fps,
        "folderId": media.folder_id
    })
}

fn media_id_for_timeline_item(item: &TimelineItem) -> Option<String> {
    match &item.source {
        TimelineSource::Media { media_id } => Some(media_id.clone()),
        TimelineSource::Generated { artifact_id } => Some(artifact_id.clone()),
        TimelineSource::Timeline { .. } => None,
        TimelineSource::Text { .. } => None,
    }
}

fn timeline_item_id_for_media(project: &VideoProject, target_media_id: &str) -> Option<String> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find_map(|item| match &item.source {
            TimelineSource::Media { media_id } if media_id == target_media_id => {
                Some(item.id.clone())
            }
            _ => None,
        })
}

fn search_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn spoken_search_word(word: &TranscriptWord) -> SpokenSearchWord {
    SpokenSearchWord {
        text: word.text.clone(),
        start_seconds: word.start_seconds,
        end_seconds: word.end_seconds,
    }
}

fn word_matches_query_tokens(word: &str, query_tokens: &[String]) -> bool {
    if query_tokens.is_empty() {
        return false;
    }
    let word_tokens = search_tokens(word);
    word_tokens.iter().any(|word_token| {
        query_tokens
            .iter()
            .any(|query_token| word_token == query_token || word_token.contains(query_token))
    })
}

fn spoken_search_score(
    exact_phrase_match: bool,
    matched_token_count: usize,
    query_token_count: usize,
) -> f64 {
    if exact_phrase_match {
        return 0.9;
    }
    if query_token_count == 0 {
        return 0.0;
    }
    0.5 + (matched_token_count as f64 / query_token_count as f64) * 0.3
}

fn round_seconds(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;

    #[test]
    fn project_dir_search_rejects_same_shape_same_timestamp_stale_content() {
        let temp = tempfile::tempdir().expect("project dir");
        let mut indexed_project = sample_project();
        indexed_project.media[0].name = Some("Old searchable marker".to_string());
        rebuild_project_search_index(temp.path(), &indexed_project).expect("rebuild index");

        let mut changed_project = indexed_project.clone();
        changed_project.media[0].name = Some("New searchable marker".to_string());
        let results = query_project_search_for_project_dir(
            temp.path(),
            &changed_project,
            ProjectSearchQuery {
                query: "new searchable marker".to_string(),
                limit: 5,
                scope: SearchScope::Metadata,
                media_id: None,
            },
        )
        .expect("query current project content");

        assert_eq!(results["indexStatus"]["stored"], json!(false));
        assert_eq!(results["indexStatus"]["reason"], json!("stale"));
        assert_eq!(
            results["groups"]["metadata"][0]["label"],
            json!("New searchable marker")
        );
    }
}
