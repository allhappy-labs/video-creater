use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use video_creater_lib::edit::render_plan::TemplateRenderLayer;
use video_creater_lib::graphics::animation::{animate_node, evaluate_animation};
use video_creater_lib::graphics::assets::{AssetRegistry, ImageAsset};
use video_creater_lib::graphics::error::{ActionableError, GraphicsErrorCode};
use video_creater_lib::graphics::ir::{
    AnimationKeyframe, Color, Dimensions, Easing, GraphicNode, GraphicRole, GraphicsLayer,
    HolographicLogoNode, ImageFit, ImageRefNode, LineNode, NodeAnimation, Point, PolygonNode, Rect,
    RectNode, RoundedRectNode, TextNode,
};
use video_creater_lib::graphics::manifest::GraphicsPlaybackMode;
use video_creater_lib::graphics::motion_presets::{
    motion_preset_animation, parse_motion_preset_id, MotionPresetId,
};
use video_creater_lib::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use video_creater_lib::graphics::shader_noise::{
    domain_warp_2d, fbm_3d, smoothstep, stable_pixel_noise, value_noise_3d,
};
use video_creater_lib::graphics::templates::template_layer_to_graphics_ir;
use video_creater_lib::graphics::validation::{
    validate_graphics_layer, validate_graphics_layer_with_assets,
};
use video_creater_lib::graphics::visual_qa::{run_cpu_visual_qa, CpuVisualQaOptions};
use video_creater_lib::graphics::webm_export::{
    build_webm_export_pipeline_description, WebmEncoder, WebmExportOptions,
};

#[test]
fn actionable_error_serializes_for_agent_repair() {
    let error = ActionableError::new(
        GraphicsErrorCode::GraphicsTextOverflow,
        "layers[0].nodes[0]",
        "Text does not fit inside safe zone.",
        "Reduce fontSize, shorten text, or increase box width.",
    );

    let serialized = serde_json::to_value(error).expect("serialize actionable error");

    assert_eq!(serialized["code"], "GRAPHICS_TEXT_OVERFLOW");
    assert_eq!(serialized["path"], "layers[0].nodes[0]");
    assert_eq!(serialized["message"], "Text does not fit inside safe zone.");
    assert_eq!(
        serialized["fix"],
        "Reduce fontSize, shorten text, or increase box width."
    );
}

#[test]
fn validates_minimal_text_overlay_ir() {
    validate_graphics_layer(&valid_text_layer()).expect("valid graphics layer");
}

#[test]
fn expands_existing_template_layer_to_graphics_ir() {
    let mut fields = BTreeMap::new();
    fields.insert("headline".to_string(), "Olha API".to_string());
    fields.insert("subline".to_string(), "Founder".to_string());
    let layer = TemplateRenderLayer {
        item_id: "template-item-1".to_string(),
        template_id: "kinetic-lower-third-v1".to_string(),
        label: "Kinetic Lower Third".to_string(),
        timeline_start_seconds: 1.2,
        duration_seconds: 2.4,
        fields,
        visual_treatment: "compact lower-third block with translucent backing".to_string(),
        motion: "slide-and-fade in".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
        preview_variant: "lower-third".to_string(),
    };

    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect("template to graphics ir");

    assert_eq!(graphics.id, "template-item-1");
    assert_eq!(graphics.timeline_start, 1.2);
    assert_eq!(graphics.duration_seconds, 2.4);
    assert!(graphics.nodes.iter().any(|node| node.id() == "headline"));
    assert!(graphics.nodes.iter().any(|node| node.id() == "subline"));
}

#[test]
fn parses_motion_preset_ids_and_builds_keyframes() {
    assert_eq!(
        parse_motion_preset_id("slide-fade-up-v1"),
        Some(MotionPresetId::SlideFadeUpV1)
    );
    assert_eq!(parse_motion_preset_id("missing-preset"), None);

    let animation = motion_preset_animation(MotionPresetId::SnapPopV1, "headline");
    assert_eq!(animation.ease, Some(Easing::OutBack));
    assert!(animation.keyframes.len() >= 3);
    assert_eq!(animation.keyframes[0].opacity, Some(0.0));
    assert_eq!(
        animation.keyframes.last().and_then(|frame| frame.opacity),
        Some(0.0)
    );
}

#[test]
fn parses_motion_v2_preset_ids_and_builds_v2_keyframes() {
    assert_eq!(
        parse_motion_preset_id("slide-rotate-settle-v2"),
        Some(MotionPresetId::SlideRotateSettleV2)
    );

    let animation = motion_preset_animation(MotionPresetId::LineDrawV2, "accent-line");
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.path_progress == Some(0.0)));
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.path_progress == Some(1.0)));
}

#[test]
fn expands_every_renderer_template_catalog_entry() {
    for (template_id, expected_role) in [
        ("kinetic-lower-third-v1", GraphicRole::LowerThird),
        ("punchy-caption-v1", GraphicRole::Overlay),
        ("metric-callout-v1", GraphicRole::Overlay),
        ("chapter-card-v1", GraphicRole::TitleCard),
        ("tracking-highlight-v1", GraphicRole::Overlay),
        ("holographic-logo-cutout-v1", GraphicRole::TitleCard),
        ("gradient-background-loop-v1", GraphicRole::TitleCard),
    ] {
        let layer = template_render_layer(template_id);

        let graphics = template_layer_to_graphics_ir(
            &layer,
            Dimensions {
                width: 1920,
                height: 1080,
            },
            30.0,
        )
        .expect("accepted template should expand to graphics ir");

        assert_eq!(graphics.id, "template-item-1");
        assert_eq!(graphics.role, expected_role);
        assert_eq!(graphics.timeline_start, 1.2);
        assert_eq!(graphics.duration_seconds, 2.4);
        assert_eq!(
            graphics.dimensions,
            Dimensions {
                width: 1920,
                height: 1080,
            }
        );
        assert_eq!(graphics.fps, 30.0);
        assert!(
            graphics.nodes.iter().any(|node| node.id() == "headline")
                || graphics.nodes.iter().any(|node| node.id() == "logo-cutout"),
            "{template_id} should include a headline or logo-cutout node"
        );
        validate_graphics_layer(&graphics).expect("adapter output should validate");
    }
}

#[test]
fn template_recipes_attach_motion_preset_animations() {
    for (template_id, expected_animated_node) in [
        ("kinetic-lower-third-v1", "backing"),
        ("punchy-caption-v1", "headline"),
        ("metric-callout-v1", "headline"),
        ("chapter-card-v1", "headline"),
        ("tracking-highlight-v1", "highlight-frame"),
        ("holographic-logo-cutout-v1", "logo-cutout"),
        ("gradient-background-loop-v1", "headline"),
    ] {
        let layer = template_render_layer(template_id);

        let graphics = template_layer_to_graphics_ir(
            &layer,
            Dimensions {
                width: 1920,
                height: 1080,
            },
            30.0,
        )
        .expect("template should expand");

        let node = graphics
            .nodes
            .iter()
            .find(|node| node.id() == expected_animated_node)
            .expect("expected animated node");
        let animation = node.animation().expect("template node should animate");
        assert!(animation.keyframes.len() >= 3);
        if matches!(
            template_id,
            "holographic-logo-cutout-v1" | "gradient-background-loop-v1"
        ) {
            assert_eq!(animation.keyframes[0].opacity, Some(1.0));
        } else {
            assert_eq!(animation.keyframes[0].opacity, Some(0.0));
        }
    }
}

#[test]
fn expands_holographic_logo_template_to_shader_like_cutout_nodes() {
    let mut layer = template_render_layer("holographic-logo-cutout-v1");
    layer.label = "Holographic Logo Cutout".to_string();
    layer.duration_seconds = 3.2;
    layer.fields.clear();
    layer.fields.insert(
        "logoAssetId".to_string(),
        "builtin:v-photo-light".to_string(),
    );
    layer.visual_treatment = "full-frame dark gradient with a holographic logo cutout".to_string();
    layer.motion = "animated shader shimmer through a crisp logo mask".to_string();
    layer.safe_zone = "keep the logo inside the central 80% safe zone".to_string();
    layer.avoid = "plain boxes, static text-only cards, and opaque caption slabs".to_string();
    layer.preview_variant = "holographic-logo".to_string();

    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect("holographic logo template to graphics ir");

    assert_eq!(graphics.role, GraphicRole::TitleCard);
    assert!(graphics.alpha);
    assert!(graphics
        .nodes
        .iter()
        .any(|node| node.id() == "dark-gradient"));
    assert!(
        !graphics
            .nodes
            .iter()
            .any(|node| { matches!(node.id(), "cyan-aurora" | "violet-aurora" | "shimmer-scan") }),
        "holographic logo template should not use visible rectangular aurora or scan bars"
    );
    let logo = graphics
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphicNode::HolographicLogo(node) => Some(node),
            _ => None,
        })
        .expect("holographic logo node");
    assert_eq!(logo.id, "logo-cutout");
    assert_eq!(logo.asset_id, "builtin:v-photo-light");
    assert_eq!(logo.view_box.width, 100.0);
    assert_eq!(logo.view_box.height, 65.0);
    assert!(logo.path_data.len() >= 7);
    assert_eq!(logo.opacity, 1.0);
    assert!(logo.box_rect.width <= 1_200.0);
    assert!(logo.box_rect.height <= 780.0);
    let animation = logo.animate.as_ref().expect("logo should animate");
    assert_eq!(
        animation.keyframes.first().and_then(|frame| frame.opacity),
        Some(1.0)
    );
    assert!(graphics.motion.contains("shader"));
}

#[test]
fn expands_gradient_background_loop_template_to_panel_text_card() {
    let mut layer = template_render_layer("gradient-background-loop-v1");
    layer.label = "Gradient Background Loop".to_string();
    layer.duration_seconds = 4.0;
    layer.fields.clear();
    layer
        .fields
        .insert("headline".to_string(), "Love\nwins.".to_string());
    layer.visual_treatment =
        "full-frame vertical blue gradient panels with oversized white text".to_string();
    layer.motion = "seamless vertical gradient loop with subtle panel drift".to_string();
    layer.safe_zone = "keep text inside 10% margins".to_string();
    layer.avoid =
        "plain boxes, opaque caption slabs, and static default-font title cards".to_string();
    layer.preview_variant = "gradient-background-loop".to_string();

    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect("gradient background loop template to graphics ir");

    assert_eq!(graphics.role, GraphicRole::TitleCard);
    assert!(graphics.alpha);
    assert_eq!(
        graphics
            .nodes
            .iter()
            .filter(|node| node.id().starts_with("gradient-loop-panel-"))
            .count(),
        5
    );
    let headline = graphics
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphicNode::Text(node) if node.id == "headline" => Some(node),
            _ => None,
        })
        .expect("headline node");
    assert_eq!(headline.text, "Love");
    let second_line = graphics
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphicNode::Text(node) if node.id == "headline-line-2" => Some(node),
            _ => None,
        })
        .expect("headline second line node");
    assert_eq!(second_line.text, "wins.");
    assert!(
        (240.0..=300.0).contains(&(second_line.box_rect.y - headline.box_rect.y)),
        "gradient title lines should have close spacing without overlapping"
    );
    assert!(headline.font_size >= 250.0);
    assert_eq!(headline.font_weight, 900);
    assert_eq!(headline.align, "center");
    assert!(headline
        .animate
        .as_ref()
        .is_some_and(|animation| animation.keyframes.len() >= 3));
}

#[test]
fn gradient_background_loop_template_uses_springy_panel_motion() {
    let mut layer = template_render_layer("gradient-background-loop-v1");
    layer.duration_seconds = 4.0;
    layer.fields.clear();
    layer
        .fields
        .insert("headline".to_string(), "Love\nwins.".to_string());

    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect("gradient background loop template to graphics ir");
    let first_panel = graphics
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphicNode::Rect(node) if node.id == "gradient-loop-panel-0" => Some(node),
            _ => None,
        })
        .expect("first gradient panel");
    let animation = first_panel
        .animate
        .as_ref()
        .expect("gradient panel should animate");
    let y_values = animation
        .keyframes
        .iter()
        .filter_map(|keyframe| keyframe.y)
        .collect::<Vec<_>>();

    assert_eq!(animation.ease, Some(Easing::InOutCubic));
    assert!(
        animation.keyframes.len() >= 6,
        "carousel motion should use slight overshoot and settle keyframes"
    );
    assert!(
        y_values
            .iter()
            .any(|value| value.abs() > 105.0 && value.abs() < 135.0),
        "panel motion should have only a slight spring overshoot"
    );
    assert_eq!(
        animation.keyframes.last().and_then(|frame| frame.y),
        Some(0.0)
    );
}

#[test]
fn gradient_background_loop_template_renders_blue_panel_preview() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut layer = template_render_layer("gradient-background-loop-v1");
    layer.duration_seconds = 0.5;
    layer.fields.clear();
    layer
        .fields
        .insert("headline".to_string(), "Love\nwins.".to_string());
    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 320,
            height: 180,
        },
        4.0,
    )
    .expect("gradient template to graphics ir");
    let output_dir = dir.path().join("gradient-background-loop-template-preview");

    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render gradient background loop template preview");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 2);
    assert_eq!(manifest.playback.mode, GraphicsPlaybackMode::Sequence);
    assert_eq!(manifest.playback.start_number, 0);
    assert_eq!(manifest.playback.frame_duration_seconds, 0.25);
    assert_eq!(
        manifest.playback.expected_frames,
        vec![
            PathBuf::from("frames/frame-000000.png"),
            PathBuf::from("frames/frame-000001.png")
        ]
    );
    assert!(output_dir.join("preview.png").exists());
    let preview = decode_png(&output_dir.join("preview.png"));
    assert!(
        average_opaque_pixel_saturation(&preview) > 0.35,
        "blue gradient panel preview should carry saturated color"
    );
    assert!(
        opaque_luminance_standard_deviation(&preview) > 35.0,
        "preview should include high-contrast white text over darker panel bands"
    );
}

#[test]
fn gradient_background_loop_template_animates_gradient_bands_between_frames() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut layer = template_render_layer("gradient-background-loop-v1");
    layer.duration_seconds = 1.0;
    layer.fields.clear();
    layer
        .fields
        .insert("headline".to_string(), "Love\nwins.".to_string());
    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 320,
            height: 180,
        },
        4.0,
    )
    .expect("gradient template to graphics ir");
    let output_dir = dir
        .path()
        .join("gradient-background-loop-template-animation");

    render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render gradient background loop template animation frames");

    let first = decode_png(&output_dir.join("frames/frame-000000.png"));
    let quarter_second = decode_png(&output_dir.join("frames/frame-000001.png"));

    assert!(
        average_background_rgb_delta(&first, &quarter_second) > 42.0,
        "gradient bands should visibly shift within the first quarter second"
    );
}

#[test]
fn holographic_logo_node_validates_path_data_and_renders_preview() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "holographic-logo-node-preview".to_string(),
        role: GraphicRole::TitleCard,
        timeline_start: 0.0,
        duration_seconds: 0.5,
        dimensions: Dimensions {
            width: 320,
            height: 180,
        },
        fps: 4.0,
        alpha: true,
        source_beat: "Show a logo reveal beat.".to_string(),
        visual_treatment: "holographic logo cutout on dark gradient".to_string(),
        motion: "animated shader shimmer through logo mask".to_string(),
        safe_zone: "central 80% safe zone".to_string(),
        avoid: "plain boxes and static text-only cards".to_string(),
        nodes: vec![GraphicNode::HolographicLogo(HolographicLogoNode {
            id: "logo-cutout".to_string(),
            asset_id: "builtin:test-logo".to_string(),
            path_data: vec!["M0 0 L100 0 L100 65 L0 65 Z".to_string()],
            view_box: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 65.0,
            },
            box_rect: Rect {
                x: 80.0,
                y: 56.0,
                width: 160.0,
                height: 104.0,
            },
            opacity: 1.0,
            animate: None,
        })],
    };

    validate_graphics_layer(&layer).expect("holographic logo node should validate");
    let output_dir = dir.path().join("holographic-logo-node-preview");
    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render holographic logo node preview");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert!(output_dir.join("preview.png").exists());
    let preview = decode_png(&output_dir.join("preview.png"));
    assert!(
        average_opaque_pixel_saturation(&preview) > 0.07,
        "holographic shader should have visible color variation, not read as a flat white logo"
    );
    assert!(
        average_opaque_pixel_saturation(&preview) > 0.18,
        "holographic shader noise should visibly disrupt the bands instead of reading as a simple gradient"
    );
    assert!(
        opaque_luminance_standard_deviation(&preview) > 24.0,
        "holographic shader should have visible light/dark wave contrast"
    );
}

#[test]
fn graphics_webm_export_pipeline_is_rust_owned_and_does_not_use_ffmpeg() {
    let options = WebmExportOptions {
        frames_pattern: PathBuf::from("frames/frame-%06d.png"),
        output_path: PathBuf::from("holographic-logo.webm"),
        dimensions: Dimensions {
            width: 1920,
            height: 1080,
        },
        fps: 24.0,
        frame_count: 480,
        encoder: WebmEncoder::Vp8,
        target_bitrate_bps: 6_000_000,
    };

    let pipeline =
        build_webm_export_pipeline_description(&options).expect("valid WebM export pipeline");

    assert!(pipeline.contains("multifilesrc"));
    assert!(pipeline.contains("pngdec"));
    assert!(pipeline.contains("vp8enc"));
    assert!(pipeline.contains("webmmux"));
    assert!(!pipeline.contains("ffmpeg"));
    assert!(!pipeline.contains("mp4mux"));
    assert!(!pipeline.contains("x264enc"));
}

#[test]
fn shared_shader_noise_is_deterministic_and_smooth_enough_for_templates() {
    let first = value_noise_3d(1.25, -0.4, 0.75, 42);
    let second = value_noise_3d(1.25, -0.4, 0.75, 42);
    assert_eq!(first, second);
    assert!((0.0..=1.0).contains(&first));

    let nearby = value_noise_3d(1.26, -0.39, 0.76, 42);
    assert!(
        (first - nearby).abs() < 0.08,
        "smooth shader noise should not jump like per-pixel random"
    );

    let fbm = fbm_3d(2.0, 3.0, 0.5, 77, 5);
    assert!((0.0..=1.0).contains(&fbm));

    let warp = domain_warp_2d(0.4, 0.7, 1.2, 99, 0.5);
    assert!(warp.strength > 0.0);
    assert!(warp.strength <= 0.75);

    assert_eq!(stable_pixel_noise(10, 20, 3), stable_pixel_noise(10, 20, 3));
    assert_eq!(smoothstep(0.0, 1.0, 0.0), 0.0);
    assert_eq!(smoothstep(0.0, 1.0, 1.0), 1.0);
}

#[test]
fn holographic_logo_template_renders_builtin_logo_paths() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut layer = template_render_layer("holographic-logo-cutout-v1");
    layer.duration_seconds = 0.5;
    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 320,
            height: 180,
        },
        4.0,
    )
    .expect("holographic template to graphics ir");
    let output_dir = dir.path().join("holographic-logo-template-preview");

    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render holographic template preview");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 2);
    assert!(output_dir.join("preview.png").exists());
}

#[test]
fn template_layer_scaled_dimensions_validate_successfully() {
    let layer = template_render_layer("tracking-highlight-v1");

    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 640,
            height: 360,
        },
        30.0,
    )
    .expect("scaled template to graphics ir");

    assert_eq!(
        graphics.dimensions,
        Dimensions {
            width: 640,
            height: 360,
        }
    );
    validate_graphics_layer(&graphics).expect("scaled adapter output should validate");
}

#[test]
fn template_layer_render_preview_succeeds_for_adapter_output() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer = template_render_layer("metric-callout-v1");
    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 640,
            height: 360,
        },
        30.0,
    )
    .expect("template to graphics ir");
    let output_dir = dir.path().join("graphics/template-adapter-preview");

    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render adapter output preview");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(
        manifest.frame_count,
        (graphics.duration_seconds * graphics.fps).ceil() as u32
    );
    assert!(output_dir.join("preview.png").exists());
}

#[test]
fn template_layer_renders_manifest_with_expected_timing_and_frames() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut fields = BTreeMap::new();
    fields.insert("headline".to_string(), "Olha API".to_string());
    fields.insert("subline".to_string(), "Founder".to_string());
    let layer = TemplateRenderLayer {
        item_id: "template-item-1".to_string(),
        template_id: "kinetic-lower-third-v1".to_string(),
        label: "Kinetic Lower Third".to_string(),
        timeline_start_seconds: 0.5,
        duration_seconds: 1.0,
        fields,
        visual_treatment: "compact lower-third block with translucent backing".to_string(),
        motion: "slide-and-fade in".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
        preview_variant: "lower-third".to_string(),
    };
    let graphics = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 640,
            height: 360,
        },
        30.0,
    )
    .expect("template to graphics ir");
    let output_dir = dir.path().join("graphics/template-gstreamer-slice");

    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render template preview manifest");
    assert_eq!(manifest.source_layer_ids, vec!["template-item-1"]);
    assert_eq!(graphics.timeline_start, 0.5);
    assert_eq!(manifest.duration_seconds, 1.0);
    assert_eq!(manifest.frames_pattern, "frames/frame-%06d.png");
    assert!(output_dir.join(&manifest.preview_path).exists());
    assert!(output_dir.join("frames/frame-000000.png").exists());
}

#[test]
fn cpu_visual_qa_rejects_static_animated_layer_frames() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let output_dir = dir.path().join("cpu-visual-qa-static");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "static-layer".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 320,
            height: 180,
        },
        fps: 3.0,
        alpha: true,
        source_beat: "static test".to_string(),
        visual_treatment: "static block".to_string(),
        motion: "claims motion".to_string(),
        safe_zone: "center".to_string(),
        avoid: "static".to_string(),
        nodes: vec![GraphicNode::Rect(RectNode {
            id: "block".to_string(),
            box_rect: Rect {
                x: 40.0,
                y: 40.0,
                width: 120.0,
                height: 60.0,
            },
            fill: Color::Hex("#FFFFFFFF".to_string()),
            animate: Some(NodeAnimation {
                ease: Some(Easing::Linear),
                delay_seconds: None,
                repeat: None,
                yoyo: None,
                origin: None,
                keyframes: vec![
                    AnimationKeyframe {
                        at: 0.0,
                        x: Some(0.0),
                        y: Some(0.0),
                        scale: None,
                        scale_x: None,
                        scale_y: None,
                        opacity: Some(1.0),
                        ..AnimationKeyframe::default()
                    },
                    AnimationKeyframe {
                        at: 1.0,
                        x: Some(0.0),
                        y: Some(0.0),
                        scale: None,
                        scale_x: None,
                        scale_y: None,
                        opacity: Some(1.0),
                        ..AnimationKeyframe::default()
                    },
                ],
            }),
        })],
    };

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("static animated layer should render");

    let errors = run_cpu_visual_qa(
        &layer,
        &output_dir,
        &manifest,
        CpuVisualQaOptions::default(),
    )
    .expect_err("static animated layer should fail QA");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsVisualQaFailed);
    assert!(errors[0].message.contains("static"));
}

#[test]
fn cpu_visual_qa_rejects_text_outside_safe_zone() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let output_dir = dir.path().join("cpu-visual-qa-text-safe-zone");
    let mut layer = valid_text_layer();
    layer.id = "unsafe-text-layer".to_string();
    layer.nodes = vec![GraphicNode::Text(TextNode {
        id: "unsafe-headline".to_string(),
        text: "Edge text".to_string(),
        box_rect: Rect {
            x: 60.0,
            y: 160.0,
            width: 420.0,
            height: 96.0,
        },
        font_size: 48.0,
        font_weight: 800,
        align: "left".to_string(),
        fill: Color::Hex("#FFFFFFFF".to_string()),
        max_lines: Some(1),
        text_reveal: None,
        emphasis: Vec::new(),
        animate: None,
    })];

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("unsafe text layer should render");

    let errors = run_cpu_visual_qa(
        &layer,
        &output_dir,
        &manifest,
        CpuVisualQaOptions::default(),
    )
    .expect_err("text outside safe zone should fail QA");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsVisualQaFailed);
    assert!(errors[0].path.contains("safeZone"));
    assert!(errors[0].message.contains("safe zone"));
}

#[test]
fn cpu_visual_qa_rejects_overdense_text_for_box_and_lines() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let output_dir = dir.path().join("cpu-visual-qa-text-density");
    let mut layer = valid_text_layer();
    layer.id = "dense-text-layer".to_string();
    layer.nodes = vec![GraphicNode::Text(TextNode {
        id: "dense-headline".to_string(),
        text: "This generated caption line is far too dense for one small visual box".to_string(),
        box_rect: Rect {
            x: 240.0,
            y: 180.0,
            width: 220.0,
            height: 64.0,
        },
        font_size: 38.0,
        font_weight: 800,
        align: "left".to_string(),
        fill: Color::Hex("#FFFFFFFF".to_string()),
        max_lines: Some(1),
        text_reveal: None,
        emphasis: Vec::new(),
        animate: None,
    })];

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("dense text layer should render");

    let errors = run_cpu_visual_qa(
        &layer,
        &output_dir,
        &manifest,
        CpuVisualQaOptions::default(),
    )
    .expect_err("overdense text should fail QA");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsVisualQaFailed);
    assert!(errors[0].path.contains("textDensity"));
    assert!(errors[0].message.contains("too dense"));
}

#[test]
fn template_layer_missing_required_field_returns_actionable_error() {
    let mut layer = template_render_layer("kinetic-lower-third-v1");
    layer.fields.remove("subline");

    let errors = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect_err("missing subline should fail");

    assert_eq!(
        errors[0].code,
        GraphicsErrorCode::GraphicsTemplateParamMissing
    );
    assert_eq!(errors[0].path, "fields.subline");
    assert!(errors[0].fix.contains("subline"));
}

#[test]
fn single_field_template_missing_headline_returns_actionable_error() {
    let mut layer = template_render_layer("tracking-highlight-v1");
    layer.fields.remove("headline");

    let errors = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect_err("missing headline should fail");

    assert_eq!(
        errors[0].code,
        GraphicsErrorCode::GraphicsTemplateParamMissing
    );
    assert_eq!(errors[0].path, "fields.headline");
    assert!(errors[0].fix.contains("headline"));
}

#[test]
fn template_layer_unsupported_template_id_returns_actionable_error() {
    let layer = template_render_layer("missing-template-v1");

    let errors = template_layer_to_graphics_ir(
        &layer,
        Dimensions {
            width: 1920,
            height: 1080,
        },
        30.0,
    )
    .expect_err("unsupported template should fail");

    assert_eq!(
        errors[0].code,
        GraphicsErrorCode::GraphicsUnsupportedPrimitive
    );
    assert_eq!(errors[0].path, "templateId");
    assert!(errors[0].message.contains("missing-template-v1"));
    assert!(errors[0].fix.contains("kinetic-lower-third-v1"));
}

#[test]
fn graphics_layer_json_round_trips_raw_color_strings() {
    let raw_layer = serde_json::json!({
        "schemaVersion": 1,
        "id": "agent.metric-pop.v1",
        "role": "overlay",
        "timelineStart": 4.2,
        "durationSeconds": 2.4,
        "dimensions": {
            "width": 1920,
            "height": 1080
        },
        "fps": 30.0,
        "alpha": true,
        "sourceBeat": "Introduce the metric without covering the speaker.",
        "visualTreatment": "floating metric tile with translucent backing",
        "motion": "scale pop, accent sweep, short hold, fade",
        "safeZone": "keep essential text inside 10% margins",
        "avoid": "covering faces, opaque caption slabs, tiny text",
        "nodes": [
            {
                "type": "text",
                "id": "headline",
                "text": "42%",
                "box": {
                    "x": 120.0,
                    "y": 160.0,
                    "width": 420.0,
                    "height": 120.0
                },
                "fontSize": 72.0,
                "fontWeight": 800,
                "align": "center",
                "fill": "#ffffff",
                "maxLines": 1
            },
            {
                "type": "roundedRect",
                "id": "backing",
                "box": {
                    "x": 100.0,
                    "y": 140.0,
                    "width": 460.0,
                    "height": 160.0
                },
                "radius": 18.0,
                "fill": "#101820cc"
            }
        ]
    });

    let layer: GraphicsLayer =
        serde_json::from_value(raw_layer).expect("deserialize raw color strings");
    validate_graphics_layer(&layer).expect("raw #RRGGBB and #RRGGBBAA colors are valid");

    let serialized = serde_json::to_value(layer).expect("serialize graphics layer");

    assert_eq!(serialized["nodes"][0]["fill"], "#ffffff");
    assert_eq!(serialized["nodes"][1]["fill"], "#101820cc");
    assert!(serialized["nodes"][0]["fill"].get("kind").is_none());
}

#[test]
fn unsupported_schema_version_returns_actionable_error() {
    let mut layer = valid_text_layer();
    layer.schema_version = 2;

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsSchemaUnsupported,
        "schemaVersion",
    );
}

#[test]
fn duplicate_or_empty_node_ids_return_actionable_errors() {
    let mut layer = valid_text_layer();
    let mut empty_id = valid_text_node("");
    empty_id.text = "Empty id".to_string();
    let mut duplicate_id = valid_text_node("headline");
    duplicate_id.text = "Duplicate id".to_string();
    layer.nodes.push(GraphicNode::Text(empty_id));
    layer.nodes.push(GraphicNode::Text(duplicate_id));

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsEmptyNodeId,
        "nodes[1].id",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsEmptyNodeId,
        "nodes[2].id",
    );
}

#[test]
fn missing_visual_metadata_returns_actionable_errors() {
    let mut layer = valid_text_layer();
    layer.source_beat = " ".to_string();
    layer.visual_treatment = "".to_string();
    layer.motion = "\t".to_string();
    layer.safe_zone = "".to_string();
    layer.avoid = " ".to_string();

    let errors = expect_validation_errors(layer);

    assert_error(&errors, GraphicsErrorCode::GraphicsTextEmpty, "sourceBeat");
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTextEmpty,
        "visualTreatment",
    );
    assert_error(&errors, GraphicsErrorCode::GraphicsTextEmpty, "motion");
    assert_error(&errors, GraphicsErrorCode::GraphicsTextEmpty, "safeZone");
    assert_error(&errors, GraphicsErrorCode::GraphicsTextEmpty, "avoid");
}

#[test]
fn invalid_dimensions_or_timing_returns_actionable_errors() {
    let mut layer = valid_text_layer();
    layer.dimensions.width = 8;
    layer.timeline_start = f64::INFINITY;
    layer.duration_seconds = 0.0;
    layer.fps = 240.0;

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "dimensions",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidTiming,
        "timelineStart",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidTiming,
        "durationSeconds",
    );
    assert_error(&errors, GraphicsErrorCode::GraphicsInvalidTiming, "fps");
}

#[test]
fn invalid_line_point_and_stroke_width_return_actionable_errors() {
    let mut layer = valid_text_layer();
    layer.nodes = vec![GraphicNode::Line(LineNode {
        id: "accent".to_string(),
        points: vec![
            Point { x: 12.0, y: 24.0 },
            Point {
                x: f64::INFINITY,
                y: f64::NAN,
            },
        ],
        stroke: Color::Hex("#ffffff".to_string()),
        stroke_width: 0.0,
        animate: None,
    })];

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].points[1].x",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].points[1].y",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].strokeWidth",
    );
}

#[test]
fn invalid_polygon_points_and_stroke_width_return_actionable_errors() {
    let mut layer = valid_text_layer();
    layer.nodes = vec![GraphicNode::Polygon(PolygonNode {
        id: "face".to_string(),
        points: vec![
            Point { x: 12.0, y: 24.0 },
            Point {
                x: f64::INFINITY,
                y: f64::NAN,
            },
        ],
        fill: Color::Hex("#39d98a".to_string()),
        stroke: Some(Color::Hex("#ffffff".to_string())),
        stroke_width: Some(0.0),
        animate: None,
    })];

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsUnsupportedPrimitive,
        "nodes[0].points",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].points[1].x",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].points[1].y",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].strokeWidth",
    );
}

#[test]
fn invalid_rounded_rect_radius_returns_actionable_error() {
    let mut layer = valid_text_layer();
    layer.nodes = vec![GraphicNode::RoundedRect(RoundedRectNode {
        id: "backing".to_string(),
        box_rect: Rect {
            x: 100.0,
            y: 100.0,
            width: 400.0,
            height: 160.0,
        },
        radius: -1.0,
        fill: Color::Hex("#101820cc".to_string()),
        animate: None,
    })];

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidDimensions,
        "nodes[0].radius",
    );
}

#[test]
fn invalid_color_returns_actionable_error_with_exact_path() {
    let mut layer = valid_text_layer();
    if let GraphicNode::Text(text) = &mut layer.nodes[0] {
        text.fill = Color::Hex("#fff".to_string());
    }

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidColor,
        "nodes[0].fill",
    );
}

#[test]
fn missing_image_ref_asset_id_returns_actionable_error() {
    let mut layer = valid_text_layer();
    layer.nodes = vec![GraphicNode::ImageRef(ImageRefNode {
        id: "badge".to_string(),
        asset_id: " ".to_string(),
        box_rect: Rect {
            x: 100.0,
            y: 100.0,
            width: 240.0,
            height: 120.0,
        },
        fit: ImageFit::Contain,
        opacity: 1.0,
        animate: None,
    })];

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsImageRefMissing,
        "nodes[0].assetId",
    );
}

#[test]
fn resolves_registered_image_ref_assets() {
    let mut registry = AssetRegistry::new(PathBuf::from("/project"));
    registry.register(ImageAsset {
        asset_id: "image-logo".to_string(),
        relative_path: "generated/graphics/logo.png".to_string(),
        width: 128,
        height: 64,
    });

    let resolved = registry
        .resolve("image-logo")
        .expect("registered image asset should resolve");

    assert_eq!(resolved.asset_id, "image-logo");
    assert_eq!(
        resolved.absolute_path,
        PathBuf::from("/project/generated/graphics/logo.png")
    );
}

#[test]
fn renders_preview_png_and_manifest_for_valid_layer() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "agent.lower-third.preview.v1".to_string(),
        role: GraphicRole::LowerThird,
        timeline_start: 0.0,
        duration_seconds: 2.0,
        dimensions: Dimensions {
            width: 640,
            height: 360,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Identify the speaker without covering the frame.".to_string(),
        visual_treatment: "compact lower third with translucent rounded backing".to_string(),
        motion: "slide in, hold, quick fade out".to_string(),
        safe_zone: "inside 10% margins".to_string(),
        avoid: "opaque slabs, centered title-card layout, tiny text".to_string(),
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: Rect {
                    x: 36.0,
                    y: 254.0,
                    width: 340.0,
                    height: 68.0,
                },
                radius: 14.0,
                fill: Color::Hex("#101820cc".to_string()),
                animate: None,
            }),
            GraphicNode::Text(TextNode {
                id: "name".to_string(),
                text: "Alex Rivera".to_string(),
                box_rect: Rect {
                    x: 58.0,
                    y: 270.0,
                    width: 296.0,
                    height: 36.0,
                },
                font_size: 26.0,
                font_weight: 400,
                align: "left".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
                text_reveal: None,
                emphasis: Vec::new(),
                animate: None,
            }),
        ],
    };
    let output_dir = dir.path().join("graphics/lower-third-preview");

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render preview graphics");

    let preview_path = output_dir.join("preview.png");
    let manifest_path = output_dir.join("manifest.json");
    let frame_path = output_dir.join(manifest.frames_pattern.replace("%06d", "000000"));
    let preview = decode_png(&preview_path);

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 1);
    assert_eq!(manifest.frames_pattern, "frames/frame-%06d.png");
    assert!(preview_path.exists());
    assert!(manifest_path.exists());
    assert!(frame_path.exists());
    assert!(
        std::fs::metadata(preview_path)
            .expect("read preview metadata")
            .len()
            > 0
    );
    assert_eq!(preview.get_pixel(0, 0).0[3], 0);
    assert!(preview.get_pixel(80, 280).0[3] > 0);
    assert!(region_contains_opaque_text_pixel(
        &preview,
        58..354,
        270..306
    ));
}

#[test]
fn renders_transparent_grain_and_vignette_finishing_layers() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "timeline-effects-clip-1".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 160,
            height: 90,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Apply restrained finishing without hiding the source.".to_string(),
        visual_treatment: "transparent grain and edge vignette".to_string(),
        motion: "static finishing texture".to_string(),
        safe_zone: "preserve all source content".to_string(),
        avoid: "opaque overlays and source obstruction".to_string(),
        nodes: vec![
            GraphicNode::Rect(RectNode {
                id: "stylize-grain".to_string(),
                box_rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 160.0,
                    height: 90.0,
                },
                fill: Color::Hex("#FFFFFF2E".to_string()),
                animate: None,
            }),
            GraphicNode::Rect(RectNode {
                id: "stylize-vignette".to_string(),
                box_rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 160.0,
                    height: 90.0,
                },
                fill: Color::Hex("#00000040".to_string()),
                animate: None,
            }),
        ],
    };
    let output_dir = dir.path().join("graphics/effects-preview");

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("effect finishing layer should render");
    let image = image::open(output_dir.join("preview.png"))
        .expect("rendered preview")
        .to_rgba8();
    let center_alpha = image.get_pixel(80, 45)[3];
    let corner_alpha = image.get_pixel(0, 0)[3];

    assert!(manifest.alpha);
    assert!(
        center_alpha > 0,
        "grain should remain visible at the frame center"
    );
    assert!(
        corner_alpha > center_alpha,
        "vignette should darken frame edges more strongly"
    );
}

#[test]
fn renders_polygon_nodes_as_filled_faces() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "agent.isometric.face.v1",
        "role": "diagram",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 160, "height": 120 },
        "fps": 30.0,
        "alpha": true,
        "sourceBeat": "Show a filled isometric face.",
        "visualTreatment": "translucent angled surface with crisp edge stroke",
        "motion": "static preview face",
        "safeZone": "inside frame bounds",
        "avoid": "wireframe-only 3D panels",
        "nodes": [
            {
                "type": "polygon",
                "id": "top-face",
                "points": [
                    { "x": 35.0, "y": 78.0 },
                    { "x": 73.0, "y": 42.0 },
                    { "x": 125.0, "y": 42.0 },
                    { "x": 88.0, "y": 78.0 }
                ],
                "fill": "#39D98ACC",
                "stroke": "#FFFFFFFF",
                "strokeWidth": 2.0
            }
        ]
    }))
    .expect("polygon graphics layer JSON should deserialize");
    let output_dir = dir.path().join("graphics/polygon-preview");

    validate_graphics_layer(&layer).expect("polygon layer should validate");
    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render polygon primitive");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert_eq!(manifest.frame_count, 1);
    assert!(matches!(layer.nodes[0], GraphicNode::Polygon(_)));
    assert!(preview.get_pixel(78, 58).0[3] > 150);
    assert_eq!(preview.get_pixel(10, 10).0[3], 0);
}

#[test]
fn renders_animated_nodes_as_frame_sequence() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "agent.arcade.shot.animated.v1",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 0.4,
        "dimensions": { "width": 240, "height": 160 },
        "fps": 10.0,
        "alpha": true,
        "sourceBeat": "Show a player shot crossing the screen.",
        "visualTreatment": "minimal neon arcade primitive",
        "motion": "shot travels left to right with easing",
        "safeZone": "center action lane",
        "avoid": "static marker",
        "nodes": [
            {
                "type": "rect",
                "id": "shot",
                "box": { "x": 80.0, "y": 80.0, "width": 40.0, "height": 24.0 },
                "fill": "#39D98AFF",
                "animate": {
                    "ease": "linear",
                    "keyframes": [
                        { "at": 0.0, "x": -40.0, "opacity": 1.0 },
                        { "at": 1.0, "x": 40.0, "opacity": 1.0 }
                    ]
                }
            }
        ]
    }))
    .expect("animated graphics layer JSON should deserialize");
    let output_dir = dir.path().join("graphics/animated-shot");

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render animated primitive");

    assert_eq!(manifest.frame_count, 4);
    assert!(output_dir.join("frames/frame-000000.png").exists());
    assert!(output_dir.join("frames/frame-000003.png").exists());
    assert!(!output_dir.join("frames/frame-000004.png").exists());

    let first = decode_png(&output_dir.join("frames/frame-000000.png"));
    let last = decode_png(&output_dir.join("frames/frame-000003.png"));
    assert!(first.get_pixel(50, 90).0[3] > 0);
    assert_eq!(first.get_pixel(130, 90).0[3], 0);
    assert_eq!(last.get_pixel(50, 90).0[3], 0);
    assert!(last.get_pixel(130, 90).0[3] > 0);
    assert_eq!(
        last.get_pixel(150, 90).0[3],
        0,
        "last frame should be sampled at frame start, not exact duration"
    );
}

#[test]
fn renders_rotated_rect_around_configured_origin() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "rotation-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 120 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Rotate a marker around its center.",
        "visualTreatment": "simple rotated rectangle",
        "motion": "settled rotation",
        "safeZone": "inside frame",
        "avoid": "axis-only transform",
        "nodes": [{
            "type": "rect",
            "id": "marker",
            "box": { "x": 45.0, "y": 35.0, "width": 30.0, "height": 50.0 },
            "fill": "#FFFFFFFF",
            "animate": {
                "origin": { "x": "center", "y": "center" },
                "keyframes": [{ "at": 0.0, "rotationDegrees": 45.0 }]
            }
        }]
    }))
    .expect("rotation layer");
    let output_dir = dir.path().join("graphics/rotation-v2");

    render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render rotated rect");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(60, 60).0[3] > 0);
    assert_eq!(
        preview.get_pixel(45, 35).0[3],
        0,
        "corner should move after rotation"
    );
}

#[test]
fn renders_line_path_progress_as_draw_on() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "line-draw-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 40 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Draw a callout line halfway.",
        "visualTreatment": "thin line draw-on",
        "motion": "path progress half",
        "safeZone": "inside frame",
        "avoid": "full line at first frame",
        "nodes": [{
            "type": "line",
            "id": "callout",
            "points": [{ "x": 10.0, "y": 20.0 }, { "x": 110.0, "y": 20.0 }],
            "stroke": "#FFFFFFFF",
            "strokeWidth": 4.0,
            "animate": { "keyframes": [{ "at": 0.0, "pathProgress": 0.5 }] }
        }]
    }))
    .expect("line layer");
    let output_dir = dir.path().join("graphics/line-draw-v2");

    render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render line draw");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(35, 20).0[3] > 0);
    assert_eq!(preview.get_pixel(90, 20).0[3], 0);
}

#[test]
fn renders_clip_progress_as_left_to_right_reveal() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "clip-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 60 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Reveal a bar halfway.",
        "visualTreatment": "rectangular mask reveal",
        "motion": "clip progress half",
        "safeZone": "inside frame",
        "avoid": "full-width first frame",
        "nodes": [{
            "type": "rect",
            "id": "bar",
            "box": { "x": 20.0, "y": 20.0, "width": 80.0, "height": 20.0 },
            "fill": "#FFFFFFFF",
            "animate": { "keyframes": [{ "at": 0.0, "clipProgress": 0.5 }] }
        }]
    }))
    .expect("clip layer");
    let output_dir = dir.path().join("graphics/clip-v2");

    render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render clip reveal");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(35, 30).0[3] > 0);
    assert_eq!(preview.get_pixel(80, 30).0[3], 0);
}

#[test]
fn renders_shadow_glow_and_blur_outside_base_box() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "effects-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 80 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Polish a small card.",
        "visualTreatment": "soft glow and shadow",
        "motion": "depth effects active",
        "safeZone": "inside frame",
        "avoid": "flat unseparated block",
        "nodes": [{
            "type": "rect",
            "id": "card",
            "box": { "x": 40.0, "y": 25.0, "width": 30.0, "height": 20.0 },
            "fill": "#39D98AFF",
            "animate": {
                "keyframes": [{
                    "at": 0.0,
                    "blurRadius": 2.0,
                    "shadowOpacity": 1.0,
                    "glowOpacity": 1.0
                }]
            }
        }]
    }))
    .expect("effects layer");
    let output_dir = dir.path().join("graphics/effects-v2");

    render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render effects");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(50, 35).0[3] > 0);
    assert!(
        max_alpha_in_region(&preview, 72..82, 47..57) > 0,
        "shadow/glow should add alpha outside the base box"
    );
}

#[test]
fn rerender_clears_stale_animation_frames() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let output_dir = dir.path().join("graphics/rerendered-shot");
    let mut layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "agent.arcade.shot.rerendered.v1",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 0.4,
        "dimensions": { "width": 240, "height": 160 },
        "fps": 10.0,
        "alpha": true,
        "sourceBeat": "Show a player shot crossing the screen.",
        "visualTreatment": "minimal neon arcade primitive",
        "motion": "shot travels left to right with easing",
        "safeZone": "center action lane",
        "avoid": "static marker",
        "nodes": [
            {
                "type": "rect",
                "id": "shot",
                "box": { "x": 80.0, "y": 80.0, "width": 40.0, "height": 24.0 },
                "fill": "#39D98AFF",
                "animate": {
                    "ease": "linear",
                    "keyframes": [
                        { "at": 0.0, "x": -40.0, "opacity": 1.0 },
                        { "at": 1.0, "x": 40.0, "opacity": 1.0 }
                    ]
                }
            }
        ]
    }))
    .expect("animated graphics layer JSON should deserialize");
    let assets = AssetRegistry::new(dir.path().to_path_buf());

    let animated_manifest = render_graphics_preview(
        &layer,
        &assets,
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render animated primitive");

    assert_eq!(animated_manifest.frame_count, 4);
    assert!(output_dir.join("frames/frame-000003.png").exists());

    if let GraphicNode::Rect(rect) = &mut layer.nodes[0] {
        rect.animate = None;
    }
    let static_manifest = render_graphics_preview(
        &layer,
        &assets,
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("rerender static primitive");

    assert_eq!(static_manifest.frame_count, 1);
    assert_eq!(
        static_manifest.playback.mode,
        GraphicsPlaybackMode::StaticHold
    );
    assert_eq!(static_manifest.playback.start_number, 0);
    assert_eq!(
        static_manifest.playback.frame_duration_seconds,
        static_manifest.duration_seconds
    );
    assert_eq!(
        static_manifest.playback.expected_frames,
        vec![PathBuf::from("frames/frame-000000.png")]
    );
    assert!(output_dir.join("frames/frame-000000.png").exists());
    assert!(
        !output_dir.join("frames/frame-000001.png").exists(),
        "rerender should remove stale frames that a video backend could otherwise consume"
    );
    assert!(!output_dir.join("frames/frame-000003.png").exists());
}

#[test]
fn rejects_invalid_node_animation_actionably() {
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "agent.bad.animation.v1",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 240, "height": 160 },
        "fps": 30.0,
        "alpha": true,
        "sourceBeat": "Reject malformed animation timing.",
        "visualTreatment": "test layer",
        "motion": "bad keyframe",
        "safeZone": "center",
        "avoid": "invalid at value",
        "nodes": [
            {
                "type": "rect",
                "id": "bad-shot",
                "box": { "x": 80.0, "y": 80.0, "width": 40.0, "height": 24.0 },
                "fill": "#39D98AFF",
                "animate": {
                    "keyframes": [
                        { "at": 1.2, "x": 40.0 }
                    ]
                }
            }
        ]
    }))
    .expect("bad animation layer JSON should deserialize");

    let errors = validate_graphics_layer(&layer).expect_err("invalid animation should fail");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsInvalidTiming);
    assert_eq!(errors[0].path, "nodes[0].animate.keyframes[0].at");
    assert!(
        errors[0].fix.contains("between 0 and 1"),
        "fix should tell the agent the valid keyframe range: {}",
        errors[0].fix
    );
}

#[test]
fn rejects_invalid_motion_v2_animation_fields_actionably() {
    let mut layer = valid_text_layer();
    if let GraphicNode::Text(text) = &mut layer.nodes[0] {
        text.text_reveal = Some(
            serde_json::from_value(serde_json::json!({
                "mode": "character",
                "staggerSeconds": -0.1,
                "order": "forward",
                "maxRevealDurationSeconds": 3.0
            }))
            .expect("text reveal"),
        );
        text.animate = Some(
            serde_json::from_value(serde_json::json!({
                "origin": { "x": "bad-x", "y": "center" },
                "keyframes": [
                    {
                        "at": 0.0,
                        "rotationDegrees": 90.0,
                        "blurRadius": -1.0,
                        "shadowOpacity": 2.0,
                        "glowOpacity": -0.2,
                        "clipProgress": 1.2,
                        "pathProgress": -0.1
                    }
                ]
            }))
            .expect("bad v2 animation JSON should still deserialize"),
        );
    }

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTemplateParamInvalid,
        "nodes[0].animate.origin.x",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTemplateParamInvalid,
        "nodes[0].animate.keyframes[0].rotationDegrees",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTemplateParamInvalid,
        "nodes[0].animate.keyframes[0].blurRadius",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTemplateParamInvalid,
        "nodes[0].animate.keyframes[0].clipProgress",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsTemplateParamInvalid,
        "nodes[0].textReveal.staggerSeconds",
    );
}

#[test]
fn rejects_animated_layers_that_exceed_render_budget() {
    let mut layer = valid_text_layer();
    layer.duration_seconds = 120.0;
    if let GraphicNode::Text(text) = &mut layer.nodes[0] {
        text.animate = Some(NodeAnimation {
            ease: Some(Easing::Linear),
            delay_seconds: None,
            repeat: None,
            yoyo: None,
            origin: None,
            keyframes: vec![
                AnimationKeyframe {
                    at: 0.0,
                    x: None,
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    opacity: Some(0.0),
                    ..AnimationKeyframe::default()
                },
                AnimationKeyframe {
                    at: 1.0,
                    x: None,
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    opacity: Some(1.0),
                    ..AnimationKeyframe::default()
                },
            ],
        });
    }

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsFrameCoverageExceeded,
        "frames",
    );
}

#[test]
fn rejects_animation_repeat_and_delay_that_break_runtime_bounds() {
    let mut layer = valid_text_layer();
    if let GraphicNode::Text(text) = &mut layer.nodes[0] {
        text.animate = Some(NodeAnimation {
            ease: Some(Easing::Linear),
            delay_seconds: Some(layer.duration_seconds),
            repeat: Some(10_000),
            yoyo: Some(true),
            origin: None,
            keyframes: vec![
                AnimationKeyframe {
                    at: 0.0,
                    x: Some(0.0),
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    opacity: None,
                    ..AnimationKeyframe::default()
                },
                AnimationKeyframe {
                    at: 0.0,
                    x: Some(10.0),
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    opacity: None,
                    ..AnimationKeyframe::default()
                },
            ],
        });
    }

    let errors = expect_validation_errors(layer);

    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidTiming,
        "nodes[0].animate.delaySeconds",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidTiming,
        "nodes[0].animate.repeat",
    );
    assert_error(
        &errors,
        GraphicsErrorCode::GraphicsInvalidTiming,
        "nodes[0].animate.keyframes[1].at",
    );
}

#[test]
fn sparse_keyframes_carry_omitted_properties_forward() {
    let animation = NodeAnimation {
        ease: Some(Easing::Linear),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                x: Some(0.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: Some(1.0),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 0.5,
                x: None,
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: Some(0.5),
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                x: Some(100.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
        ],
    };

    let state = evaluate_animation(&animation, 0.75, 1.0);

    assert!((state.x - 75.0).abs() < f64::EPSILON);
    assert!((state.opacity - 0.5).abs() < f64::EPSILON);
}

#[test]
fn property_first_defined_later_interpolates_from_default() {
    let animation = NodeAnimation {
        ease: Some(Easing::Linear),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                x: Some(0.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                x: Some(100.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: Some(0.0),
                ..AnimationKeyframe::default()
            },
        ],
    };

    let state = evaluate_animation(&animation, 0.5, 1.0);

    assert!((state.x - 50.0).abs() < f64::EPSILON);
    assert!((state.opacity - 0.5).abs() < f64::EPSILON);
}

#[test]
fn uniform_text_scale_animation_scales_glyph_size() {
    let mut node = valid_text_node("scale-text");
    node.font_size = 24.0;
    node.animate = Some(NodeAnimation {
        ease: Some(Easing::Linear),
        delay_seconds: None,
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                x: None,
                y: None,
                scale: Some(1.0),
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                x: None,
                y: None,
                scale: Some(2.0),
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
        ],
    });

    let animated = animate_node(&GraphicNode::Text(node), 1.0, 1.0);
    let GraphicNode::Text(text) = animated else {
        panic!("expected text node");
    };

    assert_eq!(text.font_size, 48.0);
}

#[test]
fn repeat_yoyo_animation_ends_on_final_cycle_endpoint() {
    let mut animation = NodeAnimation {
        ease: Some(Easing::Linear),
        delay_seconds: None,
        repeat: Some(1),
        yoyo: Some(true),
        origin: None,
        keyframes: vec![
            AnimationKeyframe {
                at: 0.0,
                x: Some(0.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
            AnimationKeyframe {
                at: 1.0,
                x: Some(100.0),
                y: None,
                scale: None,
                scale_x: None,
                scale_y: None,
                opacity: None,
                ..AnimationKeyframe::default()
            },
        ],
    };

    let reverse_final = evaluate_animation(&animation, 2.0, 2.0);
    assert!((reverse_final.x - 0.0).abs() < f64::EPSILON);

    animation.repeat = Some(2);
    let forward_final = evaluate_animation(&animation, 3.0, 3.0);
    assert!((forward_final.x - 100.0).abs() < f64::EPSILON);
}

#[test]
fn motion_v2_keyframes_deserialize_and_evaluate_new_properties() {
    let animation: NodeAnimation = serde_json::from_value(serde_json::json!({
        "ease": "linear",
        "origin": { "x": "left", "y": "bottom" },
        "keyframes": [
            {
                "at": 0.0,
                "rotationDegrees": -6.0,
                "blurRadius": 4.0,
                "shadowOpacity": 0.0,
                "glowOpacity": 0.0,
                "clipProgress": 0.0,
                "pathProgress": 0.0
            },
            {
                "at": 1.0,
                "rotationDegrees": 0.0,
                "blurRadius": 0.0,
                "shadowOpacity": 0.55,
                "glowOpacity": 0.4,
                "clipProgress": 1.0,
                "pathProgress": 1.0
            }
        ]
    }))
    .expect("motion v2 animation JSON should deserialize");

    let state = evaluate_animation(&animation, 0.5, 1.0);

    assert!((state.rotation_degrees + 3.0).abs() < f64::EPSILON);
    assert!((state.blur_radius - 2.0).abs() < f64::EPSILON);
    assert!((state.shadow_opacity - 0.275).abs() < f64::EPSILON);
    assert!((state.glow_opacity - 0.2).abs() < f64::EPSILON);
    assert!((state.clip_progress - 0.5).abs() < f64::EPSILON);
    assert!((state.path_progress - 0.5).abs() < f64::EPSILON);
}

#[test]
fn transform_origin_round_trips_with_keyword_and_numeric_coordinates() {
    let animation: NodeAnimation = serde_json::from_value(serde_json::json!({
        "origin": { "x": 120.0, "y": "center" },
        "keyframes": [
            { "at": 0.0, "rotationDegrees": 0.0 },
            { "at": 1.0, "rotationDegrees": 12.0 }
        ]
    }))
    .expect("origin should deserialize");

    let json = serde_json::to_value(&animation).expect("serialize origin");

    assert_eq!(json["origin"]["x"], serde_json::json!(120.0));
    assert_eq!(json["origin"]["y"], serde_json::json!("center"));
}

#[test]
fn text_reveal_character_mode_progressively_reveals_text() {
    let mut node = valid_text_node("reveal");
    node.text = "ABCD".to_string();
    node.text_reveal = Some(
        serde_json::from_value(serde_json::json!({
            "mode": "character",
            "staggerSeconds": 0.1,
            "order": "forward",
            "maxRevealDurationSeconds": 0.4
        }))
        .expect("text reveal"),
    );

    let early = animate_node(&GraphicNode::Text(node.clone()), 0.11, 1.0);
    let late = animate_node(&GraphicNode::Text(node), 0.45, 1.0);
    let GraphicNode::Text(early_text) = early else {
        panic!("expected text");
    };
    let GraphicNode::Text(late_text) = late else {
        panic!("expected text");
    };

    assert_eq!(early_text.text, "AB");
    assert_eq!(late_text.text, "ABCD");
}

#[test]
fn renders_semitransparent_text_with_fill_alpha() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut layer = valid_text_layer();
    layer.dimensions = Dimensions {
        width: 320,
        height: 180,
    };
    layer.nodes = vec![GraphicNode::Text(TextNode {
        id: "alpha-text".to_string(),
        text: "ALPHA".to_string(),
        box_rect: Rect {
            x: 24.0,
            y: 40.0,
            width: 260.0,
            height: 80.0,
        },
        font_size: 54.0,
        font_weight: 800,
        align: "left".to_string(),
        fill: Color::Hex("#ffffff80".to_string()),
        max_lines: Some(1),
        text_reveal: None,
        emphasis: Vec::new(),
        animate: None,
    })];
    let output_dir = dir.path().join("graphics/alpha-text");

    render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render semitransparent text");

    let preview = decode_png(&output_dir.join("preview.png"));
    let max_alpha = max_alpha_in_region(&preview, 24..284, 40..120);

    assert!(max_alpha > 0, "expected glyph pixels to be rendered");
    assert!(
        max_alpha <= 128,
        "expected #ffffff80 fill alpha to cap glyph alpha, got {max_alpha}"
    );
}

#[test]
fn renders_image_ref_stretch_into_target_box() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let asset_path = dir.path().join("assets/two-color.png");
    write_test_png(&asset_path, 2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let mut registry = AssetRegistry::new(dir.path().to_path_buf());
    registry.register(ImageAsset {
        asset_id: "two-color".to_string(),
        relative_path: "assets/two-color.png".to_string(),
        width: 2,
        height: 1,
    });
    let layer = image_layer("two-color", ImageFit::Stretch, 32, 24);
    let output_dir = dir.path().join("graphics/stretch-image");

    render_graphics_preview(
        &layer,
        &registry,
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render stretch image");

    let preview = decode_png(&output_dir.join("preview.png"));

    assert_eq!(preview.get_pixel(0, 0).0[3], 0);
    assert_color_near(preview.get_pixel(8, 12).0, [255, 0, 0, 255]);
    assert_color_near(preview.get_pixel(24, 12).0, [0, 0, 255, 255]);
}

#[test]
fn renders_image_ref_cover_without_distorting_aspect_ratio() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let asset_path = dir.path().join("assets/wide-stripes.png");
    write_test_png(
        &asset_path,
        4,
        2,
        &[
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ],
    );
    let mut registry = AssetRegistry::new(dir.path().to_path_buf());
    registry.register(ImageAsset {
        asset_id: "wide-stripes".to_string(),
        relative_path: "assets/wide-stripes.png".to_string(),
        width: 4,
        height: 2,
    });
    let layer = image_layer("wide-stripes", ImageFit::Cover, 20, 20);
    let output_dir = dir.path().join("graphics/cover-image");

    render_graphics_preview(
        &layer,
        &registry,
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("render cover image");

    let preview = decode_png(&output_dir.join("preview.png"));

    assert_eq!(preview.get_pixel(0, 0).0[3], 0);
    assert_color_near(preview.get_pixel(6, 12).0, [0, 255, 0, 255]);
    assert_color_near(preview.get_pixel(13, 12).0, [0, 0, 255, 255]);
}

#[test]
fn unreadable_image_ref_asset_returns_render_failed() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let mut registry = AssetRegistry::new(dir.path().to_path_buf());
    registry.register(ImageAsset {
        asset_id: "missing-file".to_string(),
        relative_path: "assets/missing.png".to_string(),
        width: 16,
        height: 16,
    });

    let errors = render_graphics_preview(
        &image_layer("missing-file", ImageFit::Stretch, 32, 32),
        &registry,
        GraphicsRenderOptions {
            output_dir: dir.path().join("graphics/missing-file"),
        },
    )
    .expect_err("missing file should fail");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsRenderFailed);
    assert_eq!(errors[0].path, "nodes[0].assetId");
}

#[test]
fn image_ref_validation_reports_missing_asset_with_fix() {
    let layer = layer_with_image_ref("missing-image");
    let registry = AssetRegistry::new(PathBuf::from("/project"));

    let errors = validate_graphics_layer_with_assets(&layer, &registry)
        .expect_err("missing imageRef should fail");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsImageRefMissing);
    assert_eq!(errors[0].path, "nodes[0].assetId");
    assert_eq!(
        errors[0].fix,
        "Register the image asset or replace assetId with an approved imageRef."
    );
}

#[test]
fn image_ref_resolve_rejects_unauthorized_paths() {
    let mut registry = AssetRegistry::new(PathBuf::from("/project"));
    registry.register(ImageAsset {
        asset_id: "secret".to_string(),
        relative_path: "../secret.png".to_string(),
        width: 128,
        height: 64,
    });

    let errors = registry
        .resolve("secret")
        .expect_err("parent-directory image path should fail");

    assert_eq!(
        errors[0].code,
        GraphicsErrorCode::GraphicsImageRefUnauthorized
    );
}

fn valid_text_layer() -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: "agent.metric-pop.v1".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 4.2,
        duration_seconds: 2.4,
        dimensions: Dimensions {
            width: 1920,
            height: 1080,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Introduce the metric without covering the speaker.".to_string(),
        visual_treatment: "floating metric tile with translucent backing".to_string(),
        motion: "scale pop, accent sweep, short hold, fade".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "covering faces, opaque caption slabs, tiny text".to_string(),
        nodes: vec![GraphicNode::Text(valid_text_node("headline"))],
    }
}

fn template_render_layer(template_id: &str) -> TemplateRenderLayer {
    let mut fields = BTreeMap::new();
    fields.insert("headline".to_string(), "Olha API".to_string());
    fields.insert("subline".to_string(), "Founder".to_string());
    fields.insert(
        "logoAssetId".to_string(),
        "builtin:v-photo-light".to_string(),
    );

    TemplateRenderLayer {
        item_id: "template-item-1".to_string(),
        template_id: template_id.to_string(),
        label: "Kinetic Lower Third".to_string(),
        timeline_start_seconds: 1.2,
        duration_seconds: 2.4,
        fields,
        visual_treatment: "compact lower-third block with translucent backing".to_string(),
        motion: "slide-and-fade in".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
        preview_variant: "lower-third".to_string(),
    }
}

fn layer_with_image_ref(asset_id: &str) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: "image-overlay".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 1920,
            height: 1080,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Show approved project image.".to_string(),
        visual_treatment: "small image bug in safe zone".to_string(),
        motion: "fade in and fade out".to_string(),
        safe_zone: "inside 10% margins".to_string(),
        avoid: "remote or arbitrary filesystem images".to_string(),
        nodes: vec![GraphicNode::ImageRef(ImageRefNode {
            id: "image".to_string(),
            asset_id: asset_id.to_string(),
            box_rect: Rect {
                x: 100.0,
                y: 100.0,
                width: 240.0,
                height: 120.0,
            },
            fit: ImageFit::Contain,
            opacity: 1.0,
            animate: None,
        })],
    }
}

fn image_layer(asset_id: &str, fit: ImageFit, width: u32, height: u32) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: "image-render".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions { width, height },
        fps: 30.0,
        alpha: true,
        source_beat: "Render approved image asset.".to_string(),
        visual_treatment: "imageRef test layer".to_string(),
        motion: "static preview frame".to_string(),
        safe_zone: "inside frame bounds".to_string(),
        avoid: "external images".to_string(),
        nodes: vec![GraphicNode::ImageRef(ImageRefNode {
            id: "image".to_string(),
            asset_id: asset_id.to_string(),
            box_rect: Rect {
                x: 5.0,
                y: 5.0,
                width: (width - 10) as f64,
                height: (height - 10) as f64,
            },
            fit,
            opacity: 1.0,
            animate: None,
        })],
    }
}

fn valid_text_node(id: &str) -> TextNode {
    TextNode {
        id: id.to_string(),
        text: "42%".to_string(),
        box_rect: Rect {
            x: 120.0,
            y: 160.0,
            width: 420.0,
            height: 120.0,
        },
        font_size: 72.0,
        font_weight: 800,
        align: "center".to_string(),
        fill: Color::Hex("#ffffff".to_string()),
        max_lines: Some(1),
        text_reveal: None,
        emphasis: Vec::new(),
        animate: None,
    }
}

fn expect_validation_errors(layer: GraphicsLayer) -> Vec<ActionableError> {
    validate_graphics_layer(&layer).expect_err("graphics layer should be invalid")
}

fn assert_error(errors: &[ActionableError], code: GraphicsErrorCode, path: &str) {
    assert!(
        errors
            .iter()
            .any(|error| error.code == code && error.path == path),
        "expected {code:?} at {path}, got {errors:#?}"
    );
}

fn decode_png(path: &Path) -> image::RgbaImage {
    image::open(path).expect("decode generated PNG").to_rgba8()
}

fn average_opaque_pixel_saturation(image: &image::RgbaImage) -> f64 {
    let mut total_saturation = 0.0;
    let mut samples = 0_u64;
    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 32 || red.max(green).max(blue) < 48 {
            continue;
        }
        let max_channel = red.max(green).max(blue) as f64;
        let min_channel = red.min(green).min(blue) as f64;
        if max_channel <= f64::EPSILON {
            continue;
        }
        total_saturation += (max_channel - min_channel) / max_channel;
        samples += 1;
    }

    if samples == 0 {
        return 0.0;
    }

    total_saturation / samples as f64
}

fn opaque_luminance_standard_deviation(image: &image::RgbaImage) -> f64 {
    let mut samples = 0_u64;
    let mut mean = 0.0;
    let mut m2 = 0.0;
    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 32 || red.max(green).max(blue) < 48 {
            continue;
        }
        let luminance = red as f64 * 0.2126 + green as f64 * 0.7152 + blue as f64 * 0.0722;
        samples += 1;
        let delta = luminance - mean;
        mean += delta / samples as f64;
        let delta_after_mean = luminance - mean;
        m2 += delta * delta_after_mean;
    }

    if samples < 2 {
        return 0.0;
    }

    (m2 / samples as f64).sqrt()
}

fn average_background_rgb_delta(first: &image::RgbaImage, second: &image::RgbaImage) -> f64 {
    assert_eq!(first.dimensions(), second.dimensions());
    let mut total_delta = 0.0;
    let mut samples = 0_u64;
    let width = first.width();
    let height = first.height();

    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            if x > width / 4 && x < width * 3 / 4 && y > height / 5 && y < height * 4 / 5 {
                continue;
            }
            let [first_red, first_green, first_blue, first_alpha] = first.get_pixel(x, y).0;
            let [second_red, second_green, second_blue, second_alpha] = second.get_pixel(x, y).0;
            if first_alpha < 250 || second_alpha < 250 {
                continue;
            }
            total_delta += ((first_red as f64 - second_red as f64).abs()
                + (first_green as f64 - second_green as f64).abs()
                + (first_blue as f64 - second_blue as f64).abs())
                / 3.0;
            samples += 1;
        }
    }

    if samples == 0 {
        return 0.0;
    }

    total_delta / samples as f64
}

fn write_test_png(path: &Path, width: u32, height: u32, pixels: &[[u8; 4]]) {
    assert_eq!(pixels.len(), (width * height) as usize);
    std::fs::create_dir_all(path.parent().expect("asset parent")).expect("create asset directory");
    let bytes = pixels
        .iter()
        .flat_map(|pixel| pixel.iter().copied())
        .collect::<Vec<_>>();
    image::save_buffer_with_format(
        path,
        &bytes,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("write test PNG");
}

fn region_contains_opaque_text_pixel(
    image: &image::RgbaImage,
    x_range: std::ops::Range<u32>,
    y_range: std::ops::Range<u32>,
) -> bool {
    for y in y_range {
        for x in x_range.clone() {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] == 255 && pixel[0] > 180 && pixel[1] > 180 && pixel[2] > 180 {
                return true;
            }
        }
    }
    false
}

fn max_alpha_in_region(
    image: &image::RgbaImage,
    x_range: std::ops::Range<u32>,
    y_range: std::ops::Range<u32>,
) -> u8 {
    let mut max_alpha = 0;
    for y in y_range {
        for x in x_range.clone() {
            max_alpha = max_alpha.max(image.get_pixel(x, y).0[3]);
        }
    }
    max_alpha
}

fn assert_color_near(actual: [u8; 4], expected: [u8; 4]) {
    let tolerance = 4;
    for channel in 0..4 {
        assert!(
            (actual[channel] as i16 - expected[channel] as i16).abs() <= tolerance,
            "expected {expected:?}, got {actual:?}"
        );
    }
}
