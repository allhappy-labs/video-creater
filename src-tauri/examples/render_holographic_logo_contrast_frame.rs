use std::collections::BTreeMap;
use std::path::PathBuf;

use video_creater_lib::edit::render_plan::TemplateRenderLayer;
use video_creater_lib::graphics::assets::AssetRegistry;
use video_creater_lib::graphics::ir::Dimensions;
use video_creater_lib::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use video_creater_lib::graphics::templates::template_layer_to_graphics_ir;

fn main() {
    let mut fields = BTreeMap::new();
    fields.insert(
        "logoAssetId".to_string(),
        "builtin:v-photo-light".to_string(),
    );

    let template = TemplateRenderLayer {
        item_id: "holographic-logo-contrast-frame".to_string(),
        template_id: "holographic-logo-cutout-v1".to_string(),
        label: "Holographic Logo Contrast Frame".to_string(),
        timeline_start_seconds: 0.0,
        duration_seconds: 1.0 / 24.0,
        preview_variant: "holographic-logo".to_string(),
        fields,
        visual_treatment:
            "dark gradient background with a crisp high-contrast holographic metal logo cutout"
                .to_string(),
        motion: "single review frame from the animated shader".to_string(),
        safe_zone: "logo remains centered inside the central 80% safe zone".to_string(),
        avoid: "dirty grain, dithered texture, retro scan bars, or opaque text slabs".to_string(),
    };

    let dimensions = Dimensions {
        width: 1920,
        height: 1080,
    };
    let graphics = template_layer_to_graphics_ir(&template, dimensions, 24.0)
        .expect("holographic template should expand");
    let output_dir = PathBuf::from(
        "renders/holographic-logo-contrast-frame/output/graphics/holographic-logo-contrast-frame",
    );
    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(PathBuf::from(".")),
        GraphicsRenderOptions {
            output_dir: output_dir.clone(),
        },
    )
    .expect("holographic still frame should render");

    println!("preview={}", output_dir.join("preview.png").display());
    println!("frames={}", output_dir.join("frames").display());
    println!("frame_count={}", manifest.frame_count);
}
