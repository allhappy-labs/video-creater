//! Clip transitions in Premiere XMEML and DaVinci FCPXML exports.

use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, ProjectTimeline, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTrack, TimelineTransition, TrackKind, TransitionKind, VideoProject,
};
use video_creater_lib::project::nle_export::{export_project_timeline_to_nle_xml, NleXmlFormat};

pub(super) fn media_clip(
    id: &str,
    media_id: &str,
    kind: TimelineItemKind,
    start: f64,
    duration: f64,
    source_in: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: format!("Clip {id}"),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration)),
        ]),
    }
}

/// 24 fps. `item-1` [0, 4) and `item-2` [4, 8) on the first video track, cut at
/// frame 96, with 2 s of handles on both sides; `a-1` [0, 3) and `a-2` [3, 6)
/// on the audio track; both video clips carry linked source audio.
pub(super) fn adjacent_clips_project(with_upper_video_track: bool) -> VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 8.0;
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: None,
        relative_path: "media/voice.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 20.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.timeline.tracks[0].items = vec![
        media_clip(
            "item-1",
            "media-1",
            TimelineItemKind::VideoClip,
            0.0,
            4.0,
            2.0,
        ),
        media_clip(
            "item-2",
            "media-1",
            TimelineItemKind::VideoClip,
            4.0,
            4.0,
            6.0,
        ),
    ];
    for (index, volume_db) in [-1.0, -3.0].into_iter().enumerate() {
        project.timeline.tracks[0].items[index]
            .properties
            .insert("volumeDb".to_string(), json!(volume_db));
    }
    audio_track(&mut project).items = vec![
        media_clip("a-1", "audio-1", TimelineItemKind::AudioClip, 0.0, 3.0, 1.0),
        media_clip("a-2", "audio-1", TimelineItemKind::AudioClip, 3.0, 3.0, 8.0),
    ];
    if with_upper_video_track {
        let mut upper = TimelineTrack::empty("video-2", "Upper", TrackKind::Video);
        upper.items.push(media_clip(
            "up-1",
            "media-1",
            TimelineItemKind::VideoClip,
            1.0,
            2.0,
            0.0,
        ));
        project.timeline.tracks.push(upper);
    }
    project
}

fn audio_track(project: &mut VideoProject) -> &mut TimelineTrack {
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("sample project has an audio track")
}

pub(super) fn add_transition(
    project: &mut VideoProject,
    track_index: usize,
    (left, right): (&str, &str),
    kind: TransitionKind,
    duration_seconds: f64,
) {
    let track_id = project.timeline.tracks[track_index].id.clone();
    apply_project_action(
        project,
        ProjectAction::AddTransition {
            track_id,
            transition: TimelineTransition {
                id: format!("transition-{left}-{right}"),
                left_item_id: left.to_string(),
                right_item_id: right.to_string(),
                kind,
                duration_seconds,
            },
        },
    )
    .expect("transition should be valid");
}

pub(super) fn audio_track_index(project: &VideoProject) -> usize {
    project
        .timeline
        .tracks
        .iter()
        .position(|track| track.kind == TrackKind::Audio)
        .expect("audio track")
}

pub(super) fn export(project: &VideoProject, format: NleXmlFormat) -> String {
    export_project_timeline_to_nle_xml(project, format)
        .expect("export should succeed")
        .xml
}

/// The text between `<clipitem id="{id}">` and its closing tag.
pub(super) fn clipitem<'a>(xml: &'a str, id: &str) -> &'a str {
    let start = xml
        .find(&format!("<clipitem id=\"{id}\">"))
        .unwrap_or_else(|| panic!("clipitem {id} should exist"));
    let end = start + xml[start..].find("</clipitem>").expect("closed clipitem");
    &xml[start..end]
}

pub(super) fn assert_bounds(clip: &str, start: i64, end: i64, source_in: i64, source_out: i64) {
    let expected = format!(
        "<start>{start}</start>\n            <end>{end}</end>\n            <in>{source_in}</in>\n            <out>{source_out}</out>"
    );
    assert!(clip.contains(&expected), "expected {expected} in {clip}");
}

const XMEML_TRANSITION_TAIL: &str = "              <effecttype>transition</effecttype>
              <mediatype>video</mediatype>
              <wipecode>0</wipecode>
              <wipeaccuracy>100</wipeaccuracy>
              <startratio>0</startratio>
              <endratio>1</endratio>
              <reverse>FALSE</reverse>
";

#[test]
fn xmeml_crossfade_writes_a_centered_cross_dissolve_between_the_clips() {
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );

    let xml = export(&project, NleXmlFormat::PremiereXmeml);

    let expected = format!(
        "          </clipitem>
          <transitionitem>
            <start>84</start>
            <end>108</end>
            <alignment>center</alignment>
            <rate>
              <timebase>24</timebase>
              <ntsc>FALSE</ntsc>
            </rate>
            <effect>
              <name>Cross Dissolve</name>
              <effectid>Cross Dissolve</effectid>
              <effectcategory>Dissolve</effectcategory>
{XMEML_TRANSITION_TAIL}            </effect>
          </transitionitem>
          <clipitem id=\"item-2\">"
    );
    assert!(xml.contains(&expected), "{xml}");
    // Edges at the transition are -1 and the source range spans its handles.
    assert_bounds(clipitem(&xml, "item-1"), 0, -1, 48, 156);
    assert_bounds(clipitem(&xml, "item-2"), -1, 192, 132, 240);
    assert!(!xml.contains("limitation"));
}

#[test]
fn xmeml_dips_write_dip_to_color_with_black_or_white() {
    for (kind, channel) in [
        (TransitionKind::DipToBlack, 0),
        (TransitionKind::DipToWhite, 255),
    ] {
        let mut project = adjacent_clips_project(false);
        // Five frames: the odd frame falls after the cut.
        add_transition(&mut project, 0, ("item-1", "item-2"), kind, 5.0 / 24.0);

        let xml = export(&project, NleXmlFormat::PremiereXmeml);

        let expected = format!(
            "          <transitionitem>
            <start>94</start>
            <end>99</end>
            <alignment>center</alignment>
            <rate>
              <timebase>24</timebase>
              <ntsc>FALSE</ntsc>
            </rate>
            <effect>
              <name>Dip to Color Dissolve</name>
              <effectid>Dip to Color Dissolve</effectid>
              <effectcategory>Dissolve</effectcategory>
{XMEML_TRANSITION_TAIL}              <parameter>
                <parameterid>dipcolor</parameterid>
                <name>Color</name>
                <value>
                  <alpha>255</alpha>
                  <red>{channel}</red>
                  <green>{channel}</green>
                  <blue>{channel}</blue>
                </value>
              </parameter>
            </effect>
          </transitionitem>
"
        );
        assert!(xml.contains(&expected), "{kind:?}: {xml}");
        assert_bounds(clipitem(&xml, "item-1"), 0, -1, 48, 147);
        assert_bounds(clipitem(&xml, "item-2"), -1, 192, 142, 240);
    }
}

#[test]
fn xmeml_audio_transitions_write_equal_power_cross_fades() {
    let mut project = adjacent_clips_project(false);
    let audio_index = audio_track_index(&project);
    add_transition(
        &mut project,
        audio_index,
        ("a-1", "a-2"),
        TransitionKind::Crossfade,
        0.5,
    );
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::DipToBlack,
        1.0,
    );

    let xml = export(&project, NleXmlFormat::PremiereXmeml);

    let audio_fade = |start: i64, end: i64, right_id: &str| {
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
    };
    assert!(xml.contains(&audio_fade(66, 78, "a-2")), "{xml}");
    assert_bounds(clipitem(&xml, "a-1"), 0, -1, 24, 102);
    assert_bounds(clipitem(&xml, "a-2"), -1, 144, 186, 264);
    // The video dip also crossfades the clips' linked source audio.
    assert!(xml.contains(&audio_fade(84, 108, "item-2-audio")), "{xml}");
    assert_bounds(clipitem(&xml, "item-1-audio"), 0, -1, 48, 156);
    assert_bounds(clipitem(&xml, "item-2-audio"), -1, 192, 132, 240);
}

#[test]
fn fcpxml_places_cross_dissolve_transitions_in_the_spine() {
    // Dips are covered by `fade_to_color.rs`.
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );

    let xml = export(&project, NleXmlFormat::DavinciFcpxml);

    assert!(xml.contains(
        "    <format id=\"r1\" frameDuration=\"1/24s\" width=\"1920\" height=\"1080\"/>
    <effect id=\"vc-transition-cross-dissolve\" name=\"Cross Dissolve\" uid=\"FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265\"/>
    <effect id=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\" uid=\"FFAudioTransition\"/>
"
    ));
    // Clips keep their sequential offsets; the transition overlaps both.
    assert!(xml.contains(
        "            </asset-clip>
            <transition name=\"Cross Dissolve\" offset=\"84/24s\" duration=\"24/24s\">
              <filter-video ref=\"vc-transition-cross-dissolve\" name=\"Cross Dissolve\"/>
              <filter-audio ref=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\"/>
            </transition>
            <asset-clip name=\"Clip item-2\" ref=\"media-1\" offset=\"96/24s\" duration=\"96/24s\" start=\"144/24s\">"
    ), "{xml}");
    assert!(xml.contains(
        "<asset-clip name=\"Clip item-1\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"48/24s\">"
    ));
    assert!(!xml.contains("dip-to"), "{xml}");
}

#[test]
fn fcpxml_upper_and_audio_lane_transitions_are_emitted() {
    let mut project = adjacent_clips_project(true);
    let audio_index = audio_track_index(&project);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    add_transition(
        &mut project,
        audio_index,
        ("a-1", "a-2"),
        TransitionKind::Crossfade,
        0.5,
    );

    let xml = export(&project, NleXmlFormat::DavinciFcpxml);

    // Connected storylines carry them (see `storylines.rs`).
    assert_eq!(xml.matches("<transition ").count(), 2, "{xml}");
    assert!(!xml.contains("outside the primary storyline"), "{xml}");
    // XMEML keeps one track per kind, so the same transitions still export there.
    let xmeml = export(&project, NleXmlFormat::PremiereXmeml);
    assert_eq!(xmeml.matches("<transitionitem>").count(), 3);
}

#[test]
fn nested_sequence_transitions_are_carried_into_the_flattened_export() {
    let mut project = sample_project();
    let mut nested = adjacent_clips_project(false);
    add_transition(
        &mut nested,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    project.media = nested.media.clone();
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate".to_string(),
        timeline: nested.timeline.clone(),
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 8.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::new();
    project.timeline.duration_seconds = 9.0;

    let xml = export(&project, NleXmlFormat::PremiereXmeml);

    assert!(
        xml.contains("<start>108</start>\n            <end>132</end>"),
        "{xml}"
    );
    // The video dissolve plus the crossfade of the clips' linked source audio.
    assert_eq!(xml.matches("<transitionitem>").count(), 2);
}

#[test]
fn projects_without_transitions_export_byte_identical_xml() {
    // Captured from the writers before transitions were supported.
    let project = adjacent_clips_project(true);

    assert_eq!(
        export(&project, NleXmlFormat::PremiereXmeml),
        include_str!("../fixtures/nle_export/adjacent-clips-without-transitions.xml")
    );
    assert_eq!(
        export(&project, NleXmlFormat::DavinciFcpxml),
        include_str!("../fixtures/nle_export/adjacent-clips-without-transitions.fcpxml")
    );
}
