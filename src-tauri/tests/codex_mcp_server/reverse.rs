//! Reversed clips through agent tools.

use super::audio_edits::audio_project;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use video_creater_lib::codex::tools::{call_codex_local_tool, list_codex_local_tools};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    MediaKind, MediaSilenceRange, TimelineItem, TimelineItemKind, TimelineSource, Transcript,
    TranscriptWord, VideoProject,
};
use video_creater_lib::project::split::{load_split_project, save_split_project};

fn reverse_actions(payload: &Value) -> Vec<Value> {
    payload["projectActions"]
        .as_array()
        .expect("project actions")
        .iter()
        .filter(|action| action["type"] == json!("updateClipReverse"))
        .cloned()
        .collect()
}

/// `audio_project` with `item-1` and `music-1` sharing a link group.
fn linked_project() -> VideoProject {
    let mut project = audio_project();
    for track in &mut project.timeline.tracks {
        for item in &mut track.items {
            if item.id == "item-1" || item.id == "music-1" {
                item.properties
                    .insert("linkGroupId".to_string(), json!("link-1"));
            }
        }
    }
    project
}

#[test]
fn set_clip_properties_lists_a_reverse_field() {
    let tools = list_codex_local_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "video_creater.set_clip_properties")
        .expect("set_clip_properties is listed");
    let schema = tool.input_schema.to_string();
    assert!(
        schema.contains(r#""reverse":{"type":"boolean"}"#),
        "{schema}"
    );
}

#[test]
fn set_clip_properties_reverses_clips_with_the_reverse_action() {
    let project = audio_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({ "updates": [
            { "itemId": "item-1", "reverse": true },
            { "itemId": "music-1", "reverse": false }
        ] }),
    )
    .expect("reverse validates");

    assert_eq!(result.payload["valid"], json!(true), "{}", result.payload);
    assert_eq!(
        reverse_actions(&result.payload),
        vec![
            json!({ "type": "updateClipReverse", "itemId": "item-1", "reverse": true }),
            json!({ "type": "updateClipReverse", "itemId": "music-1", "reverse": false }),
        ]
    );
}

#[test]
fn set_clip_properties_reverses_linked_partners_like_speed() {
    let project = linked_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({ "clipIds": ["item-1"], "reverse": true }),
    )
    .expect("linked reverse validates");

    assert_eq!(result.payload["valid"], json!(true), "{}", result.payload);
    assert_eq!(
        reverse_actions(&result.payload),
        vec![
            json!({ "type": "updateClipReverse", "itemId": "item-1", "reverse": true }),
            json!({ "type": "updateClipReverse", "itemId": "music-1", "reverse": true }),
        ]
    );
}

#[test]
fn apply_project_actions_reverses_a_clip_in_the_project_folder() {
    let project = audio_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save project");

    let applied = call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "actions": [{ "type": "updateClipReverse", "itemId": "item-1", "reverse": true }]
        }),
    )
    .expect("apply reverse");

    assert_eq!(applied.payload["applied"], json!(true));
    let persisted = load_split_project(project_dir.path()).expect("load project");
    let clip = persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("persisted video clip");
    assert_eq!(clip.properties.get("reverse"), Some(&json!(true)));
}

#[test]
fn set_clip_properties_skips_linked_partners_that_cannot_play_reversed() {
    let mut project = sample_project();
    project.media[0].kind = MediaKind::Generated;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("linkGroupId".to_string(), json!("link-1"));
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-audio")
        .expect("audio track")
        .items
        .push(TimelineItem {
            id: "item-1-audio".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Opening clip audio".to_string(),
            properties: BTreeMap::from([
                ("linkGroupId".to_string(), json!("link-1")),
                ("sourceIn".to_string(), json!(0.0)),
                ("sourceOut".to_string(), json!(4.0)),
            ]),
        });

    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({ "clipIds": ["item-1-audio"], "reverse": true }),
    )
    .expect("reverse on detached audio of generated video validates");

    assert_eq!(result.payload["valid"], json!(true), "{}", result.payload);
    assert_eq!(
        reverse_actions(&result.payload),
        vec![json!({ "type": "updateClipReverse", "itemId": "item-1-audio", "reverse": true })]
    );
}

/// `sample_project` with `item-1` playing source 0-10 s of `media-1` reversed at `speed`.
fn reversed_opening_clip(speed: f64) -> VideoProject {
    let mut project = sample_project();
    project.timeline.duration_seconds = 10.0 / speed;
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 10.0 / speed;
    item.properties.extend([
        ("sourceIn".to_string(), json!(0.0)),
        ("sourceOut".to_string(), json!(10.0)),
        ("reverse".to_string(), json!(true)),
    ]);
    if speed != 1.0 {
        item.properties.insert("speed".to_string(), json!(speed));
    }
    project
}

fn media_1_transcript(words: &[(&str, f64, f64)]) -> Transcript {
    Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: words
            .iter()
            .map(|(text, start_seconds, end_seconds)| TranscriptWord {
                text: (*text).to_string(),
                start_seconds: *start_seconds,
                end_seconds: *end_seconds,
                confidence: Some(0.9),
                speaker: None,
            })
            .collect(),
    }
}

fn timeline_range(start: f64, end: f64) -> Value {
    json!({ "startSeconds": start, "endSeconds": end, "trackIds": ["track-video"] })
}

#[test]
fn remove_silence_cuts_stored_dead_air_where_a_reversed_clip_plays_it() {
    let mut project = reversed_opening_clip(1.0);
    // Source 1-2 s once the 0.12 s boundary padding is removed.
    project.media_silence_ranges = vec![MediaSilenceRange {
        media_id: "media-1".to_string(),
        source_in: 0.88,
        source_out: 2.12,
        confidence: 0.9,
        label: "quiet".to_string(),
    }];

    let result = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect("remove_silence maps through the reversed clip");

    assert_eq!(result.payload["ranges"], json!([timeline_range(8.0, 9.0)]));
    assert_eq!(
        result.payload["removedSilenceRanges"][0]["sourceStartSeconds"],
        json!(1.0)
    );
}

#[test]
fn remove_silence_cuts_transcript_gaps_where_a_reversed_clip_plays_them() {
    let mut project = reversed_opening_clip(1.0);
    project.transcripts.push(media_1_transcript(&[
        ("first", 1.0, 2.0),
        ("second", 5.0, 6.0),
    ]));

    let result = call_codex_local_tool(&project, "remove_silence", json!({}))
        .expect("remove_silence maps transcript gaps through the reversed clip");

    // `second` plays at 4-5 s and `first` at 8-9 s; the source gap is 2-5 s.
    assert_eq!(
        result.payload["ranges"],
        json!([timeline_range(5.12, 7.88)])
    );
    let removed = &result.payload["removedSilenceRanges"][0];
    assert_eq!(removed["sourceStartSeconds"], json!(2.12), "{removed}");
    assert_eq!(removed["sourceEndSeconds"], json!(4.88), "{removed}");
}

#[test]
fn remove_words_cuts_media_words_where_a_reversed_clip_plays_them() {
    for (speed, expected) in [(1.0, (8.0, 9.0)), (2.0, (4.0, 4.5))] {
        let mut project = reversed_opening_clip(speed);
        project
            .transcripts
            .push(media_1_transcript(&[("keep", 5.0, 6.0), ("cut", 1.0, 2.0)]));

        let result = call_codex_local_tool(
            &project,
            "video_creater.remove_words",
            json!({ "mediaId": "media-1", "wordIndexes": [1] }),
        )
        .expect("remove_words maps through the reversed clip");

        assert_eq!(
            result.payload["ranges"],
            json!([timeline_range(expected.0, expected.1)]),
            "speed {speed}"
        );
    }
}

#[test]
fn remove_words_cuts_timeline_words_in_reversed_play_order() {
    let mut project = reversed_opening_clip(1.0);
    project.transcripts.push(media_1_transcript(&[
        ("first", 1.0, 2.0),
        ("second", 5.0, 6.0),
    ]));

    // Timeline word 1 is `first`, which plays after `second`.
    let result = call_codex_local_tool(&project, "remove_words", json!({ "words": [1] }))
        .expect("timeline remove_words maps through the reversed clip");

    assert_eq!(result.payload["ranges"], json!([timeline_range(8.0, 9.0)]));
    assert_eq!(
        result.payload["removedWordRanges"][0]["text"],
        json!("first")
    );
}

#[test]
fn ripple_delete_ranges_maps_clip_source_seconds_through_a_reversed_clip() {
    for (speed, expected) in [(1.0, (8.0, 9.0)), (2.0, (4.0, 4.5))] {
        let project = reversed_opening_clip(speed);
        let result = call_codex_local_tool(
            &project,
            "ripple_delete_ranges",
            json!({ "clipId": "item-1", "ranges": [[1.0, 2.0]], "units": "seconds" }),
        )
        .expect("clip source seconds map through the reversed clip");

        assert_eq!(
            result.payload["ranges"],
            json!([timeline_range(expected.0, expected.1)]),
            "speed {speed}"
        );
    }
}

#[test]
fn transcript_words_report_where_a_reversed_clip_plays_them() {
    let mut project = reversed_opening_clip(1.0);
    project
        .transcripts
        .push(media_1_transcript(&[("word", 1.0, 2.0)]));

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({ "mediaId": "media-1" }),
    )
    .expect("transcript words map through the reversed clip");

    let word = &result.payload["words"][0];
    assert_eq!(word["timelineStartSeconds"], json!(8.0), "{word}");
    assert_eq!(word["timelineEndSeconds"], json!(9.0), "{word}");
}

#[test]
fn add_captions_refuses_reversed_clips() {
    let mut project = reversed_opening_clip(1.0);
    project
        .transcripts
        .push(media_1_transcript(&[("word", 1.0, 2.0)]));

    let error = call_codex_local_tool(&project, "add_captions", json!({ "clipIds": ["item-1"] }))
        .expect_err("captions from a reversed clip's transcript would run backwards");

    assert!(error.to_string().contains("plays reversed"), "{error}");
}
