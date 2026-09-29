//! Graphics artifact manifests shared with video render backends.

use super::ir::Dimensions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsPlaybackManifest {
    pub mode: GraphicsPlaybackMode,
    pub start_number: u32,
    pub frame_duration_seconds: f64,
    pub expected_frames: Vec<PathBuf>,
}

impl Default for GraphicsPlaybackManifest {
    fn default() -> Self {
        Self {
            mode: GraphicsPlaybackMode::StaticHold,
            start_number: 0,
            frame_duration_seconds: 0.0,
            expected_frames: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GraphicsPlaybackMode {
    Sequence,
    StaticHold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsArtifactManifest {
    pub schema_version: u32,
    pub artifact_id: String,
    pub kind: String,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub duration_seconds: f64,
    pub alpha: bool,
    pub frame_count: u32,
    pub frames_pattern: String,
    #[serde(default)]
    pub playback: GraphicsPlaybackManifest,
    pub preview_path: PathBuf,
    pub source_layer_ids: Vec<String>,
    pub checksums: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WrittenGraphicsArtifact {
    pub manifest: GraphicsArtifactManifest,
    pub manifest_path: PathBuf,
    pub preview_path: PathBuf,
}

pub fn graphics_playback_manifest(
    frame_count: u32,
    fps: f64,
    duration_seconds: f64,
    animated: bool,
    frames_pattern: &str,
) -> GraphicsPlaybackManifest {
    let mode = if animated {
        GraphicsPlaybackMode::Sequence
    } else {
        GraphicsPlaybackMode::StaticHold
    };
    let frame_duration_seconds = if animated && fps.is_finite() && fps > 0.0 {
        1.0 / fps
    } else {
        duration_seconds
    };

    GraphicsPlaybackManifest {
        mode,
        start_number: 0,
        frame_duration_seconds,
        expected_frames: (0..frame_count)
            .map(|index| frame_path_from_pattern(frames_pattern, index))
            .collect(),
    }
}

pub fn frame_path_from_pattern(frames_pattern: &str, frame_index: u32) -> PathBuf {
    PathBuf::from(frames_pattern.replace("%06d", &format!("{frame_index:06}")))
}
