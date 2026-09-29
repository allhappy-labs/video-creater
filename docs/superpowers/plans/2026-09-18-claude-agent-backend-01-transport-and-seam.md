# Claude Agent Backend 01 — Transport and Backend Seam Implementation Plan

> **Amended 2026-09-18** — after this plan shipped, the user asked for Claude to be the
> preferred backend rather than the fallback, on their own subscription and with no API key
> required. The wording below is kept as it was executed; where it no longer matches the code,
> the reason is recorded in the design's
> [Amendments](../specs/2026-09-18-claude-agent-backend-design.md#amendments) section.
> In short: Automatic now prefers Claude; readiness means "a turn would authenticate", not
> "the binary exists"; the `agent.claude` summaries name the subscription and never the
> account's email; and `ANTHROPIC_API_KEY` is withheld from a subscription turn instead of
> being left to the CLI.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Each task lists the files it owns, its
> exact test commands and its commit message.

**Goal:** Make the conversation turn backend-pluggable and add a working Claude backend, without
changing any behaviour the AI tab already has.

- A new `AgentTurnTransport` trait in `src-tauri/src/agent/` is the only seam. Codex keeps
  working through an adapter over today's app-server code.
- A Claude transport runs the user's own installed `claude` once per turn in print mode, passes
  the existing proposal schema through `--json-schema`, and reads the proposal back from
  `result.structured_output`.
- Claude sees the app's own MCP sidecar with a read-only tool allowlist and no built-in tools.
- Cancel, deadlines, sessions and the "no echoed hidden context in stored history" rule work for
  both backends.
- `conversation.rs` and its `requests`/`context`/`risk`/`impact`/`apply`/`undo` submodules are
  untouched in behaviour.

**Spec:** `docs/superpowers/specs/2026-09-18-claude-agent-backend-design.md`, decisions 1–10 and
15–18.
**Research of record:** `docs/research/2026-09-18-claude-agent-backend.md`. Every flag, frame
shape and cost figure below comes from there; do not re-derive them by spending tokens.
**Backlog row:** VC-029 (added by plan 02).
**Depends on:** nothing. **Feeds:** plan 02, which needs `AgentBackendKind` and
`probe_claude_cli`.

**Tech Stack:** Rust (Tauri 2 backend, serde, `serde_json`, `std::process`, `libc` for signals),
React 19 + Zustand frontend (untouched by this plan), Vitest, Playwright (untouched).

## Global Constraints

- **Commands.**
  - Prefix every shell command with `rtk`. `rg` isn't installed, so use `rtk grep -rn`.
  - Run long commands in the foreground and wait for them to finish.
  - Tool calls time out at 10 minutes, so run large Rust suites one `--test` target at a time.
- **Commits.**
  - Use Conventional Commits. End every commit body with the trailer
    `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  - Stage only the files named in the task (`rtk git add <paths>`). Never use `git add -A` or
    `git add .`.
  - Never push.
- **File size.** Every new source file stays under 600 lines, tests included. `app_server.rs`
  (5256 lines) and `tools.rs` (21068 lines) are already far over; they get only minimal wiring,
  and this plan **reduces** `app_server.rs` by moving the schema builders out.
- **Cargo environment.** Replace the placeholder literally; shell state does not persist between
  calls.
  - `{CARGO_ENV}` = `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
- **No token spend outside task 5.** Every other task is proved by fixtures and a fake transport.
  Task 5 is the only task allowed to run a real Claude turn, on `--model haiku`, with
  `--max-budget-usd 0.05`, one turn. If it must be retried, say so and report the added cost.
- **Licence policy.** Nothing is bundled by this plan. Do not add `@anthropic-ai/*` to
  `package.json`, do not add an `externalBin` entry, do not add a provenance file. LGPL/MIT/BSD
  only for anything new; nothing GPL.
- **Evidence.** Never claim evidence you didn't observe. A skipped, filtered-out or
  environment-blocked test is reported as not run, with the reason.
- **Behaviour boundaries.**
  - The four Tauri command names and every exported TS type keep their current names.
  - Risk classification, proposal validation, deterministic `codex-action-N-<sha>` ids, atomic
    apply and snapshot undo are not modified. If a task thinks it needs to, stop and re-read the
    spec.
  - `VideoProject::codex_thread_id` keeps its name and its Codex-only meaning.
  - No user-visible string added by this plan names a backend.

## File Map

### New — `src-tauri/src/agent/`
- `mod.rs`: `AgentBackendKind`, `AgentBackendSelection`, `resolve_agent_backend`,
  `AgentTurnRequest`, `AgentTurnOutcome`, `AgentTurnError`, the `AgentTurnTransport` trait, the
  `mod` declarations, and the module's own tests.
- `schema.rs`: the proposal output schema, moved verbatim from `codex/app_server.rs`.
- `prompt.rs`: `render_agent_turn_prompt`, moved from `codex/app_server.rs`.
- `claude_cli.rs`: `ClaudeCliInvocation`, `build_claude_turn_argv`, `claude_mcp_config_json`,
  `CLAUDE_READ_ONLY_TOOLS`, `resolve_claude_executable`.
- `claude_frames.rs`: `ClaudeStreamFrame`, `parse_claude_stream_frame`,
  `ClaudeTurnSummary`, `summarize_claude_stream`.
- `claude_transport.rs`: `ClaudeTurnTransport` and its `impl AgentTurnTransport`.
- `codex_transport.rs`: `CodexTurnTransport` and its `impl AgentTurnTransport`.
- `fake_transport.rs`: `#[cfg(test)] FakeAgentTurnTransport`.

### New — tests and fixtures
- `src-tauri/tests/agent_claude.rs` plus `src-tauri/tests/agent_claude/` (`argv.rs`,
  `frames.rs`, `sessions.rs`, `real_turn.rs`).
- `src-tauri/tests/fixtures/claude_agent/`: `success-with-mcp-tool.jsonl`,
  `success-no-proposal.jsonl`, `auth-failed.jsonl`, `rate-limited.jsonl`,
  `cancelled-no-result.jsonl`, `real-turn-<date>.jsonl`.

### Modified
- `src-tauri/src/codex/app_server.rs`: loses the schema builders and the prompt renderer, gains
  `pub use crate::agent::schema::*;` style re-exports so existing call sites compile unchanged.
- `src-tauri/src/main.rs`: `mod agent;`, and the backend selection inside
  `start_codex_conversation_edit_for_project`.
- `src-tauri/src/project/split.rs`: two optional fields on `SplitAgentSession`.
- `src/lib/project.ts`: the two matching optional fields on `ProjectAgentSession`.

---

## Task 1 — Move the proposal output schema into `agent/schema.rs`

**Owns:** `src-tauri/src/agent/mod.rs` (new, minimal), `src-tauri/src/agent/schema.rs` (new),
`src-tauri/src/codex/app_server.rs` (modify), `src-tauri/src/main.rs` (modify, `mod agent;`).

A pure move. The schema is already provider-neutral JSON Schema; the Claude transport needs it
without depending on the Codex transport.

- [ ] Record the baseline: `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --list | rtk tail -3`. Save the count.
- [ ] Create `src-tauri/src/agent/mod.rs` with only `pub mod schema;` and add `mod agent;` to `main.rs`.
- [ ] Move, byte-for-byte, from `app_server.rs` to `agent/schema.rs`: `project_action_schema`, every `*_action_schema` helper it calls, `codex_conversation_proposal_output_schema`, and `codex_edit_proposal_output_schema`. Keep the names; add `pub` where the move requires it.
- [ ] In `app_server.rs`, replace them with `pub use crate::agent::schema::{codex_conversation_proposal_output_schema, codex_edit_proposal_output_schema, project_action_schema};` so no call site changes.
- [ ] `{CARGO_ENV} rtk cargo build --manifest-path src-tauri/Cargo.toml` must pass with no call-site edits outside these files. If a private helper is needed elsewhere, re-export it too rather than editing callers.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1`: the same count as the baseline, all passing.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib codex:: -- --test-threads=1`.
- [ ] `rtk wc -l src-tauri/src/agent/schema.rs src-tauri/src/codex/app_server.rs`. If `schema.rs` exceeds 600 lines, split it into `agent/schema.rs` (the two proposal schemas) plus `agent/schema/actions.rs` (the per-action builders) in this same task.

**Commit:** `refactor(agent): move the proposal output schema out of the Codex transport`

---

## Task 2 — Add the `AgentTurnTransport` seam and a fake transport

**Owns:** `src-tauri/src/agent/mod.rs`, `src-tauri/src/agent/fake_transport.rs`.

TDD: the tests here are about backend resolution, which is pure logic.

- [ ] Write the types. Keep them owned and backend-neutral:

  ```rust
  pub enum AgentBackendKind { Codex, Claude }

  pub struct AgentTurnRequest {
      pub cwd: PathBuf,
      pub developer_instructions: String,
      pub prompt: String,
      pub output_schema: serde_json::Value,
      pub session: Option<String>,
      pub project_dir: Option<PathBuf>,
      pub cancellation_key: String,
  }

  pub struct AgentTurnOutcome {
      pub session: String,
      pub proposal: Option<serde_json::Value>,
      pub model: Option<String>,
      pub notice: Option<String>,
      pub transcript: serde_json::Value,
  }

  pub enum AgentTurnError { Unavailable(String), NotAuthenticated(String), UsageLimit { detail: String }, Interrupted, Deadline, TurnFailed(String), Io(std::io::Error), Json(serde_json::Error) }

  pub trait AgentTurnTransport {
      fn backend(&self) -> AgentBackendKind;
      fn run_conversation_turn(&mut self, request: &AgentTurnRequest, deadline: Instant, cancellation: Option<&dyn CancellationSignal>) -> Result<AgentTurnOutcome, AgentTurnError>;
  }
  ```

  `transcript` is what the app persists in `app-server-conversations.json`; it must already be
  scrubbed of echoed hidden context by the transport that produced it.
- [ ] Write `resolve_agent_backend(preference: Option<AgentBackendKind>, codex_ready: bool, claude_ready: bool) -> Result<AgentBackendSelection, AgentTurnError>` with an `AgentBackendSelection { kind, fell_back_from: Option<AgentBackendKind> }`.
- [ ] Write the failing tests first, in `agent/mod.rs`'s `mod tests`, one per row:

  | preference | codex_ready | claude_ready | expected |
  | --- | --- | --- | --- |
  | `None` (Auto) | true | true | Codex, no fallback |
  | `None` | false | true | Claude, no fallback |
  | `None` | false | false | `Unavailable` |
  | `Codex` | true | true | Codex |
  | `Codex` | false | true | Claude, `fell_back_from: Codex` |
  | `Claude` | true | true | Claude |
  | `Claude` | true | false | Codex, `fell_back_from: Claude` |
  | `Claude` | false | false | `Unavailable` |

- [ ] Implement until green.
- [ ] Add `fake_transport.rs`: `FakeAgentTurnTransport { outcomes: VecDeque<Result<AgentTurnOutcome, AgentTurnError>>, pub seen: Vec<AgentTurnRequest> }` behind `#[cfg(test)]`, recording every request and popping a scripted result.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib agent:: -- --test-threads=1`: 8 or more tests pass.

**Commit:** `feat(agent): add an AgentTurnTransport seam with backend resolution`

**Parallel with:** task 4. Both only add new files.

---

## Task 3 — Lift the turn prompt renderer into `agent/prompt.rs`

**Owns:** `src-tauri/src/agent/prompt.rs` (new), `src-tauri/src/codex/app_server.rs` (modify).

- [ ] Move `build_codex_conversation_prompt` (and the video-edit prompt builder, if it is in the same shape) into `agent/prompt.rs` as `render_agent_turn_prompt(context: &CodexConversationContext) -> String`, byte-for-byte, keeping the old name as a re-export in `app_server.rs`.
- [ ] Move `without_conversation_user_input` into `agent/prompt.rs` as `pub fn scrub_echoed_user_input(value: Value) -> Value`, with the same doc comment explaining *why* (the echoed text is the hidden adaptive context, and it must never reach stored history). Re-export the old name.
- [ ] Any test that covered either function moves with it.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1` and `--test codex_conversation -- --test-threads=1`: unchanged counts.

**Commit:** `refactor(agent): share the turn prompt renderer and the user-input scrub`

---

## Task 4 — Build the Claude invocation: argv, MCP config, executable resolution

**Owns:** `src-tauri/src/agent/claude_cli.rs` (new), `src-tauri/tests/agent_claude.rs` (new),
`src-tauri/tests/agent_claude/argv.rs` (new), `src-tauri/Cargo.toml` (modify, one `[[test]]`
entry only if the default discovery is not used).

The argv is the contract with the CLI. It is fixed except for the marked parts (spec decision 6).

- [ ] Write the failing tests first, in `tests/agent_claude/argv.rs`:
  - a first turn (no stored session) contains, in order: `--print`, `--output-format stream-json`, `--verbose`, `--input-format stream-json`, `--model sonnet`, `--system-prompt <instructions>`, `--json-schema <compact JSON>`, `--session-id <the passed UUID>`, `--tools ""`, `--allowed-tools`, `--mcp-config`, `--strict-mcp-config`, `--permission-mode dontAsk`, `--permission-prompts none`, `--setting-sources ""`, `--safe-mode`, `--disable-slash-commands`, `--max-budget-usd`;
  - a resumed turn has `--resume <stored id>` and **no** `--session-id` and **no** `--system-prompt` (the recorded snapshot wins — see research §2.3);
  - `--bare` never appears (it refuses OAuth);
  - `--dangerously-skip-permissions` and `--allow-dangerously-skip-permissions` never appear;
  - `--json-schema`'s value round-trips through `serde_json::from_str` to the schema that went in, and is a single line;
  - every entry of `--allowed-tools` matches `mcp__video-creater__<name>` and `<name>` is in `CLAUDE_READ_ONLY_TOOLS`;
  - `CLAUDE_READ_ONLY_TOOLS` contains exactly the 19 names in spec decision 9, and a test asserts each one resolves through `codex::tools::resolve_codex_tool_name` **and** that `call_codex_local_tool` reports `mutates_project == false` for each against a fixture project. That last assertion is the real guard: it fails if someone later makes a listed tool mutating.
  - `claude_mcp_config_json(executable, project_dir)` produces `{"mcpServers":{"video-creater":{"type":"stdio","command":<exe>,"args":["--project-dir",<dir>]}}}` and the `project_dir` is absolute.
  - `resolve_claude_executable(None)` searches `PATH`, then `~/.local/bin/claude`, then `/usr/local/bin/claude`, and an explicit override path is used verbatim when it is an executable file. Model it on `codex::app_server::resolve_codex_executable` and reuse `is_path_executable`.
- [ ] Implement `claude_cli.rs` until green. Constants live here: `CLAUDE_TURN_TIMEOUT: Duration = Duration::from_secs(300)`, `CLAUDE_TURN_BUDGET_USD: f64 = 0.50`, `CLAUDE_DEFAULT_MODEL: &str = "sonnet"`, `CLAUDE_FALLBACK_MODEL: &str = "haiku"`.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude argv -- --test-threads=1`.
- [ ] `rtk wc -l src-tauri/src/agent/claude_cli.rs`.

**Commit:** `feat(agent): build the Claude print-mode invocation and its read-only tool surface`

**Parallel with:** task 2.

---

## Task 5 — Real-CLI schema probe, and capture the frame fixtures

**Owns:** `src-tauri/tests/agent_claude/real_turn.rs` (new),
`src-tauri/tests/fixtures/claude_agent/*.jsonl` (new),
`output/claude-agent-backend/` (evidence, not committed as source).

This is the plan's only paid task, and it de-risks the biggest unknown: the real
`project_action_schema()` has about 70 `anyOf` variants, and only a real run proves the CLI and
the API accept it. **One turn.** `--model haiku`, `--max-budget-usd 0.05`. Expected cost, from
the research note's measurements: about $0.005–0.02.

- [ ] Write the test `real_turn.rs::real_claude_turn_returns_a_schema_shaped_proposal`, marked `#[ignore]` **and** gated on `VIDEO_CREATER_CLAUDE_REAL_TURN=1`, so it can never run in a normal suite or in `verify:native:release`. Do **not** add it to `REQUIRED_NATIVE_LANES` in `scripts/run-native-rust-tests.mjs`.
- [ ] The test: materialize the smallest existing project fixture; build the argv with task 4's builder against the real `claude` and the real `codex_conversation_proposal_output_schema()`; send one short prompt ("Trim the first clip to its first two seconds."); write the raw stdout to `output/claude-agent-backend/real-turn-<date>.jsonl`; assert only that a `result` frame arrived with `is_error == false` and a `structured_output` object carrying a `summary` string and a `projectActions` array. Do not assert the edit is correct — that is the model's judgment, not the contract.
- [ ] Run it: `VIDEO_CREATER_CLAUDE_REAL_TURN=1 {CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude real_claude_turn -- --ignored --test-threads=1 --nocapture`.
- [ ] Record in the run notes: the observed `total_cost_usd`, `num_turns`, the model in `modelUsage`, and whether `permission_denials` was empty.
- [ ] **If the schema is rejected** (an `is_error` result mentioning the schema, or a startup validation error): stop, record the exact message, and implement the documented fallback instead — a `focused_project_action_schema(focus, project)` in `agent/schema.rs` that filters the same per-action builders down to the actions reachable from the turn's focus, then re-run once. Never hand-write a second schema. Report which path was taken.
- [ ] Copy the retained stream into `src-tauri/tests/fixtures/claude_agent/real-turn-<date>.jsonl`, then derive the other five fixtures from it by hand-editing frames (auth failure, rate limit, no `structured_output`, truncated-before-`result`), using the frame shapes quoted in research §2.1, §2.4 and §2.7. Keep every fixture under 200 lines; trim `thinking` signatures.
- [ ] Add a one-line `src-tauri/tests/fixtures/claude_agent/README.md` saying where the real fixture came from, its date, its cost, and that the others are hand-derived.

**Commit:** `test(agent): prove the real Claude CLI accepts the proposal schema`

**Blocks:** task 6.

---

## Task 6 — Parse the Claude stream-json frames

**Owns:** `src-tauri/src/agent/claude_frames.rs` (new),
`src-tauri/tests/agent_claude/frames.rs` (new).

TDD against the task-5 fixtures. No process is spawned here; the parser takes an iterator of
lines, so it is trivially testable.

- [ ] Write the failing tests, one per fixture:
  - `success-with-mcp-tool.jsonl` → `ClaudeTurnSummary { session_id, model, proposal: Some(_), mcp_servers: [("video-creater", "connected")], tool_calls: ["mcp__video-creater__…", "StructuredOutput"], permission_denials: [], cost_usd: Some(_), notice: None }`;
  - `success-no-proposal.jsonl` → `proposal: None`, no error;
  - `auth-failed.jsonl` → `Err(AgentTurnError::NotAuthenticated(_))`, and the message is the CLI's own `result` text so settings can echo it;
  - `rate-limited.jsonl` → `Err(AgentTurnError::UsageLimit { detail })` with the reset time in `detail` when `rate_limit_event` carried one;
  - `cancelled-no-result.jsonl` → `Err(AgentTurnError::Interrupted)`, because a stream that ends without a `result` frame is the verified cancel signal;
  - an unparseable line is skipped, not fatal (the CLI prints plain warnings such as `Warning: no stdin data received in 3s…` to the same stream);
  - `structured_output` is preferred over `result`, and when `structured_output` is absent the parser falls back to `conversation_proposal_from_text(result)`.
- [ ] Implement `parse_claude_stream_frame` and `summarize_claude_stream`. Branch classification on the `result` frame's `is_error` / `terminal_reason` / the assistant frame's `error` field — **never on the exit code alone** (research §2.7 verified exit 1 alongside a well-formed error result).
- [ ] Add a test that the summary's retained transcript contains **no** `user` frame whose text equals the sent prompt: Claude echoes the hidden context in a synthetic `user` frame exactly as Codex echoes it in `userMessage`, so `scrub_echoed_user_input` from task 3 must be applied before the transcript is returned.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude frames -- --test-threads=1`.

**Commit:** `feat(agent): parse the Claude stream-json turn frames`

---

## Task 7 — The Claude transport: spawn, stdin, deadline, cancel

**Owns:** `src-tauri/src/agent/claude_transport.rs` (new), `src-tauri/src/agent/mod.rs` (modify).

- [ ] Write the failing tests first, using a **stub script** rather than the real CLI, so no tokens are spent. A small shell or Rust-built stub that replays a fixture to stdout and optionally sleeps proves every branch:
  - a replayed success fixture yields an `AgentTurnOutcome` with the right session and proposal;
  - a stub that sleeps past the deadline yields `AgentTurnError::Deadline` and the child is gone afterwards (assert the pid is reaped);
  - a stub that sleeps while the cancellation signal flips yields `AgentTurnError::Interrupted` within the grace period, and the child is gone;
  - a stub that exits 127 (missing binary) yields `AgentTurnError::Unavailable` with a plain message;
  - the prompt reaches stdin as exactly one `{"type":"user","message":{"role":"user","content":[{"type":"text","text":…}]}}` line, and stdin is then closed.
- [ ] Implement. Cancel is SIGTERM, then SIGKILL after a 500 ms grace, mirroring `interrupt_and_terminate`'s shape. Reuse `CancellationSignal`; do not introduce a second cancellation mechanism. Cap stderr at 64 KiB as the Codex probe does, and include the tail in `Unavailable`/`TurnFailed` messages.
- [ ] Set the child's cwd to the project folder and inherit the environment unchanged (the OAuth credential is a file under `~/.claude`; nothing needs forwarding). Do not set or read `ANTHROPIC_API_KEY` — if the user has one exported, the CLI picks it up itself. **Amended 2026-09-18: leaving it to the CLI was wrong, because the CLI prefers the key over the subscription login. A `claude.ai` session now removes `ANTHROPIC_API_KEY` from the child's environment; a key-backed session still passes it through. The value is never read or set. See the design's Amendments.**
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib agent::claude_transport -- --test-threads=1`.

**Commit:** `feat(agent): run a Claude conversation turn as a per-turn CLI process`

---

## Task 8 — The Codex adapter, and route the turn through the trait

**Owns:** `src-tauri/src/agent/codex_transport.rs` (new), `src-tauri/src/codex/app_server.rs`
(modify, wiring only), `src-tauri/src/main.rs` (modify).

The behaviour-preserving refactor. The existing Codex tests are the specification; none of them
may change.

- [ ] Record the baseline counts for `--test codex_app_server`, `--test codex_conversation`, `--lib codex::`.
- [ ] Implement `CodexTurnTransport` whose `run_conversation_turn` performs today's sequence — `initialize`, `thread/start` or `thread/resume`, `turn/start`, `choose_codex_turn_model`, `run_codex_turn_with_supported_model`, `interrupt_and_terminate` on cancel — and maps its result into `AgentTurnOutcome` (`session` = the thread id, `notice` = the model notice, `transcript` = the scrubbed thread and turn responses).
- [ ] Map `CodexAppServerError` onto `AgentTurnError`: `MissingExecutable` → `Unavailable`, `Interrupted` → `Interrupted`, `Deadline` → `Deadline`, `TurnFailed` → `TurnFailed`, the rest → `TurnFailed` with the stable message.
- [ ] In `main.rs`, change `start_codex_conversation_edit_for_project` so the transport is chosen by `resolve_agent_backend` and then used through the trait. The command's name, signature, return type, lease handling, revision snapshot, `request.validate`, cancellation registration and `persist_codex_turn_for_project` call all stay exactly as they are.
- [ ] Every one of the baseline suites passes with the same counts:
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib codex:: -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater codex -- --test-threads=1`
- [ ] `rtk wc -l src-tauri/src/codex/app_server.rs`: must be lower than before task 1.

**Commit:** `refactor(agent): route the Codex conversation turn through the shared transport`

---

## Task 9 — Map chats onto Claude sessions

**Owns:** `src-tauri/src/project/split.rs` (modify, two fields plus the resolver),
`src-tauri/tests/agent_claude/sessions.rs` (new), `src/lib/project.ts` (modify, two fields).

Rust/TS lockstep: both sides land in this one commit.

- [ ] Write the failing tests first:
  - a new chat created for the Claude backend stores `provider: Some("claude")` and a
    `provider_session_id` that parses as a UUID;
  - a second turn in that chat reuses the same `provider_session_id`, and task 4's argv builder
    then emits `--resume <that id>`;
  - a chat created for Codex keeps writing `thread_id` and leaves the new fields `None`, so
    existing projects are unaffected;
  - switching the backend inside one chat mints a new `provider_session_id`, keeps the old
    `thread_id`, and records the new `provider`;
  - `load_agent_session_manifest` still reads a manifest written before this change (no
    `provider` keys) without error — the fields are `#[serde(default)]`.
- [ ] Add `provider: Option<String>` and `provider_session_id: Option<String>` to `SplitAgentSession`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`. Do **not** bump the manifest schema version; the fields are additive and defaulted.
- [ ] Extend `record_app_server_conversation_turn`'s session resolution: match on `provider_session_id` first when the turn carries one, then fall back to today's `thread_id` match, then `active_session_id`, then create.
- [ ] Mirror the fields on `ProjectAgentSession` in `src/lib/project.ts` as optional, and nothing else.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split agent_session -- --test-threads=1`
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude sessions -- --test-threads=1`
- [ ] `rtk pnpm test` (Vitest) and `rtk pnpm lint`.

**Commit:** `feat(agent): map chats onto per-backend agent sessions`

---

## Task 10 — End-to-end turn through the fake transport

**Owns:** `src-tauri/tests/agent_claude/mod.rs` additions (a new `turn.rs`),
`src-tauri/src/main.rs` (modify only if the command needs a test seam).

This is the task that proves the seam did not weaken anything.

- [ ] Write the failing tests, all driven by `FakeAgentTurnTransport`:
  - a scripted Claude-shaped outcome produces the same `CodexConversationEditCommandResult` a Codex-shaped outcome does: `prepared_proposal` present, the same `action_ids` for the same actions, `risk.level == "safe"` for a clip-property edit and `"review"` for a delete;
  - the `AgentTurnRequest` the fake saw carries the rendered hidden context and the real schema, and its `prompt` is **not** the user's raw words alone;
  - `AgentTurnError::Interrupted` leaves `proposal` and `prepared_proposal` `None` and writes no history entry;
  - `AgentTurnError::NotAuthenticated` surfaces as the "agent unavailable" shape the frontend already classifies;
  - the persisted `app-server-conversations.json` entry contains no substring of the hidden context;
  - `apply_codex_conversation_proposal` on the result still refuses a mutated `action_ids` list (`StaleProposal`) and an unapproved `Review` bundle (`ReviewRequired`).
- [ ] Implement whatever narrow test seam is needed — prefer a `pub(crate)` inner function taking `&mut dyn AgentTurnTransport` over any change to the command's public shape.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude -- --test-threads=1`
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`

**Commit:** `test(agent): cover a backend-agnostic conversation turn end to end`

---

## Parallelization Notes

- Tasks 2 and 4 are independent (new files only) and can run at the same time.
- Task 3 touches `app_server.rs`, as task 1 does. Run them in order, not together.
- Task 5 must precede task 6 (it produces the fixtures) and should be scheduled early, because a
  schema rejection changes task 6's and task 7's inputs.
- Task 8 touches `app_server.rs` and `main.rs`; it must not overlap with tasks 1 or 3.
- Task 9 is independent of tasks 4–7 and can run beside them.
- Task 10 is last.
- Plan 02 may start its Rust preference task (its task 1) as soon as task 2 has landed.

## Acceptance

This plan is done when all of the following were observed and reported:

1. `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib agent:: -- --test-threads=1` passes, with the count.
2. `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude -- --test-threads=1` passes, with the count.
3. `--test codex_app_server`, `--test codex_conversation`, `--test project_split` and `--lib codex::` pass with counts greater than or equal to their pre-task-1 baselines, and no existing test was edited to make it pass.
4. The one real Claude turn ran, its raw stream is retained under `output/claude-agent-backend/`, and its observed cost, model and `permission_denials` are reported. If the real schema had to be narrowed, the rejection message and the narrowing are recorded.
5. `rtk pnpm test` and `rtk pnpm lint` pass.
6. `rtk grep -rn "anthropic" package.json src-tauri/tauri.conf.json src-tauri/tauri.linux.conf.json` returns nothing: nothing proprietary was bundled.
7. `rtk grep -rn "Claude" src/editor` returns nothing: no backend name reached an editor string in this plan.
8. `rtk wc -l` on every new file is under 600, and `app_server.rs` is smaller than it was.
9. Anything not run — macOS paths, a packaged run — is reported as not run, with the reason. (The `ANTHROPIC_API_KEY` auth mode was closed on 2026-09-18 with stub-executable tests for all four auth states.)
