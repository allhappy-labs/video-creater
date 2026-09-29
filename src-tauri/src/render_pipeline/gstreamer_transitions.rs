//! GES rendering of clip transitions.
//!
//! The render plan extends both clips of a transition into their handles so
//! they overlap across the window `[start, start + d]`, with progress
//! `p = (t - start) / d`. GES auto-transitions are not used: they always ramp
//! over the actual overlap, which is wrong for range renders whose window
//! starts before the output. Every transition is instead drawn with explicit
//! control envelopes evaluated over the full window, matching the editor
//! preview:
//!
//! - Layers: the incoming clip sits on a layer above its outgoing clip (one
//!   extra layer per chained transition on a track). Dips add a layer below
//!   the track's clips holding a black or white `GESTestClip`.
//! - Crossfade: outgoing alpha 1, incoming alpha `p`.
//! - Dip to black or white: the solid covers the window; outgoing alpha
//!   `max(0, 1 - 2p)`, incoming alpha `max(0, 2p - 1)`.
//! - Wipe: the incoming clip is masked in canvas space so pixel `x` shows when
//!   `x < p * W`. A top-most `videocrop` effect trims the clip's right side and
//!   the frame positioner `width` shrinks by the same fraction, so the visible
//!   part keeps its scale and position.
//! - Audio: outgoing gain `cos(p * pi / 2)`, incoming gain `sin(p * pi / 2)`.
//!
//! Clip fades are evaluated at canonical clip time clamped to the original
//! clip bounds and keyframes hold their edge values in the handles; both
//! multiply with the transition factor.

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::gstreamer_backend::{
    interpolated_control_value, visual_fade_multiplier, GesTimedControlPoint, GesVisualFadeEnvelope,
};
use super::transition_plan::{TRANSITION_HEAD_SECONDS_PROPERTY, TRANSITION_TAIL_SECONDS_PROPERTY};
use crate::edit::render_plan::{RenderClip, RenderPlan, RenderTransition};
use crate::project::model::TransitionKind;
use std::f64::consts::FRAC_PI_2;

/// Audio equal-power curves are sampled with this many linear segments.
const AUDIO_CURVE_SEGMENTS: usize = 32;
const TIME_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionRole {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TransitionWindow {
    pub(super) kind: TransitionKind,
    pub(super) start_seconds: f64,
    pub(super) duration_seconds: f64,
}

impl TransitionWindow {
    fn new(transition: &RenderTransition) -> Self {
        Self {
            kind: transition.kind,
            start_seconds: transition.start_seconds,
            duration_seconds: transition.duration_seconds,
        }
    }

    fn end_seconds(&self) -> f64 {
        self.start_seconds + self.duration_seconds
    }

    fn progress(&self, output_seconds: f64) -> f64 {
        ((output_seconds - self.start_seconds) / self.duration_seconds).clamp(0.0, 1.0)
    }
}

/// Transition timing of one plan clip, in clip-local seconds from its GES start.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct GesClipTransitions {
    pub(super) clip_start_seconds: f64,
    pub(super) duration_seconds: f64,
    pub(super) head_seconds: f64,
    pub(super) tail_seconds: f64,
    pub(super) incoming: Option<TransitionWindow>,
    pub(super) outgoing: Option<TransitionWindow>,
    /// Layers above the track's base layer; each incoming clip is one above its outgoing clip.
    pub(super) depth: usize,
    pub(super) canvas_width: f64,
    /// Canvas x of the clip's left edge, for the wipe mask.
    pub(super) wipe_box_left: f64,
}

impl GesClipTransitions {
    fn for_clip(clip: &RenderClip, duration_seconds: f64, canvas_width: f64) -> Self {
        let number = |key: &str| {
            clip.properties
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(0.0)
        };
        Self {
            clip_start_seconds: clip.timeline_start_seconds.unwrap_or(0.0),
            duration_seconds,
            head_seconds: number(TRANSITION_HEAD_SECONDS_PROPERTY),
            tail_seconds: number(TRANSITION_TAIL_SECONDS_PROPERTY),
            canvas_width,
            ..Self::default()
        }
    }

    /// Whether the clip draws any part of a transition or includes handles.
    pub(super) fn is_transitioned(&self) -> bool {
        self.incoming.is_some()
            || self.outgoing.is_some()
            || self.head_seconds > 0.0
            || self.tail_seconds > 0.0
    }

    /// Whether the transitions change this clip's alpha anywhere.
    pub(super) fn affects_alpha(&self) -> bool {
        self.incoming.is_some()
            || self.outgoing.is_some_and(|window| {
                matches!(
                    window.kind,
                    TransitionKind::DipToBlack | TransitionKind::DipToWhite
                )
            })
    }

    pub(super) fn wipe(&self) -> Option<TransitionWindow> {
        self.incoming
            .filter(|window| window.kind == TransitionKind::Wipe)
    }

    fn windows(&self) -> impl Iterator<Item = (TransitionRole, TransitionWindow)> {
        [
            self.outgoing
                .map(|window| (TransitionRole::Outgoing, window)),
            self.incoming
                .map(|window| (TransitionRole::Incoming, window)),
        ]
        .into_iter()
        .flatten()
    }

    /// The canonical clip's duration, without transition handles.
    pub(super) fn canonical_duration_seconds(&self, timeline_duration_seconds: f64) -> f64 {
        (timeline_duration_seconds - self.head_seconds - self.tail_seconds).max(0.0)
    }

    /// Canonical clip time at `local` seconds, clamped to the canonical clip.
    pub(super) fn canonical_seconds(&self, local: f64, canonical_duration: f64) -> f64 {
        (local - self.head_seconds).clamp(0.0, canonical_duration)
    }

    fn output_seconds(&self, local: f64) -> f64 {
        self.clip_start_seconds + local
    }

    /// Wipe reveal of canvas pixels at `local`: `p * W`.
    pub(super) fn wipe_reveal_pixels(&self, local: f64) -> Option<f64> {
        self.wipe()
            .map(|window| window.progress(self.output_seconds(local)) * self.canvas_width)
    }

    /// Alpha multiplier the transitions apply to this clip at `local` seconds.
    pub(super) fn visual_alpha_factor(&self, local: f64) -> f64 {
        let output = self.output_seconds(local);
        self.windows()
            .map(|(role, window)| {
                let progress = window.progress(output);
                match (role, window.kind) {
                    (TransitionRole::Outgoing, TransitionKind::DipToBlack)
                    | (TransitionRole::Outgoing, TransitionKind::DipToWhite) => {
                        (1.0 - 2.0 * progress).max(0.0)
                    }
                    (TransitionRole::Incoming, TransitionKind::DipToBlack)
                    | (TransitionRole::Incoming, TransitionKind::DipToWhite) => {
                        (2.0 * progress - 1.0).max(0.0)
                    }
                    (TransitionRole::Incoming, TransitionKind::Crossfade) => progress,
                    // Hides the one-pixel sliver the mask keeps before the reveal
                    // reaches the clip: the crop cannot remove the whole frame.
                    (TransitionRole::Incoming, TransitionKind::Wipe) => {
                        (progress * self.canvas_width - self.wipe_box_left).clamp(0.0, 1.0)
                    }
                    (TransitionRole::Outgoing, TransitionKind::Crossfade)
                    | (TransitionRole::Outgoing, TransitionKind::Wipe) => 1.0,
                }
            })
            .product()
    }

    /// Gain multiplier the transitions apply to this audio clip at `local` seconds.
    pub(super) fn audio_gain_factor(&self, local: f64) -> f64 {
        let output = self.output_seconds(local);
        self.windows()
            .map(|(role, window)| {
                let angle = window.progress(output) * FRAC_PI_2;
                match role {
                    TransitionRole::Outgoing => angle.cos(),
                    TransitionRole::Incoming => angle.sin(),
                }
            })
            .product()
    }

    /// Local times at which visual transition envelopes are sampled: the
    /// window edges, the cut, the wipe's first revealed pixel, and every
    /// output frame inside the window.
    pub(super) fn visual_event_seconds(&self, fps: f64) -> Vec<f64> {
        let mut times = Vec::new();
        for (role, window) in self.windows() {
            let start = window.start_seconds;
            let end = window.end_seconds();
            times.extend([start, start + window.duration_seconds / 2.0, end]);
            if role == TransitionRole::Incoming && window.kind == TransitionKind::Wipe {
                let first_pixel = window.duration_seconds / self.canvas_width.max(1.0);
                let reveal_start = start
                    + window.duration_seconds * self.wipe_box_left / self.canvas_width.max(1.0);
                times.extend([reveal_start, reveal_start + first_pixel]);
            }
            if fps.is_finite() && fps > 0.0 {
                let first_frame = (start * fps).ceil() as i64;
                let last_frame = (end * fps).floor() as i64;
                times.extend((first_frame..=last_frame).map(|frame| frame as f64 / fps));
            }
        }
        self.local_times(times)
    }

    /// Local times at which audio transition curves are sampled.
    pub(super) fn audio_event_seconds(&self) -> Vec<f64> {
        let times = self
            .windows()
            .flat_map(|(_, window)| {
                (0..=AUDIO_CURVE_SEGMENTS).map(move |segment| {
                    window.start_seconds
                        + window.duration_seconds * segment as f64 / AUDIO_CURVE_SEGMENTS as f64
                })
            })
            .collect();
        self.local_times(times)
    }

    fn local_times(&self, output_times: Vec<f64>) -> Vec<f64> {
        output_times
            .into_iter()
            .map(|output| output - self.clip_start_seconds)
            .filter(|local| (0.0..=self.duration_seconds).contains(local))
            .collect()
    }
}

/// A solid colour clip beneath a dip transition, in output seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GesTransitionSolid {
    pub(super) track_index: u32,
    pub(super) start_seconds: f64,
    pub(super) duration_seconds: f64,
    pub(super) white: bool,
}

/// How a plan's transitions map onto GES layers and clips.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct GesTransitionLayout {
    pub(super) clips: Vec<GesClipTransitions>,
    pub(super) audio_clips: Vec<GesClipTransitions>,
    pub(super) solids: Vec<GesTransitionSolid>,
}

impl GesTransitionLayout {
    pub(super) fn new(plan: &RenderPlan) -> PipelineResult<Self> {
        let canvas_width = f64::from(plan.width);
        let clips = clip_transitions(
            &plan.clips,
            &plan.transitions,
            "renderPlan.transitions",
            canvas_width,
            |clip| {
                clip.properties
                    .get("timelineDurationSeconds")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(clip.source_out - clip.source_in)
            },
        )?;
        let audio_clips = clip_transitions(
            &plan.audio_clips,
            &plan.audio_transitions,
            "renderPlan.audioTransitions",
            canvas_width,
            // GES audio clips last `sourceOut - sourceIn`.
            |clip| clip.source_out - clip.source_in,
        )?;
        let solids = plan
            .transitions
            .iter()
            .filter_map(|transition| {
                let white = match transition.kind {
                    TransitionKind::DipToBlack => false,
                    TransitionKind::DipToWhite => true,
                    TransitionKind::Crossfade | TransitionKind::Wipe => return None,
                };
                let start_seconds = transition.start_seconds.max(0.0);
                let end_seconds = transition.start_seconds + transition.duration_seconds;
                (end_seconds - start_seconds > TIME_EPSILON).then(|| GesTransitionSolid {
                    track_index: plan.clips[transition.left_clip_index].timeline_track_index,
                    start_seconds,
                    duration_seconds: end_seconds - start_seconds,
                    white,
                })
            })
            .collect();
        Ok(Self {
            clips,
            audio_clips,
            solids,
        })
    }

    /// The highest clip layer depth used on a visual track.
    pub(super) fn max_video_depth(&self, plan: &RenderPlan, track_index: u32) -> usize {
        plan.clips
            .iter()
            .zip(&self.clips)
            .filter(|(clip, _)| clip.timeline_track_index == track_index)
            .map(|(_, transitions)| transitions.depth)
            .max()
            .unwrap_or(0)
    }

    pub(super) fn has_solid(&self, track_index: u32) -> bool {
        self.solids
            .iter()
            .any(|solid| solid.track_index == track_index)
    }
}

fn invalid_transition(path: String, message: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderPlanInvalidClip,
        path,
        message,
        "Rebuild the render plan so each transition joins two overlapping clips on one track.",
    )]
}

fn clip_transitions(
    clips: &[RenderClip],
    transitions: &[RenderTransition],
    path: &str,
    canvas_width: f64,
    duration: impl Fn(&RenderClip) -> f64,
) -> PipelineResult<Vec<GesClipTransitions>> {
    let mut result = clips
        .iter()
        .map(|clip| GesClipTransitions::for_clip(clip, duration(clip), canvas_width))
        .collect::<Vec<_>>();
    let mut order = (0..transitions.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        transitions[*left]
            .start_seconds
            .total_cmp(&transitions[*right].start_seconds)
    });
    for index in order {
        let transition = &transitions[index];
        let path = format!("{path}[{index}]");
        let (left, right) = (transition.left_clip_index, transition.right_clip_index);
        if left >= clips.len() || right >= clips.len() || left == right {
            return Err(invalid_transition(
                path,
                "Render transition clip indices must name two different plan clips.",
            ));
        }
        if clips[left].timeline_track_index != clips[right].timeline_track_index
            || clips[left].timeline_start_seconds.is_none()
            || clips[right].timeline_start_seconds.is_none()
        {
            return Err(invalid_transition(
                path,
                "Render transition clips must be placed on the same track.",
            ));
        }
        if !transition.start_seconds.is_finite()
            || !transition.duration_seconds.is_finite()
            || transition.duration_seconds <= 0.0
        {
            return Err(invalid_transition(
                path,
                "Render transition timing must be finite with a positive duration.",
            ));
        }
        if result[left].outgoing.is_some() || result[right].incoming.is_some() {
            return Err(invalid_transition(
                path,
                "Each clip end can take part in only one render transition.",
            ));
        }
        let window = TransitionWindow::new(transition);
        result[left].outgoing = Some(window);
        result[right].incoming = Some(window);
        result[right].depth = result[left].depth + 1;
    }
    Ok(result)
}

/// Samples `value` at `times` plus the clip bounds, sorted and deduplicated.
fn sample_points(
    mut times: Vec<f64>,
    duration_seconds: f64,
    value: impl Fn(f64) -> f64,
) -> Vec<GesTimedControlPoint> {
    times.extend([0.0, duration_seconds]);
    times.retain(|seconds| seconds.is_finite() && (0.0..=duration_seconds).contains(seconds));
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() <= TIME_EPSILON);
    times
        .into_iter()
        .map(|seconds| GesTimedControlPoint {
            seconds,
            value: value(seconds),
        })
        .collect()
}

fn fade_event_seconds(
    transitions: &GesClipTransitions,
    fade: Option<GesVisualFadeEnvelope>,
    canonical_duration: f64,
) -> Vec<f64> {
    let head = transitions.head_seconds;
    let mut times = vec![head, head + canonical_duration];
    if let Some(fade) = fade {
        times.push(head + fade.fade_in_seconds.min(canonical_duration));
        times.push(head + (canonical_duration - fade.fade_out_seconds).max(fade.fade_in_seconds));
    }
    times
}

/// Alpha envelope of a clip that takes part in a transition: opacity,
/// keyframes and fades at clamped canonical time, times the transition factor.
pub(super) fn transition_alpha_control_points(
    opacity: f64,
    timeline_duration_seconds: f64,
    fade: Option<GesVisualFadeEnvelope>,
    opacity_keyframes: &[GesTimedControlPoint],
    transitions: &GesClipTransitions,
    fps: f64,
) -> Vec<GesTimedControlPoint> {
    let canonical_duration = transitions.canonical_duration_seconds(timeline_duration_seconds);
    let mut times = transitions.visual_event_seconds(fps);
    times.extend(fade_event_seconds(transitions, fade, canonical_duration));
    times.extend(
        opacity_keyframes
            .iter()
            .map(|keyframe| keyframe.seconds + transitions.head_seconds),
    );
    sample_points(times, transitions.duration_seconds, |local| {
        let canonical = transitions.canonical_seconds(local, canonical_duration);
        (interpolated_control_value(opacity_keyframes, canonical, opacity)
            * visual_fade_multiplier(fade, canonical)
            * transitions.visual_alpha_factor(local))
        .clamp(0.0, 1.0)
    })
}

/// Linear gain envelope of an audio clip: gain, fades and transition curves.
/// It always ends with a point at the clip end: GStreamer's value-array
/// interpolation, which `volume` uses, extends the slope of the final segment
/// past the last point (measured on GStreamer 1.24.2).
pub(super) fn audio_gain_control_points(
    gain: f64,
    timeline_duration_seconds: f64,
    fade: Option<GesVisualFadeEnvelope>,
    transitions: &GesClipTransitions,
) -> Vec<GesTimedControlPoint> {
    let canonical_duration = transitions.canonical_duration_seconds(timeline_duration_seconds);
    let mut times = transitions.audio_event_seconds();
    times.extend(fade_event_seconds(transitions, fade, canonical_duration));
    sample_points(times, transitions.duration_seconds, |local| {
        let canonical = transitions.canonical_seconds(local, canonical_duration);
        let value =
            gain * visual_fade_multiplier(fade, canonical) * transitions.audio_gain_factor(local);
        // cos(pi / 2) is not exactly zero; keep silent ends exactly silent.
        if value < TIME_EPSILON {
            0.0
        } else {
            value
        }
    })
}

/// Wipe mask envelopes of an incoming clip, sampled at `times`:
/// `(frame positioner width, top-most videocrop right)` per local time.
pub(super) struct WipeMask<'a> {
    pub(super) transitions: &'a GesClipTransitions,
    /// Clip box left edge (canvas pixels) at a local time.
    pub(super) box_left: &'a dyn Fn(f64) -> f64,
    /// Clip box width (canvas pixels) at a local time.
    pub(super) box_width: &'a dyn Fn(f64) -> f64,
    /// Width in pixels of the frame reaching the wipe crop at a local time.
    pub(super) frame_width: &'a dyn Fn(f64) -> f64,
}

impl WipeMask<'_> {
    fn visible_fraction(&self, local: f64) -> f64 {
        let reveal = self
            .transitions
            .wipe_reveal_pixels(local)
            .unwrap_or(self.transitions.canvas_width);
        let width = (self.box_width)(local).max(1.0);
        ((reveal - (self.box_left)(local)) / width).clamp(0.0, 1.0)
    }

    pub(super) fn points(
        &self,
        times: Vec<f64>,
    ) -> (Vec<GesTimedControlPoint>, Vec<GesTimedControlPoint>) {
        let widths = sample_points(times.clone(), self.transitions.duration_seconds, |local| {
            ((self.visible_fraction(local) * (self.box_width)(local)).round()).max(1.0)
        });
        let crops = sample_points(times, self.transitions.duration_seconds, |local| {
            let frame_width = (self.frame_width)(local).round().max(1.0);
            ((1.0 - self.visible_fraction(local)) * frame_width)
                .round()
                .clamp(0.0, frame_width - 1.0)
        });
        (widths, crops)
    }
}

/// Value of a hold-at-edges linear envelope, like a GStreamer interpolation
/// control source.
pub(super) fn held_control_value(
    points: &[GesTimedControlPoint],
    seconds: f64,
    fallback: f64,
) -> f64 {
    match (points.first(), points.last()) {
        (Some(first), _) if seconds <= first.seconds => first.value,
        (_, Some(last)) if seconds >= last.seconds => last.value,
        _ => interpolated_control_value(points, seconds, fallback),
    }
}

#[cfg(test)]
#[path = "gstreamer_transitions_tests.rs"]
mod tests;
