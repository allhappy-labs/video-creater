//! Agent tools for clip transitions centered on the cut between two adjacent
//! clips: `video_creater.add_transition`, `video_creater.update_transition`,
//! and `video_creater.remove_transition`.
//!
//! Each tool builds one `addTransition`, `updateTransition`, or
//! `removeTransition` project action and runs it through the same validation
//! and split-project application as the other mutation tools, so bad input
//! returns the project's user-facing validation message. Every tool also
//! accepts `{ projectDir, actions }` like the other mutation tools.

use super::{
    decode_args, mutating_project_actions_payload, project_actions_tool_schema,
    resolve_timeline_item_id, tool_descriptor, CodexLocalToolDescriptor, CodexLocalToolError,
    ToolProjectActionsArgs,
};
use crate::project::action::ProjectAction;
use crate::project::model::{TimelineTransition, TransitionKind, VideoProject};
use crate::project::transitions::{frame_seconds, transition_max_duration};
use serde::Deserialize;
use serde_json::{json, Value};

/// The duration a new transition gets when the caller does not choose one.
const DEFAULT_TRANSITION_SECONDS: f64 = 0.5;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddTransitionArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    track_id: Option<String>,
    #[serde(default)]
    transition_id: Option<String>,
    #[serde(alias = "leftClipId")]
    left_item_id: String,
    #[serde(alias = "rightClipId")]
    right_item_id: String,
    kind: TransitionKind,
    #[serde(default)]
    duration_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateTransitionArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    track_id: Option<String>,
    transition_id: String,
    #[serde(default)]
    kind: Option<TransitionKind>,
    #[serde(default)]
    duration_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveTransitionArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    track_id: Option<String>,
    transition_id: String,
}

pub(super) fn transition_tool_descriptors() -> Vec<CodexLocalToolDescriptor> {
    let kind =
        json!({ "type": "string", "enum": ["crossfade", "dipToBlack", "dipToWhite", "wipe"] });
    let duration = json!({ "type": "number", "exclusiveMinimum": 0, "maximum": 5 });
    let id = json!({ "type": "string", "minLength": 1 });
    vec![
        tool_descriptor(
            "video_creater.add_transition",
            "Add Transition",
            "mutation",
            "Add a crossfade, dipToBlack, dipToWhite, or wipe transition centered on the cut where leftItemId ends and rightItemId starts on the same track. Both clips need durationSeconds / 2 of unused source media beyond the cut; durationSeconds defaults to 0.5 s or the maximum the handles allow.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["leftItemId", "rightItemId", "kind"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": id,
                    "trackId": id,
                    "transitionId": id,
                    "leftItemId": id,
                    "rightItemId": id,
                    "kind": kind,
                    "durationSeconds": duration
                }
            })),
        ),
        tool_descriptor(
            "video_creater.update_transition",
            "Update Transition",
            "mutation",
            "Change an existing transition's kind or durationSeconds. The track is found from transitionId when trackId is omitted.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["transitionId", "kind"] },
                    { "required": ["transitionId", "durationSeconds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": id,
                    "trackId": id,
                    "transitionId": id,
                    "kind": kind,
                    "durationSeconds": duration
                }
            })),
        ),
        tool_descriptor(
            "video_creater.remove_transition",
            "Remove Transition",
            "mutation",
            "Remove a transition so its two clips meet at a straight cut. The clips are not changed.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["transitionId"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": id,
                    "trackId": id,
                    "transitionId": id
                }
            })),
        ),
    ]
}

/// Dispatches `video_creater.add_transition`, `update_transition`, or
/// `remove_transition`.
pub(super) fn call_transition_tool(
    project: &VideoProject,
    tool_name: &str,
    args: Value,
) -> Result<Value, CodexLocalToolError> {
    match tool_name {
        "video_creater.add_transition" => with_project_actions(project, args, add_transition),
        "video_creater.update_transition" => with_project_actions(project, args, update_transition),
        "video_creater.remove_transition" => with_project_actions(project, args, remove_transition),
        _ => Err(CodexLocalToolError::UnknownTool(tool_name.to_string())),
    }
}

/// Routes `{ projectDir, actions }` to the shared mutation path; anything else
/// decodes as the tool's direct arguments so field errors stay specific.
fn with_project_actions<T>(
    project: &VideoProject,
    args: Value,
    direct: fn(&VideoProject, T) -> Result<Value, CodexLocalToolError>,
) -> Result<Value, CodexLocalToolError>
where
    T: for<'de> Deserialize<'de>,
{
    if args.get("actions").is_some() {
        return mutating_project_actions_payload(project, decode_args(args)?);
    }
    direct(project, decode_args(args)?)
}

fn add_transition(
    project: &VideoProject,
    args: AddTransitionArgs,
) -> Result<Value, CodexLocalToolError> {
    let left_item_id = resolve_timeline_item_id(project, &args.left_item_id, "leftItemId")?;
    let right_item_id = resolve_timeline_item_id(project, &args.right_item_id, "rightItemId")?;
    let left_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.items.iter().any(|item| item.id == left_item_id))
        .expect("resolved timeline item is on a track");
    let track_id = args.track_id.unwrap_or_else(|| left_track.id.clone());
    let left = left_track.items.iter().find(|item| item.id == left_item_id);
    let right = left_track
        .items
        .iter()
        .find(|item| item.id == right_item_id);
    let max_duration_seconds = left
        .zip(right)
        .map(|(left, right)| transition_max_duration(project, left, right));
    let duration_seconds = args.duration_seconds.unwrap_or_else(|| {
        max_duration_seconds.map_or(DEFAULT_TRANSITION_SECONDS, |max| {
            // Fall back to the default when not even a frame fits, so validation explains why.
            if max >= frame_seconds(project) {
                DEFAULT_TRANSITION_SECONDS.min(max)
            } else {
                DEFAULT_TRANSITION_SECONDS
            }
        })
    });
    let transition_id = args
        .transition_id
        .map(|id| id.trim().to_string())
        .unwrap_or_else(|| unused_transition_id(project, &left_item_id, &right_item_id));
    let action = ProjectAction::AddTransition {
        track_id: track_id.clone(),
        transition: TimelineTransition {
            id: transition_id.clone(),
            left_item_id: left_item_id.clone(),
            right_item_id: right_item_id.clone(),
            kind: args.kind,
            duration_seconds,
        },
    };
    let mut payload = apply(project, args.project_dir, action)?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("transitionId".to_string(), json!(transition_id));
        object.insert("trackId".to_string(), json!(track_id));
        object.insert("kind".to_string(), json!(args.kind));
        object.insert("durationSeconds".to_string(), json!(duration_seconds));
        if let Some(max) = max_duration_seconds.filter(|max| max.is_finite()) {
            object.insert("maxDurationSeconds".to_string(), json!(max));
        }
        object.insert(
            "affectedItemIds".to_string(),
            json!([left_item_id, right_item_id]),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn update_transition(
    project: &VideoProject,
    args: UpdateTransitionArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.kind.is_none() && args.duration_seconds.is_none() {
        return Err(CodexLocalToolError::InvalidArguments(
            "update_transition needs kind or durationSeconds".to_string(),
        ));
    }
    let located = locate_transition(project, args.track_id.as_deref(), &args.transition_id)?;
    let action = ProjectAction::UpdateTransition {
        track_id: located.track_id.clone(),
        transition_id: located.transition.id.clone(),
        kind: args.kind,
        duration_seconds: args.duration_seconds,
    };
    let payload = apply(project, args.project_dir, action)?;
    Ok(with_transition_fields(payload, &located))
}

fn remove_transition(
    project: &VideoProject,
    args: RemoveTransitionArgs,
) -> Result<Value, CodexLocalToolError> {
    let located = locate_transition(project, args.track_id.as_deref(), &args.transition_id)?;
    let action = ProjectAction::RemoveTransition {
        track_id: located.track_id.clone(),
        transition_id: located.transition.id.clone(),
    };
    let payload = apply(project, args.project_dir, action)?;
    Ok(with_transition_fields(payload, &located))
}

fn apply(
    project: &VideoProject,
    project_dir: Option<String>,
    action: ProjectAction,
) -> Result<Value, CodexLocalToolError> {
    mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )
}

struct LocatedTransition<'a> {
    track_id: String,
    transition: &'a TimelineTransition,
}

/// Finds a transition by exact id or unique id prefix, on `track_id` when given.
fn locate_transition<'a>(
    project: &'a VideoProject,
    track_id: Option<&str>,
    raw_id: &str,
) -> Result<LocatedTransition<'a>, CodexLocalToolError> {
    let id = raw_id.trim();
    if id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "transitionId must not be blank".to_string(),
        ));
    }
    let candidates = project
        .timeline
        .tracks
        .iter()
        .filter(|track| track_id.is_none_or(|track_id| track.id == track_id))
        .flat_map(|track| {
            track
                .transitions
                .iter()
                .map(move |transition| (track, transition))
        })
        .collect::<Vec<_>>();
    let exact = candidates
        .iter()
        .copied()
        .find(|(_, transition)| transition.id == id);
    let prefixed = candidates
        .iter()
        .filter(|(_, transition)| transition.id.starts_with(id))
        .copied()
        .collect::<Vec<_>>();
    let (track, transition) = match (exact, prefixed.as_slice()) {
        (Some(found), _) | (None, &[found]) => found,
        (None, []) => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "transitionId did not match a transition{}: {id}",
                track_id
                    .map(|track_id| format!(" on track {track_id}"))
                    .unwrap_or_default()
            )))
        }
        (None, matches) => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "transitionId is ambiguous: {id} matches {} transitions",
                matches.len()
            )))
        }
    };
    Ok(LocatedTransition {
        track_id: track.id.clone(),
        transition,
    })
}

fn with_transition_fields(mut payload: Value, located: &LocatedTransition<'_>) -> Value {
    if let Some(object) = payload.as_object_mut() {
        object.insert("transitionId".to_string(), json!(located.transition.id));
        object.insert("trackId".to_string(), json!(located.track_id));
        object.insert(
            "affectedItemIds".to_string(),
            json!([
                located.transition.left_item_id,
                located.transition.right_item_id
            ]),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    payload
}

fn unused_transition_id(project: &VideoProject, left_item_id: &str, right_item_id: &str) -> String {
    let base = format!("transition-{left_item_id}-{right_item_id}");
    let taken = |candidate: &str| {
        project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.transitions.iter())
            .any(|transition| transition.id == candidate)
    };
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| !taken(candidate))
        .expect("an unused transition id exists")
}
