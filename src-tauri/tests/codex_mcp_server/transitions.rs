//! `video_creater.add_transition`, `update_transition`, and `remove_transition`.

use serde_json::{json, Value};
use video_creater_lib::codex::mcp_server::{handle_mcp_request, handle_scoped_mcp_request};
use video_creater_lib::codex::tools::{
    call_codex_local_tool, list_codex_local_tools, CodexLocalToolError,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    TimelineItem, TimelineItemKind, TimelineTransition, TransitionKind, VideoProject,
};
use video_creater_lib::project::split::{load_split_project, save_split_project};

/// `item-1` [0, 4) with 8 s of media after it, then `item-2` [4, 8) whose
/// `sourceIn` leaves `head_handle_seconds` of media before the cut.
fn adjacent_clips_project(head_handle_seconds: f64) -> VideoProject {
    let mut project = sample_project();
    let mut right = project.timeline.tracks[0].items[0].clone();
    right.id = "item-2".to_string();
    right.label = "Closing clip".to_string();
    right.start_seconds = 4.0;
    right
        .properties
        .insert("sourceIn".to_string(), json!(head_handle_seconds));
    right
        .properties
        .insert("sourceOut".to_string(), json!(head_handle_seconds + 4.0));
    project.timeline.tracks[0].items.push(right);
    project.timeline.duration_seconds = 8.0;
    project
}

fn with_transition(mut project: VideoProject) -> VideoProject {
    project.timeline.tracks[0]
        .transitions
        .push(TimelineTransition {
            id: "fade-1".to_string(),
            left_item_id: "item-1".to_string(),
            right_item_id: "item-2".to_string(),
            kind: TransitionKind::Crossfade,
            duration_seconds: 0.5,
        });
    project
}

fn validation_message(error: CodexLocalToolError) -> String {
    match error {
        CodexLocalToolError::ProjectActionValidation(message) => message,
        other => panic!("expected a project action validation error, got {other:?}"),
    }
}

fn invalid_arguments(error: CodexLocalToolError) -> String {
    match error {
        CodexLocalToolError::InvalidArguments(message) => message,
        other => panic!("expected invalid arguments, got {other:?}"),
    }
}

#[test]
fn local_tools_list_the_transition_tools() {
    let tools = list_codex_local_tools();
    let tool = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("{name} should be listed"))
    };
    let kinds = json!(["crossfade", "dipToBlack", "dipToWhite", "wipe"]);

    let add = tool("video_creater.add_transition");
    assert_eq!(add.category, "mutation");
    assert_eq!(
        add.input_schema["anyOf"],
        json!([
            { "required": ["leftItemId", "rightItemId", "kind"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(add.input_schema["properties"]["kind"]["enum"], kinds);
    assert_eq!(
        add.input_schema["properties"]["durationSeconds"],
        json!({ "type": "number", "exclusiveMinimum": 0, "maximum": 5 })
    );

    let update = tool("video_creater.update_transition");
    assert_eq!(
        update.input_schema["anyOf"],
        json!([
            { "required": ["transitionId", "kind"] },
            { "required": ["transitionId", "durationSeconds"] },
            { "required": ["projectDir", "actions"] }
        ])
    );
    assert_eq!(update.input_schema["properties"]["kind"]["enum"], kinds);

    let remove = tool("video_creater.remove_transition");
    assert_eq!(
        remove.input_schema["anyOf"],
        json!([
            { "required": ["transitionId"] },
            { "required": ["projectDir", "actions"] }
        ])
    );

    let response = handle_mcp_request(
        &sample_project(),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    );
    let names = response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    for name in [
        "video_creater.add_transition",
        "video_creater.update_transition",
        "video_creater.remove_transition",
    ] {
        assert!(names.contains(&name), "MCP should expose {name}");
    }
}

#[test]
fn add_transition_validates_with_a_default_duration() {
    let project = adjacent_clips_project(6.0);

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_transition",
        json!({ "leftItemId": "item-1", "rightItemId": "item-2", "kind": "dipToWhite" }),
    )
    .expect("add_transition should validate");

    assert!(result.mutates_project);
    assert_eq!(result.payload["valid"], json!(true));
    assert_eq!(
        result.payload["projectActions"],
        json!([{ "type": "addTransition", "trackId": project.timeline.tracks[0].id, "transition": {
            "id": "transition-item-1-item-2", "leftItemId": "item-1", "rightItemId": "item-2",
            "kind": "dipToWhite", "durationSeconds": 0.5,
        }}])
    );
    assert_eq!(result.payload["maxDurationSeconds"], json!(4.0));
    assert_eq!(
        result.payload["affectedItemIds"],
        json!(["item-1", "item-2"])
    );
}

#[test]
fn add_transition_shortens_the_default_to_the_available_handles() {
    let project = adjacent_clips_project(0.2);

    let result = call_codex_local_tool(
        &project,
        "video_creater.add_transition",
        json!({ "leftItemId": "item-1", "rightItemId": "item-2", "kind": "crossfade" }),
    )
    .expect("the default duration should fit the handles");

    assert_eq!(result.payload["durationSeconds"], json!(0.4));
    assert_eq!(result.payload["maxDurationSeconds"], json!(0.4));
}

#[test]
fn transition_tools_return_validation_messages_for_bad_input() {
    let project = adjacent_clips_project(0.2);
    let call = |tool: &str, args: Value| call_codex_local_tool(&project, tool, args);

    let short = call(
        "video_creater.add_transition",
        json!({ "leftItemId": "item-1", "rightItemId": "item-2", "kind": "crossfade", "durationSeconds": 1.0 }),
    )
    .expect_err("handles are too short");
    assert_eq!(
        validation_message(short),
        "Not enough unused media before Closing clip for a 1.0s transition. Maximum is 0.4s."
    );

    let reversed = call(
        "video_creater.add_transition",
        json!({ "leftItemId": "item-2", "rightItemId": "item-1", "kind": "crossfade" }),
    )
    .expect_err("clips in the wrong order");
    assert_eq!(
        validation_message(reversed),
        "Closing clip must end where Opening clip starts to add a transition."
    );

    let unknown_kind = call(
        "video_creater.add_transition",
        json!({ "leftItemId": "item-1", "rightItemId": "item-2", "kind": "starWipe" }),
    )
    .expect_err("unknown kind");
    assert!(invalid_arguments(unknown_kind).contains("unknown variant `starWipe`"));

    let missing_clip = call(
        "video_creater.add_transition",
        json!({ "leftItemId": "item-1", "rightItemId": "missing", "kind": "crossfade" }),
    )
    .expect_err("missing clip");
    assert_eq!(
        invalid_arguments(missing_clip),
        "rightItemId did not match a timeline item: missing"
    );

    let project = with_transition(project);
    let call = |tool: &str, args: Value| call_codex_local_tool(&project, tool, args);
    let no_change = call(
        "video_creater.update_transition",
        json!({ "transitionId": "fade-1" }),
    )
    .expect_err("nothing to update");
    assert_eq!(
        invalid_arguments(no_change),
        "update_transition needs kind or durationSeconds"
    );

    let too_long = call(
        "video_creater.update_transition",
        json!({ "transitionId": "fade-1", "durationSeconds": 2.0 }),
    )
    .expect_err("longer than the handles allow");
    assert!(validation_message(too_long).ends_with("Maximum is 0.4s."));

    let unknown = call(
        "video_creater.remove_transition",
        json!({ "transitionId": "fade-9" }),
    )
    .expect_err("unknown transition");
    assert_eq!(
        invalid_arguments(unknown),
        "transitionId did not match a transition: fade-9"
    );
}

#[test]
fn transition_tools_apply_update_and_remove_in_the_project_folder() {
    let project = adjacent_clips_project(6.0);
    let project_dir = tempfile::tempdir().expect("project dir");
    save_split_project(project_dir.path(), &project).expect("save project");
    let dir = project_dir.path().display().to_string();

    let added = call_codex_local_tool(
        &project,
        "video_creater.add_transition",
        json!({
            "projectDir": dir, "leftItemId": "item-1", "rightItemId": "item-2",
            "kind": "crossfade", "durationSeconds": 1.0, "transitionId": "fade-1",
        }),
    )
    .expect("add_transition applies");
    assert_eq!(added.payload["applied"], json!(true));
    let persisted = load_split_project(project_dir.path()).expect("load project");
    assert_eq!(
        persisted.timeline.tracks[0].transitions,
        vec![TimelineTransition {
            id: "fade-1".to_string(),
            left_item_id: "item-1".to_string(),
            right_item_id: "item-2".to_string(),
            kind: TransitionKind::Crossfade,
            duration_seconds: 1.0,
        }]
    );

    let timeline = call_codex_local_tool(&persisted, "video_creater.get_timeline", json!({}))
        .expect("timeline");
    assert_eq!(
        timeline.payload["tracks"][0]["transitions"][0]["kind"],
        json!("crossfade")
    );

    let updated = call_codex_local_tool(
        &persisted,
        "video_creater.update_transition",
        json!({ "projectDir": dir, "transitionId": "fade-1", "kind": "dipToBlack", "durationSeconds": 2.0 }),
    )
    .expect("update_transition applies");
    assert_eq!(
        updated.payload["trackId"],
        json!(persisted.timeline.tracks[0].id)
    );
    let persisted = load_split_project(project_dir.path()).expect("load updated project");
    let transition = &persisted.timeline.tracks[0].transitions[0];
    assert_eq!(
        (transition.kind, transition.duration_seconds),
        (TransitionKind::DipToBlack, 2.0)
    );

    call_codex_local_tool(
        &persisted,
        "video_creater.remove_transition",
        json!({ "projectDir": dir, "transitionId": "fade-1" }),
    )
    .expect("remove_transition applies");
    let persisted = load_split_project(project_dir.path()).expect("load project after removal");
    assert!(persisted.timeline.tracks[0].transitions.is_empty());
    let clips: Vec<&TimelineItem> = persisted.timeline.tracks[0].items.iter().collect();
    assert_eq!(clips.len(), 2);
    assert!(clips
        .iter()
        .all(|clip| clip.kind == TimelineItemKind::VideoClip));
}

#[test]
fn scoped_mcp_calls_add_transitions_to_the_launch_project() {
    let project = adjacent_clips_project(6.0);
    let launch = tempfile::tempdir().expect("launch project");
    save_split_project(launch.path(), &project).expect("save launch project");

    let response = handle_scoped_mcp_request(
        &project,
        launch.path(),
        json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": {
                "name": "video_creater.add_transition",
                "arguments": { "leftItemId": "item-1", "rightItemId": "item-2", "kind": "wipe" }
            }
        }),
    );

    assert_eq!(response["result"]["isError"], json!(false), "{response}");
    let persisted = load_split_project(launch.path()).expect("load launch project");
    assert_eq!(
        persisted.timeline.tracks[0].transitions[0].kind,
        TransitionKind::Wipe
    );
}
