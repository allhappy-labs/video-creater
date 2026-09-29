//! Motion template to graphics IR adapters.

use crate::edit::render_plan::TemplateRenderLayer;
use crate::graphics::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use crate::graphics::ir::{
    AnimationKeyframe, Color, Dimensions, Easing, GraphicNode, GraphicRole, GraphicsLayer,
    HolographicLogoNode, NodeAnimation, Rect, RectNode, RoundedRectNode, TextNode,
};
use crate::graphics::motion_presets::{motion_preset_animation, MotionPresetId};
use crate::graphics::validation::validate_graphics_layer;

pub fn template_layer_to_graphics_ir(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let graphics = match layer.template_id.as_str() {
        "kinetic-lower-third-v1" => kinetic_lower_third(layer, dimensions, fps)?,
        "punchy-caption-v1" => punchy_caption(layer, dimensions, fps)?,
        "metric-callout-v1" => metric_callout(layer, dimensions, fps)?,
        "chapter-card-v1" => chapter_card(layer, dimensions, fps)?,
        "tracking-highlight-v1" => tracking_highlight(layer, dimensions, fps)?,
        "holographic-logo-cutout-v1" => holographic_logo_cutout(layer, dimensions, fps)?,
        "gradient-background-loop-v1" => gradient_background_loop(layer, dimensions, fps)?,
        template_id => {
            return Err(vec![ActionableError::new(
                GraphicsErrorCode::GraphicsUnsupportedPrimitive,
                "templateId",
                format!("Template id '{template_id}' cannot be expanded to graphics IR."),
                "Use one of kinetic-lower-third-v1, punchy-caption-v1, metric-callout-v1, chapter-card-v1, tracking-highlight-v1, holographic-logo-cutout-v1, or gradient-background-loop-v1; or add an adapter for this template id.",
            )]);
        }
    };

    validate_graphics_layer(&graphics)?;
    Ok(graphics)
}

fn kinetic_lower_third(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    let subline = required_field(layer, "subline")?;

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::LowerThird,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 144.0,
                        y: 756.0,
                        width: 680.0,
                        height: 176.0,
                    },
                ),
                radius: scaled_size(&dimensions, 28.0),
                fill: Color::Hex("#101820cc".to_string()),
                animate: preset(MotionPresetId::SlideFadeUpV1, "backing"),
            }),
            GraphicNode::Rect(RectNode {
                id: "accent".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 176.0,
                        y: 788.0,
                        width: 8.0,
                        height: 108.0,
                    },
                ),
                fill: Color::Hex("#63e6be".to_string()),
                animate: preset(MotionPresetId::SlideFadeUpV1, "accent"),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 208.0,
                        y: 782.0,
                        width: 560.0,
                        height: 66.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 52.0),
                font_weight: 800,
                align: "left".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::SlideFadeUpV1, "headline"),
            }),
            GraphicNode::Text(TextNode {
                id: "subline".to_string(),
                text: subline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 210.0,
                        y: 850.0,
                        width: 520.0,
                        height: 44.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 32.0),
                font_weight: 500,
                align: "left".to_string(),
                fill: Color::Hex("#dbe7f5".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::SlideFadeUpV1, "subline"),
            }),
        ],
    })
}

fn punchy_caption(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::Overlay,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 180.0,
                        y: 686.0,
                        width: 1560.0,
                        height: 220.0,
                    },
                ),
                radius: scaled_size(&dimensions, 32.0),
                fill: Color::Hex("#101820bf".to_string()),
                animate: preset(MotionPresetId::SnapPopV1, "backing"),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 260.0,
                        y: 720.0,
                        width: 1400.0,
                        height: 126.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 64.0),
                font_weight: 900,
                align: "center".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(2),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::SnapPopV1, "headline"),
            }),
            GraphicNode::Rect(RectNode {
                id: "accent".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 620.0,
                        y: 862.0,
                        width: 620.0,
                        height: 8.0,
                    },
                ),
                fill: Color::Hex("#ffcf5a".to_string()),
                animate: preset(MotionPresetId::UnderlineWipeV1, "accent"),
            }),
        ],
    })
}

fn metric_callout(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    let subline = required_field(layer, "subline")?;

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::Overlay,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 1180.0,
                        y: 180.0,
                        width: 560.0,
                        height: 260.0,
                    },
                ),
                radius: scaled_size(&dimensions, 34.0),
                fill: Color::Hex("#0f172acc".to_string()),
                animate: preset(MotionPresetId::MetricCountPopV1, "backing"),
            }),
            GraphicNode::Rect(RectNode {
                id: "accent".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 1228.0,
                        y: 380.0,
                        width: 252.0,
                        height: 10.0,
                    },
                ),
                fill: Color::Hex("#38bdf8".to_string()),
                animate: preset(MotionPresetId::MetricCountPopV1, "accent"),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 1228.0,
                        y: 226.0,
                        width: 460.0,
                        height: 112.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 86.0),
                font_weight: 900,
                align: "left".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(2),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::MetricCountPopV1, "headline"),
            }),
            GraphicNode::Text(TextNode {
                id: "subline".to_string(),
                text: subline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 1232.0,
                        y: 330.0,
                        width: 380.0,
                        height: 64.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 30.0),
                font_weight: 600,
                align: "left".to_string(),
                fill: Color::Hex("#dbeafe".to_string()),
                max_lines: Some(2),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::MetricCountPopV1, "subline"),
            }),
        ],
    })
}

fn chapter_card(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    let subline = required_field(layer, "subline")?;

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::TitleCard,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::Rect(RectNode {
                id: "backing".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1920.0,
                        height: 1080.0,
                    },
                ),
                fill: Color::Hex("#0b1120e6".to_string()),
                animate: preset(MotionPresetId::VerticalRevealV1, "backing"),
            }),
            GraphicNode::Rect(RectNode {
                id: "accent-top".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 400.0,
                        y: 336.0,
                        width: 360.0,
                        height: 10.0,
                    },
                ),
                fill: Color::Hex("#f97316".to_string()),
                animate: preset(MotionPresetId::VerticalRevealV1, "accent-top"),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 400.0,
                        y: 388.0,
                        width: 1120.0,
                        height: 104.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 74.0),
                font_weight: 900,
                align: "left".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::VerticalRevealV1, "headline"),
            }),
            GraphicNode::Text(TextNode {
                id: "subline".to_string(),
                text: subline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 404.0,
                        y: 512.0,
                        width: 900.0,
                        height: 54.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 34.0),
                font_weight: 500,
                align: "left".to_string(),
                fill: Color::Hex("#cbd5e1".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::VerticalRevealV1, "subline"),
            }),
        ],
    })
}

fn tracking_highlight(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::Overlay,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "highlight-frame".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 132.0,
                        y: 138.0,
                        width: 640.0,
                        height: 360.0,
                    },
                ),
                radius: scaled_size(&dimensions, 26.0),
                fill: Color::Hex("#22c55e33".to_string()),
                animate: preset(MotionPresetId::TrackingDrawV1, "highlight-frame"),
            }),
            GraphicNode::Rect(RectNode {
                id: "accent-top".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 132.0,
                        y: 138.0,
                        width: 640.0,
                        height: 8.0,
                    },
                ),
                fill: Color::Hex("#22c55e".to_string()),
                animate: preset(MotionPresetId::TrackingDrawV1, "accent-top"),
            }),
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "label-backing".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 132.0,
                        y: 520.0,
                        width: 780.0,
                        height: 82.0,
                    },
                ),
                radius: scaled_size(&dimensions, 20.0),
                fill: Color::Hex("#052e16cc".to_string()),
                animate: preset(MotionPresetId::TrackingDrawV1, "label-backing"),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline.to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 168.0,
                        y: 538.0,
                        width: 720.0,
                        height: 46.0,
                    },
                ),
                font_size: readable_font_size(&dimensions, 38.0),
                font_weight: 800,
                align: "left".to_string(),
                fill: Color::Hex("#f0fdf4".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: preset(MotionPresetId::TrackingDrawV1, "headline"),
            }),
        ],
    })
}

fn holographic_logo_cutout(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let logo_asset_id = required_field(layer, "logoAssetId")?;
    if logo_asset_id != BUILT_IN_V_PHOTO_LOGO_ASSET_ID {
        return Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            "fields.logoAssetId",
            format!(
                "Holographic Logo Cutout only supports '{BUILT_IN_V_PHOTO_LOGO_ASSET_ID}' in this template slice."
            ),
            "Use logoAssetId 'builtin:v-photo-light' or add approved project logo asset support before rendering a custom logo.",
        )]);
    }

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::TitleCard,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions: dimensions.clone(),
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes: vec![
            GraphicNode::Rect(RectNode {
                id: "dark-gradient".to_string(),
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1920.0,
                        height: 1080.0,
                    },
                ),
                fill: Color::Hex("#00020bff".to_string()),
                animate: None,
            }),
            GraphicNode::HolographicLogo(HolographicLogoNode {
                id: "logo-cutout".to_string(),
                asset_id: logo_asset_id.to_string(),
                path_data: BUILT_IN_V_PHOTO_LOGO_PATHS
                    .iter()
                    .map(|path| (*path).to_string())
                    .collect(),
                view_box: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 65.0,
                },
                box_rect: scaled_rect(
                    &dimensions,
                    Rect {
                        x: 380.0,
                        y: 210.0,
                        width: 1160.0,
                        height: 754.0,
                    },
                ),
                opacity: 1.0,
                animate: Some(crisp_logo_shader_animation()),
            }),
        ],
    })
}

fn gradient_background_loop(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    let headline_lines = gradient_headline_lines(headline);
    let panel_width = 1920.0 / 5.0;
    let mut nodes = Vec::new();

    for panel_index in 0..5 {
        nodes.push(GraphicNode::Rect(RectNode {
            id: format!("gradient-loop-panel-{panel_index}"),
            box_rect: scaled_rect(
                &dimensions,
                Rect {
                    x: panel_width * panel_index as f64,
                    y: -180.0,
                    width: panel_width + 1.0,
                    height: 1440.0,
                },
            ),
            fill: Color::Hex("#ffffffff".to_string()),
            animate: Some(gradient_panel_loop_animation(&dimensions, panel_index)),
        }));
    }

    for (line_index, line) in headline_lines.iter().enumerate() {
        nodes.push(GraphicNode::Text(TextNode {
            id: if line_index == 0 {
                "headline".to_string()
            } else {
                format!("headline-line-{}", line_index + 1)
            },
            text: line.clone(),
            box_rect: scaled_rect(
                &dimensions,
                gradient_headline_line_rect(headline_lines.len(), line_index),
            ),
            font_size: readable_font_size(&dimensions, 292.0),
            font_weight: 900,
            align: "center".to_string(),
            fill: Color::Hex("#ffffff".to_string()),
            max_lines: Some(1),
            text_reveal: None,
            emphasis: Vec::new(),
            animate: Some(gradient_headline_hold_animation()),
        }));
    }

    Ok(GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role: GraphicRole::TitleCard,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions,
        fps,
        alpha: true,
        source_beat: source_beat(layer),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes,
    })
}

fn gradient_headline_lines(headline: &str) -> Vec<String> {
    let mut lines = headline
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(2)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(headline.trim().to_string());
    }
    lines
}

fn gradient_headline_line_rect(line_count: usize, line_index: usize) -> Rect {
    if line_count <= 1 {
        return Rect {
            x: 210.0,
            y: 340.0,
            width: 1500.0,
            height: 360.0,
        };
    }

    match line_index {
        0 => Rect {
            x: 210.0,
            y: 158.0,
            width: 1500.0,
            height: 340.0,
        },
        _ => Rect {
            x: 210.0,
            y: 416.0,
            width: 1500.0,
            height: 360.0,
        },
    }
}

const BUILT_IN_V_PHOTO_LOGO_ASSET_ID: &str = "builtin:v-photo-light";

const BUILT_IN_V_PHOTO_LOGO_PATHS: &[&str] = &[
    "M20.3935 0L22.4099 5.33333L32.0305 30.6667L28.6892 40H28.4588L15.324 5.33333C13.8261 1.38889 12.1555 0.388891 7.1435 0.222223V0H20.3935ZM52.4241 33.5556C53.173 37.5 54.6708 38.5 59.6252 38.6667V38.8889H40.7871V38.6667C44.8197 38.5556 46.3175 37.8333 46.3175 35.4444C46.3175 34.8889 46.2599 34.2778 46.1447 33.5556L39.9806 0H52.5969V0.222223C48.5643 0.333336 47.0665 1.05556 47.0665 3.55556C47.0665 4.05556 47.1241 4.66667 47.2393 5.33333L52.4241 33.5556ZM9.7359 32.2778C9.7359 37.0556 11.2337 38.3889 16.0729 38.6667V38.8889H0V38.6667C5.53045 38.3333 8.4685 35.5 9.79351 28.3333H10.1392C9.90873 29.7222 9.7359 30.9444 9.7359 32.2778Z",
    "M55.5836 0.222223V0H75.2282V0.222223C71.714 0.333336 70.2162 0.888891 70.2162 2.66667C70.2162 3.33333 70.389 4.11111 70.8499 5.33333L80.5858 31L77.1293 40H76.8988L63.764 5.33333C62.2662 1.38889 60.5955 0.388891 55.5836 0.222223ZM87.9597 10.5556C88.9391 7.88889 89.3424 5.94444 89.3424 4.5C89.3424 0.833333 86.9804 0.444445 83.9271 0.222223V0H100V0.222223C95.6217 0.5 92.2228 1.27778 88.3054 10.5556H87.9597Z",
    "M0 64.7986V64.6979C2.419 64.6224 3.06594 64.1692 3.06594 62.3818V49.593C3.06594 47.8056 2.419 47.3524 0 47.2769V47.1762H6.13187V62.3818C6.13187 64.1692 6.77881 64.6224 9.19781 64.6979V64.7986H0ZM9.53534 55.4839H8.1852V55.3832H8.69151C11.1386 55.3832 12.0106 54.2252 12.0106 51.9594C12.0106 49.6182 11.1668 47.2769 8.24146 47.2769H8.1852V47.1762H8.69151C12.9107 47.1762 15.0765 48.9636 15.0765 51.3049C15.0765 53.6462 12.9107 55.4839 9.53534 55.4839Z",
    "M22.613 62.3818C22.613 64.1692 23.26 64.6224 25.679 64.6979V64.7986H16.4812V64.6979C18.9002 64.6224 19.5471 64.1692 19.5471 62.3818V49.593C19.5471 47.8056 18.9002 47.3524 16.4812 47.2769V47.1762H25.679V47.2769C23.26 47.3524 22.613 47.8056 22.613 49.593V62.3818ZM39.1803 47.2769C36.7613 47.3524 36.1144 47.8056 36.1144 49.593V62.3818C36.1144 64.1692 36.7613 64.6224 39.1803 64.6979V64.7986H29.9825V64.6979C32.4015 64.6224 33.0485 64.1692 33.0485 62.3818V49.593C33.0485 47.8056 32.4015 47.3524 29.9825 47.2769V47.1762H39.1803V47.2769ZM24.3288 55.5594V55.4084H31.3608V55.5594H24.3288Z",
    "M53.7134 64.5972L53.6572 64.4462C56.2168 63.4895 57.2857 60.9972 57.2857 57.6741C57.2857 52.2615 55.4855 47.1259 50.5631 47.1259V47C56.3293 47 60.8298 51.028 60.8298 55.9874C60.8298 60.0154 57.8201 63.4643 53.7134 64.5972ZM51.0694 65C45.2469 65 40.7184 60.9217 40.7184 55.9874C40.7184 51.9091 43.7562 48.4853 47.9472 47.3776L48.0035 47.5035C45.3876 48.435 44.2906 50.9021 44.2906 54.3007C44.2906 59.7888 46.0908 64.849 51.0694 64.849V65Z",
    "M67.1834 47.1762V47.2769C64.0331 47.5538 62.8798 48.9636 61.9235 51.8839H61.811L62.486 47.1762H67.1834ZM71.4869 62.3818C71.4869 64.1692 72.1339 64.6224 74.5529 64.6979V64.7986H65.3551V64.6979C67.7741 64.6224 68.421 64.1692 68.421 62.3818V47.1762H71.4869V62.3818ZM77.3938 47.1762L78.0688 51.8839H77.9563C77 48.9636 75.8467 47.5538 72.6964 47.2769V47.1762H77.3938Z",
    "M92.8837 64.5972L92.8274 64.4462C95.387 63.4895 96.4559 60.9972 96.4559 57.6741C96.4559 52.2615 94.6557 47.1259 89.7333 47.1259V47C95.4995 47 100 51.028 100 55.9874C100 60.0154 96.9903 63.4643 92.8837 64.5972ZM90.2396 65C84.4172 65 79.8886 60.9217 79.8886 55.9874C79.8886 51.9091 82.9264 48.4853 87.1174 47.3776L87.1737 47.5035C84.5578 48.435 83.4608 50.9021 83.4608 54.3007C83.4608 59.7888 85.261 64.849 90.2396 64.849V65Z",
];

fn crisp_logo_shader_animation() -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::InOutCubic),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.5,
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
        ],
    }
}

fn gradient_panel_loop_animation(dimensions: &Dimensions, panel_index: usize) -> NodeAnimation {
    let direction = if panel_index.is_multiple_of(2) {
        -1.0
    } else {
        1.0
    };
    let distance = scaled_size(dimensions, 122.0) * direction;
    let settle = scaled_size(dimensions, 102.0) * direction;
    NodeAnimation {
        ease: Some(Easing::InOutCubic),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                y: Some(0.0),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.16,
                y: Some(distance),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.28,
                y: Some(settle),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.50,
                y: Some(0.0),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.66,
                y: Some(-distance),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.78,
                y: Some(-settle),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                y: Some(0.0),
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
        ],
    }
}

fn gradient_headline_hold_animation() -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::InOutCubic),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                opacity: Some(1.0),
                scale: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.5,
                opacity: Some(1.0),
                scale: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                opacity: Some(1.0),
                scale: Some(1.0),
                ..AnimationKeyframe::default()
            },
        ],
    }
}

fn required_field<'a>(layer: &'a TemplateRenderLayer, field: &str) -> ActionableResult<&'a str> {
    match layer.fields.get(field).map(String::as_str).map(str::trim) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamMissing,
            format!("fields.{field}"),
            format!(
                "Template '{}' is missing required field '{field}'.",
                layer.template_id
            ),
            format!("Set templateFields.{field} to a short non-empty string."),
        )]),
    }
}

fn source_beat(layer: &TemplateRenderLayer) -> String {
    let label = layer.label.trim();
    if label.is_empty() {
        format!(
            "Expand {} template into a renderable graphics layer.",
            layer.template_id
        )
    } else {
        format!("{label} template graphic")
    }
}

fn scaled_rect(dimensions: &Dimensions, rect: Rect) -> Rect {
    let scale_x = dimensions.width as f64 / 1920.0;
    let scale_y = dimensions.height as f64 / 1080.0;
    Rect {
        x: rect.x * scale_x,
        y: rect.y * scale_y,
        width: rect.width * scale_x,
        height: rect.height * scale_y,
    }
}

fn scaled_size(dimensions: &Dimensions, value: f64) -> f64 {
    let scale_x = dimensions.width as f64 / 1920.0;
    let scale_y = dimensions.height as f64 / 1080.0;
    value * scale_x.min(scale_y)
}

fn readable_font_size(dimensions: &Dimensions, value: f64) -> f64 {
    scaled_size(dimensions, value).max(14.0)
}

fn preset(preset: MotionPresetId, node_id: &str) -> Option<NodeAnimation> {
    Some(motion_preset_animation(preset, node_id))
}
