//! DaVinci FCPXML transitions.
//!
//! FCPXML uses a sequential spine: clips keep their offsets and durations and a
//! `<transition>` placed between them overlaps both through their handles, with
//! `offset` at the transition start. Effects are declared once in
//! `<resources>`, only when used, in a fixed order. The ids and names cite
//! `docs/research/2026-09-16-nle-transition-interchange-references.md`:
//!
//! - crossfade: `Cross Dissolve` (`FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265`, R1/R2);
//! - dip to black: `Fade To Color` (`FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C`,
//!   Resolve export R1) without a color param, since black is Final Cut Pro's
//!   default (R6);
//! - dip to white: a cross dissolve with a limitation note, because the
//!   `color` param encoding is not verified;
//! - the audio of every video transition: `Audio Crossfade` (`FFAudioTransition`, R2/R3).
//!
//! Transitions on the primary storyline (lane 0) sit in the sequence spine.
//! Transitions on other lanes (upper video tracks and audio tracks) sit inside
//! connected secondary storylines, `<spine lane="N">`, anchored in the covering
//! primary clip or in a gap (see `fcpxml_storylines.rs`; structure verified
//! from Final Cut Pro exports, R3). Transitions on audio lanes, of every kind,
//! write only `<filter-audio>` (the DTD allows `filter-video?`; no reference
//! export was found, so this is the recorded fallback). Video wipes export as
//! cuts, and so do storylines that would anchor on a retimed clip and
//! transitions touching a reversed clip (see `transitions.rs`).

use super::transitions::{NleTransitionSpan, TransitionMedia};
use super::NleClip;
use crate::project::model::TransitionKind;

pub(super) const FCPXML_CROSS_DISSOLVE_ID: &str = "vc-transition-cross-dissolve";
pub(super) const FCPXML_FADE_TO_COLOR_ID: &str = "vc-transition-fade-to-color";
pub(super) const FCPXML_AUDIO_CROSSFADE_ID: &str = "vc-transition-audio-crossfade";

/// The video effect an emitted FCPXML transition uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FcpxmlVideoTransition {
    CrossDissolve,
    /// `color` is a verified `<param name="color">` value; `None` keeps the
    /// effect's default (black).
    FadeToColor {
        color: Option<&'static str>,
    },
}

impl FcpxmlVideoTransition {
    pub(super) fn for_span(span: &NleTransitionSpan) -> Self {
        match span.kind {
            TransitionKind::DipToBlack => Self::FadeToColor { color: None },
            TransitionKind::Crossfade | TransitionKind::DipToWhite | TransitionKind::Wipe => {
                Self::CrossDissolve
            }
        }
    }
}

/// Whether a transition can join its clips in FCPXML, before anchoring is
/// considered: transitions touching a reversed clip are cut; otherwise audio
/// always crossfades, and video wipes have no verified effect.
pub(super) fn fcpxml_joins(span: &NleTransitionSpan) -> bool {
    !span.reversed_clip
        && (span.media == TransitionMedia::Audio || span.kind != TransitionKind::Wipe)
}

pub(super) fn fcpxml_emits(span: &NleTransitionSpan) -> bool {
    fcpxml_joins(span) && !span.retimed_host
}

/// Declares the transition effects the emitted transitions use, once each, in a
/// fixed order: cross dissolve, fade to color, audio crossfade.
pub(super) fn push_fcpxml_transition_effects(xml: &mut String, clips: &[NleClip<'_>]) {
    let spans: Vec<NleTransitionSpan> = clips
        .iter()
        .filter_map(|clip| clip.outgoing_transition.filter(fcpxml_emits))
        .collect();
    if spans.is_empty() {
        return;
    }
    let used: Vec<FcpxmlVideoTransition> = spans
        .iter()
        .filter(|span| span.media == TransitionMedia::Video)
        .map(FcpxmlVideoTransition::for_span)
        .collect();
    if used.contains(&FcpxmlVideoTransition::CrossDissolve) {
        xml.push_str(&format!(
            "    <effect id=\"{FCPXML_CROSS_DISSOLVE_ID}\" name=\"Cross Dissolve\" uid=\"FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265\"/>\n"
        ));
    }
    if used
        .iter()
        .any(|effect| matches!(effect, FcpxmlVideoTransition::FadeToColor { .. }))
    {
        xml.push_str(&format!(
            "    <effect id=\"{FCPXML_FADE_TO_COLOR_ID}\" name=\"Fade To Color\" uid=\"FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C\"/>\n"
        ));
    }
    xml.push_str(&format!(
        "    <effect id=\"{FCPXML_AUDIO_CROSSFADE_ID}\" name=\"Audio Crossfade\" uid=\"FFAudioTransition\"/>\n"
    ));
}

/// Writes an emitted transition at `offset_frames` in its spine, indented by
/// `indent` spaces. Video writes its video effect and the audio crossfade;
/// audio writes only the audio crossfade.
pub(super) fn push_fcpxml_transition(
    xml: &mut String,
    span: NleTransitionSpan,
    offset_frames: i64,
    fps: i64,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let effect =
        (span.media == TransitionMedia::Video).then(|| FcpxmlVideoTransition::for_span(&span));
    let name = match effect {
        None => "Audio Crossfade",
        Some(FcpxmlVideoTransition::CrossDissolve) => "Cross Dissolve",
        Some(FcpxmlVideoTransition::FadeToColor { .. }) => "Dip to Color",
    };
    xml.push_str(&format!(
        "{pad}<transition name=\"{name}\" offset=\"{offset_frames}/{fps}s\" duration=\"{}/{fps}s\">\n",
        span.end_frames - span.start_frames,
    ));
    match effect {
        None => {}
        Some(FcpxmlVideoTransition::CrossDissolve) => xml.push_str(&format!(
            "{pad}  <filter-video ref=\"{FCPXML_CROSS_DISSOLVE_ID}\" name=\"Cross Dissolve\"/>\n"
        )),
        Some(FcpxmlVideoTransition::FadeToColor { color: None }) => xml.push_str(&format!(
            "{pad}  <filter-video ref=\"{FCPXML_FADE_TO_COLOR_ID}\" name=\"Fade To Color\"/>\n"
        )),
        Some(FcpxmlVideoTransition::FadeToColor { color: Some(color) }) => {
            xml.push_str(&format!(
                "{pad}  <filter-video ref=\"{FCPXML_FADE_TO_COLOR_ID}\" name=\"Fade To Color\">\n"
            ));
            xml.push_str(&format!(
                "{pad}    <param name=\"color\" key=\"3\" value=\"{color}\"/>\n"
            ));
            xml.push_str(&format!("{pad}  </filter-video>\n"));
        }
    }
    xml.push_str(&format!(
        "{pad}  <filter-audio ref=\"{FCPXML_AUDIO_CROSSFADE_ID}\" name=\"Audio Crossfade\"/>\n"
    ));
    xml.push_str(&format!("{pad}</transition>\n"));
}
