//! FCPXML connected secondary storylines for transitions outside the primary
//! storyline. Times are 24 fps frames. In `adjacent_clips_project(true)`,
//! `item-1`/`item-2` sit on lane 1, `up-1` on lane 0 at `[24, 72)`, and
//! `a-1`/`a-2` on lane -1.

use super::transitions::{
    add_transition, adjacent_clips_project, audio_track_index, export, media_clip,
};
use serde_json::json;
use video_creater_lib::project::model::{TimelineItemKind, TransitionKind, VideoProject};
use video_creater_lib::project::nle_export::NleXmlFormat;

const RETIMED_NOTE: &str = "transitions connected to a retimed clip (exported as cuts)";

fn fcpxml(project: &VideoProject) -> String {
    export(project, NleXmlFormat::DavinciFcpxml)
}

fn upper_track_index(project: &VideoProject) -> usize {
    project.timeline.tracks.len() - 1
}

fn lane_crossfade_project() -> VideoProject {
    let mut project = adjacent_clips_project(true);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );
    project
}

fn three_lane_clips_project() -> VideoProject {
    let mut project = adjacent_clips_project(true);
    project.timeline.tracks[0].items = vec![
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
    project
}

#[test]
fn fcpxml_upper_lane_transition_writes_a_gap_hosted_storyline() {
    let xml = fcpxml(&lane_crossfade_project());

    assert!(
        xml.contains(
            "          <spine>
            <gap name=\"Gap\" offset=\"0/24s\" duration=\"24/24s\">
              <spine lane=\"1\" offset=\"0/24s\">
                <asset-clip name=\"Clip item-1\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"48/24s\">
                  <adjust-volume amount=\"-1\"/>
                </asset-clip>
                <transition name=\"Cross Dissolve\" offset=\"84/24s\" duration=\"24/24s\">
                  <filter-video ref=\"vc-transition-cross-dissolve\" name=\"Cross Dissolve\"/>
                  <filter-audio ref=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\"/>
                </transition>
                <asset-clip name=\"Clip item-2\" ref=\"media-1\" offset=\"96/24s\" duration=\"96/24s\" start=\"144/24s\">
                  <adjust-volume amount=\"-3\"/>
                </asset-clip>
              </spine>
            </gap>
            <asset-clip name=\"Clip up-1\" ref=\"media-1\" offset=\"24/24s\" duration=\"48/24s\" start=\"0/24s\"/>
"
        ),
        "{xml}"
    );
    assert!(!xml.contains("outside the primary storyline"), "{xml}");
}

#[test]
fn fcpxml_storyline_anchors_inside_the_covering_primary_clip() {
    for (chain_start_seconds, anchor_offset) in [(0.0, "48/24s"), (1.0, "72/24s")] {
        let mut project = lane_crossfade_project();
        project.timeline.duration_seconds = 8.0 + chain_start_seconds;
        for item in &mut project.timeline.tracks[0].items {
            item.start_seconds += chain_start_seconds;
        }
        let upper = upper_track_index(&project);
        let host = &mut project.timeline.tracks[upper].items[0];
        host.start_seconds = 0.0;
        host.duration_seconds = 8.0;
        host.properties.insert("sourceIn".to_string(), json!(2.0));
        host.properties.insert("sourceOut".to_string(), json!(10.0));

        let xml = fcpxml(&project);

        assert!(
            xml.contains(&format!(
                "            <asset-clip name=\"Clip up-1\" ref=\"media-1\" offset=\"0/24s\" duration=\"192/24s\" start=\"48/24s\">
              <spine lane=\"1\" offset=\"{anchor_offset}\">
                <asset-clip name=\"Clip item-1\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"48/24s\">"
            )),
            "{chain_start_seconds}: {xml}"
        );
        // Offsets inside the storyline are relative to the chain start.
        assert!(
            xml.contains(
                "                <transition name=\"Cross Dissolve\" offset=\"84/24s\" duration=\"24/24s\">"
            ),
            "{chain_start_seconds}: {xml}"
        );
        assert!(
            xml.contains(
                "                <asset-clip name=\"Clip item-2\" ref=\"media-1\" offset=\"96/24s\" duration=\"96/24s\" start=\"144/24s\">"
            ),
            "{chain_start_seconds}: {xml}"
        );
        assert!(!xml.contains("<gap"), "{xml}");
        assert_eq!(xml.matches("<transition ").count(), 1, "{xml}");
    }
}

#[test]
fn fcpxml_gap_hosted_storyline_offsets_are_relative_to_the_gap() {
    // `item-1`/`item-2` move 4 s later, into the hole between `up-1` [24, 72)
    // and `up-2` [120, 168).
    let mut project = lane_crossfade_project();
    project.timeline.duration_seconds = 12.0;
    for item in &mut project.timeline.tracks[0].items {
        item.start_seconds += 4.0;
    }
    let upper = upper_track_index(&project);
    project.timeline.tracks[upper].items.push(media_clip(
        "up-2",
        "media-1",
        TimelineItemKind::VideoClip,
        5.0,
        2.0,
        0.0,
    ));

    let xml = fcpxml(&project);

    assert!(
        xml.contains(
            "            <gap name=\"Gap\" offset=\"96/24s\" duration=\"24/24s\">
              <spine lane=\"1\" offset=\"0/24s\">
                <asset-clip name=\"Clip item-1\" ref=\"media-1\" offset=\"0/24s\" duration=\"96/24s\" start=\"48/24s\">
                  <adjust-volume amount=\"-1\"/>
                </asset-clip>
                <transition name=\"Cross Dissolve\" offset=\"84/24s\" duration=\"24/24s\">"
        ),
        "{xml}"
    );
}

#[test]
fn fcpxml_storyline_on_a_retimed_host() {
    // Not verified: anchors on a retimed host stay cuts with a note.
    let mut project = lane_crossfade_project();
    let upper = upper_track_index(&project);
    let host = &mut project.timeline.tracks[upper].items[0];
    host.start_seconds = 0.0;
    host.duration_seconds = 2.0;
    host.properties.insert("speed".to_string(), json!(2.0));
    host.properties.insert("sourceOut".to_string(), json!(4.0));

    let xml = fcpxml(&project);

    assert!(!xml.contains("<transition"), "{xml}");
    assert!(!xml.contains("<spine lane"), "{xml}");
    assert_eq!(xml.matches(RETIMED_NOTE).count(), 2, "{xml}");
    assert!(
        xml.contains(
            "<asset-clip name=\"Clip item-1\" ref=\"media-1\" lane=\"1\" offset=\"0/24s\""
        ),
        "{xml}"
    );
}

#[test]
fn fcpxml_audio_lane_transition_writes_an_audio_storyline() {
    for kind in [
        TransitionKind::Crossfade,
        TransitionKind::DipToBlack,
        TransitionKind::DipToWhite,
        TransitionKind::Wipe,
    ] {
        let mut project = adjacent_clips_project(true);
        let audio_index = audio_track_index(&project);
        add_transition(&mut project, audio_index, ("a-1", "a-2"), kind, 0.5);

        let xml = fcpxml(&project);

        assert!(
            xml.contains(
                "            <gap name=\"Gap\" offset=\"0/24s\" duration=\"24/24s\">
              <spine lane=\"-1\" offset=\"0/24s\">
                <asset-clip name=\"Clip a-1\" ref=\"audio-1\" offset=\"0/24s\" duration=\"72/24s\" start=\"24/24s\" audioRole=\"dialogue\"/>
                <transition name=\"Audio Crossfade\" offset=\"66/24s\" duration=\"12/24s\">
                  <filter-audio ref=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\"/>
                </transition>
                <asset-clip name=\"Clip a-2\" ref=\"audio-1\" offset=\"72/24s\" duration=\"72/24s\" start=\"192/24s\" audioRole=\"dialogue\"/>
              </spine>
            </gap>
"
            ),
            "{kind:?}: {xml}"
        );
        assert!(
            xml.contains(
                "    <format id=\"r1\" frameDuration=\"1/24s\" width=\"1920\" height=\"1080\"/>
    <effect id=\"vc-transition-audio-crossfade\" name=\"Audio Crossfade\" uid=\"FFAudioTransition\"/>
    <asset "
            ),
            "{kind:?}: {xml}"
        );
        assert!(!xml.contains("limitation"), "{kind:?}: {xml}");
    }
}

#[test]
fn fcpxml_chained_transitions_share_one_storyline() {
    let mut project = three_lane_clips_project();
    for pair in [("item-1", "item-2"), ("item-2", "item-3")] {
        add_transition(&mut project, 0, pair, TransitionKind::Crossfade, 1.0);
    }

    let xml = fcpxml(&project);

    assert_eq!(xml.matches("<spine lane=\"1\"").count(), 1, "{xml}");
    assert_eq!(xml.matches("<transition ").count(), 2, "{xml}");
    assert!(!xml.contains("ref=\"media-1\" lane=\"1\""), "{xml}");
}

#[test]
fn fcpxml_unchained_lane_clips_keep_the_flat_layout() {
    let mut project = three_lane_clips_project();
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );

    let xml = fcpxml(&project);

    assert_eq!(xml.matches("<spine lane=\"1\"").count(), 1, "{xml}");
    assert!(
        xml.contains(
            "            <asset-clip name=\"Clip item-3\" ref=\"media-1\" lane=\"1\" offset=\"144/24s\" duration=\"48/24s\" start=\"228/24s\"/>"
        ),
        "{xml}"
    );
}

#[test]
fn fcpxml_primary_lane_transitions_are_unchanged() {
    // The exact spine text is asserted by
    // `transitions::fcpxml_places_cross_dissolve_transitions_in_the_spine`.
    let mut project = adjacent_clips_project(false);
    add_transition(
        &mut project,
        0,
        ("item-1", "item-2"),
        TransitionKind::Crossfade,
        1.0,
    );

    let xml = fcpxml(&project);

    assert!(!xml.contains("<gap"), "{xml}");
    assert!(!xml.contains("<spine lane"), "{xml}");
    assert_eq!(xml.matches("<transition ").count(), 1, "{xml}");
}

/// The `offset` frames of the primary spine's lane-0 story elements, in document order.
fn primary_spine_offsets(xml: &str) -> Vec<(String, i64)> {
    xml.lines()
        .filter(|line| line.starts_with("            <") && !line.starts_with("             "))
        .filter(|line| !line.contains(" lane=\""))
        .filter_map(|line| {
            let name = line.trim_start().split([' ', '>']).next()?.to_string();
            let offset = line.split(" offset=\"").nth(1)?.split('/').next()?;
            Some((name, offset.parse().ok()?))
        })
        .collect()
}

#[test]
fn fcpxml_primary_spine_writes_gaps_in_time_order() {
    // The FCPXML DTD: "A 'spine' is a container for elements ordered serially in time."
    // `item-1`/`item-2` sit in the hole between `up-1` [24, 72) and `up-2` [120, 168), so their
    // gap [96, 120) belongs between the two primary clips.
    let mut project = lane_crossfade_project();
    project.timeline.duration_seconds = 12.0;
    for item in &mut project.timeline.tracks[0].items {
        item.start_seconds += 4.0;
    }
    let upper = upper_track_index(&project);
    project.timeline.tracks[upper].items.push(media_clip(
        "up-2",
        "media-1",
        TimelineItemKind::VideoClip,
        5.0,
        2.0,
        0.0,
    ));

    let xml = fcpxml(&project);

    assert_eq!(
        primary_spine_offsets(&xml),
        vec![
            ("<asset-clip".to_string(), 24),
            ("<gap".to_string(), 96),
            ("<asset-clip".to_string(), 120),
        ],
        "{xml}"
    );
}

#[test]
fn fcpxml_a_gap_after_the_last_primary_clip_follows_it_and_its_transition() {
    // `up-1` [24, 72) and `up-2` [72, 120) are joined by a primary transition; the lane-1 chain
    // moves after them, so its gap runs from 144 to the sequence end.
    let mut project = lane_crossfade_project();
    project.timeline.duration_seconds = 16.0;
    for item in &mut project.timeline.tracks[0].items {
        item.start_seconds += 6.0;
    }
    let upper = upper_track_index(&project);
    project.timeline.tracks[upper].items.push(media_clip(
        "up-2",
        "media-1",
        TimelineItemKind::VideoClip,
        3.0,
        2.0,
        2.0,
    ));
    add_transition(
        &mut project,
        upper,
        ("up-1", "up-2"),
        TransitionKind::Crossfade,
        0.5,
    );

    let xml = fcpxml(&project);

    let offsets = primary_spine_offsets(&xml);
    let names: Vec<&str> = offsets.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        vec!["<asset-clip", "<transition", "<asset-clip", "<gap"],
        "{xml}"
    );
    assert!(
        offsets.windows(2).all(|pair| pair[0].1 <= pair[1].1),
        "{offsets:?}\n{xml}"
    );
}
