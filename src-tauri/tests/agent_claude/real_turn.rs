//! One real turn against the user's own installed `claude`.
//!
//! This is the only test in the repo that spends money, so it is gated twice: `#[ignore]`
//! keeps it out of every normal run, and `VIDEO_CREATER_CLAUDE_REAL_TURN=1` keeps it out of a
//! `--include-ignored` sweep. It is deliberately not in `REQUIRED_NATIVE_LANES`.
//!
//! What it proves is narrow and specific: the real `project_action_schema()` has about 70
//! `anyOf` variants, and only a real run can show that the CLI and the API accept a schema
//! that large. It asserts nothing about whether the proposed edit is any good — that is the
//! model's judgment, not the contract.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use video_creater_lib::agent::claude_cli::{
    build_claude_turn_argv, claude_mcp_config_json, claude_turn_stdin_line,
    resolve_claude_executable, ClaudeSessionHandle, ClaudeTurnInvocation, CLAUDE_FALLBACK_MODEL,
};
use video_creater_lib::agent::schema::codex_conversation_proposal_output_schema;
use video_creater_lib::project::model::{MediaAsset, MediaKind, VideoProject};
use video_creater_lib::project::split::save_split_project;

/// The cap for this one turn.
///
/// The research measured comparable Haiku turns at $0.002–$0.016, but those used toy schemas.
/// The real proposal schema is about 20k input tokens on its own, and a turn that also reads
/// two MCP tools costs about $0.05 on Haiku — a $0.05 cap aborts it mid-flight with
/// `error_max_budget_usd`. Measured on 2026-09-18; see the fixture README.
const REAL_TURN_BUDGET_USD: f64 = 0.15;

#[test]
#[ignore = "spends money against the real Claude CLI; set VIDEO_CREATER_CLAUDE_REAL_TURN=1"]
fn real_claude_turn_returns_a_schema_shaped_proposal() {
    if std::env::var("VIDEO_CREATER_CLAUDE_REAL_TURN").as_deref() != Ok("1") {
        eprintln!("skipped: VIDEO_CREATER_CLAUDE_REAL_TURN is not 1");
        return;
    }

    let claude = resolve_claude_executable(None).expect("the claude CLI must be installed");
    let sidecar = mcp_sidecar_path();
    assert!(
        sidecar.is_file(),
        "build the sidecar first: cargo build --bin video-creater-mcp-server --features mcp-server"
    );

    let directory = tempfile::tempdir().expect("a temp dir");
    let project_dir = match std::env::var_os("VIDEO_CREATER_CLAUDE_REAL_TURN_PROJECT_DIR") {
        Some(configured) => PathBuf::from(configured),
        None => directory.path().join("Probe"),
    };
    fs::create_dir_all(&project_dir).expect("a project dir");
    save_split_project(&project_dir, &probe_project()).expect("a saved project");

    let invocation = ClaudeTurnInvocation {
        // The cheapest model the plan allows. The schema question is model-independent.
        model: CLAUDE_FALLBACK_MODEL.to_string(),
        fallback_model: None,
        developer_instructions:
            "You propose edits to a video project. Inspect the project with the read-only tools, \
             then return one proposal through the StructuredOutput tool. Never invent ids."
                .to_string(),
        output_schema: codex_conversation_proposal_output_schema(),
        session: ClaudeSessionHandle::New(uuid::Uuid::new_v4().to_string()),
        mcp_config: claude_mcp_config_json(&sidecar, &project_dir),
        budget_usd: REAL_TURN_BUDGET_USD,
    };

    let mut child = Command::new(&claude)
        .args(build_claude_turn_argv(&invocation))
        .current_dir(&project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("claude must start");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(
            claude_turn_stdin_line("Trim the first clip to its first two seconds.").as_bytes(),
        )
        .expect("the prompt must reach stdin");

    let output = child.wait_with_output().expect("claude must finish");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let evidence = retain_stream(&stdout, &stderr);
    eprintln!("raw stream retained at {}", evidence.display());

    let result = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|frame| frame.get("type").and_then(Value::as_str) == Some("result"))
        .unwrap_or_else(|| {
            panic!(
                "no result frame arrived. exit {:?}\nstderr:\n{stderr}",
                output.status.code()
            )
        });

    eprintln!(
        "cost_usd={:?} num_turns={:?} model_usage={:?} permission_denials={:?} \
         mcp_servers_from_init={:?}",
        result.get("total_cost_usd"),
        result.get("num_turns"),
        result.get("modelUsage").map(|usage| usage.to_string()),
        result.get("permission_denials"),
        init_mcp_servers(&stdout),
    );

    assert_eq!(
        result.get("is_error").and_then(Value::as_bool),
        Some(false),
        "the turn failed: {}",
        result
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or_default()
    );

    let structured = result
        .get("structured_output")
        .and_then(Value::as_object)
        .expect("the result frame must carry structured_output");
    assert!(
        structured
            .get("summary")
            .and_then(Value::as_str)
            .is_some_and(|summary| !summary.is_empty()),
        "structured_output must carry a non-empty summary"
    );
    assert!(
        structured
            .get("projectActions")
            .and_then(Value::as_array)
            .is_some(),
        "structured_output must carry a projectActions array"
    );
}

fn mcp_sidecar_path() -> PathBuf {
    if let Some(configured) = std::env::var_os("VIDEO_CREATER_CLAUDE_REAL_TURN_MCP_SERVER") {
        return PathBuf::from(configured);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("video-creater-mcp-server")
}

fn retain_stream(stdout: &str, stderr: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("output")
        .join("claude-agent-backend");
    fs::create_dir_all(&directory).expect("an output dir");
    let path = directory.join("real-turn-2026-09-18.jsonl");
    fs::write(&path, stdout).expect("the raw stream must be retained");
    if !stderr.trim().is_empty() {
        fs::write(directory.join("real-turn-2026-09-18.stderr.txt"), stderr)
            .expect("the stderr must be retained");
    }
    path
}

fn init_mcp_servers(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|frame| frame.get("subtype").and_then(Value::as_str) == Some("init"))
        .and_then(|frame| frame.get("mcp_servers").map(Value::to_string))
}

fn probe_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "probe-project".to_string(),
        "Claude Schema Probe".to_string(),
        "2026-09-18T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: Some("input.mp4".to_string()),
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    project
}
