use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsLayer {
    pub schema_version: u32,
    pub id: String,
    pub role: GraphicRole,
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
    pub nodes: Vec<GraphicNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphicRole {
    Caption,
    Overlay,
    LowerThird,
    TitleCard,
    Diagram,
    Transition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Color {
    Hex(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GraphicNode {
    Text(TextNode),
    RoundedRect(RoundedRectNode),
    Rect(RectNode),
    Polygon(PolygonNode),
    Line(LineNode),
    ImageRef(ImageRefNode),
    HolographicLogo(HolographicLogoNode),
}

impl GraphicNode {
    pub fn id(&self) -> &str {
        match self {
            GraphicNode::Text(node) => &node.id,
            GraphicNode::RoundedRect(node) => &node.id,
            GraphicNode::Rect(node) => &node.id,
            GraphicNode::Polygon(node) => &node.id,
            GraphicNode::Line(node) => &node.id,
            GraphicNode::ImageRef(node) => &node.id,
            GraphicNode::HolographicLogo(node) => &node.id,
        }
    }

    pub fn animation(&self) -> Option<&NodeAnimation> {
        match self {
            GraphicNode::Text(node) => node.animate.as_ref(),
            GraphicNode::RoundedRect(node) => node.animate.as_ref(),
            GraphicNode::Rect(node) => node.animate.as_ref(),
            GraphicNode::Polygon(node) => node.animate.as_ref(),
            GraphicNode::Line(node) => node.animate.as_ref(),
            GraphicNode::ImageRef(node) => node.animate.as_ref(),
            GraphicNode::HolographicLogo(node) => node.animate.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeAnimation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Easing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yoyo: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<TransformOrigin>,
    #[serde(default)]
    pub keyframes: Vec<AnimationKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformOrigin {
    pub x: TransformOriginCoordinate,
    pub y: TransformOriginCoordinate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TransformOriginCoordinate {
    Keyword(String),
    Pixels(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextReveal {
    pub mode: TextRevealMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stagger_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<TextRevealOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_reveal_duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TextRevealMode {
    Whole,
    Line,
    Word,
    Character,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TextRevealOrder {
    Forward,
    Reverse,
    CenterOut,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Easing {
    Linear,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    OutBack,
    OutElastic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnimationKeyframe {
    pub at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_degrees: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_progress: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_progress: Option<f64>,
}

impl Default for AnimationKeyframe {
    fn default() -> Self {
        Self {
            at: 0.0,
            x: None,
            y: None,
            scale: None,
            scale_x: None,
            scale_y: None,
            opacity: None,
            rotation_degrees: None,
            blur_radius: None,
            shadow_opacity: None,
            glow_opacity: None,
            clip_progress: None,
            path_progress: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextNode {
    pub id: String,
    pub text: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub font_size: f64,
    pub font_weight: u16,
    pub align: String,
    pub fill: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_reveal: Option<TextReveal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emphasis: Vec<TextEmphasisRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextEmphasisRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub fill: Color,
    pub font_weight: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_end_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter_start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter_end_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_end_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_end_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emphasis_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emphasis_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<Easing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoundedRectNode {
    pub id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub radius: f64,
    pub fill: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RectNode {
    pub id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub fill: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolygonNode {
    pub id: String,
    pub points: Vec<Point>,
    pub fill: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineNode {
    pub id: String,
    pub points: Vec<Point>,
    pub stroke: Color,
    pub stroke_width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageRefNode {
    pub id: String,
    pub asset_id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub fit: ImageFit,
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HolographicLogoNode {
    pub id: String,
    pub asset_id: String,
    pub path_data: Vec<String>,
    pub view_box: Rect,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<NodeAnimation>,
}
