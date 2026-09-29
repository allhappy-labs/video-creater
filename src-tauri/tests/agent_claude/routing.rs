//! Which backend a turn runs on, and on which model.
//!
//! Everything here goes through the real `AppPreferencesStore`, because the bug this guards
//! against is the setting being stored and then ignored. Nothing spawns a process and nothing
//! spends a token: the Claude arm is inspected as argv, which is the whole of its contract with
//! a binary this repo never ships.

use std::fs;
use std::path::{Path, PathBuf};
use video_creater_lib::agent::claude_cli::{build_claude_turn_argv, ClaudeSessionHandle};
use video_creater_lib::agent::claude_cli::{ClaudeAuthMode, ClaudeTurnReadiness};
use video_creater_lib::agent::turn::{
    agent_turn_preferences, plan_agent_turn, AgentTurnBackendPlan, AgentTurnPreferences,
};
use video_creater_lib::agent::{AgentBackendKind, AgentTurnError, AgentTurnRequest};
use video_creater_lib::settings::preferences::{
    AgentBackendPreference, AppPreferencesPatch, AppPreferencesStore, ClaudeModelPreference,
};

/// A file that exists and is executable, so `resolve_claude_executable` accepts it as the
/// user's own `claude` without anything ever being run.
fn installed_claude(directory: &Path) -> PathBuf {
    let path = directory.join("claude");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("a stub claude");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("an executable stub");
    }
    path
}

/// A `claude` path the user configured that is not runnable, so Claude is not ready. Pointing
/// at an explicit path keeps the test off whatever is installed on the machine running it.
fn uninstalled_claude(directory: &Path) -> PathBuf {
    directory.join("no-such-claude")
}

fn mcp_server() -> PathBuf {
    PathBuf::from("/opt/video-creater/video-creater-mcp-server")
}

/// Readiness as the probe would report it for these stubs: installed means ready on a
/// subscription, and a path that is not runnable means no Claude at all. Passing it in keeps
/// the routing tests from spawning anything.
fn readiness(preferences: &AgentTurnPreferences) -> ClaudeTurnReadiness {
    if preferences.claude_installed() {
        ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription)
    } else {
        ClaudeTurnReadiness::Unavailable
    }
}

fn stored_preferences(
    directory: &Path,
    backend: AgentBackendPreference,
    model: ClaudeModelPreference,
    claude_executable: &Path,
) -> AgentTurnPreferences {
    let store = AppPreferencesStore::new(directory.join("preferences.json"));
    store
        .update(AppPreferencesPatch {
            agent_backend: Some(backend),
            claude_model: Some(model),
            claude_executable_path: Some(claude_executable.display().to_string()),
            ..AppPreferencesPatch::default()
        })
        .expect("the preference must be stored");
    agent_turn_preferences(&store)
}

fn backend_of(plan: &AgentTurnBackendPlan) -> AgentBackendKind {
    match plan {
        AgentTurnBackendPlan::Codex => AgentBackendKind::Codex,
        AgentTurnBackendPlan::Claude(_) => AgentBackendKind::Claude,
    }
}

fn turn_request() -> AgentTurnRequest {
    AgentTurnRequest {
        cwd: PathBuf::from("/home/user/Movies"),
        developer_instructions: "You propose video edits.".to_string(),
        prompt: "Trim the opening.".to_string(),
        output_schema: serde_json::json!({ "type": "object" }),
        session: None,
        project_dir: Some(PathBuf::from("/home/user/Movies/Project")),
        cancellation_key: "/home/user/Movies".to_string(),
    }
}

fn claude_argv(plan: AgentTurnBackendPlan) -> Vec<String> {
    let AgentTurnBackendPlan::Claude(transport) = plan else {
        panic!("the turn must be planned for Claude");
    };
    build_claude_turn_argv(&transport.invocation(
        &turn_request(),
        ClaudeSessionHandle::New("0406a3dc-a644-4397-ae6e-a9552d78429d".to_string()),
    ))
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1).cloned())
}

#[test]
fn automatic_prefers_claude_when_both_agents_are_ready() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Automatic,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    let (selection, plan) = plan_agent_turn(
        &preferences,
        true,
        ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription),
        mcp_server(),
    )
    .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Claude);
    // Automatic had no preference to abandon, so nothing is reported as a fallback.
    assert_eq!(selection.fell_back_from, None);
    assert_eq!(backend_of(&plan), AgentBackendKind::Claude);
}

#[test]
fn automatic_falls_back_to_codex_when_claude_is_not_signed_in() {
    let temp = tempfile::tempdir().expect("a temp dir");
    // The binary is installed, so only the missing sign-in can send the turn to Codex.
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Automatic,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    let (selection, plan) = plan_agent_turn(
        &preferences,
        true,
        ClaudeTurnReadiness::Unavailable,
        mcp_server(),
    )
    .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Codex);
    assert_eq!(selection.fell_back_from, None);
    assert_eq!(backend_of(&plan), AgentBackendKind::Codex);
}

#[test]
fn automatic_uses_claude_when_codex_is_unavailable() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Automatic,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    let (selection, plan) =
        plan_agent_turn(&preferences, false, readiness(&preferences), mcp_server())
            .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Claude);
    assert_eq!(selection.fell_back_from, None);
    assert_eq!(backend_of(&plan), AgentBackendKind::Claude);
}

#[test]
fn automatic_reports_no_agent_when_neither_is_ready() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Automatic,
        ClaudeModelPreference::Sonnet,
        &uninstalled_claude(temp.path()),
    );

    assert!(matches!(
        plan_agent_turn(&preferences, false, readiness(&preferences), mcp_server()),
        Err(AgentTurnError::Unavailable(_))
    ));
}

#[test]
fn a_stored_claude_preference_routes_the_turn_to_claude() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Claude,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    // Codex is ready too, so only the stored preference can be what sends the turn to Claude.
    let (selection, plan) =
        plan_agent_turn(&preferences, true, readiness(&preferences), mcp_server())
            .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Claude);
    assert_eq!(selection.fell_back_from, None);
    assert_eq!(backend_of(&plan), AgentBackendKind::Claude);
}

#[test]
fn a_stored_codex_preference_routes_the_turn_to_codex() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Codex,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    let (selection, plan) =
        plan_agent_turn(&preferences, true, readiness(&preferences), mcp_server())
            .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Codex);
    assert_eq!(backend_of(&plan), AgentBackendKind::Codex);
}

#[test]
fn a_stored_claude_preference_falls_back_to_codex_when_claude_is_not_installed() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Claude,
        ClaudeModelPreference::Sonnet,
        &uninstalled_claude(temp.path()),
    );

    let (selection, plan) =
        plan_agent_turn(&preferences, true, readiness(&preferences), mcp_server())
            .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Codex);
    // The user asked for Claude, so the turn reports which preference it had to abandon.
    assert_eq!(selection.fell_back_from, Some(AgentBackendKind::Claude));
    assert_eq!(backend_of(&plan), AgentBackendKind::Codex);
}

#[test]
fn a_stored_codex_preference_falls_back_to_claude() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Codex,
        ClaudeModelPreference::Sonnet,
        &installed_claude(temp.path()),
    );

    let (selection, _) =
        plan_agent_turn(&preferences, false, readiness(&preferences), mcp_server())
            .expect("a planned turn");

    assert_eq!(selection.kind, AgentBackendKind::Claude);
    assert_eq!(selection.fell_back_from, Some(AgentBackendKind::Codex));
}

#[test]
fn the_stored_model_preference_reaches_the_claude_argv() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let claude = installed_claude(temp.path());

    for (stored, model, fallback) in [
        (ClaudeModelPreference::Sonnet, "sonnet", Some("haiku")),
        (ClaudeModelPreference::Haiku, "haiku", None),
        (ClaudeModelPreference::Opus, "opus", Some("sonnet")),
    ] {
        let preferences =
            stored_preferences(temp.path(), AgentBackendPreference::Claude, stored, &claude);
        let (_, plan) = plan_agent_turn(&preferences, true, readiness(&preferences), mcp_server())
            .expect("a planned turn");
        let args = claude_argv(plan);

        assert_eq!(value_after(&args, "--model").as_deref(), Some(model));
        assert_eq!(
            value_after(&args, "--fallback-model").as_deref(),
            fallback,
            "the fallback for {model} must be the next cheaper alias"
        );
    }
}

#[test]
fn the_stored_executable_path_is_the_binary_the_turn_would_run() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let claude = installed_claude(temp.path());
    let preferences = stored_preferences(
        temp.path(),
        AgentBackendPreference::Claude,
        ClaudeModelPreference::Sonnet,
        &claude,
    );

    assert_eq!(
        preferences.claude_executable.as_deref(),
        Some(claude.as_path())
    );
    assert!(preferences.claude_installed());
}

#[test]
fn a_missing_preferences_file_behaves_like_automatic() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let store = AppPreferencesStore::new(temp.path().join("never-written.json"));

    let preferences = agent_turn_preferences(&store);

    assert_eq!(preferences, AgentTurnPreferences::default());
    assert_eq!(preferences.backend, None);
    assert_eq!(preferences.claude_model, "sonnet");
    // Reading the preference must not create one.
    assert!(!temp.path().join("never-written.json").exists());
}

#[test]
fn an_unreadable_preferences_file_behaves_like_automatic() {
    let temp = tempfile::tempdir().expect("a temp dir");
    let path = temp.path().join("preferences.json");
    fs::write(&path, "{ this is not JSON").expect("a corrupt preferences file");

    let preferences = agent_turn_preferences(&AppPreferencesStore::new(path));

    assert_eq!(preferences, AgentTurnPreferences::default());
    assert_eq!(preferences.backend, None);
}
