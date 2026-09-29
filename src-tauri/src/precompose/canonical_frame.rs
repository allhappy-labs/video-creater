//! Canonical frame sampling at one timeline time.
//!
//! The same sampler flattened composites use (`flatten.rs`), exposed for a
//! single frame: which clips draw, at which source time and opacity, how they
//! take part in transitions, and the composited RGBA frame.

use super::flatten::{
    collect_dependencies, composite_group_layers, frame_program_for_item, sample_clip_at,
    DependencyRenderContext, FlattenGroup,
};
use super::flatten_transitions::FlattenTransitions;
use super::numeric_property;
use crate::frame_compositor::{FrameBlendCompositor, TransitionFrameState};
use crate::project::model::{TimelineSource, TrackKind, TransitionKind, VideoProject};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::Serialize;
use std::path::Path;

/// One clip drawn at a sampled time.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalLayerSample {
    pub track_index: usize,
    pub item_id: String,
    /// Source seconds read at the sampled time, continuing into transition handles.
    pub source_seconds: f64,
    /// Opacity, fades and keyframes at canonical clip time, times the transition factor.
    pub opacity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionFrameState>,
}

/// A dip's opaque solid drawn at a sampled time, beneath its track's clips.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalSolidSample {
    pub track_index: usize,
    pub kind: TransitionKind,
    pub progress: f64,
    pub rgba: [u8; 4],
}

/// What canonical sampling draws at one time, bottom to top.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalFrameSample {
    pub layers: Vec<CanonicalLayerSample>,
    pub solids: Vec<CanonicalSolidSample>,
}

/// The media-backed clips and dip solids canonical sampling draws at `seconds`.
pub fn sample_canonical_frame(
    project: &VideoProject,
    seconds: f64,
) -> PipelineResult<CanonicalFrameSample> {
    let transitions = FlattenTransitions::plan(project);
    let mut frame = CanonicalFrameSample::default();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        frame.solids.extend(
            transitions
                .solids(track_index, f64::NEG_INFINITY, f64::INFINITY)
                .into_iter()
                .filter(|solid| solid.track_index == track_index && solid.window.contains(seconds))
                .map(|solid| CanonicalSolidSample {
                    track_index,
                    kind: solid.window.kind,
                    progress: solid.window.progress(seconds),
                    rgba: solid.rgba,
                }),
        );
        let mut drawn = track
            .items
            .iter()
            .filter(|item| matches!(item.source, TimelineSource::Media { .. }))
            .filter_map(|item| {
                let source_in = numeric_property(&item.properties, "sourceIn").unwrap_or(0.0);
                let speed = numeric_property(&item.properties, "speed").unwrap_or(1.0);
                let item_transitions = transitions.for_item(track_index, &item.id);
                sample_clip_at(item, &item_transitions, source_in, speed, seconds)
                    .map(|sample| (item, sample))
            })
            .collect::<Vec<_>>();
        drawn.sort_by(|left, right| left.0.start_seconds.total_cmp(&right.0.start_seconds));
        for (item, sample) in drawn {
            let local_seconds = sample
                .offset_seconds
                .clamp(0.0, item.duration_seconds.max(0.0));
            let program_opacity = frame_program_for_item(item)?
                .sample(local_seconds)
                .map_err(|error| {
                    vec![sample_error(&format!(
                        "Frame program sampling failed: {error}"
                    ))]
                })?
                .opacity;
            frame.layers.push(CanonicalLayerSample {
                track_index,
                item_id: item.id.clone(),
                source_seconds: sample.source_seconds,
                opacity: program_opacity
                    * sample
                        .transition
                        .map_or(1.0, |transition| transition.opacity),
                transition: sample.transition,
            });
        }
    }
    Ok(frame)
}

/// Composites the canonical RGBA frame at `seconds` for every enabled video
/// track, through the flattened-composite path: straight-alpha sRGB RGBA8 at
/// the project render size, transparent where nothing draws.
pub fn render_canonical_frame_rgba(
    project_dir: &Path,
    project: &VideoProject,
    seconds: f64,
    cancellation: Option<&RenderCancellationToken>,
) -> PipelineResult<Vec<u8>> {
    let fps = project.render_settings.fps;
    let (fps_numerator, fps_denominator) = super::fps_rational(fps)
        .ok_or_else(|| vec![sample_error("Canonical frame rate is invalid.")])?;
    let width = project.render_settings.width;
    let height = project.render_settings.height;
    let Some(top_track_index) = project
        .timeline
        .tracks
        .iter()
        .rposition(|track| track.enabled && track.kind == TrackKind::Video)
    else {
        return Ok(vec![0; width as usize * height as usize * 4]);
    };
    let group = FlattenGroup {
        start_seconds: seconds,
        end_seconds: seconds + 1.0 / fps,
        top_track_index,
        top_item_id: String::new(),
    };
    let transitions = FlattenTransitions::plan(project);
    let dependencies = collect_dependencies(project_dir, project, &group, &transitions)?;
    let solids = transitions.solids(top_track_index, group.start_seconds, group.end_seconds);
    let mut compositor = FrameBlendCompositor::new(cfg!(feature = "gpu-render"));
    let mut canvases = vec![vec![0_u8; width as usize * height as usize * 4]];
    composite_group_layers(
        &dependencies,
        &solids,
        DependencyRenderContext {
            group: &group,
            fps,
            fps_numerator,
            fps_denominator,
            width,
            height,
            canvases: &mut canvases,
            compositor: &mut compositor,
            cancellation,
        },
    )?;
    Ok(canvases.remove(0))
}

fn sample_error(message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.canonicalFrame",
        message,
        "Inspect the project's visual items and render settings.",
    )
}

#[cfg(test)]
#[path = "canonical_frame_tests.rs"]
mod tests;
