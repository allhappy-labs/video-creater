use std::collections::BTreeMap;
use std::path::PathBuf;

use video_creater_lib::edit::render_plan::TemplateRenderLayer;
use video_creater_lib::graphics::assets::AssetRegistry;
use video_creater_lib::graphics::ir::Dimensions;
use video_creater_lib::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use video_creater_lib::graphics::templates::template_layer_to_graphics_ir;
use video_creater_lib::graphics::webm_export::{
    export_frame_sequence_to_webm, WebmEncoder, WebmExportOptions,
};

fn main() {
    let mut fields = BTreeMap::new();
    fields.insert(
        "logoAssetId".to_string(),
        "builtin:v-photo-light".to_string(),
    );

    let template = TemplateRenderLayer {
        item_id: "holographic-logo-20s-full-hd".to_string(),
        template_id: "holographic-logo-cutout-v1".to_string(),
        label: "Holographic Logo 20s Full HD".to_string(),
        timeline_start_seconds: 0.0,
        duration_seconds: 20.0,
        preview_variant: "holographic-logo".to_string(),
        fields,
        visual_treatment:
            "dark gradient background with a polished pearlescent holographic metal logo cutout"
                .to_string(),
        motion: "slow organic shader shimmer across the logo mask".to_string(),
        safe_zone: "logo remains centered inside the central 80% safe zone".to_string(),
        avoid:
            "dirty grain, dithered texture, retro scan bars, hard game-like bands, or text slabs"
                .to_string(),
    };

    let dimensions = Dimensions {
        width: 1920,
        height: 1080,
    };
    let fps = 24.0;
    let graphics = template_layer_to_graphics_ir(&template, dimensions.clone(), fps)
        .expect("holographic template");
    let render_output_dir = PathBuf::from("renders/holographic-logo-20s-full-hd/output");
    let output_dir = render_output_dir.join("graphics/holographic-logo-20s-full-hd");
    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(PathBuf::from(".")),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("holographic 20s frame sequence should render");

    println!("preview={}", output_dir.join("preview.png").display());
    println!("frames={}", output_dir.join("frames").display());
    println!("frame_count={}", manifest.frame_count);

    let webm_path = render_output_dir.join("holographic-logo-20s-full-hd.webm");
    export_frame_sequence_to_webm(&WebmExportOptions {
        frames_pattern: output_dir.join(&manifest.frames_pattern),
        output_path: webm_path.clone(),
        dimensions,
        fps,
        frame_count: manifest.frame_count,
        encoder: WebmEncoder::Vp8,
        target_bitrate_bps: 6_000_000,
    })
    .expect("holographic 20s WebM should export through Rust-owned GStreamer");
    println!("webm={}", webm_path.display());
}
