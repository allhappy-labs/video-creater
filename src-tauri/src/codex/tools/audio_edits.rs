//! Agent tools for audio clip edits: `video_creater.detach_audio`.
//!
//! The tool builds one `detachAudio` project action (preceded by `createTrack`
//! when no unlocked audio track has room) and runs it through the same
//! validation and split-project application as the other mutation tools. It
//! also accepts `{ projectDir, actions }` like the other mutation tools.

use super::{
    decode_args, mutating_project_actions_payload, project_actions_tool_schema,
    resolve_timeline_item_id, tool_descriptor, CodexLocalToolDescriptor, CodexLocalToolError,
    ToolProjectActionsArgs,
};
use crate::project::action::ProjectAction;
use crate::project::model::{TimelineItem, TimelineTrack, TrackKind, VideoProject};
use serde::Deserialize;
use serde_json::{json, Value};

const OVERLAP_EPSILON_SECONDS: f64 = 0.000_001;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetachAudioArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(alias = "clipId")]
    item_id: String,
}

pub(super) fn audio_edit_tool_descriptors() -> Vec<CodexLocalToolDescriptor> {
    let id = json!({ "type": "string", "minLength": 1 });
    vec![tool_descriptor(
        "video_creater.detach_audio",
        "Detach Audio",
        "mutation",
        "Move a video clip's sound onto its own linked audio clip: the new audio clip plays the same media range on the first unlocked audio track with room (a new audio track is created otherwise), takes the clip's volume, volume keyframes and audio effects, and stays linked to the video clip. Fails when the clip's sound is already on a linked audio clip.",
        project_actions_tool_schema(json!({
            "type": "object",
            "required": ["itemId"],
            "additionalProperties": false,
            "properties": {
                "projectDir": id,
                "itemId": id
            }
        })),
    )]
}

/// Dispatches `video_creater.detach_audio`.
pub(super) fn call_audio_edit_tool(
    project: &VideoProject,
    tool_name: &str,
    args: Value,
) -> Result<Value, CodexLocalToolError> {
    match tool_name {
        "video_creater.detach_audio" => {
            if args.get("actions").is_some() {
                let args: ToolProjectActionsArgs = decode_args(args)?;
                return mutating_project_actions_payload(project, args);
            }
            detach_audio(project, decode_args(args)?)
        }
        _ => Err(CodexLocalToolError::UnknownTool(tool_name.to_string())),
    }
}

fn detach_audio(
    project: &VideoProject,
    args: DetachAudioArgs,
) -> Result<Value, CodexLocalToolError> {
    let item_id = resolve_timeline_item_id(project, &args.item_id, "itemId")?;
    let item = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == item_id)
        .expect("resolved timeline item exists");
    let audio_item_id = unused_item_id(project, &format!("{item_id}-audio"));
    let link_group_id = item
        .properties
        .get("linkGroupId")
        .and_then(Value::as_str)
        .filter(|group| !group.trim().is_empty())
        .map_or_else(|| format!("link-{item_id}"), str::to_string);

    let mut actions = Vec::new();
    let target_track_id = match free_audio_track(project, item) {
        Some(track) => track.id.clone(),
        None => {
            let track_id = unused_audio_track_id(project);
            let after_track_id = project
                .timeline
                .tracks
                .iter()
                .rev()
                .find(|track| track.kind == TrackKind::Audio)
                .map(|track| track.id.clone());
            actions.push(ProjectAction::CreateTrack {
                track: TimelineTrack::empty(&track_id, "Audio", TrackKind::Audio),
                after_track_id,
            });
            track_id
        }
    };
    actions.push(ProjectAction::DetachAudio {
        item_id: item_id.clone(),
        audio_item_id: audio_item_id.clone(),
        target_track_id: target_track_id.clone(),
        link_group_id: link_group_id.clone(),
    });
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("audioItemId".to_string(), json!(audio_item_id));
        object.insert("targetTrackId".to_string(), json!(target_track_id));
        object.insert("linkGroupId".to_string(), json!(link_group_id));
        object.insert(
            "affectedItemIds".to_string(),
            json!([item_id, audio_item_id]),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

/// The first unlocked audio track with no item over the clip's span.
fn free_audio_track<'a>(
    project: &'a VideoProject,
    item: &TimelineItem,
) -> Option<&'a TimelineTrack> {
    let start = item.start_seconds;
    let end = item.start_seconds + item.duration_seconds;
    project.timeline.tracks.iter().find(|track| {
        track.kind == TrackKind::Audio
            && !track.locked
            && track.items.iter().all(|candidate| {
                candidate.start_seconds + candidate.duration_seconds
                    <= start + OVERLAP_EPSILON_SECONDS
                    || candidate.start_seconds >= end - OVERLAP_EPSILON_SECONDS
            })
    })
}

/// `base`, else `base-2`, `base-3`, ... not used by any timeline item.
fn unused_item_id(project: &VideoProject, base: &str) -> String {
    let taken = |id: &str| {
        project
            .timeline
            .tracks
            .iter()
            .any(|track| track.items.iter().any(|item| item.id == id))
    };
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|id| !taken(id))
        .expect("an unused suffix exists")
}

/// `track-audio-<n>`, counting up from the track count plus one.
fn unused_audio_track_id(project: &VideoProject) -> String {
    let tracks = &project.timeline.tracks;
    (tracks.len() + 1..)
        .map(|suffix| format!("track-audio-{suffix}"))
        .find(|id| tracks.iter().all(|track| track.id != *id))
        .expect("an unused track id exists")
}
