//! Audio clip edits through agent tools: retiming audio clips and detaching a
//! video clip's sound.

use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::codex::tools::{call_codex_local_tool, list_codex_local_tools};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, VideoProject,
};
use video_creater_lib::project::split::{load_split_project, save_split_project};

/// `sample_project` with `music-1`, a 4 s clip of a 10 s audio file on the audio track.
pub(crate) fn audio_project() -> VideoProject {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "music".to_string(),
        name: None,
        relative_path: "media/music.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 10.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-audio")
        .expect("audio track");
    track.items.push(TimelineItem {
        id: "music-1".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "music".to_string(),
        },
        label: "Music".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(4.0)),
        ]),
    });
    project
}

#[test]
fn set_clip_properties_retimes_audio_clips_with_the_audio_speed_action() {
    let project = audio_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({ "clipIds": ["music-1"], "speed": 2.0 }),
    )
    .expect("audio speed validates");

    assert_eq!(result.payload["valid"], json!(true), "{}", result.payload);
    let actions = result.payload["projectActions"]
        .as_array()
        .expect("project actions");
    assert_eq!(
        actions[0],
        json!({ "type": "updateAudioClipSpeed", "itemId": "music-1", "speed": 2.0 }),
        "{actions:?}"
    );
    assert!(
        actions
            .iter()
            .all(|action| action["type"] != json!("updateItemProperties")),
        "{actions:?}"
    );
}

#[test]
fn set_clip_properties_keeps_the_property_update_for_visual_clips() {
    let project = audio_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.set_clip_properties",
        json!({ "clipIds": ["item-1"], "speed": 2.0 }),
    )
    .expect("visual speed validates");

    let actions = result.payload["projectActions"]
        .as_array()
        .expect("project actions");
    assert_eq!(
        actions[0]["type"],
        json!("updateItemProperties"),
        "{actions:?}"
    );
    assert_eq!(
        actions[0]["updates"][0]["set"],
        json!({ "speed": 2.0 }),
        "{actions:?}"
    );
    assert!(
        actions
            .iter()
            .all(|action| action["type"] != json!("updateAudioClipSpeed")),
        "{actions:?}"
    );
}

#[test]
fn apply_project_actions_retimes_an_audio_clip_in_the_project_folder() {
    let project = audio_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save project");

    let applied = call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": project_dir.path().display().to_string(),
            "actions": [{ "type": "updateAudioClipSpeed", "itemId": "music-1", "speed": 2.0 }]
        }),
    )
    .expect("apply audio speed");

    assert_eq!(applied.payload["applied"], json!(true));
    let persisted = load_split_project(project_dir.path()).expect("load project");
    let clip = persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "music-1")
        .expect("persisted audio clip");
    assert_eq!(clip.properties.get("speed"), Some(&json!(2.0)));
}

#[test]
fn local_tools_list_detach_audio_as_a_mutation() {
    let tools = list_codex_local_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "video_creater.detach_audio")
        .expect("detach_audio should be listed");
    assert_eq!(tool.category, "mutation");
    assert_eq!(
        tool.input_schema["anyOf"],
        json!([
            { "required": ["itemId"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
}

#[test]
fn detach_audio_uses_a_free_audio_track() {
    let project = sample_project();
    let result = call_codex_local_tool(
        &project,
        "video_creater.detach_audio",
        json!({ "itemId": "item-1" }),
    )
    .expect("detach validates");

    assert_eq!(result.payload["valid"], json!(true), "{}", result.payload);
    assert_eq!(
        result.payload["projectActions"],
        json!([{
            "type": "detachAudio", "itemId": "item-1", "audioItemId": "item-1-audio",
            "targetTrackId": "track-audio", "linkGroupId": "link-item-1",
        }])
    );
    assert_eq!(result.payload["audioItemId"], json!("item-1-audio"));
}

#[test]
fn detach_audio_creates_a_track_and_a_unique_id_when_needed() {
    let mut project = audio_project();
    // Take the default id so the tool picks the next suffix.
    let track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-audio")
        .expect("audio track");
    track.items[0].id = "item-1-audio".to_string();
    let result = call_codex_local_tool(
        &project,
        "video_creater.detach_audio",
        json!({ "itemId": "item-1" }),
    )
    .expect("detach validates with a new track");

    let actions = result.payload["projectActions"]
        .as_array()
        .expect("project actions");
    assert_eq!(actions.len(), 2, "{actions:?}");
    assert_eq!(actions[0]["type"], json!("createTrack"));
    assert_eq!(actions[0]["track"]["kind"], json!("audio"));
    let new_track_id = actions[0]["track"]["id"].clone();
    assert_eq!(
        actions[1],
        json!({
            "type": "detachAudio", "itemId": "item-1", "audioItemId": "item-1-audio-2",
            "targetTrackId": new_track_id, "linkGroupId": "link-item-1",
        })
    );
}

#[test]
fn detach_audio_applies_in_the_project_folder() {
    let project = sample_project();
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save project");

    let applied = call_codex_local_tool(
        &project,
        "video_creater.detach_audio",
        json!({ "projectDir": project_dir.path().display().to_string(), "itemId": "item-1" }),
    )
    .expect("detach applies");
    assert_eq!(applied.payload["applied"], json!(true));

    let persisted = load_split_project(project_dir.path()).expect("load project");
    let items = persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .collect::<Vec<_>>();
    let audio = items
        .iter()
        .find(|item| item.id == "item-1-audio")
        .expect("detached audio clip");
    assert_eq!(audio.kind, TimelineItemKind::AudioClip);
    let video = items
        .iter()
        .find(|item| item.id == "item-1")
        .expect("video clip");
    assert_eq!(video.properties.get("audioDetached"), Some(&json!(true)));
}

#[test]
fn set_clip_properties_speed_schema_matches_the_clip_speed_range() {
    let tools = list_codex_local_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "video_creater.set_clip_properties")
        .expect("set_clip_properties should be listed");
    assert_eq!(
        tool.input_schema["properties"]["speed"],
        json!({ "type": "number", "minimum": 0.1, "maximum": 8.0 })
    );
}

#[test]
fn set_clip_properties_rejects_speeds_outside_the_clip_speed_range() {
    let project = audio_project();
    for (clip_id, speed) in [("item-1", 12.0), ("music-1", 12.0), ("item-1", 0.05)] {
        let error = call_codex_local_tool(
            &project,
            "video_creater.set_clip_properties",
            json!({ "clipIds": [clip_id], "speed": speed }),
        )
        .expect_err("out-of-range speed is rejected");
        assert!(
            error.to_string().contains("between 0.1 and 8"),
            "{clip_id} at {speed}: {error}"
        );
    }
    for speed in [0.1, 8.0] {
        call_codex_local_tool(
            &project,
            "video_creater.set_clip_properties",
            json!({ "clipIds": ["item-1"], "speed": speed }),
        )
        .expect("boundary speed validates");
    }
}
