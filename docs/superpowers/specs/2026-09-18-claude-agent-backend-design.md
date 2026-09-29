# Claude Agent Backend — Design

Date: 2026-09-18. Status: approved for autonomous execution (the user asked for Claude support
as a must-have alongside Codex, and asked to work without check-ins).

Research of record: [`docs/research/2026-09-18-claude-agent-backend.md`](../../research/2026-09-18-claude-agent-backend.md).
Every CLI flag, frame shape and licence quote used below was verified there against the
installed `claude` 2.1.270 and Anthropic's own documentation.

Prior art this builds on:
- [The Codex rail proposal workspace design](2026-07-25-codex-rail-proposal-workspace-design.md) (VC-001) — the conversation contract.
- [The editor UI/UX redesign design](2026-09-13-editor-ui-ux-redesign-design.md) — the AI tab and *Backend Additions*.
- [`docs/superpowers/plans/2026-09-13-editor-redesign-06-ai-tab-conversation-backend.md`](../plans/2026-09-13-editor-redesign-06-ai-tab-conversation-backend.md) — the shipped AI tab.

## Amendments

The decisions below are kept as they were made. Where later work changed one, the change is
recorded here with its date rather than by editing the original.

### 2026-09-18 — Claude becomes the preferred backend, not the fallback

The user asked why the product should wait on the Codex account at all when they have their own
Claude subscription: *"Why do we need to wait for the Codex asset if we already have Claude? …
But make sure that we do it in a way that users can use their own Claude subscription and do not
require an API key."* Four things follow, and all four are implemented.

1. **Automatic prefers Claude.** Decision 4 said Auto resolves "Codex first, for continuity with
   existing projects". It now resolves **Claude first**, and takes Codex when Claude is not
   ready. Continuity is preserved where it actually matters — `codex_thread_id` still keeps its
   Codex-only meaning and an existing Codex chat still resumes its thread when the turn routes to
   Codex — but a new turn goes to the backend that runs on the user's own subscription, needs
   nothing bundled, and has no shared account to exhaust. An explicit preference is still
   honoured, and still falls back to the other agent rather than refusing the turn.
   (`resolve_agent_backend`, and `resolveAgentBackend` in `agent-backend-settings.tsx`, which
   mirrors it.)

2. **Readiness means "a turn would authenticate", not "the binary exists".** Because Automatic
   now prefers Claude, an installed-but-signed-out `claude` must yield to Codex instead of
   failing in the child. `plan_agent_turn` therefore takes a `ClaudeTurnReadiness` from the same
   two zero-token probes the settings row uses, rather than deriving readiness from
   `resolve_claude_executable` alone. The probes are local (`claude --version` ~55 ms, `claude
   auth status` ~230 ms on this host) against a turn measured in seconds.

3. **Subscription auth is explicit and an API key is optional at most.** Decision 11's readiness
   table is superseded:

   | Condition | State | Diagnostic | Summary |
   | --- | --- | --- | --- |
   | `loggedIn`, `authMethod: "claude.ai"` | `Ready` | `agent.claude.ready` | "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed." |
   | `loggedIn`, any other `authMethod` | `Ready` | `agent.claude.readyOtherAuth` | "Claude 2.1.270 is signed in, but not with a Claude subscription." |
   | not signed in, `ANTHROPIC_API_KEY` set | `Ready` | `agent.claude.apiKey` | "Claude 2.1.270 isn't signed in, so turns will use the Anthropic API key from the environment. Sign in with your subscription to use it instead." |
   | not signed in, no key | `ActionRequired` | `agent.claude.notLoggedIn` | "Claude is installed but not signed in. Sign in to Claude with your subscription: run `claude` in a terminal and use /login." |
   | binary not found | `NotConfigured` | `agent.claude.missing` | "Claude isn't installed. Install it from claude.com, then sign in with your subscription." |
   | binary found but fails to run | `Failed` | `agent.claude.launchFailed` | unchanged, plus the diagnostic detail |

   The old `Ready` summary named the account's email. It no longer does, and `ClaudeAuthStatus`
   no longer deserializes `email`, `orgId` or `orgName` at all — not deserializing them is the
   simplest guarantee that none of them can reach a summary, a provenance map or a log. Only
   `loggedIn`, `authMethod` and `subscriptionType` are read; `authMethod` and `subscriptionType`
   stay in provenance, as before.

4. **`ANTHROPIC_API_KEY` is withheld from a subscription turn.** The CLI prefers the environment
   key over the OAuth login, so an app that merely inherited its environment could bill a user's
   API account for a turn they expected their plan to cover, and could keep working after the key
   expired while the readiness row still said "subscription". The safest behaviour that does not
   break anyone is **conditional**: a `claude.ai` session removes `ANTHROPIC_API_KEY` from the
   child's environment (`SupervisedCommand::env_remove`), so the login the user chose is the one
   that pays; a session whose only credential *is* a key — nothing signed in, or a
   non-subscription login that may itself be key-backed — passes the environment through
   untouched, because removing the variable would be removing the credential. Nothing in the
   Claude path ever *sets* `ANTHROPIC_API_KEY` or requires it, and nothing reads its value —
   the probe only asks whether a non-empty one exists. This closes the
   "not proven, stated as such" item about the API-key auth path with stub-executable tests for
   all four states rather than with a real key.

5. **Codex is supported but not required.** Consequences recorded under
   [Codex is no longer required](#codex-is-no-longer-required).

### Codex is no longer required

Codex keeps working, keeps its sidecar, its pin, its provenance and its readiness row, and an
explicit `codex` preference still routes every turn to it. What changed is that nothing about the
product waits on it:

- `agent_category_health` already treated the two backends as alternatives; that is unchanged.
- A missing bundled sidecar is now `NotConfigured` — the same state an absent `claude` reports —
  with the summary "…It is optional: turns can run on Claude instead." and the recovery action
  "Use Claude instead, or reinstall Video Creater to restore the bundled Codex runtime." It used
  to be `ActionRequired` with "Update or reinstall Video Creater", which told a user with a
  working Claude install to repair an app that was not broken.
- The Linux smoke gate's agent self-test step demanded `agent.codex`. It now runs all four rows,
  still records every diagnosis, and requires the two support rows (`agent.mcpServer`,
  `agent.proposalValidator`) plus **at least one** ready backend (`agentSelfTestVerdict`).
- `--agent-flows` defaults to `--agent-backend claude`.
- Codex-backed AI-flow evidence is no longer a blocker in VC-001 or the hand-off: Claude covered
  flows 1 and 2 on a packaged Linux app on 2026-09-18, and the Codex pin bump is optional.

Still Codex-only, deliberately and as before (decision: *Claude for the non-conversation paths*
is out of scope): the legacy `start_codex_video_edit_for_project` edit-job path and the Temporal
edit-proposal activity. Those are not on the AI tab's path, so a Claude-only install can use the
conversation flows fully.

## Goal

A user with Claude installed can do everything in the AI tab that a user with Codex can do, and
neither user can tell which backend produced a result except by looking in settings. Concretely:

- The AI tab's turn, apply, undo, cancel, session and history behaviour is byte-identical across
  backends, because only the *transport* differs.
- Risk classification, proposal validation, deterministic action ids, atomic apply and snapshot
  undo stay in Rust and stay backend-independent. A backend produces untrusted JSON; nothing
  more.
- The backend is a user setting that defaults to whichever agent is actually available.
- Nothing proprietary is bundled. Claude support means "drive the `claude` the user installed".

Done means: a fake transport and fixtures cover the behaviour, one real Haiku turn is retained
as evidence that the real CLI honours the real proposal schema, and agent settings tell a user
with neither agent installed exactly what to do.

## Decisions (made autonomously, recorded here for review)

1. **Drive the CLI, not the SDK, not the API.** Anthropic's Agent SDK overview says, verbatim:
   *"To drive the same agent loop from another language, run the CLI as a subprocess with the
   `-p` flag and `--output-format json`."* The backend is Rust. The SDK is a TypeScript/Python
   wrapper that spawns the same binary and exposes the same options as the flags; adding it
   would mean adding a Node runtime this repo does not ship, for no capability gain.
   Calling the Messages API directly is also rejected: it would mean re-implementing the agent
   loop, and it would force API-key-only auth on users who have a subscription.

2. **Never bundle Claude. Require the user's own installed, unmodified binary.**
   `@anthropic-ai/claude-agent-sdk` and `@anthropic-ai/claude-code` are
   `SEE LICENSE IN README.md` → *"© Anthropic PBC. All rights reserved."* That is neither
   LGPL, MIT nor BSD, so the repo's licence policy forbids bundling it, and Anthropic's legal
   page puts redistribution under the Commercial ToS with a no-modification and
   no-intermediated-usage condition. Running the user's own binary under the user's own login is
   the case that page explicitly carves out. Consequences: no `externalBin` entry, no
   provenance row, no version pin, no `REQUIRED_PAYLOAD` row, no codesign block — unlike the
   Apache-2.0 `@openai/codex` sidecar.

3. **The seam is a Rust trait, `AgentTurnTransport`, in a new `src-tauri/src/agent/` module.**
   One method, one owned request type, one owned result type:

   ```rust
   pub trait AgentTurnTransport {
       fn run_conversation_turn(
           &mut self,
           request: &AgentTurnRequest,
           deadline: Instant,
           cancellation: Option<&dyn CancellationSignal>,
       ) -> Result<AgentTurnOutcome, AgentTurnError>;
   }
   ```

   `AgentTurnRequest` carries only backend-neutral material: the rendered hidden-context prompt,
   the developer/system instructions, the output schema `Value`, the session handle, and the
   read-only tool configuration. `AgentTurnOutcome` carries the parsed proposal `Value`, the
   resolved session handle, the model actually used, an optional plain-language notice, and the
   raw transcript the app persists. Two implementations: `agent/codex_transport.rs` (delegates to
   today's `codex::app_server` code, no behaviour change) and `agent/claude_transport.rs`.
   `conversation.rs` and its submodules (requests, context, risk, impact, apply, undo) are not
   touched.

4. **The backend choice is an enum, not a trait object in the settings.**
   `AgentBackend { Codex, Claude }` in `src-tauri/src/settings/preferences.rs`, mirrored as
   `type AgentBackend = "codex" | "claude"` in `src/lib/app-settings.ts`, plus a third stored
   value `Auto` that resolves at turn time to whichever is ready (Codex first, for continuity
   with existing projects). Stored in `AppPreferencesV2` next to `generationExecutionBackend`,
   persisted by the existing Rust `AppPreferencesStore`, defaulting to `Auto`.
   **Amended 2026-09-18: Auto prefers Claude, not Codex — see Amendments.** No schema-version
   bump: a new field with a default is additive, but the Rust struct is
   `deny_unknown_fields`, so the Rust field and the TS field land in the same commit.

5. **One process per turn.** `claude -p` with `--resume` recovers full history from
   `~/.claude/projects/<slug>/<session-id>.jsonl` in a *fresh* process (verified). A per-turn
   process is therefore not a compromise: it is simpler than the Codex long-lived app-server,
   it needs no supervisor, and cancel is `SIGTERM`. A stream that ends without a `result` frame
   is the cancelled signal (verified: exit 143, no `result`).

6. **The exact Claude invocation.** Fixed argv, in this order, with only the marked parts varying:

   ```
   claude --print
          --output-format stream-json --verbose
          --input-format stream-json
          --model <alias from settings, default "sonnet">
          --fallback-model <next cheaper alias>
          --system-prompt <developer instructions>           # first turn of a session only
          --json-schema <the proposal schema, one line>
          --session-id <app-minted UUID>   |   --resume <stored session id>
          --tools ""
          --allowed-tools mcp__video-creater__<tool> …        # the read-only list, §Tools
          --mcp-config <one-line JSON, the app's own MCP sidecar>
          --strict-mcp-config
          --permission-mode dontAsk
          --permission-prompts none
          --setting-sources ""
          --safe-mode --disable-slash-commands
          --max-budget-usd <cap>
   ```

   The hidden adaptive context is written to stdin as one
   `{"type":"user","message":{"role":"user","content":[{"type":"text","text":"…"}]}}` line, then
   stdin closes. cwd is the project folder, as with Codex.

   Rationale for the non-obvious ones:
   - `--system-prompt` (not `--append-system-prompt`) replaces Claude Code's coding-agent
     prompt: measured 7178 → ~1100 input tokens, a 4–8× per-turn cost reduction, and it removes
     instructions about editing code that this product does not want.
   - `--system-prompt-snapshot` defaults to `on`, so a resumed session reuses the recorded
     prompt and ignores later `--system-prompt` text. **Per-turn context must therefore ride the
     user message, never the system prompt.** That matches Codex, where the context also rides
     `turn/start`'s input.
   - `--safe-mode --disable-slash-commands --setting-sources ""` isolate the turn from the
     user's own `CLAUDE.md`, hooks, plugins and skills, which must not influence a video edit.
   - `--bare` is explicitly **not** used: it refuses OAuth and demands `ANTHROPIC_API_KEY`
     (verified: `Not logged in · Please run /login`, exit 1).
   - `--max-budget-usd` is a guard rail, not a feature; it is generous and not surfaced.

7. **The proposal comes back as `result.structured_output`, not as prose.** `--json-schema`
   installs a synthetic end-turn tool `StructuredOutput`; the turn ends `stop_reason: "tool_use"`
   and the `result` frame carries the value both parsed (`structured_output`) and as a string
   (`result`). The backend reads `structured_output`, falls back to
   `conversation_proposal_from_text(result)` for robustness, and hands the `Value` to the
   existing `conversation_proposal_from_value` → `prepare_codex_conversation_proposal` path
   unchanged. A missing `structured_output` is the existing "the agent didn't suggest an edit"
   state, not an error.

8. **The schema is the existing one, verbatim.** `codex::app_server::project_action_schema()`
   and `codex_conversation_proposal_output_schema()` are provider-neutral JSON Schema and move
   to `agent/schema.rs` unchanged (re-exported so Codex call sites keep compiling). A real Haiku
   turn already returned a correct answer for a nested `anyOf` action union with `const`
   discriminators and `additionalProperties:false`, which is the shape of the real schema.
   **Risk:** the real schema has ~70 variants and is large; a size or complexity rejection is the
   plan's first real-CLI task, with a documented fallback — a focus-derived action subset, built
   by filtering the same schema, never by hand-writing a second one.

9. **Claude gets the app's own MCP sidecar, read-only.** `video-creater-mcp-server` already
   exists, ships in `externalBin`, and is already sandboxed to its launch `--project-dir`. The
   Claude transport writes a temporary one-line `--mcp-config` pointing at the staged binary for
   the active project, and allow-lists only inspection tools:

   `project_context`, `timeline`, `get_timeline`, `inspect_timeline`, `inspect_media`,
   `media_library`, `get_media`, `list_folders`, `get_transcript`, `transcript_words`,
   `search_media`, `list_effects`, `inspect_color`, `generated_assets`, `render_reports`,
   `export_artifacts`, `export_profiles`, `generation_defaults`, `read_skill`.

   Nothing that mutates, nothing that starts a job, nothing that costs money, nothing that opens
   or creates a project. Defence is in depth and every layer is independent: `--tools ""` removes
   the built-ins; `--allowed-tools` names only the list above; `--permission-mode dontAsk` plus
   `--permission-prompts none` makes anything else a recorded denial instead of a prompt; the
   sidecar forces its own `projectDir`; and Rust would refuse an unvalidated action anyway.
   `result.permission_denials` is logged, and a non-empty list on a successful turn is a bug
   signal, not a user-facing error.

   **Editing still returns as a proposal.** The turn's only output channel is
   `StructuredOutput`. A mutating MCP tool is not merely discouraged: it is not in the session's
   tool list.

10. **Sessions map one-to-one onto Claude sessions.** `SplitAgentSession` gains
    `provider: Option<String>` and `provider_session_id: Option<String>`; `thread_id` stays for
    Codex compatibility and existing projects keep working. A new chat mints a UUID, passes it as
    `--session-id`, and stores it. Each later turn in that chat passes `--resume <that uuid>`.
    Switching backends inside one chat starts a new provider session for the new backend and
    records both, so history never lies about which agent produced a turn. `VideoProject` gains
    nothing: `codex_thread_id` stays Codex-only and the new fields live on the session manifest,
    where the per-chat mapping belongs.

11. **Readiness is a new `agent.claude` settings component.**
    **Amended 2026-09-18: the table below is superseded — see Amendments.** Two zero-token probes:
    `claude --version` (found and runnable?) and `claude auth status` (JSON:
    `loggedIn`, `authMethod`, `subscriptionType`). States and plain messages:

    | Condition | State | Summary |
    | --- | --- | --- |
    | binary found, `loggedIn: true` | `Ready` | "Claude 2.1.270 is signed in as <email>." |
    | binary found, `loggedIn: false`, no `ANTHROPIC_API_KEY` | `ActionRequired` | "Claude is installed but not signed in. Run `claude auth login` in a terminal, or set an Anthropic API key." |
    | binary found, `ANTHROPIC_API_KEY` set | `Ready` | "Claude will use the Anthropic API key from the environment." |
    | binary not found | `NotConfigured` | "Claude isn't installed. Install it from claude.com, then reopen settings." |
    | binary found but fails to run | `Failed` | "Claude is installed but didn't start." plus the diagnostic detail |

    Diagnostic codes follow the existing convention: `agent.claude.{ready,missing,notLoggedIn,
    apiKey,launchFailed,malformedStatus}` (plus `readyOtherAuth`, added 2026-09-18). A self-test action re-runs both probes. Detection
    order for the binary: the user's optional explicit path setting, then `PATH`, then
    `~/.local/bin/claude` and `/usr/local/bin/claude`.

12. **The setting lives in Advanced → Agent, next to MCP client configuration.** A new
    `data-settings-target="advanced:agent"` section holds the backend select
    ("Automatic" / "Codex" / "Claude"), the resolved-backend line, the Claude model select
    (Sonnet default, Haiku, Opus), an optional Claude executable path field, and the two
    readiness rows. `AppSettingsTarget`'s `item` union gains `"agent"`, so
    `MissingAgentState`'s "Open Agent settings" button can deep-link to it instead of to
    `advanced:mcp`.

13. **The AI tab gains no backend switch and no model picker.** The redesign's principle holds:
    the AI tab is a conversation, not a control panel. The only AI-tab change is that
    `MissingAgentState` speaks about "an AI agent" rather than implying one product, and links to
    the new settings section. User-visible strings never name the backend beyond the word
    "Claude" in settings, and the existing ban on the literal `Codex` in `src/editor/**` strings
    stands.

14. **Names in the UI.** Settings may say "Claude"; per Anthropic's branding guidance the app may
    not use "Claude Code" as a feature name or mimic its visuals. No Anthropic logo is added.

15. **Failure classification is plain-language and backend-shaped.** The `result` frame is the
    source of truth, **not** the exit code — verified: an unauthenticated run exits 1 *and*
    prints a well-formed `{"is_error":true,"terminal_reason":"api_error",
    "result":"Not logged in · Please run /login"}`. Mapping:

    | Observation | App failure kind | Copy |
    | --- | --- | --- |
    | no `result` frame after cancel | interrupted | existing interrupted path |
    | `is_error`, `error: "authentication_failed"` | agentUnavailable | existing "isn't available right now" + the readiness message |
    | `terminal_reason: "api_error"` with rate-limit text, or `rate_limit_event` exhausted | agentUnavailable | "Claude's usage limit is reached. Try again after <reset>." |
    | success, no `structured_output` | noProposal | existing "didn't suggest an edit" |
    | schema validation failure in Rust | existing validation issues path | unchanged |

16. **Tauri commands keep their names.** `start_codex_conversation_edit_for_project` and friends
    are renamed in no commit of this work. Renaming four commands and a dozen exported TS types
    would touch the whole frontend for zero user benefit, and the redesign already decided the
    user never sees these names. The backend is selected *inside* the existing command. A
    follow-up backlog note records the naming debt.

17. **Turn deadline and cost guard.** Claude turns get their own budget: a 300 s deadline
    (a Claude turn that reads several MCP tools is slower than a Codex app-server turn) and a
    `--max-budget-usd` cap. Both are constants in `agent/claude_transport.rs`, not settings.

18. **Testing does not spend tokens.** A `FakeAgentTurnTransport` plus recorded CLI frame
    fixtures carry all behaviour tests: frame parsing, `structured_output` extraction, cancel
    without a `result` frame, auth failure, rate limit, argv construction, MCP config generation,
    session resume argv. Exactly one real turn is kept as evidence, on Haiku, at an expected
    cost under $0.02, run behind an `#[ignore]` gate and an env var, with its output retained
    under `output/claude-agent-backend/`.

## Architecture

### Module layout

```
src-tauri/src/agent/
  mod.rs               AgentBackendKind, resolve_agent_backend(), the trait, request/result/error types
  schema.rs            the proposal output schema, moved verbatim from codex/app_server.rs
  codex_transport.rs   impl AgentTurnTransport for the existing app-server path
  claude_transport.rs  impl AgentTurnTransport over `claude -p`
  claude_cli.rs        argv construction, --mcp-config generation, the read-only tool allowlist
  claude_frames.rs     stream-json frame parsing: init, assistant, result, rate_limit_event
  fake_transport.rs    #[cfg(test)] FakeAgentTurnTransport
```

`src-tauri/src/codex/` keeps everything else. `conversation.rs`, `conversation/{apply,context,
impact,parse,risk,undo}.rs`, `proposal.rs`, `tools.rs` and `mcp_server.rs` are unchanged in
behaviour; `app_server.rs` loses the schema builders to `agent/schema.rs` and gains a thin
`impl AgentTurnTransport`.

### Turn flow

Unchanged up to and after the transport call:

1. `start_codex_conversation_edit_for_project` resolves the project root, takes the lease,
   snapshots `content_revision`, and calls `request.validate(&project)`.
2. `resolve_agent_backend(preferences, readiness)` picks Codex or Claude.
3. `build_codex_conversation_context` builds the hidden context; a new
   `render_agent_turn_prompt(context)` (lifted from the existing prompt builder) renders it.
4. The chosen transport runs the turn under `codex_app_server_deadline()`-style bounds and the
   existing cancellation registry.
5. `conversation_proposal_from_value` → `prepare_codex_conversation_proposal` → risk → impact →
   deterministic ids. Identical for both backends.
6. The turn is persisted with its provider and provider session id, and the response transcript
   is scrubbed of echoed user input before it is stored — Claude's `user` frames echo the
   hidden context exactly as Codex's `userMessage` items do, so the Claude transport applies the
   same rule `without_conversation_user_input` applies today.

### Cancel

The existing per-project registry is reused unchanged. The Claude transport's cancellation check
kills the child (SIGTERM, then SIGKILL after a grace period) and returns `Interrupted`; a stream
that ended without a `result` frame maps to the same error. The frontend's
`interruptedPattern` copy therefore needs no change.

### Readiness and MCP

`settings/agent.rs` gains `probe_claude_cli()` returning `SettingsComponentHealth` for
`agent.claude`, and `agent_category_health` gains that row. `mcp_client_configuration` is
untouched for the copy-snippet dialog; the Claude transport generates its own config inline
because it points at the same staged binary with the active project's folder.

### What does not change

- The AI tab's state machine, cards, quick edits, mentions, result frames and undo.
- `CodexConversationEditRequest` / `CodexPreparedProposal` / `CodexConversationEditCommandResult`
  and the four Tauri command names.
- Risk allowlist, `apply.rs`'s stale-id and review-approval guards, snapshot undo, compressed
  history.
- The Codex sidecar, its pin, its provenance, its packaging and its readiness component.
- The "Connect external agents" dialog, which stays a copy-a-snippet affordance for *external*
  agents. Its `claudeCode` snippet keeps working and is now complemented by first-class support.

## Workstreams

Two plans, both under `docs/superpowers/plans/2026-09-18-claude-agent-backend-*`:

| # | Plan | Scope |
| --- | --- | --- |
| 01 | Transport and backend seam | `agent/` module, the trait, the schema move, the Codex adapter, the Claude transport, argv and frame parsing, fake-transport and fixture tests, cancel, sessions, the one real Haiku turn |
| 02 | Settings, UI, docs and evidence | `AgentBackend` preference (Rust + TS in lockstep), the `agent.claude` readiness component, the Advanced → Agent settings section, `MissingAgentState` deep link, failure copy, docs and the backlog row |

Plan 01 lands first; plan 02's settings work depends on plan 01's `AgentBackendKind` and
`probe_claude_cli`. Within plan 01, the frame-parsing tasks and the argv tasks are independent
and can run in parallel; within plan 02, the Rust preference task must precede the TS control.

## Testing and evidence

- **Unit, Rust.** `agent/claude_cli.rs` argv and MCP-config tests; `agent/claude_frames.rs`
  parsing tests over recorded fixtures (success with MCP tool use, success without a proposal,
  auth failure, rate limit, cancelled stream with no `result`); `agent/mod.rs` backend-resolution
  tests over every preference × readiness combination.
- **Integration, Rust.** The conversation turn driven by `FakeAgentTurnTransport`: proposal
  parsed, risk classified, ids deterministic, apply refuses a stale id, cancel withholds the
  proposal, the persisted transcript contains no echoed hidden context.
- **Frontend.** `verify:frontend`; the settings section's Vitest coverage; the existing AI-panel
  no-internal-ids test must stay green; `settings-visual-qa-fixtures.ts` updated for the new
  preference field.
- **One real turn, retained.** `#[ignore]`-gated, env-gated, `--model haiku`,
  `--max-budget-usd`, against the real installed `claude`, sending the real proposal schema and
  a one-sentence edit request over a small fixture project, asserting only that a schema-shaped
  `structured_output` came back. Output retained under `output/claude-agent-backend/`. Cost
  budget: under $0.02. Evidence that could not be observed is reported as not run, with the
  reason — never as passed.
- **Not proven here, stated as such.** macOS keychain inheritance by a Tauri-spawned child, and
  any packaged-app run. (The `ANTHROPIC_API_KEY` path was closed on 2026-09-18 with
  stub-executable tests for all four auth states — see Amendments.) Those become follow-up rows, not claims.

## Out of scope

- **Bundling Claude, in any form.** Decision 2. Also out: an in-app sign-in flow, which
  Anthropic's terms forbid third parties from offering.
- **Calling the Anthropic Messages API directly.** Decision 1. If a future product need
  (headless servers, CI, a Temporal worker with no user session) demands it, it is a third
  backend behind the same trait, not a change to this one.
- **Renaming the `codex_*` Tauri commands and TS types.** Decision 16; recorded as naming debt.
- **A backend switch or model picker in the AI tab.** Decision 13.
- **Claude for the non-conversation paths** — the legacy `start_codex_video_edit_for_project`
  edit-job path, the Temporal edit request builder, and the MCP-driven external-agent flows keep
  their current behaviour. Only the conversation turn becomes backend-pluggable.
- **Multi-backend fan-out** (asking both agents and comparing). Interesting, unrequested.
- **macOS verification.** No Mac on this host; every macOS-only item is listed as not run.
