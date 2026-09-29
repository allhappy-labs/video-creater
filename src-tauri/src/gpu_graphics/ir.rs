use crate::graphics::ir::Dimensions;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuGraphicsLayer {
    pub schema_version: u32,
    pub id: String,
    pub role: GpuGraphicRole,
    pub timeline_start: f64,
    pub duration_seconds: f64,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub alpha: bool,
    pub source_beat: String,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ShaderPass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<PrimitiveScene>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GpuGraphicRole {
    ShaderBackground,
    PrimitiveScene,
    HybridScene,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPass {
    pub shader_language: ShaderLanguage,
    pub fragment_source: String,
    #[serde(default)]
    pub uniforms: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShaderLanguage {
    Glsl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveScene {
    pub camera: Camera,
    #[serde(default)]
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub preset: CameraPreset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fov_degrees: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CameraPreset {
    Fixed,
    Orbit,
    Dolly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Primitive {
    pub id: String,
    #[serde(rename = "type")]
    pub primitive_type: String,
    pub transform: Transform3,
    pub material: Material,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<PrimitiveAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transform3 {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub kind: MaterialKind,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    Flat,
    Emissive,
    Wire,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveAnimation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<RotationAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RotationAnimation {
    pub axis: [f32; 3],
    pub turns: f32,
}
