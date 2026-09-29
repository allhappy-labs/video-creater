//! Graphics layers (captions, overlays, HyperFrames scenes, effect layers) in canonical
//! captures.
//!
//! GES stacks every graphics layer above every source layer, and a later graphics layer
//! (in `build_project_graphics_render_layers` order: timeline start, then id) above an
//! earlier one. The capture renders the same one-frame graphics layers the range render
//! would and composites frame 0 of each over the sampled video in that order.

use super::super::{
    build_project_graphics_render_layers, project_graphics_artifact_paths,
    render_project_graphics_layers, render_project_graphics_report, RenderedProjectGraphics,
};
use crate::graphics::manifest::GraphicsArtifactManifest;
use crate::project::model::VideoProject;
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::report::RenderGraphicsReport;
use std::path::{Path, PathBuf};

/// Graphics that start within a frame of the playhead are not visible on it in GES.
const LAYER_START_EPSILON_SECONDS: f64 = 1e-6;

/// The graphics drawn on one captured frame.
pub(super) struct CaptureGraphics {
    /// Frame 0 of each visible layer, in composite order (bottom first).
    pub(super) frames: Vec<PathBuf>,
    pub(super) reports: Vec<RenderGraphicsReport>,
    /// Project-relative graphics artifacts (manifests, previews and frames).
    pub(super) artifacts: Vec<String>,
}

/// Renders the graphics layers active on `[playhead, frame_end)` of the prepared, expanded
/// project into `<render_dir>/graphics`.
pub(super) fn render_capture_graphics(
    project_dir: &Path,
    project: &VideoProject,
    playhead_seconds: f64,
    frame_end_seconds: f64,
    render_dir: &str,
    cancellation: &RenderCancellationToken,
) -> PipelineResult<CaptureGraphics> {
    let settings = &project.render_settings;
    let layers = build_project_graphics_render_layers(
        project,
        settings.width,
        settings.height,
        settings.fps,
        Some((playhead_seconds, frame_end_seconds)),
    )?;
    let rendered: Vec<RenderedProjectGraphics> = render_project_graphics_layers(
        &layers,
        project_dir,
        project_dir.join(render_dir).join("graphics"),
        Some(cancellation),
    )?
    .into_iter()
    .filter(|graphics| {
        graphics.timeline_start_seconds <= LAYER_START_EPSILON_SECONDS
            && graphics.manifest.frame_count > 0
    })
    .collect();
    Ok(CaptureGraphics {
        frames: rendered
            .iter()
            .map(|graphics| graphics_frame_path(&graphics.manifest, &graphics.artifact_dir, 0))
            .collect(),
        reports: rendered
            .iter()
            .map(|graphics| render_project_graphics_report(graphics, None))
            .collect(),
        artifacts: project_graphics_artifact_paths(&rendered, project_dir),
    })
}

/// Composites each graphics frame over `canvas` (RGBA, `width` x `height`) in order.
pub(super) fn composite_graphics_frames(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    frames: &[PathBuf],
) -> PipelineResult<()> {
    for frame in frames {
        let layer = image::open(frame)
            .map_err(|error| {
                vec![graphics_error(&format!(
                    "Read graphics frame {} failed: {error}",
                    frame.display()
                ))]
            })?
            .to_rgba8();
        if (layer.width(), layer.height()) != (width, height) {
            return Err(vec![graphics_error(&format!(
                "Graphics frame {} is {}x{}, not the project's {width}x{height}.",
                frame.display(),
                layer.width(),
                layer.height()
            ))]);
        }
        composite_over(canvas, layer.as_raw());
    }
    Ok(())
}

/// Straight-alpha sRGB source-over of `layer` onto `canvas`, per pixel.
pub(super) fn composite_over(canvas: &mut [u8], layer: &[u8]) {
    for (destination, source) in canvas.chunks_exact_mut(4).zip(layer.chunks_exact(4)) {
        let source_alpha = u32::from(source[3]);
        if source_alpha == 0 {
            continue;
        }
        if source_alpha == 255 {
            destination.copy_from_slice(source);
            continue;
        }
        let destination_alpha = u32::from(destination[3]);
        // Alphas scaled to 255 * 255.
        let source_weight = source_alpha * 255;
        let destination_weight = destination_alpha * (255 - source_alpha);
        let out_alpha = source_weight + destination_weight;
        for channel in 0..3 {
            let blended = u32::from(source[channel]) * source_weight
                + u32::from(destination[channel]) * destination_weight;
            destination[channel] = ((blended + out_alpha / 2) / out_alpha) as u8;
        }
        destination[3] = ((out_alpha + 127) / 255) as u8;
    }
}

/// Expands the manifest's frame pattern the way the GES backend does.
fn graphics_frame_path(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: &Path,
    frame_index: u32,
) -> PathBuf {
    let frame = manifest
        .frames_pattern
        .replace("%06d", &format!("{frame_index:06}"))
        .replace("%05d", &format!("{frame_index:05}"))
        .replace("%04d", &format!("{frame_index:04}"))
        .replace("%d", &frame_index.to_string());
    artifact_dir.join(frame)
}

fn graphics_error(message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "canonicalPreview.graphics",
        message,
        "Regenerate the graphics layers and retry the capture.",
    )
}

#[cfg(test)]
mod tests {
    use super::composite_over;

    #[test]
    fn opaque_layers_replace_the_canvas() {
        let mut canvas = vec![10, 20, 30, 255];
        composite_over(&mut canvas, &[200, 100, 50, 255]);
        assert_eq!(canvas, vec![200, 100, 50, 255]);
    }

    #[test]
    fn transparent_layers_leave_the_canvas() {
        let mut canvas = vec![10, 20, 30, 255];
        composite_over(&mut canvas, &[200, 100, 50, 0]);
        assert_eq!(canvas, vec![10, 20, 30, 255]);
    }

    #[test]
    fn half_transparent_layers_blend_over_an_opaque_canvas() {
        let mut canvas = vec![100, 0, 255, 255];
        composite_over(&mut canvas, &[200, 255, 0, 128]);
        // (200 * 128 + 100 * 127) / 255 = 150.2, (255 * 128) / 255 = 128,
        // (255 * 127) / 255 = 127; an opaque canvas stays opaque.
        assert_eq!(canvas, vec![150, 128, 127, 255]);
    }
}
