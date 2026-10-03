pub mod error;
pub mod ir;
pub mod mesh;
pub mod primitive_renderer;
pub mod profile;
#[cfg(feature = "gpu-render")]
pub mod renderer;
#[cfg(not(feature = "gpu-render"))]
pub mod renderer {
    use std::path::PathBuf;

    use crate::graphics::manifest::GraphicsArtifactManifest;

    use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
    use super::ir::GpuGraphicsLayer;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GpuRenderOptions {
        pub output_dir: PathBuf,
    }

    pub fn render_gpu_graphics_layer(
        _layer: &GpuGraphicsLayer,
        _options: GpuRenderOptions,
    ) -> GpuGraphicsResult<GraphicsArtifactManifest> {
        Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
            "gpuRender",
            "GPU rendering is not enabled in this build.",
            "Build with the gpu-render feature or use a profile with software fallback.",
        )])
    }
    pub fn render_gpu_graphics_layer_cancellable(
        layer: &GpuGraphicsLayer,
        options: GpuRenderOptions,
        _is_cancelled: impl Fn() -> bool,
    ) -> GpuGraphicsResult<GraphicsArtifactManifest> {
        render_gpu_graphics_layer(layer, options)
    }
}
pub mod shader;
pub mod software_renderer;
pub mod templates;
pub mod validation;
pub mod visual_qa;
