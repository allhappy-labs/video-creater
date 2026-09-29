//! Correct source timing for GES visual clip speed (`videorate rate=`).
//!
//! GES models a `videorate rate=s` time effect as reading source time
//! `inpoint + t * s` at clip-local time `t`, and seeks the source that way.
//! `videorate` before GStreamer 1.28 does not: on a new segment it divides the
//! segment start by the rate and then uses that divided value as the origin of
//! both its output and its input time (`base_ts`), so output time `out` reads
//! input time `inpoint / s + (out - inpoint / s) * s`. Measured on the staged
//! 1.24.2 runtime with source 1.0-3.0 s at speed 2, clip-local time `t` showed
//! source `0.5 + 2t`. Frames before the source in-point never arrive, so the
//! first `inpoint * (1 - 1 / s)` source seconds froze on the in-point frame
//! (frames 0-7), and slowed clips ran past their source range and ended early.
//! Upstream fixed this in 1.28 (commit 51d43529, "videorate: Refactor so
//! upstream/downstream time domains are properly decoupled").
//!
//! The fix shifts each buffer entering `videorate` by `start / s - start`,
//! where `start` is the current input segment start, so the time `videorate`
//! reads for a frame is the one its own mapping expects. Segments, seeks and
//! the output are unchanged, and no frames are duplicated.

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_editing_services as ges;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Whether `videorate` from GStreamer `version` (`major.minor[.micro]`)
/// mixes its input and output time origins.
fn videorate_mixes_time_origins(version: &str) -> bool {
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(major), Some(minor)) => (major, minor) < (1, 28),
        _ => true,
    }
}

/// The buffer time `videorate` must see for a frame at `pts`, given the input
/// segment `start` and the effect `rate`.
fn corrected_input_pts(pts: u64, start: u64, rate: f64) -> u64 {
    let shifted = pts as f64 + start as f64 / rate - start as f64;
    shifted.max(0.0).round() as u64
}

/// Makes the `videorate` element of a GES speed effect read source time
/// `inpoint + t * speed`, on runtimes whose `videorate` needs it.
pub(super) fn correct_speed_effect_timing(
    effect: &ges::Effect,
    index: usize,
) -> PipelineResult<()> {
    let path = format!("gstreamer.ges.clips[{index}].properties.speed");
    let element = TimelineElementExt::lookup_child(effect, "rate")
        .and_then(|(child, _)| child.downcast::<gst::Element>().ok())
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                path.clone(),
                "GStreamer/GES did not create the videorate element for clip speed.",
                "Use a GStreamer runtime with the approved videorate effect available.",
            )]
        })?;
    let version = element
        .factory()
        .and_then(|factory| factory.plugin())
        .map(|plugin| plugin.version().to_string())
        .unwrap_or_default();
    if !videorate_mixes_time_origins(&version) {
        return Ok(());
    }
    let rate = element.property::<f64>("rate");
    let sink = element.static_pad("sink").ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            path,
            "GStreamer/GES videorate has no sink pad.",
            "Use a GStreamer runtime with the approved videorate effect available.",
        )]
    })?;
    let segment_start = Arc::new(AtomicU64::new(0));
    sink.add_probe(
        gst::PadProbeType::EVENT_DOWNSTREAM
            | gst::PadProbeType::BUFFER
            | gst::PadProbeType::BUFFER_LIST,
        move |_, info| {
            match info.data.as_mut() {
                Some(gst::PadProbeData::Event(event)) => {
                    if let gst::EventView::Segment(segment) = event.view() {
                        if let Some(segment) = segment.segment().downcast_ref::<gst::ClockTime>() {
                            let start = segment.start().map_or(0, gst::ClockTime::nseconds);
                            segment_start.store(start, Ordering::SeqCst);
                        }
                    }
                }
                Some(gst::PadProbeData::Buffer(buffer)) => {
                    shift_buffer(buffer, segment_start.load(Ordering::SeqCst), rate);
                }
                Some(gst::PadProbeData::BufferList(list)) => {
                    let start = segment_start.load(Ordering::SeqCst);
                    list.make_mut().foreach_mut(|mut buffer, _| {
                        shift_buffer(&mut buffer, start, rate);
                        std::ops::ControlFlow::Continue(Some(buffer))
                    });
                }
                _ => {}
            }
            gst::PadProbeReturn::Ok
        },
    );
    Ok(())
}

fn shift_buffer(buffer: &mut gst::Buffer, start: u64, rate: f64) {
    let Some(pts) = buffer.pts() else {
        return;
    };
    let corrected = corrected_input_pts(pts.nseconds(), start, rate);
    buffer
        .make_mut()
        .set_pts(gst::ClockTime::from_nseconds(corrected));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_runtimes_before_1_28_need_the_correction() {
        assert!(videorate_mixes_time_origins("1.24.2"));
        assert!(videorate_mixes_time_origins("1.26.5"));
        assert!(!videorate_mixes_time_origins("1.28.0"));
        assert!(!videorate_mixes_time_origins("2.0"));
        assert!(videorate_mixes_time_origins(""));
    }

    #[test]
    fn input_times_move_to_the_origin_videorate_expects() {
        const SECOND: u64 = 1_000_000_000;
        // In-point 1 s at speed 2: videorate reads 0.5 s for output 0.5 s, so
        // the in-point frame must arrive stamped 0.5 s.
        assert_eq!(corrected_input_pts(SECOND, SECOND, 2.0), SECOND / 2);
        assert_eq!(corrected_input_pts(2 * SECOND, SECOND, 2.0), 3 * SECOND / 2);
        // Slowed clips move later: in-point 1 s at speed 0.5 reads from 2 s.
        assert_eq!(corrected_input_pts(SECOND, SECOND, 0.5), 2 * SECOND);
        // A zero in-point needs no shift.
        assert_eq!(corrected_input_pts(SECOND, 0, 2.0), SECOND);
    }
}
