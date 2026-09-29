//! GES timeline elements for clip transitions: dip solids and the geometry a
//! wipe mask needs.

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::gstreamer_backend::GesCanvasGeometry;
use super::gstreamer_transitions::GesTransitionSolid;
use crate::edit::render_plan::{RenderClip, RenderPlan};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_editing_services as ges;

fn seconds_to_clock_time(seconds: f64) -> gst::ClockTime {
    gst::ClockTime::from_nseconds((seconds.max(0.0) * 1_000_000_000.0).round() as u64)
}

/// Adds the opaque black or white `GESTestClip` beneath a dip transition.
pub(super) fn add_transition_solid(
    layer: &ges::Layer,
    solid: &GesTransitionSolid,
    index: usize,
) -> PipelineResult<()> {
    let path = format!("gstreamer.ges.transitions.solids[{index}]");
    let asset = ges::Asset::request::<ges::TestClip>(None).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            path.clone(),
            "GStreamer/GES could not load the test clip asset for a dip transition.",
            "Use a GStreamer runtime with GES test sources and videotestsrc available.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;
    let clip = layer
        .add_asset(
            &asset,
            Some(seconds_to_clock_time(solid.start_seconds)),
            Some(gst::ClockTime::ZERO),
            Some(seconds_to_clock_time(solid.duration_seconds)),
            ges::TrackType::VIDEO,
        )
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                path.clone(),
                "GStreamer/GES could not add the dip transition solid to the timeline.",
                "Inspect the transition window and retry the render.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })?;
    let test_clip = clip.downcast::<ges::TestClip>().map_err(|_| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            path,
            "GStreamer/GES did not create a test clip for the dip transition solid.",
            "Use a GStreamer runtime with GES test clips available.",
        )]
    })?;
    test_clip.set_vpattern(if solid.white {
        ges::VideoTestPattern::White
    } else {
        ges::VideoTestPattern::Black
    });
    Ok(())
}

fn child_int(ges_clip: &ges::Clip, property: &str) -> Option<i32> {
    ges_clip
        .child_property(property)
        .and_then(|value| value.get::<i32>().ok())
}

/// The clip's box on the canvas as `(left, width)` pixels: the canonical
/// transform when set, otherwise the frame positioner's default placement.
/// A zero default width means the source fills the canvas.
pub(super) fn visual_box_for_render_clip(
    ges_clip: &ges::Clip,
    geometry: Option<GesCanvasGeometry>,
    plan: &RenderPlan,
) -> (f64, f64) {
    if let Some(geometry) = geometry {
        return (f64::from(geometry.x), f64::from(geometry.width));
    }
    match (child_int(ges_clip, "posx"), child_int(ges_clip, "width")) {
        (Some(left), Some(width)) if width > 0 => (f64::from(left), f64::from(width)),
        _ => (0.0, f64::from(plan.width)),
    }
}

/// Pins a canvas-filling clip's height before the wipe binds its width, so
/// the frame positioner keeps the full height while the width animates.
pub(super) fn set_default_visual_box_height(
    ges_clip: &ges::Clip,
    plan: &RenderPlan,
    index: usize,
) -> PipelineResult<()> {
    if child_int(ges_clip, "height").is_some_and(|height| height > 0) {
        return Ok(());
    }
    ges_clip
        .set_child_property("height", plan.height as i32)
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.clips[{index}].transition.wipe"),
                "GStreamer/GES could not pin the clip height for a wipe transition.",
                "Use a GStreamer runtime with GES video source geometry child properties available.",
            )
            .with_detail("gstreamerError", error.to_string())]
        })
}

/// Width in pixels of the clip's decoded source frames.
pub(super) fn visual_natural_width(
    clip: &RenderClip,
    asset: &ges::UriClipAsset,
    index: usize,
) -> PipelineResult<f64> {
    let from_plan = clip
        .properties
        .get("sourceWidth")
        .and_then(serde_json::Value::as_f64)
        .filter(|width| width.is_finite() && *width >= 1.0);
    let from_asset = || {
        asset
            .info()
            .video_streams()
            .first()
            .map(|stream| f64::from(stream.width()))
            .filter(|width| *width >= 1.0)
    };
    from_plan.or_else(from_asset).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderPlanInvalidClip,
            format!("renderPlan.clips[{index}].properties.sourceWidth"),
            "A wipe transition requires the incoming clip's source width.",
            "Probe the source media before exporting a wipe transition.",
        )]
    })
}
