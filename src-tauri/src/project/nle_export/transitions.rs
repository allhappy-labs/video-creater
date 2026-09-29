//! Clip transitions in NLE interchange exports.
//!
//! # Timing
//!
//! A transition is centered on the cut, which sits at the right clip's
//! timeline start (the same cut the render plan and the preview window use).
//! In whole frames a transition of `d` frames covers
//! `[cut - floor(d / 2), cut - floor(d / 2) + d)`. The canonical clips never
//! overlap; both writers borrow the unused source media (handles) instead.
//!
//! # Premiere XMEML
//!
//! A `<transitionitem>` sits between the two `<clipitem>`s of its track, with
//! `alignment` `center`, the sequence `rate`, and an FCP7 effect:
//!
//! - crossfade: `Cross Dissolve` (category `Dissolve`),
//! - dips: `Dip to Color Dissolve` with a `dipcolor` of black or white,
//! - audio clips on audio tracks, and the linked source audio of two video
//!   clips, for every transition kind (wipes included): `Cross Fade (+3dB)`
//!   (`KGAudioTransCrossFade3dB`), the equal-power crossfade the renderer uses
//!   for the audio of every transition.
//!
//! Like Premiere and FCP7 exports, a clip edge that meets a transition is
//! written as `-1`, and the clip's `in`/`out` extend into the handle so the
//! source range spans the whole transition.
//!
//! # DaVinci FCPXML
//!
//! The FCPXML transition writers live in `fcpxml_transitions.rs`; this module
//! only resolves the spans they read and labels what FCPXML can't carry
//! (`docs/research/2026-09-16-nle-transition-interchange-references.md`): dip
//! to white exports as a cross dissolve because the Fade To Color color param
//! is unverified, and transitions whose connected storyline would anchor on a
//! retimed primary clip export as cuts because anchor offsets on retimed clips
//! are unverified.
//!
//! # Wipes
//!
//! No public reference export verifies a wipe effect id in either format
//! (`docs/research/2026-09-16-nle-transition-interchange-references.md`), so
//! video wipes export as cuts with a limitation note on both clips. In XMEML
//! their linked source audio and audio-track wipes still crossfade.
//!
//! # Reversed clips
//!
//! Neither format writes reverse, so a reversed clip exports playing forward
//! over its source window. Its handles lie past the opposite ends of that
//! window (the head after `sourceOut`, the tail before `sourceIn`, see
//! `SourceWindow::head_handle_seconds`), so borrowing the forward handles
//! would read media the render never uses, or media before the file starts.
//! Every transition touching a reversed clip, audio included, exports as a
//! cut with a limitation note on both clips in both formats.

use super::fcpxml_storylines::mark_retimed_storyline_hosts;
use super::{
    push_number_element, push_text_element, seconds_to_frames, xmeml_has_source_audio, NleClip,
};
use crate::project::model::{
    TimelineTrack, TimelineTransition, TrackKind, TransitionKind, VideoProject,
};
use crate::project::reverse::is_reversed;

/// A transition resolved to sequence frames and attached to its two clips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NleTransitionSpan {
    pub(super) kind: TransitionKind,
    pub(super) start_frames: i64,
    pub(super) cut_frames: i64,
    pub(super) end_frames: i64,
    /// Audio for transitions on audio tracks, video otherwise.
    pub(super) media: TransitionMedia,
    /// Set when FCPXML would have to anchor this transition's storyline on a
    /// retimed primary clip, which is unverified, so FCPXML cuts it.
    pub(super) retimed_host: bool,
    /// Set when either clip plays reversed; both formats cut it (see the
    /// module docs).
    pub(super) reversed_clip: bool,
}

impl NleTransitionSpan {
    fn head_frames(self) -> i64 {
        self.cut_frames - self.start_frames
    }

    fn tail_frames(self) -> i64 {
        self.end_frames - self.cut_frames
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransitionMedia {
    Video,
    Audio,
}

/// Carries a track's transitions through nested-sequence expansion, keeping
/// only those whose clips both survived and namespacing their item ids the
/// same way as the expanded items.
pub(super) fn expanded_track_transitions(
    track: &TimelineTrack,
    kept_item_ids: &[&str],
    namespace: &str,
) -> Vec<TimelineTransition> {
    let expanded_id = |item_id: &str| {
        if namespace == "root" {
            item_id.to_string()
        } else {
            format!("{namespace}:{item_id}")
        }
    };
    track
        .transitions
        .iter()
        .filter(|transition| {
            kept_item_ids.contains(&transition.left_item_id.as_str())
                && kept_item_ids.contains(&transition.right_item_id.as_str())
        })
        .map(|transition| TimelineTransition {
            id: expanded_id(&transition.id),
            left_item_id: expanded_id(&transition.left_item_id),
            right_item_id: expanded_id(&transition.right_item_id),
            kind: transition.kind,
            duration_seconds: transition.duration_seconds,
        })
        .collect()
}

/// Attaches each transition to its clips. A transition is attached only when
/// its clips are emitted back to back and still meet within one frame;
/// anything else exports as a plain cut.
pub(super) fn attach_nle_transitions(project: &VideoProject, clips: &mut [NleClip<'_>], fps: i64) {
    for track in &project.timeline.tracks {
        if !matches!(track.kind, TrackKind::Video | TrackKind::Audio) {
            continue;
        }
        for transition in &track.transitions {
            let Some(left_index) = clips
                .iter()
                .position(|clip| clip.item.id == transition.left_item_id)
            else {
                continue;
            };
            let right_index = left_index + 1;
            let Some(right) = clips.get(right_index) else {
                continue;
            };
            let left = &clips[left_index];
            if right.item.id != transition.right_item_id
                || (left.timeline_end_frames - right.timeline_start_frames).abs() > 1
            {
                continue;
            }
            let duration_frames = seconds_to_frames(transition.duration_seconds, fps).max(1);
            let cut_frames = right.timeline_start_frames;
            let start_frames = cut_frames - duration_frames / 2;
            let span = NleTransitionSpan {
                kind: transition.kind,
                start_frames,
                cut_frames,
                end_frames: start_frames + duration_frames,
                media: if matches!(track.kind, TrackKind::Audio) {
                    TransitionMedia::Audio
                } else {
                    TransitionMedia::Video
                },
                retimed_host: false,
                reversed_clip: is_reversed(left.item) || is_reversed(right.item),
            };
            clips[left_index].outgoing_transition = Some(span);
            clips[right_index].incoming_transition = Some(span);
        }
    }
    mark_retimed_storyline_hosts(clips);
}

/// Whether XMEML writes `span` for `media`: transitions touching a reversed
/// clip are cut; otherwise audio always crossfades, and video cuts wipes, which
/// have no verified XMEML effect.
fn xmeml_emits(span: &NleTransitionSpan, media: TransitionMedia) -> bool {
    !span.reversed_clip && (media == TransitionMedia::Audio || span.kind != TransitionKind::Wipe)
}

/// The transitions XMEML writes on a clip's own track.
pub(super) fn xmeml_clip_transitions(
    clip: &NleClip<'_>,
) -> (Option<NleTransitionSpan>, Option<NleTransitionSpan>) {
    let emits = |span: &NleTransitionSpan| xmeml_emits(span, span.media);
    (
        clip.incoming_transition.filter(emits),
        clip.outgoing_transition.filter(emits),
    )
}

/// The audio crossfades XMEML writes on a video clip's linked source audio:
/// only between two clips that both carry a source audio clip.
pub(super) fn xmeml_source_audio_transitions(
    clips: &[NleClip<'_>],
    index: usize,
) -> (Option<NleTransitionSpan>, Option<NleTransitionSpan>) {
    let clip = &clips[index];
    let emits = |span: &NleTransitionSpan| xmeml_emits(span, TransitionMedia::Audio);
    let incoming = clip
        .incoming_transition
        .filter(emits)
        .filter(|_| index > 0 && xmeml_has_source_audio(&clips[index - 1]));
    let outgoing = clip
        .outgoing_transition
        .filter(emits)
        .filter(|_| clips.get(index + 1).is_some_and(xmeml_has_source_audio));
    (incoming, outgoing)
}

/// Writes `start`, `end`, `in` and `out`. An edge that meets a written
/// transition is `-1`, and the source range extends through its handle.
pub(super) fn push_xmeml_clip_bounds(
    xml: &mut String,
    clip: &NleClip<'_>,
    incoming: Option<NleTransitionSpan>,
    outgoing: Option<NleTransitionSpan>,
) {
    let source_frames = |timeline_frames: i64| (timeline_frames as f64 * clip.speed).round() as i64;
    let start = incoming.map_or(clip.timeline_start_frames, |_| -1);
    let end = outgoing.map_or(clip.timeline_end_frames, |_| -1);
    let source_in = incoming.map_or(clip.source_in_frames, |span| {
        clip.source_in_frames - source_frames(span.head_frames())
    });
    let source_out = outgoing.map_or(clip.source_out_frames, |span| {
        clip.source_out_frames + source_frames(span.tail_frames())
    });
    push_number_element(xml, 12, "start", start);
    push_number_element(xml, 12, "end", end);
    push_number_element(xml, 12, "in", source_in);
    push_number_element(xml, 12, "out", source_out);
}

pub(super) fn push_xmeml_transition(
    xml: &mut String,
    span: NleTransitionSpan,
    fps: i64,
    media: TransitionMedia,
) {
    xml.push_str("          <transitionitem>\n");
    push_number_element(xml, 12, "start", span.start_frames);
    push_number_element(xml, 12, "end", span.end_frames);
    push_text_element(xml, 12, "alignment", "center");
    xml.push_str("            <rate>\n");
    push_number_element(xml, 14, "timebase", fps);
    xml.push_str("              <ntsc>FALSE</ntsc>\n");
    xml.push_str("            </rate>\n");
    xml.push_str("            <effect>\n");
    let dip_color = match span.kind {
        TransitionKind::DipToBlack => Some(0),
        TransitionKind::DipToWhite => Some(255),
        TransitionKind::Crossfade | TransitionKind::Wipe => None,
    };
    match (media, dip_color) {
        (TransitionMedia::Audio, _) => {
            push_text_element(xml, 14, "name", "Cross Fade (+3dB)");
            push_text_element(xml, 14, "effectid", "KGAudioTransCrossFade3dB");
        }
        (TransitionMedia::Video, None) => {
            push_text_element(xml, 14, "name", "Cross Dissolve");
            push_text_element(xml, 14, "effectid", "Cross Dissolve");
            push_text_element(xml, 14, "effectcategory", "Dissolve");
        }
        (TransitionMedia::Video, Some(_)) => {
            push_text_element(xml, 14, "name", "Dip to Color Dissolve");
            push_text_element(xml, 14, "effectid", "Dip to Color Dissolve");
            push_text_element(xml, 14, "effectcategory", "Dissolve");
        }
    }
    push_text_element(xml, 14, "effecttype", "transition");
    push_text_element(
        xml,
        14,
        "mediatype",
        match media {
            TransitionMedia::Video => "video",
            TransitionMedia::Audio => "audio",
        },
    );
    push_number_element(xml, 14, "wipecode", 0);
    push_number_element(xml, 14, "wipeaccuracy", 100);
    push_number_element(xml, 14, "startratio", 0);
    push_number_element(xml, 14, "endratio", 1);
    push_text_element(xml, 14, "reverse", "FALSE");
    if let (TransitionMedia::Video, Some(channel)) = (media, dip_color) {
        xml.push_str("              <parameter>\n");
        push_text_element(xml, 16, "parameterid", "dipcolor");
        push_text_element(xml, 16, "name", "Color");
        xml.push_str("                <value>\n");
        push_number_element(xml, 18, "alpha", 255);
        push_number_element(xml, 18, "red", channel);
        push_number_element(xml, 18, "green", channel);
        push_number_element(xml, 18, "blue", channel);
        xml.push_str("                </value>\n");
        xml.push_str("              </parameter>\n");
    }
    xml.push_str("            </effect>\n");
    xml.push_str("          </transitionitem>\n");
}

/// Limitation labels for transitions touching `clip` that `format` cannot carry.
pub(super) fn transition_limitations(clip: &NleClip<'_>, format: &str) -> Vec<&'static str> {
    let mut labels = Vec::new();
    for span in [clip.incoming_transition, clip.outgoing_transition]
        .into_iter()
        .flatten()
    {
        let fcpxml = format == "davinciFcpxml";
        let label = match span.kind {
            _ if span.reversed_clip => {
                Some("transitions touching a reversed clip (exported as cuts)")
            }
            TransitionKind::Wipe if span.media == TransitionMedia::Video => {
                Some("wipe transitions (exported as cuts)")
            }
            _ if fcpxml && span.retimed_host => {
                Some("transitions connected to a retimed clip (exported as cuts)")
            }
            TransitionKind::DipToWhite if fcpxml && span.media == TransitionMedia::Video => {
                Some("dip-to-white transitions (exported as cross dissolves)")
            }
            _ => None,
        };
        if let Some(label) = label.filter(|label| !labels.contains(label)) {
            labels.push(label);
        }
    }
    labels
}
