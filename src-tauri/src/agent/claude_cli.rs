//! The Claude print-mode invocation.
//!
//! The argv here is the contract with the user's own installed `claude`. It is fixed except
//! for the model, the instructions, the schema, the session handle and the MCP config, and
//! every flag earns its place:
//!
//! - `--system-prompt` (not `--append-system-prompt`) replaces Claude Code's coding-agent
//!   prompt, which costs 7k input tokens a turn and instructs the model about editing code
//!   this product does not want edited. Because `--system-prompt-snapshot` defaults to `on`,
//!   a resumed session reuses the recorded prompt, so the flag is only sent on the first turn
//!   of a session and per-turn context always rides the user message instead.
//! - `--tools ""`, `--allowed-tools`, `--permission-mode dontAsk` and `--permission-prompts
//!   none` are four independent layers that leave exactly one read-only tool surface. Anything
//!   else is denied and recorded rather than prompted for.
//! - `--setting-sources ""`, `--strict-mcp-config` and `--disable-slash-commands` isolate the
//!   turn from the user's own CLAUDE.md, hooks, plugins, skills and MCP servers, which must not
//!   steer a video edit. `--safe-mode` would be the tidier way to say that, and it is
//!   deliberately **not** used: it disables MCP servers too, including the ones `--mcp-config`
//!   passes, which leaves the turn with no way to read the project at all. Verified on 2.1.270:
//!   with `--safe-mode` the init frame reports `mcp_servers: []`.
//! - `--bare` is deliberately absent: it refuses OAuth and demands an API key, which breaks
//!   every subscription user.
//! - `--max-budget-usd` is a guard rail, not a feature. It is generous and never surfaced.

use crate::codex::app_server::is_path_executable;
use crate::codex::tools::{list_codex_local_tools, resolve_codex_tool_name};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long one Claude turn may take. A turn that reads several MCP tools is slower than a
/// Codex app-server turn, so this is longer than `APP_SERVER_TURN_TIMEOUT`.
pub const CLAUDE_TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// The per-turn spend cap handed to `--max-budget-usd`.
pub const CLAUDE_TURN_BUDGET_USD: f64 = 0.50;
pub const CLAUDE_DEFAULT_MODEL: &str = "sonnet";
pub const CLAUDE_FALLBACK_MODEL: &str = "haiku";
/// The executable name looked up on `PATH`.
pub const CLAUDE_EXECUTABLE_NAME: &str = "claude";
/// The MCP server name the app registers. Tool names are `mcp__<server>__<tool>`.
pub const CLAUDE_MCP_SERVER_NAME: &str = "video-creater";
/// The environment variable that would make a turn bill an API account.
pub const ANTHROPIC_API_KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// Which credential a Claude turn will spend.
///
/// The product is built around the user's own subscription, so this is not decoration: it
/// decides whether an `ANTHROPIC_API_KEY` in the app's environment reaches the child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeAuthMode {
    /// A `claude.ai` login — the user's own Claude subscription. This is the supported path
    /// and the one the app protects.
    Subscription,
    /// Signed in, but by some other route the CLI reports (an enterprise login, a stored key,
    /// a gateway). The app does not second-guess it.
    OtherLogin,
    /// Not signed in at all: an `ANTHROPIC_API_KEY` in the environment is the only credential
    /// the turn could use. Optional, never required.
    ApiKey,
}

/// Whether a Claude turn can run, and on which credential — the whole of what the turn router
/// needs from the zero-token readiness probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeTurnReadiness {
    /// Installed and holding a credential a turn can use.
    Ready(ClaudeAuthMode),
    /// Either not installed, not runnable, or holding no credential. A turn must not be sent
    /// here: it would fail in the child instead of falling back to the other agent.
    Unavailable,
}

impl ClaudeTurnReadiness {
    pub fn is_ready(self) -> bool {
        matches!(self, ClaudeTurnReadiness::Ready(_))
    }

    pub fn auth(self) -> Option<ClaudeAuthMode> {
        match self {
            ClaudeTurnReadiness::Ready(auth) => Some(auth),
            ClaudeTurnReadiness::Unavailable => None,
        }
    }
}

/// The environment variables withheld from a turn's child process.
///
/// `ANTHROPIC_API_KEY` takes precedence over the OAuth login inside the CLI, so an app that
/// merely inherited its environment could silently bill a user's API account for a turn they
/// expected their subscription to cover — and could keep working after the key expired while
/// the readiness row said "subscription". So when the user is signed in with their
/// subscription the variable is withheld and the login they chose is the one that pays.
/// When the login *is* the key (nothing signed in, or a non-subscription login that may itself
/// be key-backed) the environment passes through untouched, because removing the variable
/// would be removing the only credential.
///
/// The value is never read anywhere in this path — only its presence, and only by the
/// readiness probe.
pub fn claude_child_env_removals(auth: ClaudeAuthMode) -> Vec<String> {
    match auth {
        ClaudeAuthMode::Subscription => vec![ANTHROPIC_API_KEY_ENV.to_string()],
        ClaudeAuthMode::OtherLogin | ClaudeAuthMode::ApiKey => Vec::new(),
    }
}

/// The only tools a conversation turn may call.
///
/// Every entry inspects and none mutates: nothing starts a job, nothing costs money, nothing
/// opens or creates a project. An edit still comes back as a proposal, because the turn's only
/// output channel is the synthetic `StructuredOutput` tool that `--json-schema` installs.
pub const CLAUDE_READ_ONLY_TOOLS: [&str; 19] = [
    "project_context",
    "timeline",
    "get_timeline",
    "inspect_timeline",
    "inspect_media",
    "media_library",
    "get_media",
    "list_folders",
    "get_transcript",
    "transcript_words",
    "search_media",
    "list_effects",
    "inspect_color",
    "generated_assets",
    "render_reports",
    "export_artifacts",
    "export_profiles",
    "generation_defaults",
    "read_skill",
];

/// Whether the turn starts a session or continues one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeSessionHandle {
    /// A new chat. The app mints the UUID so it can store it before the process runs.
    New(String),
    /// A chat that already has a Claude session.
    Resume(String),
}

/// Everything that varies between two Claude invocations.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeTurnInvocation {
    pub model: String,
    pub fallback_model: Option<String>,
    pub developer_instructions: String,
    pub output_schema: Value,
    pub session: ClaudeSessionHandle,
    /// The one-line `--mcp-config` JSON from [`claude_mcp_config_json`].
    pub mcp_config: String,
    pub budget_usd: f64,
}

/// The MCP tool name Claude uses for one of the app's own tools.
pub fn claude_mcp_tool_name(tool: &str) -> String {
    format!("mcp__{CLAUDE_MCP_SERVER_NAME}__{tool}")
}

/// Split the app's own MCP tools into the ones this turn may call and the ones it may not.
///
/// Both lists are *derived* from the sidecar's own tool table rather than written out, so a
/// tool added later is disallowed by default instead of quietly inheriting permission. The
/// sidecar exposes each tool under its canonical `video_creater.<name>` name and some also
/// under a bare alias; Claude turns the dot into an underscore. Both forms are resolved here,
/// because the model may call either.
pub fn claude_tool_partition() -> (Vec<String>, Vec<String>) {
    let read_only: BTreeSet<&str> = CLAUDE_READ_ONLY_TOOLS.iter().copied().collect();
    let mut allowed = BTreeSet::new();
    let mut disallowed = BTreeSet::new();

    for tool in list_codex_local_tools() {
        let canonical = resolve_codex_tool_name(&tool.name).unwrap_or_else(|| tool.name.clone());
        let bare = canonical
            .strip_prefix("video_creater.")
            .unwrap_or(canonical.as_str());
        let exposed = claude_mcp_tool_name(&tool.name.replace('.', "_"));
        if read_only.contains(bare) {
            allowed.insert(exposed);
        } else {
            disallowed.insert(exposed);
        }
    }

    (
        allowed.into_iter().collect(),
        disallowed.into_iter().collect(),
    )
}

/// The `--allowed-tools` value: the read-only surface, comma separated.
pub fn claude_allowed_tools_argument() -> String {
    claude_tool_partition().0.join(",")
}

/// The `--disallowed-tools` value: everything else the sidecar exposes.
///
/// `--allowed-tools` is a permission allowlist, not an advertisement filter: the init frame
/// still lists every MCP tool the server exposes (verified on 2.1.270). Naming the rest
/// explicitly makes the deny a decision rather than a side effect of the permission mode.
pub fn claude_disallowed_tools_argument() -> String {
    claude_tool_partition().1.join(",")
}

/// The one-line `--mcp-config` JSON pointing at the app's own sidecar for one project.
///
/// The sidecar sandboxes itself to its launch `--project-dir`, so the path must be absolute:
/// the child's cwd is the project folder, but nothing guarantees that for a future caller.
pub fn claude_mcp_config_json(executable: &Path, project_dir: &Path) -> String {
    json!({
        "mcpServers": {
            CLAUDE_MCP_SERVER_NAME: {
                "type": "stdio",
                "command": executable.display().to_string(),
                "args": ["--project-dir", project_dir.display().to_string()],
            }
        }
    })
    .to_string()
}

/// Build the argv for one turn, without the program name.
pub fn build_claude_turn_argv(invocation: &ClaudeTurnInvocation) -> Vec<String> {
    let mut args = vec![
        "--print".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--input-format".to_string(),
        "stream-json".to_string(),
        "--model".to_string(),
        invocation.model.clone(),
    ];
    if let Some(fallback) = &invocation.fallback_model {
        args.push("--fallback-model".to_string());
        args.push(fallback.clone());
    }

    match &invocation.session {
        ClaudeSessionHandle::New(session_id) => {
            // The recorded system-prompt snapshot is written on the session's first request,
            // so this is the only turn where the instructions can still take effect.
            args.push("--system-prompt".to_string());
            args.push(invocation.developer_instructions.clone());
            args.push("--json-schema".to_string());
            args.push(invocation.output_schema.to_string());
            args.push("--session-id".to_string());
            args.push(session_id.clone());
        }
        ClaudeSessionHandle::Resume(session_id) => {
            args.push("--json-schema".to_string());
            args.push(invocation.output_schema.to_string());
            args.push("--resume".to_string());
            args.push(session_id.clone());
        }
    }

    args.extend([
        "--tools".to_string(),
        String::new(),
        "--allowed-tools".to_string(),
        claude_allowed_tools_argument(),
        "--disallowed-tools".to_string(),
        claude_disallowed_tools_argument(),
        "--mcp-config".to_string(),
        invocation.mcp_config.clone(),
        "--strict-mcp-config".to_string(),
        "--permission-mode".to_string(),
        "dontAsk".to_string(),
        "--permission-prompts".to_string(),
        "none".to_string(),
        "--setting-sources".to_string(),
        String::new(),
        "--disable-slash-commands".to_string(),
        "--max-budget-usd".to_string(),
        format_budget(invocation.budget_usd),
    ]);
    args
}

fn format_budget(budget_usd: f64) -> String {
    let rendered = format!("{budget_usd:.4}");
    let trimmed = rendered.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The single stdin line that carries the turn's prompt.
///
/// `--input-format stream-json` emits no frames at all until the first user message arrives,
/// so this line is what starts the turn. stdin is closed straight after it.
pub fn claude_turn_stdin_line(prompt: &str) -> String {
    let line = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": prompt }] }
    })
    .to_string();
    format!("{line}\n")
}

/// Find the user's own `claude`.
///
/// Nothing is ever bundled: Claude support means driving the binary the user installed, under
/// the user's own login. An explicit path from settings wins verbatim so a launch failure can
/// be reported as a launch failure instead of as "missing".
pub fn resolve_claude_executable(configured_path: Option<&Path>) -> Option<PathBuf> {
    resolve_claude_executable_in(configured_path, env::var_os("PATH").as_deref(), home_dir())
}

/// The injectable form, so the search order can be tested without touching the real
/// environment.
pub fn resolve_claude_executable_in(
    configured_path: Option<&Path>,
    path: Option<&std::ffi::OsStr>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(configured) = configured_path {
        return is_path_executable(configured).then(|| configured.to_path_buf());
    }

    let on_path = path.and_then(|path| {
        env::split_paths(path)
            .map(|directory| directory.join(CLAUDE_EXECUTABLE_NAME))
            .find(|candidate| is_path_executable(candidate))
    });
    if on_path.is_some() {
        return on_path;
    }

    let mut fallbacks: Vec<PathBuf> = Vec::new();
    if let Some(home) = home {
        fallbacks.push(home.join(".local").join("bin").join(CLAUDE_EXECUTABLE_NAME));
    }
    fallbacks.push(Path::new("/usr/local/bin").join(CLAUDE_EXECUTABLE_NAME));
    fallbacks
        .into_iter()
        .find(|candidate| is_path_executable(candidate))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}
