//! Wipes, and audio-track transitions of every kind.
//!
//! No public reference export verifies a wipe effect in XMEML or FCPXML
//! (`docs/research/2026-09-16-nle-transition-interchange-references.md`), so
//! wipes export as cuts with a note in both formats. The renderer crossfades
//! the audio of every transition kind, so XMEML writes `Cross Fade (+3dB)` for
//! audio-track and linked-source-audio transitions of every kind.

use super::transitions::{
    add_transition, adjacent_clips_project, assert_bounds, audio_track_index, clipitem, export,
};
use video_creater_lib::project::model::TransitionKind;
use video_creater_lib::project::nle_export::NleXmlFormat;

const WIPE_NOTE: &str = "Video Creater NLE export limitation: wipe transitions (exported as cuts) are not round-tripped into";

fn audio_cross_fade(start: i64, end: i64, right_id: &str) -> String {
    format!(
        "          </clipitem>
          <transitionitem>
            <start>{start}</start>
            <end>{end}</end>
            <alignment>center</alignment>
            <rate>
              <timebase>24</timebase>
              <ntsc>FALSE</ntsc>
            </rate>
            <effect>
              <name>Cross Fade (+3dB)</name>
              <effectid>KGAudioTransCrossFade3dB</effectid>
              <effecttype>transition</effecttype>
              <mediatype>audio</mediatype>
              <wipecode>0</wipecode>
              <wipeaccuracy>100</wipeaccuracy>
              <startratio>0</startratio>
              <endratio>1</endratio>
              <reverse>FALSE</reverse>
            </effect>
          </transitionitem>
          <clipitem id=\"{right_id}\">"
    )
}

fn video_wipe_project() -> video_creater_lib::project::model::VideoProject {
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Wipe,
        1.0,
    );
    project
}

#[test]
fn xmeml_wipes_export_as_cuts_with_a_limitation_note() {
    let xmeml = export(&video_wipe_project(), NleXmlFormat::PremiereXmeml);

    assert!(!xmeml.contains("<mediatype>video</mediatype>\n              <wipecode>"));
    assert_bounds(clipitem(&xmeml, "item-1"), 0, 96, 48, 144);
    assert_bounds(clipitem(&xmeml, "item-2"), 96, 192, 144, 240);
    for id in ["item-1", "item-2"] {
        assert!(
            clipitem(&xmeml, id).contains(&format!("<description>{WIPE_NOTE} premiereXmeml;")),
            "{xmeml}"
        );
    }
    // The clips' linked source audio still crossfades under the wipe.
    assert_eq!(xmeml.matches("<transitionitem>").count(), 1, "{xmeml}");
    assert!(
        xmeml.contains(&audio_cross_fade(84, 108, "item-2-audio")),
        "{xmeml}"
    );
    assert_bounds(clipitem(&xmeml, "item-1-audio"), 0, -1, 48, 156);
    assert_bounds(clipitem(&xmeml, "item-2-audio"), -1, 192, 132, 240);
}

#[test]
fn fcpxml_wipes_export_as_cuts_with_a_limitation_note() {
    let fcpxml = export(&video_wipe_project(), NleXmlFormat::DavinciFcpxml);

    assert!(!fcpxml.contains("<transition"));
    assert!(!fcpxml.contains("<effect id="));
    assert_eq!(
        fcpxml
            .matches(&format!("<note>{WIPE_NOTE} davinciFcpxml;"))
            .count(),
        2,
        "{fcpxml}"
    );
}

#[test]
fn xmeml_audio_track_transitions_of_every_kind_write_cross_fades() {
    for kind in [
        TransitionKind::Crossfade,
        TransitionKind::DipToBlack,
        TransitionKind::DipToWhite,
        TransitionKind::Wipe,
    ] {
        let mut project = adjacent_clips_project(false);
        let audio_index = audio_track_index(&project);
        add_transition(&mut project, audio_index, ("a-1", "a-2"), kind, 0.5);

        let xml = export(&project, NleXmlFormat::PremiereXmeml);

        assert_eq!(
            xml.matches("<transitionitem>").count(),
            1,
            "{kind:?}: {xml}"
        );
        assert!(
            xml.contains(&audio_cross_fade(66, 78, "a-2")),
            "{kind:?}: {xml}"
        );
        assert_bounds(clipitem(&xml, "a-1"), 0, -1, 24, 102);
        assert_bounds(clipitem(&xml, "a-2"), -1, 144, 186, 264);
        for id in ["a-1", "a-2"] {
            assert!(
                !clipitem(&xml, id).contains("limitation"),
                "{kind:?}: {xml}"
            );
        }
    }
}
