//! How a Claude turn authenticates.
//!
//! The product's promise is that a user with their own Claude subscription needs nothing else —
//! no API key, no account to create, no credential the app stores. Two things have to hold for
//! that to be true, and both are checked here without a real turn and without a token:
//!
//! 1. The readiness probe can tell a subscription login from a key-backed one, and says so in
//!    words that point a signed-out user at `/login` rather than at buying API credit.
//! 2. `ANTHROPIC_API_KEY` in the app's environment cannot silently take over a subscription
//!    session. The CLI prefers the key over the login, so an app that merely inherited its
//!    environment would bill an API account for a turn the user expected their plan to cover.
//!    A subscription session therefore withholds the variable from the child; a session whose
//!    only credential *is* a key passes it through untouched.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use video_creater_lib::agent::claude_cli::{
    claude_child_env_removals, ClaudeAuthMode, ClaudeTurnReadiness, ANTHROPIC_API_KEY_ENV,
};
use video_creater_lib::agent::claude_transport::ClaudeTurnTransport;
use video_creater_lib::agent::{AgentTurnRequest, AgentTurnTransport};
use video_creater_lib::settings::agent::claude_readiness;

/// `ANTHROPIC_API_KEY` is process-wide state, so the tests that touch it take a turn. Only
/// this file reads or writes it; the real-turn tests in this binary are `#[ignore]`d.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Sets `ANTHROPIC_API_KEY` for the duration of one test and puts the environment back, so a
/// later test in the same binary cannot inherit it.
struct ApiKeyEnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous: Option<std::ffi::OsString>,
}

impl ApiKeyEnvGuard {
    fn set(value: Option<&str>) -> Self {
        let lock = ENV_LOCK.lock().expect("the api key env lock");
        let previous = std::env::var_os(ANTHROPIC_API_KEY_ENV);
        match value {
            Some(value) => std::env::set_var(ANTHROPIC_API_KEY_ENV, value),
            None => std::env::remove_var(ANTHROPIC_API_KEY_ENV),
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for ApiKeyEnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(previous) => std::env::set_var(ANTHROPIC_API_KEY_ENV, previous),
            None => std::env::remove_var(ANTHROPIC_API_KEY_ENV),
        }
    }
}

fn write_stub(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("an exec bit");
    }
    path
}

/// A stand-in `claude` that answers the two zero-token probes from a script.
fn claude_stub(directory: &Path, version: &str, auth_status: &str) -> PathBuf {
    write_stub(
        directory,
        "claude",
        &format!(
            r#"case "$1" in
  --version) printf '%s\n' '{version}' ;;
  auth) printf '%s\n' '{auth_status}' ;;
  *) exit 64 ;;
esac"#
        ),
    )
}

fn request(cwd: &Path) -> AgentTurnRequest {
    AgentTurnRequest {
        cwd: cwd.to_path_buf(),
        developer_instructions: "You propose video edits.".to_string(),
        prompt: "Trim the opening.".to_string(),
        output_schema: serde_json::json!({ "type": "object" }),
        session: None,
        project_dir: Some(cwd.to_path_buf()),
        cancellation_key: cwd.display().to_string(),
    }
}

/// Runs a turn against a stub that records what the child saw in `ANTHROPIC_API_KEY`, and
/// returns that recording. The stub answers with a frame-shaped stream so the turn completes
/// through the transport's normal path rather than an error branch.
fn key_seen_by_the_child(auth: ClaudeAuthMode, exported: &str) -> String {
    let directory = tempfile::tempdir().expect("a temp dir");
    let seen = directory.path().join("seen-key.txt");
    let stub = write_stub(
        directory.path(),
        "claude",
        &format!(
            "cat >/dev/null\nprintf '%s' \"${{{ANTHROPIC_API_KEY_ENV}-<unset>}}\" > {}\n\
             printf '%s\\n' '{{\"type\":\"result\",\"subtype\":\"success\",\"session_id\":\"s1\"}}'",
            seen.display()
        ),
    );

    let _guard = ApiKeyEnvGuard::set(Some(exported));
    ClaudeTurnTransport::with_executable(stub, PathBuf::from("/opt/video-creater-mcp-server"))
        .with_auth(auth)
        .run_conversation_turn(
            &request(directory.path()),
            Instant::now() + Duration::from_secs(30),
            None,
        )
        .expect("the stub turn must complete");

    fs::read_to_string(&seen).expect("the child's view of the environment")
}

#[test]
fn a_subscription_login_is_ready_and_needs_no_api_key() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let stub = claude_stub(
        directory.path(),
        "2.1.270 (Claude Code)",
        r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max","email":"a@b.c","orgId":"org_1","orgName":"Acme"}"#,
    );
    let _guard = ApiKeyEnvGuard::set(None);

    let (health, readiness) = claude_readiness(Some(stub.to_str().expect("a stub path")));

    assert_eq!(
        readiness,
        ClaudeTurnReadiness::Ready(ClaudeAuthMode::Subscription)
    );
    assert!(
        health.summary.contains("Claude subscription"),
        "{}",
        health.summary
    );
    assert!(
        health.summary.contains("No API key is needed"),
        "{}",
        health.summary
    );
    // Nothing about the account's identity reaches the row.
    let rendered = format!("{health:?}");
    for secret in ["a@b.c", "org_1", "Acme"] {
        assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
    }
}

#[test]
fn a_login_that_is_not_a_subscription_is_ready_without_claiming_one() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let stub = claude_stub(
        directory.path(),
        "2.1.270 (Claude Code)",
        r#"{"loggedIn":true,"authMethod":"apiKey","apiKeySource":"ANTHROPIC_API_KEY"}"#,
    );
    let _guard = ApiKeyEnvGuard::set(None);

    let (health, readiness) = claude_readiness(Some(stub.to_str().expect("a stub path")));

    assert_eq!(
        readiness,
        ClaudeTurnReadiness::Ready(ClaudeAuthMode::OtherLogin)
    );
    assert!(
        health.summary.contains("not with a Claude subscription"),
        "{}",
        health.summary
    );
}

#[test]
fn a_signed_out_claude_is_not_ready_and_points_at_the_subscription_login() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let stub = claude_stub(
        directory.path(),
        "2.1.270 (Claude Code)",
        r#"{"loggedIn":false}"#,
    );
    let _guard = ApiKeyEnvGuard::set(None);

    let (health, readiness) = claude_readiness(Some(stub.to_str().expect("a stub path")));

    assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
    assert!(health.summary.contains("/login"), "{}", health.summary);
    assert!(
        !health.summary.to_lowercase().contains("api key"),
        "the instruction must not ask for an API key: {}",
        health.summary
    );
}

#[test]
fn a_signed_out_claude_with_an_environment_key_is_ready_on_that_key() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let stub = claude_stub(
        directory.path(),
        "2.1.270 (Claude Code)",
        r#"{"loggedIn":false}"#,
    );
    let _guard = ApiKeyEnvGuard::set(Some("sk-ant-not-a-real-key"));

    let (health, readiness) = claude_readiness(Some(stub.to_str().expect("a stub path")));

    assert_eq!(
        readiness,
        ClaudeTurnReadiness::Ready(ClaudeAuthMode::ApiKey)
    );
    assert!(
        health
            .summary
            .contains("Anthropic API key from the environment"),
        "{}",
        health.summary
    );
    // The key itself is a secret and must never be echoed back.
    let rendered = format!("{health:?}");
    assert!(!rendered.contains("sk-ant-not-a-real-key"), "{rendered}");
}

#[test]
fn a_claude_that_is_not_installed_is_not_ready() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let missing = directory.path().join("no-such-claude");
    let _guard = ApiKeyEnvGuard::set(None);

    let (health, readiness) = claude_readiness(Some(missing.to_str().expect("a path")));

    assert_eq!(readiness, ClaudeTurnReadiness::Unavailable);
    assert!(
        health.summary.contains("isn't installed"),
        "{}",
        health.summary
    );
}

#[test]
fn the_child_env_removals_withhold_the_key_only_from_a_subscription_session() {
    assert_eq!(
        claude_child_env_removals(ClaudeAuthMode::Subscription),
        vec![ANTHROPIC_API_KEY_ENV.to_string()]
    );
    assert!(claude_child_env_removals(ClaudeAuthMode::OtherLogin).is_empty());
    assert!(claude_child_env_removals(ClaudeAuthMode::ApiKey).is_empty());
}

/// The behaviour the decision exists for: the user's subscription pays, even though the app's
/// environment carries a key the CLI would otherwise prefer.
#[test]
fn a_subscription_turn_does_not_pass_the_environment_api_key_to_the_child() {
    assert_eq!(
        key_seen_by_the_child(ClaudeAuthMode::Subscription, "sk-ant-not-a-real-key"),
        "<unset>"
    );
}

/// The other half: when the key is the only credential, removing it would remove the login.
#[test]
fn a_key_backed_turn_passes_the_environment_api_key_through() {
    assert_eq!(
        key_seen_by_the_child(ClaudeAuthMode::ApiKey, "sk-ant-not-a-real-key"),
        "sk-ant-not-a-real-key"
    );
    assert_eq!(
        key_seen_by_the_child(ClaudeAuthMode::OtherLogin, "sk-ant-not-a-real-key"),
        "sk-ant-not-a-real-key"
    );
}
