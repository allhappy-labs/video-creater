pub mod animation;
pub mod assets;
pub mod error;
pub mod ir;
pub mod manifest;
pub mod motion_presets;
#[cfg(feature = "graphics-render")]
pub mod renderer;
#[cfg(not(feature = "graphics-render"))]
pub mod renderer {
    use std::path::PathBuf;

    use super::assets::AssetRegistry;
    use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
    use super::ir::GraphicsLayer;
    use super::manifest::GraphicsArtifactManifest;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GraphicsRenderOptions {
        pub output_dir: PathBuf,
    }

    pub fn render_graphics_preview(
        _layer: &GraphicsLayer,
        _assets: &AssetRegistry,
        _options: GraphicsRenderOptions,
    ) -> ActionableResult<GraphicsArtifactManifest> {
        Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "graphicsRender",
            "Graphics rasterization is not enabled in this build.",
            "Build with the graphics-render feature before rendering graphics previews.",
        )])
    }

    pub fn render_graphics_preview_cancellable(
        layer: &GraphicsLayer,
        assets: &AssetRegistry,
        options: GraphicsRenderOptions,
        is_cancelled: impl Fn() -> bool,
    ) -> ActionableResult<GraphicsArtifactManifest> {
        if is_cancelled() {
            return Err(vec![ActionableError::new(
                GraphicsErrorCode::GraphicsRenderFailed,
                "cancelled",
                "Graphics rendering was cancelled.",
                "Start a new render attempt when you are ready to retry.",
            )]);
        }

        render_graphics_preview(layer, assets, options)
    }
    pub fn render_graphics_preview_range_cancellable(
        layer: &GraphicsLayer,
        _source_range: Option<(f64, f64)>,
        assets: &AssetRegistry,
        options: GraphicsRenderOptions,
        is_cancelled: impl Fn() -> bool,
    ) -> ActionableResult<GraphicsArtifactManifest> {
        render_graphics_preview_cancellable(layer, assets, options, is_cancelled)
    }
}
pub mod shader_noise;
pub mod templates;
pub mod validation;
pub mod visual_qa;
pub mod webm_export;
