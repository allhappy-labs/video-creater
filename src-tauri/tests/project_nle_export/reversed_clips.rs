//! Reversed clips: transitions touching them, and nested-sequence trims.
//!
//! Neither format writes reverse, so a reversed clip exports playing forward
//! over its source window, while its handles sit at the other ends of that
//! window (head after `sourceOut`, tail before `sourceIn`). Borrowing the
//! forward handles would read media the clip never had, so these transitions
//! export as cuts with a limitation note on both clips.

use super::transitions::{
    add_transition, adjacent_clips_project, assert_bounds, audio_track_index, clipitem, export,
};
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    ProjectTimeline, TimelineSource, TransitionKind, VideoProject,
};
use video_creater_lib::project::nle_export::NleXmlFormat;

const REVERSED_NOTE: &str = "transitions touching a reversed clip (exported as cuts)";

/// `item-2` reads `[0, 4)` of the 12 s media reversed, so its 8 s head handle
/// lies after `sourceOut` and nothing lies before `sourceIn`.
pub(super) fn reversed_transition_project() -> VideoProject {
    let mut project = adjacent_clips_project(false);
    let right = &mut project.timeline.tracks[0].items[1];
    right.properties.insert("sourceIn".to_string(), json!(0.0));
    right.properties.insert("sourceOut".to_string(), json!(4.0));
    right.properties.insert("reverse".to_string(), json!(true));
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    project
}

#[test]
fn xmeml_transitions_touching_a_reversed_clip_export_as_cuts() {
    let xml = export(&reversed_transition_project(), NleXmlFormat::PremiereXmeml);

    assert_eq!(xml.matches("<transitionitem>").count(), 0, "{xml}");
    assert_bounds(clipitem(&xml, "item-1"), 0, 96, 48, 144);
    assert_bounds(clipitem(&xml, "item-2"), 96, 192, 0, 96);
    assert_bounds(clipitem(&xml, "item-1-audio"), 0, 96, 48, 144);
    assert_bounds(clipitem(&xml, "item-2-audio"), 96, 192, 0, 96);
    for id in ["item-1", "item-2"] {
        assert!(clipitem(&xml, id).contains(REVERSED_NOTE), "{xml}");
    }
}

#[test]
fn fcpxml_transitions_touching_a_reversed_clip_export_as_cuts() {
    let xml = export(&reversed_transition_project(), NleXmlFormat::DavinciFcpxml);

    assert!(!xml.contains("<transition"), "{xml}");
    assert!(!xml.contains("<effect id="), "{xml}");
    assert!(
        xml.contains(
            "<asset-clip name=\"Clip item-2\" ref=\"media-1\" offset=\"96/24s\" duration=\"96/24s\" start=\"0/24s\">"
        ),
        "{xml}"
    );
    assert_eq!(xml.matches(REVERSED_NOTE).count(), 2, "{xml}");
}

#[test]
fn audio_track_transitions_touching_a_reversed_clip_export_as_cuts() {
    let mut project = adjacent_clips_project(false);
    let audio_index = audio_track_index(&project);
    project.timeline.tracks[audio_index].items[0]
        .properties
        .insert("reverse".to_string(), json!(true));
    add_transition(
        &mut project,
        audio_index,
        ("a-1", "a-2"),
        TransitionKind::Crossfade,
        0.5,
    );

    let xmeml = export(&project, NleXmlFormat::PremiereXmeml);
    assert_eq!(xmeml.matches("<transitionitem>").count(), 0, "{xmeml}");
    assert_bounds(clipitem(&xmeml, "a-1"), 0, 72, 24, 96);
    assert_bounds(clipitem(&xmeml, "a-2"), 72, 144, 192, 264);
    for id in ["a-1", "a-2"] {
        assert!(clipitem(&xmeml, id).contains(REVERSED_NOTE), "{xmeml}");
    }

    let fcpxml = export(&project, NleXmlFormat::DavinciFcpxml);
    assert!(!fcpxml.contains("<transition"), "{fcpxml}");
    assert_eq!(fcpxml.matches(REVERSED_NOTE).count(), 2, "{fcpxml}");
}

#[test]
fn nested_sequence_end_trims_a_reversed_clip_from_its_source_start() {
    // A reversed child reading [0, 4) is cut after 3 s by its wrapper's end. It
    // plays from sourceOut backwards, so the kept part reads [1, 4) as in render.
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    let child = &mut alternate.tracks[0].items[0];
    child.properties.insert("sourceIn".to_string(), json!(0.0));
    child.properties.insert("sourceOut".to_string(), json!(4.0));
    child.properties.insert("reverse".to_string(), json!(true));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties = BTreeMap::new();

    let xmeml = export(&project, NleXmlFormat::PremiereXmeml);
    let child_id = "root:0:item-1:item-1";
    assert_bounds(clipitem(&xmeml, child_id), 0, 72, 24, 96);

    let fcpxml = export(&project, NleXmlFormat::DavinciFcpxml);
    assert!(
        fcpxml.contains(
            "<asset-clip name=\"Opening clip\" ref=\"media-1\" offset=\"0/24s\" duration=\"72/24s\" start=\"24/24s\">"
        ),
        "{fcpxml}"
    );
}
