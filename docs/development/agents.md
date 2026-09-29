# Agent backends

The AI tab's conversation turn runs behind one Rust seam, `AgentTurnTransport` in
`src-tauri/src/agent/`. Two backends implement it:

| Backend | How it runs | Shipped with the app | Preferred |
| --- | --- | --- | --- |
| Claude | The user's own `claude`, one process per turn in print mode | **No** | **Yes** — Automatic picks it when it is ready |
| Codex | The bundled `video-creater-codex` app-server over stdio, long-lived | Yes, Apache-2.0 | Fallback, or an explicit choice |

Neither backend is required. One ready backend is a working install: a user with Claude signed in
needs no Codex, and a user with only the bundled Codex needs no Claude.

Everything after the turn is shared: proposal validation, risk classification, deterministic
action ids, atomic apply and snapshot undo all live in `codex::conversation` and never see which
agent produced the proposal.

**State, 2026-09-18.** Complete: the seam, the proposal schema, the Claude invocation builder,
the frame parser, the per-turn Claude transport, per-chat sessions, the readiness probe, the
settings choice, and the wiring between them — `start_codex_conversation_edit_for_project` reads
the stored preference and routes the turn accordingly. Claude is the preferred backend: Automatic
resolves to it whenever it is ready, because it runs on the user's own subscription, needs nothing
bundled and needs no API key. One real turn has run end to end on that path (below), and AI flows
1 and 2 passed on a packaged Linux app through it. Still unproven: whether a Tauri-spawned child
inherits the macOS keychain login. The `ANTHROPIC_API_KEY` mode is covered by stub-executable
tests rather than by a real key.

Design of record: [the Claude agent backend design](../superpowers/specs/2026-09-18-claude-agent-backend-design.md).
Verified CLI contract: [the research note](../research/2026-09-18-claude-agent-backend.md).

## Why Claude is not bundled

The repo's licence rule is LGPL/MIT/BSD only, nothing GPL, nothing proprietary
(`docs/development/linux.md`). `@anthropic-ai/claude-code` and `@anthropic-ai/claude-agent-sdk`
are `SEE LICENSE IN README.md` → "© Anthropic PBC. All rights reserved.", so they fail that rule
outright. Anthropic's own terms then set two conditions on redistributing the binary:

> **The Claude Code binary must not be modified.** … customers may not remove, disable, or
> restrict any authentication method built into it …

> **Customers may not pay for, resell, or intermediate Claude usage on their end users'
> behalf.** Each end user must authenticate with their own Anthropic API key, Claude
> subscription plan credentials, or 3P inference provider credential …

Running the user's own unmodified binary under the user's own login is the case those terms
carve out, and it is what this app does. Consequences, all deliberate:

- no `externalBin` entry, no provenance row, no `REQUIRED_PAYLOAD` row, no codesign block;
- no `@anthropic-ai/*` dependency in `package.json`;
- no version pin — the app reports whatever version the user has;
- the app never reads, stores or forwards a credential. `~/.claude/.credentials.json` and the
  macOS keychain belong to the CLI.

Naming follows Anthropic's branding guidance: settings may say "Claude", but "Claude Code" is
never used as a feature name and no Anthropic logo is added.

## Choosing a backend

`Settings → Advanced → Agent` holds the choice, the Claude model alias, an optional explicit
`claude` path, and a readiness row per agent.

`agentBackend` is `automatic` (the default), `codex` or `claude`, stored in `AppPreferencesV2`.
`agent::preferred_agent_backend` turns it into `resolve_agent_backend`'s input: `automatic` names
no backend, and resolves to **Claude first**, then Codex. Claude is preferred because it runs on
the user's own Claude subscription, ships nothing, pins nothing, and has no shared account whose
usage limit the product could end up waiting on. A chosen agent that is not ready yields to the
other one rather than refusing the turn, and the settings section says so. Existing projects keep
working: `codex_thread_id` still means what it meant, and a turn that routes to Codex still
resumes that thread.

`resolveAgentBackend` in `src/components/settings/agent-backend-settings.tsx` mirrors the same
rule, so the "Automatic is using …" line cannot disagree with what a turn would do.

"Ready" for Claude means a turn would *authenticate*, not just that the binary exists — see
below. An installed but signed-out `claude` yields to Codex instead of failing mid-turn.

The turn command reads all three agent settings — `agentBackend`, `claudeModel` and
`claudeExecutablePath` — itself, from the managed `AppPreferencesState`, before it starts its
blocking task. They are deliberately **not** command arguments: the command's name, its
TypeScript-visible signature and its result shape are the frontend's contract, and none of them
mentions an agent. `agent::turn::plan_agent_turn` then makes the choice and builds the Claude
transport with the stored model, so the setting reaches `--model` on the same path a real turn
takes. A preferences file that is missing, unreadable or unparseable behaves exactly like
Automatic on the default model — a settings problem must never be the reason a turn cannot run.
`--fallback-model` is the next cheaper alias (Opus → Sonnet → Haiku, and Haiku has none).

The routing rules are pinned in `src-tauri/tests/agent_claude/routing.rs`, all through the real
`AppPreferencesStore` and none of them spending a token.

## Readiness and subscription auth, without spending tokens

`settings::agent::claude_readiness` runs two local commands through the shared process probe and
returns both answers at once — the settings row and the router's verdict — so the row can never
describe a credential the turn would not use. `probe_claude_cli` and `claude_turn_readiness` are
thin wrappers that take one half each.

- `claude --version` — found and runnable? It prints a plain line rather than JSON, so the probe
  reports it as a malformed-JSON error whose captured line is the answer; only the leading
  version number is kept.
- `claude auth status` — signed in, and how? It prints a JSON document read from the on-disk
  credentials. Exactly three fields are deserialized: `loggedIn`, `authMethod` and
  `subscriptionType`. The real payload also carries the account's email, organisation id and
  organisation name; **not** deserializing them is the simplest guarantee that none of them can
  reach a summary, a provenance map or a log.

Neither reaches the API, so opening settings costs nothing.

| `auth status` | State | Diagnostic | What the row says | Router verdict |
| --- | --- | --- | --- | --- |
| `loggedIn`, `authMethod: "claude.ai"` | `ready` | `agent.claude.ready` | signed in with your Claude subscription (with the plan, when reported) — "No API key is needed." | ready, `Subscription` |
| `loggedIn`, another `authMethod` | `ready` | `agent.claude.readyOtherAuth` | signed in, but not with a Claude subscription | ready, `OtherLogin` |
| not signed in, `ANTHROPIC_API_KEY` set | `ready` | `agent.claude.apiKey` | turns will use the environment's API key; sign in to use the subscription instead | ready, `ApiKey` |
| not signed in, no key | `actionRequired` | `agent.claude.notLoggedIn` | "Sign in to Claude with your subscription: run `claude` in a terminal and use /login." | not ready |
| not installed | `notConfigured` | `agent.claude.missing` | install it, then sign in with your subscription | not ready |
| installed, won't run or unreadable status | `failed` | `agent.claude.launchFailed` / `agent.claude.malformedStatus` | plus the diagnostic detail | not ready |

**An API key is never required, and never shadows a subscription.** The CLI prefers
`ANTHROPIC_API_KEY` over the OAuth login, so inheriting the app's environment could bill a user's
API account for a turn they expected their plan to cover — and could keep working after the key
expired while the row still said "subscription". So a `claude.ai` session removes
`ANTHROPIC_API_KEY` from the child's environment (`SupervisedCommand::env_remove`, decided by
`claude_child_env_removals`), and a session whose only credential *is* a key passes the
environment through untouched, because removing the variable would remove the credential. Nothing
in the Claude path sets `ANTHROPIC_API_KEY` or requires it, and nothing reads its value: the probe
only asks whether a non-empty one exists. The four auth states and both env behaviours are pinned
by stub executables in
`src-tauri/tests/agent_claude/auth.rs` and the `settings::agent` unit tests.

Only the version, auth method, subscription type and resolved path reach provenance; the account's
identity, organisation identifiers and local directory paths do not.

Readiness is checked **on request**, never when the settings page opens: probing starts
processes, and a preference page must not. A turn asks for it too, through
`claude_turn_readiness`, because Automatic has to know whether Claude would authenticate before
it routes there — that is the only place readiness runs without a user asking, and it costs two
local process spawns against a turn measured in seconds.

Executable resolution order: the user's explicit path setting, then `PATH`, then
`~/.local/bin/claude`, then `/usr/local/bin/claude`. An absent binary is `notConfigured` — a
configuration state, not a broken install — and the Agent category stays ready as long as
*either* agent is.

An absent bundled Codex sidecar reads the same way: `notConfigured`, "It is optional: turns can
run on Claude instead.", with "Use Claude instead, or reinstall Video Creater…" as the recovery
action. The Agent system-health section is registered as **not required**, so an agent row never
darkens the overall badge. The Linux smoke gate's agent self-test step exercises all four rows and
requires the two support rows plus *at least one* ready backend
(`agentSelfTestVerdict` in `scripts/linux-desktop-smoke-agent.mjs`); `--agent-flows` defaults to
`--agent-backend claude`.

Still Codex-only, deliberately: the legacy `start_codex_video_edit_for_project` edit-job path and
the Temporal edit-proposal activity. Neither is on the AI tab's path, so a Claude-only install can
use the conversation flows fully. The "Connect external agents" dialog is unaffected — it is a
copy-a-snippet affordance for *external* agents (Codex, Claude Code, Claude Desktop, Cursor)
reaching the app over MCP, and it gates on nothing.

## The Claude invocation

`agent::claude_cli::build_claude_turn_argv` builds a fixed argv. Why each part is there:

| Flag | Reason |
| --- | --- |
| `--print` | One turn, one process. `--resume` recovers full history in a fresh process, so a long-lived supervisor buys nothing. |
| `--output-format stream-json --verbose` | The frame stream the app parses. |
| `--input-format stream-json` | The prompt rides one stdin line, then stdin closes. |
| `--model <alias>` / `--fallback-model` | Aliases resolve server-side, so no dated model id is stored. |
| `--system-prompt <instructions>` | Replaces the coding-agent prompt: ~7178 → ~1100 input tokens, and it drops instructions about editing code this product does not want. First turn of a session only — `--system-prompt-snapshot` defaults to `on`, so a resumed session reuses the recorded prompt. **Per-turn context therefore rides the user message, never the system prompt.** |
| `--json-schema <schema>` | Installs the synthetic `StructuredOutput` end-turn tool; the proposal comes back as `result.structured_output`. The schema is the existing provider-neutral one, verbatim. |
| `--session-id <uuid>` or `--resume <id>` | The app mints the UUID so it can store it before the process runs. |
| `--tools ""` | Removes every built-in tool. |
| `--allowed-tools` / `--disallowed-tools` | Only the 19 read-only MCP tools below. |
| `--mcp-config` + `--strict-mcp-config` | The app's own sidecar, pinned to the active project, and nothing from the user's own MCP config. |
| `--permission-mode dontAsk` + `--permission-prompts none` | Anything outside the allowlist becomes a recorded denial instead of a prompt nobody can answer. |
| `--setting-sources ""` + `--disable-slash-commands` | Isolates the turn from the user's `CLAUDE.md`, hooks, plugins and skills, which must not influence a video edit. |
| `--max-budget-usd` | A guard rail, not a feature. Generous and not surfaced. |

`--bare` is deliberately absent: it refuses OAuth and demands `ANTHROPIC_API_KEY` (verified:
`Not logged in · Please run /login`, exit 1). `--dangerously-skip-permissions` never appears.

### The read-only tool surface

`CLAUDE_READ_ONLY_TOOLS` names 19 inspection tools: `project_context`, `timeline`, `get_timeline`,
`inspect_timeline`, `inspect_media`, `media_library`, `get_media`, `list_folders`,
`get_transcript`, `transcript_words`, `search_media`, `list_effects`, `inspect_color`,
`generated_assets`, `render_reports`, `export_artifacts`, `export_profiles`,
`generation_defaults`, `read_skill`.

Nothing there mutates, starts a job, costs money, or opens a project. Defence is layered and each
layer is independent: the built-ins are gone, the allowlist names only these, denials are
recorded rather than prompted, the sidecar forces its own `projectDir`, and Rust would refuse an
unvalidated action anyway. Editing still returns as a proposal, because `StructuredOutput` is the
turn's only output channel.

## Sessions

`SplitAgentSession` carries `provider` and `provider_session_id` beside the existing `thread_id`.
A new chat mints a UUID and passes it as `--session-id`; each later turn passes
`--resume <that uuid>`. Switching backends inside one chat starts a new provider session and
records both, so history never lies about which agent produced a turn. `codex_thread_id` on
`VideoProject` keeps its name and its Codex-only meaning.

**Later turns are already the cheap ones**, and no further change was needed:

- A resumed turn passes `--resume`, so the chat's history is recovered from
  `~/.claude/projects/<slug>/<session-id>.jsonl` by a fresh process instead of being re-sent.
- `--system-prompt` is sent on a session's **first** request only. `--system-prompt-snapshot`
  defaults to `on`, so a resumed session reuses the recorded prompt; sending it again would be
  ignored, and replacing Claude Code's own coding-agent prompt is what takes a turn's input from
  ~7178 to ~1100 tokens in the first place.
- Measured on the retained fixtures: the first turn of a session cost $0.004156 and the `--resume`
  turn that followed cost $0.002155. The second turn of a chat is cheaper than the first.
- `src-tauri/tests/agent_claude/argv.rs::a_resumed_turn_replaces_the_session_id_and_drops_the_system_prompt`
  and `sessions.rs::a_second_turn_reuses_the_handle_and_the_argv_then_resumes_it` pin both halves.

## Re-running the retained real turn

Fixtures carry every behaviour test, so the suite spends nothing. Two real turns are kept as
evidence, and every other fixture there is hand-derived from them:

| Fixture | What it proves | Cost |
| --- | --- | --- |
| `real-turn-2026-09-18.jsonl` | The real ~70-variant proposal schema is accepted at full size by `claude` 2.1.270 on Haiku (`claude-haiku-4-5-20251001`). | $0.0138613 |
| `real-command-turn-2026-09-18.json` | With `agentBackend: claude` and `claudeModel: haiku` stored, the app's own path chose Claude over a ready Codex, attached the sidecar (`video-creater: connected`), returned a proposal that validated into one `trimItems` action, recorded no permission denial, and leaked no id into the summary. | $0.057903 |

The second one is the transcript the app itself persists, so
`agent_claude::real_command_turn::the_retained_command_turn_still_satisfies_the_evidence_checks`
replays it through the same assertions for free on every run.

Both are gated twice — `#[ignore]` and an environment variable — so neither can run in a normal
suite. The command-path turn also needs the sidecar staged, because that is the binary
`--mcp-config` points at:

```
TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' \
cargo build --manifest-path src-tauri/Cargo.toml \
  --bin video-creater-mcp-server --features mcp-server

VIDEO_CREATER_CLAUDE_REAL_TURN=1 \
TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' \
cargo test --manifest-path src-tauri/Cargo.toml --test agent_claude real_command_turn \
  -- --ignored --test-threads=1 --nocapture
```

Swap `real_command_turn` for `real_claude_turn` to re-run the schema probe instead.

It costs real money. Run it only when the CLI contract itself is in question, never in a loop.
