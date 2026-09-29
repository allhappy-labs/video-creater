use crate::gpu_graphics::ir::GpuGraphicsLayer;
use crate::graphics::ir::Dimensions;
use crate::graphics::ir::GraphicsLayer;
use crate::graphics::manifest::GraphicsArtifactManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const CACHE_METADATA_FILE_NAME: &str = "cache-metadata.json";
const GRAPHICS_MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsCacheMetadata {
    pub schema_version: u32,
    pub fingerprint: String,
    pub renderer: String,
    pub quality_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsCacheLookup {
    Hit(GraphicsArtifactManifest),
    Miss(String),
}

pub fn graphics_layer_fingerprint(
    layer: &GraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "layer": layer,
        "renderer": renderer,
        "qualityProfile": quality_profile,
    });
    let bytes = serde_json::to_vec(&payload)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

pub fn gpu_graphics_layer_fingerprint(
    layer: &GpuGraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "layer": layer,
        "renderer": renderer,
        "qualityProfile": quality_profile,
    });
    let bytes = serde_json::to_vec(&payload)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

pub fn write_graphics_cache_metadata(
    artifact_dir: &Path,
    fingerprint: &str,
    renderer: &str,
    quality_profile: Option<&str>,
) -> Result<(), std::io::Error> {
    let metadata = GraphicsCacheMetadata {
        schema_version: 1,
        fingerprint: fingerprint.to_string(),
        renderer: renderer.to_string(),
        quality_profile: quality_profile.map(str::to_string),
    };
    let json = serde_json::to_string_pretty(&metadata).map_err(std::io::Error::other)?;
    std::fs::write(artifact_dir.join(CACHE_METADATA_FILE_NAME), json)
}

pub fn validate_graphics_cache_hit(
    artifact_dir: &Path,
    layer: &GraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
    expected_fingerprint: &str,
) -> Result<GraphicsCacheLookup, std::io::Error> {
    validate_cache_hit(
        artifact_dir,
        CacheExpectation {
            dimensions: &layer.dimensions,
            fps: layer.fps,
            duration_seconds: layer.duration_seconds,
            alpha: layer.alpha,
            renderer,
            quality_profile,
            fingerprint: expected_fingerprint,
        },
    )
}

pub fn validate_gpu_graphics_cache_hit(
    artifact_dir: &Path,
    layer: &GpuGraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
    expected_fingerprint: &str,
) -> Result<GraphicsCacheLookup, std::io::Error> {
    validate_cache_hit(
        artifact_dir,
        CacheExpectation {
            dimensions: &layer.dimensions,
            fps: layer.fps,
            duration_seconds: layer.duration_seconds,
            alpha: layer.alpha,
            renderer,
            quality_profile,
            fingerprint: expected_fingerprint,
        },
    )
}

struct CacheExpectation<'a> {
    dimensions: &'a Dimensions,
    fps: f64,
    duration_seconds: f64,
    alpha: bool,
    renderer: &'a str,
    quality_profile: Option<&'a str>,
    fingerprint: &'a str,
}

fn validate_cache_hit(
    artifact_dir: &Path,
    expectation: CacheExpectation<'_>,
) -> Result<GraphicsCacheLookup, std::io::Error> {
    let CacheExpectation {
        dimensions,
        fps,
        duration_seconds,
        alpha,
        renderer,
        quality_profile,
        fingerprint: expected_fingerprint,
    } = expectation;
    let metadata_path = artifact_dir.join(CACHE_METADATA_FILE_NAME);
    if !metadata_path.is_file() {
        return Ok(GraphicsCacheLookup::Miss("metadata missing".to_string()));
    }

    let metadata_json = std::fs::read_to_string(&metadata_path)?;
    let metadata: GraphicsCacheMetadata =
        serde_json::from_str(&metadata_json).map_err(std::io::Error::other)?;
    if metadata.schema_version != 1 {
        return Ok(GraphicsCacheLookup::Miss(
            "metadata schema mismatch".to_string(),
        ));
    }
    if metadata.fingerprint != expected_fingerprint {
        return Ok(GraphicsCacheLookup::Miss(
            "fingerprint mismatch".to_string(),
        ));
    }
    if metadata.renderer != renderer || metadata.quality_profile.as_deref() != quality_profile {
        return Ok(GraphicsCacheLookup::Miss("renderer mismatch".to_string()));
    }

    let manifest_path = artifact_dir.join(GRAPHICS_MANIFEST_FILE_NAME);
    if !manifest_path.is_file() {
        return Ok(GraphicsCacheLookup::Miss("manifest missing".to_string()));
    }

    let manifest_json = std::fs::read_to_string(&manifest_path)?;
    let manifest: GraphicsArtifactManifest =
        serde_json::from_str(&manifest_json).map_err(std::io::Error::other)?;
    if manifest.dimensions != *dimensions
        || (manifest.fps - fps).abs() > 0.001
        || (manifest.duration_seconds - duration_seconds).abs() > 0.001
        || manifest.alpha != alpha
        || manifest.frame_count == 0
    {
        return Ok(GraphicsCacheLookup::Miss(
            "manifest contract mismatch".to_string(),
        ));
    }

    if !nonempty_file(&artifact_dir.join(&manifest.preview_path)) {
        return Ok(GraphicsCacheLookup::Miss("preview missing".to_string()));
    }
    for frame_index in 0..manifest.frame_count {
        if !nonempty_file(&graphics_frame_path(&manifest, artifact_dir, frame_index)) {
            return Ok(GraphicsCacheLookup::Miss("frame missing".to_string()));
        }
    }

    Ok(GraphicsCacheLookup::Hit(manifest))
}

fn nonempty_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn graphics_frame_path(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: &Path,
    frame_index: u32,
) -> PathBuf {
    let six = format!("{frame_index:06}");
    let five = format!("{frame_index:05}");
    let four = format!("{frame_index:04}");
    let raw = frame_index.to_string();
    let frame = manifest
        .frames_pattern
        .replace("%06d", &six)
        .replace("%05d", &five)
        .replace("%04d", &four)
        .replace("%d", &raw);
    artifact_dir.join(frame)
}
