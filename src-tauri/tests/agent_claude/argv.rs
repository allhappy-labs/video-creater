//! The Claude invocation is a contract with a binary this repo never ships, so the argv, the
//! MCP config and the read-only tool surface are pinned by tests rather than by trust.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use video_creater_lib::agent::claude_cli::{
    build_claude_turn_argv, claude_allowed_tools_argument, claude_disallowed_tools_argument,
    claude_mcp_config_json, claude_mcp_tool_name, claude_tool_partition, claude_turn_stdin_line,
    resolve_claude_executable, resolve_claude_executable_in, ClaudeSessionHandle,
    ClaudeTurnInvocation, CLAUDE_DEFAULT_MODEL, CLAUDE_FALLBACK_MODEL, CLAUDE_READ_ONLY_TOOLS,
};
use video_creater_lib::agent::schema::codex_conversation_proposal_output_schema;
use video_creater_lib::codex::tools::{
    call_codex_local_tool, list_codex_local_tools, resolve_codex_tool_name, CodexLocalToolError,
};
use video_creater_lib::project::model::VideoProject;

fn invocation(session: ClaudeSessionHandle) -> ClaudeTurnInvocation {
    ClaudeTurnInvocation {
        model: CLAUDE_DEFAULT_MODEL.to_string(),
        fallback_model: Some(CLAUDE_FALLBACK_MODEL.to_string()),
        developer_instructions: "You propose video edits.".to_string(),
        output_schema: codex_conversation_proposal_output_schema(),
        session,
        mcp_config: claude_mcp_config_json(
            Path::new("/opt/video-creater/video-creater-mcp-server"),
            Path::new("/home/user/Movies/Project"),
        ),
        budget_usd: 0.5,
    }
}

fn first_turn_argv() -> Vec<String> {
    build_claude_turn_argv(&invocation(ClaudeSessionHandle::New(
        "0406a3dc-a644-4397-ae6e-a9552d78429d".to_string(),
    )))
}

fn resumed_argv() -> Vec<String> {
    build_claude_turn_argv(&invocation(ClaudeSessionHandle::Resume(
        "0406a3dc-a644-4397-ae6e-a9552d78429d".to_string(),
    )))
}

fn index_of(args: &[String], value: &str) -> Option<usize> {
    args.iter().position(|arg| arg == value)
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    index_of(args, flag).and_then(|index| args.get(index + 1).cloned())
}

#[test]
fn a_first_turn_passes_the_fixed_print_mode_flags_in_order() {
    let args = first_turn_argv();
    let expected_order = [
        "--print",
        "--output-format",
        "--verbose",
        "--input-format",
        "--model",
        "--system-prompt",
        "--json-schema",
        "--session-id",
        "--tools",
        "--allowed-tools",
        "--disallowed-tools",
        "--mcp-config",
        "--strict-mcp-config",
        "--permission-mode",
        "--permission-prompts",
        "--setting-sources",
        "--disable-slash-commands",
        "--max-budget-usd",
    ];

    let mut previous = 0usize;
    for flag in expected_order {
        let position = index_of(&args, flag).unwrap_or_else(|| panic!("{flag} is missing"));
        assert!(
            position >= previous,
            "{flag} appears out of order at {position}, after {previous}"
        );
        previous = position;
    }

    assert_eq!(
        value_after(&args, "--output-format").as_deref(),
        Some("stream-json")
    );
    assert_eq!(
        value_after(&args, "--input-format").as_deref(),
        Some("stream-json")
    );
    assert_eq!(value_after(&args, "--model").as_deref(), Some("sonnet"));
    assert_eq!(
        value_after(&args, "--system-prompt").as_deref(),
        Some("You propose video edits.")
    );
    assert_eq!(
        value_after(&args, "--session-id").as_deref(),
        Some("0406a3dc-a644-4397-ae6e-a9552d78429d")
    );
    assert_eq!(value_after(&args, "--tools").as_deref(), Some(""));
    assert_eq!(
        value_after(&args, "--permission-mode").as_deref(),
        Some("dontAsk")
    );
    assert_eq!(
        value_after(&args, "--permission-prompts").as_deref(),
        Some("none")
    );
    assert_eq!(value_after(&args, "--setting-sources").as_deref(), Some(""));
    assert_eq!(
        value_after(&args, "--max-budget-usd").as_deref(),
        Some("0.5")
    );
    assert_eq!(
        value_after(&args, "--fallback-model").as_deref(),
        Some("haiku")
    );
}

#[test]
fn a_resumed_turn_replaces_the_session_id_and_drops_the_system_prompt() {
    let args = resumed_argv();

    assert_eq!(
        value_after(&args, "--resume").as_deref(),
        Some("0406a3dc-a644-4397-ae6e-a9552d78429d")
    );
    // `--system-prompt-snapshot` defaults to `on`, so a resumed session reuses the prompt it
    // was created with and silently ignores new text. Sending it again would only mislead.
    assert_eq!(index_of(&args, "--system-prompt"), None);
    assert_eq!(index_of(&args, "--session-id"), None);
    assert!(index_of(&args, "--json-schema").is_some());
}

#[test]
fn dangerous_and_oauth_refusing_flags_never_appear() {
    for args in [first_turn_argv(), resumed_argv()] {
        // `--bare` refuses OAuth and demands an API key, which breaks subscription users.
        assert_eq!(index_of(&args, "--bare"), None);
        assert_eq!(index_of(&args, "--dangerously-skip-permissions"), None);
        assert_eq!(
            index_of(&args, "--allow-dangerously-skip-permissions"),
            None
        );
        assert_eq!(index_of(&args, "--append-system-prompt"), None);
    }
}

#[test]
fn the_json_schema_argument_is_one_line_and_round_trips() {
    let args = first_turn_argv();
    let schema = value_after(&args, "--json-schema").expect("--json-schema is missing");

    assert!(!schema.contains('\n'), "the schema must be a single line");
    let parsed: Value = serde_json::from_str(&schema).expect("the schema must be valid JSON");
    assert_eq!(parsed, codex_conversation_proposal_output_schema());
}

#[test]
fn every_allowed_tool_is_a_read_only_app_tool() {
    let args = first_turn_argv();
    let allowed = value_after(&args, "--allowed-tools").expect("--allowed-tools is missing");
    let entries: Vec<&str> = allowed.split(',').collect();

    // Every read-only tool must be reachable, under every name the sidecar exposes it as.
    assert!(entries.len() >= CLAUDE_READ_ONLY_TOOLS.len());
    for entry in &entries {
        let exposed = entry
            .strip_prefix("mcp__video-creater__")
            .unwrap_or_else(|| panic!("{entry} is not an app MCP tool"));
        let descriptor = list_codex_local_tools()
            .into_iter()
            .find(|tool| tool.name.replace('.', "_") == exposed)
            .unwrap_or_else(|| panic!("{exposed} is not a tool the app exposes"));
        let canonical = resolve_codex_tool_name(&descriptor.name).expect("a canonical name");
        let bare = canonical
            .strip_prefix("video_creater.")
            .unwrap_or(canonical.as_str());
        assert!(
            CLAUDE_READ_ONLY_TOOLS.contains(&bare),
            "{bare} is not on the read-only list"
        );
    }
    for tool in CLAUDE_READ_ONLY_TOOLS {
        assert!(
            entries.contains(&format!("mcp__video-creater__video_creater_{tool}").as_str()),
            "{tool} is not reachable under its canonical MCP name"
        );
    }
    assert_eq!(allowed, claude_allowed_tools_argument());
    assert_eq!(
        claude_mcp_tool_name("video_creater_timeline"),
        "mcp__video-creater__video_creater_timeline"
    );
}

#[test]
fn every_other_sidecar_tool_is_explicitly_disallowed() {
    // `--allowed-tools` is a permission allowlist, not an advertisement filter: the session
    // still sees every tool the sidecar exposes. The complement is derived, so a tool added
    // later is denied by default rather than inheriting permission.
    let (allowed, disallowed) = claude_tool_partition();
    let exposed: BTreeSet<String> = list_codex_local_tools()
        .into_iter()
        .map(|tool| format!("mcp__video-creater__{}", tool.name.replace('.', "_")))
        .collect();

    let covered: BTreeSet<String> = allowed.iter().chain(disallowed.iter()).cloned().collect();
    assert_eq!(covered, exposed, "every exposed tool must be classified");
    assert!(
        disallowed.iter().all(|tool| !allowed.contains(tool)),
        "no tool may be both"
    );

    for mutating in [
        "mcp__video-creater__video_creater_delete_media",
        "mcp__video-creater__video_creater_generate_video",
        "mcp__video-creater__video_creater_apply_project_actions",
        "mcp__video-creater__video_creater_export_project",
        "mcp__video-creater__video_creater_undo_agent_edit",
    ] {
        assert!(
            disallowed.contains(&mutating.to_string()),
            "{mutating} must be denied"
        );
    }
    assert_eq!(
        value_after(&first_turn_argv(), "--disallowed-tools").as_deref(),
        Some(claude_disallowed_tools_argument().as_str())
    );
}

#[test]
fn the_read_only_list_is_the_nineteen_inspection_tools() {
    assert_eq!(CLAUDE_READ_ONLY_TOOLS.len(), 19);
    let unique: BTreeSet<&str> = CLAUDE_READ_ONLY_TOOLS.iter().copied().collect();
    assert_eq!(unique.len(), 19);

    for tool in CLAUDE_READ_ONLY_TOOLS {
        assert!(
            resolve_codex_tool_name(tool).is_some(),
            "{tool} is not a real app tool"
        );
    }
}

#[test]
fn no_allow_listed_tool_can_mutate_a_project() {
    // The real guard: this fails the day somebody makes a listed tool mutating.
    let project = sample_project();
    let mut ran = 0usize;
    for tool in CLAUDE_READ_ONLY_TOOLS {
        match call_codex_local_tool(&project, tool, serde_json::json!({})) {
            Ok(result) => {
                ran += 1;
                assert!(
                    !result.mutates_project,
                    "{tool} reports that it mutates the project"
                );
            }
            // A tool that refuses the empty argument set never got far enough to mutate, but
            // it must still be a tool the app knows about.
            Err(error) => assert!(
                !matches!(error, CodexLocalToolError::UnknownTool(_)),
                "{tool} is not a tool the app exposes: {error}"
            ),
        }
    }
    assert!(
        ran >= CLAUDE_READ_ONLY_TOOLS.len() / 2,
        "only {ran} of the listed tools actually ran; the guard is not proving much"
    );
}

#[test]
fn the_mcp_config_points_at_the_app_sidecar_for_an_absolute_project_dir() {
    let config = claude_mcp_config_json(
        Path::new("/opt/video-creater/video-creater-mcp-server"),
        Path::new("/home/user/Movies/Project"),
    );
    assert!(!config.contains('\n'));

    let parsed: Value = serde_json::from_str(&config).expect("the config must be valid JSON");
    let server = &parsed["mcpServers"]["video-creater"];
    assert_eq!(server["type"], "stdio");
    assert_eq!(
        server["command"],
        "/opt/video-creater/video-creater-mcp-server"
    );
    assert_eq!(
        server["args"],
        serde_json::json!(["--project-dir", "/home/user/Movies/Project"])
    );
    let project_dir = server["args"][1].as_str().expect("a project dir");
    assert!(Path::new(project_dir).is_absolute());
}

#[test]
fn the_prompt_reaches_stdin_as_one_stream_json_user_line() {
    let line = claude_turn_stdin_line("Trim the first clip.");
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1);

    let parsed: Value = serde_json::from_str(line.trim_end()).expect("valid JSON");
    assert_eq!(parsed["type"], "user");
    assert_eq!(parsed["message"]["role"], "user");
    assert_eq!(parsed["message"]["content"][0]["type"], "text");
    assert_eq!(
        parsed["message"]["content"][0]["text"],
        "Trim the first clip."
    );
}

#[test]
fn an_explicit_executable_path_is_used_verbatim_when_it_is_executable() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let configured = directory.path().join("claude");
    write_executable(&configured);

    assert_eq!(
        resolve_claude_executable(Some(&configured)),
        Some(configured.clone())
    );

    let missing = directory.path().join("absent");
    assert_eq!(resolve_claude_executable(Some(&missing)), None);
}

#[test]
fn the_executable_is_searched_on_path_then_in_the_two_known_install_dirs() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let on_path_dir = directory.path().join("bin");
    let home = directory.path().join("home");
    fs::create_dir_all(&on_path_dir).expect("a bin dir");
    fs::create_dir_all(home.join(".local").join("bin")).expect("a local bin dir");

    let on_path = on_path_dir.join("claude");
    let in_home = home.join(".local").join("bin").join("claude");
    write_executable(&on_path);
    write_executable(&in_home);

    let path = std::ffi::OsString::from(on_path_dir.display().to_string());
    assert_eq!(
        resolve_claude_executable_in(None, Some(&path), Some(home.clone())),
        Some(on_path)
    );

    // Nothing on PATH: `~/.local/bin` is the native installer's location.
    assert_eq!(
        resolve_claude_executable_in(None, None, Some(home)),
        Some(in_home)
    );
}

fn write_executable(path: &Path) {
    fs::write(path, "#!/bin/sh\nexit 0\n").expect("a stub binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("an executable mode");
    }
}

fn sample_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Claude Test".to_string(),
        "2026-09-18T00:00:00Z".to_string(),
    );
    project
        .media
        .push(video_creater_lib::project::model::MediaAsset {
            id: "media-1".to_string(),
            name: None,
            relative_path: "media/input.mp4".to_string(),
            kind: video_creater_lib::project::model::MediaKind::Video,
            duration_seconds: 12.0,
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            folder_id: None,
        });
    project
}
