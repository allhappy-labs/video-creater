//! FCPXML dips as Fade To Color transitions, following the verdicts in
//! `docs/research/2026-09-16-nle-transition-interchange-references.md`:
//! dip to black emits Fade To Color without a color param (FCP's default is
//! black), and dip to white stays a cross dissolve with a note because the
//! color param encoding is unverified.

use super::transitions::{add_transition, adjacent_clips_project, export, media_clip};
use video_creater_lib::project::model::{TimelineItemKind, TransitionKind};
use video_creater_lib::project::nle_export::NleXmlFormat;

const CROSS_DISSOLVE_EFFECT: &str = "    <effect id=\"vc-transition-cross-dissolve\" name=\"Cross Dissolve\" uid=\"FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265\"/>\n";
const FADE_TO_COLOR_EFFECT: &str = "    <effect id=\"vc-transition-fade-to-color\" name=\"Fade To Color\" uid=\"FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C\"/>\n";
const AUDIO_CROSSFADE_EFFECT: &str = "    <effect id=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\" uid=\"FFAudioTransition\"/>\n";

#[test]
fn fcpxml_dip_to_black_writes_a_fade_to_color_transition() {
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::DipToBlack,
        1.0,
    );

    let xml = export(&project, NleXmlFormat::DavinciFcpxml);

    assert!(
        xml.contains(&format!("{FADE_TO_COLOR_EFFECT}{AUDIO_CROSSFADE_EFFECT}")),
        "{xml}"
    );
    assert!(!xml.contains("name=\"Cross Dissolve\""), "{xml}");
    assert!(
        xml.contains(
            "            <transition name=\"Dip to Color\" offset=\"84/24s\" duration=\"24/24s\">
              <filter-video ref=\"vc-transition-fade-to-color\" name=\"Fade To Color\"/>
              <filter-audio ref=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\"/>
            </transition>
"
        ),
        "{xml}"
    );
    assert!(!xml.contains("dip-to"), "{xml}");
}

#[test]
fn fcpxml_dip_to_white_follows_the_verified_color_param() {
    // Not verified: dip to white exports as a cross dissolve with a note.
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::DipToWhite,
        1.0,
    );

    let xml = export(&project, NleXmlFormat::DavinciFcpxml);

    assert!(
        xml.contains(&format!("{CROSS_DISSOLVE_EFFECT}{AUDIO_CROSSFADE_EFFECT}")),
        "{xml}"
    );
    assert!(!xml.contains("Fade To Color"), "{xml}");
    assert!(
        xml.contains(
            "            <transition name=\"Cross Dissolve\" offset=\"84/24s\" duration=\"24/24s\">
              <filter-video ref=\"vc-transition-cross-dissolve\" name=\"Cross Dissolve\"/>"
        ),
        "{xml}"
    );
    assert_eq!(
        xml.matches("dip-to-white transitions (exported as cross dissolves)")
            .count(),
        2,
        "{xml}"
    );
    assert!(!xml.contains("dip-to-color"), "{xml}");
}

#[test]
fn fcpxml_declares_only_the_transition_effects_it_uses() {
    let mut crossfade = adjacent_clips_project(false);
    add_transition(
        &mut crossfade,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    let xml = export(&crossfade, NleXmlFormat::DavinciFcpxml);
    assert!(
        xml.contains(&format!(
            "    <format id=\"r1\" frameDuration=\"1/24s\" width=\"1920\" height=\"1080\"/>\n{CROSS_DISSOLVE_EFFECT}{AUDIO_CROSSFADE_EFFECT}    <asset "
        )),
        "{xml}"
    );

    let mut mixed = adjacent_clips_project(false);
    mixed.timeline.tracks[0].items = vec![
        media_clip(
            "item-1",
            "media-1",
            TimelineItemKind::VideoClip,
            0.0,
            3.0,
            2.0,
        ),
        media_clip(
            "item-2",
            "media-1",
            TimelineItemKind::VideoClip,
            3.0,
            3.0,
            6.0,
        ),
        media_clip(
            "item-3",
            "media-1",
            TimelineItemKind::VideoClip,
            6.0,
            2.0,
            9.5,
        ),
    ];
    add_transition(
        &mut mixed,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    add_transition(
        &mut mixed,
        0,
        ("item-2", "item-3"),
        TransitionKind::DipToBlack,
        1.0,
    );
    let xml = export(&mixed, NleXmlFormat::DavinciFcpxml);
    assert!(
        xml.contains(&format!(
            "{CROSS_DISSOLVE_EFFECT}{FADE_TO_COLOR_EFFECT}{AUDIO_CROSSFADE_EFFECT}"
        )),
        "{xml}"
    );
    for effect in [
        CROSS_DISSOLVE_EFFECT,
        FADE_TO_COLOR_EFFECT,
        AUDIO_CROSSFADE_EFFECT,
    ] {
        assert_eq!(xml.matches(effect).count(), 1, "{xml}");
    }
    assert_eq!(xml.matches("<transition ").count(), 2, "{xml}");
}
