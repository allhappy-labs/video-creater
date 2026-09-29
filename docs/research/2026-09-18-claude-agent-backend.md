# Claude as an AI Agent Backend — Research Note

Date: 2026-09-18. Host: this Linux box (Ubuntu, kernel 6.8). Author: autonomous research run.

Question: can Video Creater drive Claude as a second conversation-turn backend next to the
Codex app-server, and if so through which exact interface, under which licence, with which
auth, and at what cost?

Answer: yes, by running the user's own installed `claude` CLI once per turn in print mode with
`--json-schema`. Every claim below was checked against the installed binary or a primary
Anthropic source. Verified real turns cost **$0.036 in total** (seven Haiku turns).

Companion documents:
- Spec: `docs/superpowers/specs/2026-09-18-claude-agent-backend-design.md`
- Plans: `docs/superpowers/plans/2026-09-18-claude-agent-backend-01-transport-and-seam.md`,
  `docs/superpowers/plans/2026-09-18-claude-agent-backend-02-settings-packaging-evidence.md`

## 1. What is installed

```
$ ~/.local/bin/claude --version
2.1.270 (Claude Code)
```

A native (not npm) install at `/home/olhapi/.local/bin/claude`. Config lives in `~/.claude`
(`settings.json`, `.credentials.json`, `projects/`, `plugins/`, `sessions/`).

## 2. Verified CLI contract

Everything in this section is quoted from `claude --help` on 2.1.270, or observed from a real
run on this host. The flags the app needs are grouped by purpose.

### 2.1 Non-interactive (print) mode and output formats

```
  -p, --print                           Print response and exit (useful for
                                        pipes). Note: The workspace trust dialog
                                        is skipped when Claude is run in
                                        non-interactive mode (via -p, or when
                                        stdout is not a TTY, e.g. piped or
                                        redirected output). Only use this in
                                        directories you trust. Settings files
                                        that fail validation are silently
                                        ignored in this mode (no error dialog is
                                        shown).
  --output-format <format>              Output format (only works with --print):
                                        "text" (default), "json" (single
                                        result), or "stream-json" (realtime
                                        streaming) (choices: "text", "json",
                                        "stream-json")
  --input-format <format>               Input format (only works with --print):
                                        "text" (default), or "stream-json"
                                        (realtime streaming input) (choices:
                                        "text", "stream-json")
  --include-partial-messages            Include partial message chunks as they
                                        arrive (only works with --print and
                                        --output-format=stream-json)
  --verbose                             Override verbose mode setting from
                                        config
```

`--output-format json` prints exactly one JSON object, the `result` frame. Observed fields
(trimmed from a real run):

```json
{"type":"result","subtype":"success","is_error":false,"stop_reason":"tool_use",
 "session_id":"0406a3dc-a644-4397-ae6e-a9552d78429d","num_turns":2,
 "result":"{\"n\":7}","structured_output":{"n":7},
 "total_cost_usd":0.004156,"terminal_reason":"completed","permission_denials":[],
 "usage":{"input_tokens":1111,"output_tokens":252,"cache_read_input_tokens":0, "...":"..."},
 "modelUsage":{"claude-haiku-4-5-20251001":{"canonicalModel":"claude-haiku-4-5","...":"..."}}}
```

`--output-format stream-json` (with `--verbose`) emits newline-delimited JSON. Observed frame
sequence for a turn that called one MCP tool and then returned structured output:

| # | `type` | `subtype` | Notes |
| --- | --- | --- | --- |
| 1 | `system` | `init` | `session_id`, `cwd`, `tools`, `mcp_servers`, `model`, `permissionMode`, `apiKeySource`, `claude_code_version`, `capabilities` |
| 2 | `rate_limit_event` | — | `rate_limit_info.status`, five-hour / seven-day utilization |
| 3 | `system` | `thinking_tokens` | running estimate, one per delta |
| 4 | `assistant` | — | `message.content[]` with `thinking` / `tool_use` / `text` blocks |
| 5 | `user` | — | synthetic `tool_result` echo |
| 6 | `result` | `success` \| `error_during_execution` | the object above |

`--input-format stream-json` accepts this envelope on stdin, one per line (verified):

```json
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Reply with the number 7."}]}}
```

An empty or immediately-closed stdin in `--input-format stream-json` mode produces **no
frames at all** — the `system/init` frame is only emitted once the first user message
arrives. So there is no zero-token way to read the init frame (see Limits, §7).

### 2.2 Structured JSON output — the important one

```
  --json-schema <schema>                JSON Schema for structured output
                                        validation. Example:
                                        {"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}
```

Verified mechanics:
- The schema is implemented as a **synthetic end-turn tool named `StructuredOutput`**. It
  appears in the `system/init` frame's `tools` array even when `--tools ""` removed every
  built-in tool, and the turn ends with `stop_reason: "tool_use"`.
- The result frame carries the value twice: `structured_output` (parsed object) and `result`
  (the same JSON as a string). Read `structured_output`.
- **Nested discriminated unions work.** A schema shaped like the app's proposal
  (`{summary, actions: [{anyOf: [...]}]}` with `const` discriminators and
  `additionalProperties:false`) returned, from a real Haiku turn:

  ```json
  {"summary":"Slowed clip c1 to half speed (0.5x) and trimmed clip c2 to the 0-1000ms range.",
   "actions":[{"kind":"setClipSpeed","clipId":"c1","speed":0.5},
              {"kind":"trimClip","clipId":"c2","startMs":0,"endMs":1000}]}
  ```

  That is the single most important result in this note: `codex/app_server.rs`'s
  `project_action_schema()` (~70 action variants under one `anyOf`) can be handed to Claude
  verbatim.
- The Agent SDK's typings call these "end-turn tool sessions" and warn about one resume
  subtlety (`sdk.d.ts`, quoted):

  > End-turn tool sessions (`outputFormat: {type: 'json_schema'}`, or any MCP tool using
  > `_meta['claude/endTurn']`): a completed turn there ends on a successful tool_result
  > carrier — with no trailing assistant message — followed by a `structured_output`
  > attachment holding the turn's actual output (the carrier's data is a placeholder).

  This only matters for *truncating* resumes (`--resume` plus a fork point); a plain
  `--resume` is unaffected (verified in §2.5).

### 2.3 System prompt

```
  --system-prompt <prompt>              System prompt to use for the session
  --append-system-prompt <prompt>       Append a system prompt to the default
                                        system prompt
  --system-prompt-snapshot <on|off>     Record the system prompt once per
                                        conversation and reuse it verbatim on
                                        every request and resume. on (the
                                        default): the prompt is rendered on the
                                        conversation's first request ... every later
                                        request and resume sends the record as-is,
                                        even when a later launch passes different text,
                                        until the conversation is compacted.
```

`--system-prompt` **replaces** Claude Code's coding-agent prompt. Measured effect on one
trivial turn:

| Invocation | Input tokens billed | Cost |
| --- | --- | --- |
| default system prompt, `--tools ""` | 10 input + 7178 cache-creation | $0.015924 |
| `--system-prompt "You output JSON only."` | 1111 input | $0.003325 |
| `--safe-mode --disable-slash-commands --system-prompt …` | 1136 input | $0.001911 |

So a replaced system prompt is roughly **4–8× cheaper per turn** than the default, and it
removes coding-agent instructions the video editor does not want. The `--system-prompt-snapshot`
default (`on`) is a design constraint: **per-turn hidden project context must travel in the
user message, not the system prompt**, because a resumed session reuses the recorded prompt
and ignores later `--system-prompt` text.

### 2.4 Tools, MCP and permissions

```
  --tools <tools...>                    Specify the list of available tools from
                                        the built-in set. Use "" to disable all
                                        tools, "default" to use all tools, or
                                        specify tool names (e.g.
                                        "Bash,Edit,Read").
  --allowedTools, --allowed-tools <tools...>
      Comma or space-separated list of tool names to allow (e.g. "Bash(git *)
      Edit")
  --disallowedTools, --disallowed-tools <tools...>
      Comma or space-separated list of tool names to deny (e.g. "Bash(git *)
      Edit")
  --mcp-config <configs...>             Load MCP servers from JSON files or
                                        strings (space-separated)
  --strict-mcp-config                   Only use MCP servers from --mcp-config,
                                        ignoring all other MCP configurations
  --permission-mode <mode>              Permission mode to use for the session
                                        (choices: "acceptEdits", "auto",
                                        "bypassPermissions", "manual",
                                        "dontAsk", "plan")
  --permission-prompts <target>         Who answers permission prompts with
                                        --print: "host" (the SDK host or
                                        --permission-prompt-tool) or "none"
                                        (nobody: anything that would prompt is
                                        denied automatically; the permission
                                        mode still decides everything else)
                                        (choices: "host", "none", default:
                                        "host")
  --setting-sources <sources>           Comma-separated list of setting sources
                                        to load (user, project, local).
  --safe-mode                           Start with all customizations
                                        (CLAUDE.md, skills, plugins, hooks, MCP
                                        servers, custom commands and agents,
                                        output styles, workflows, custom themes,
                                        keybindings, and more) disabled ...
  --disable-slash-commands              Disable all skills
  --max-budget-usd <amount>             Maximum dollar amount to spend on API
                                        calls (only works with --print)
```

Verified with `--mcp-config` pointing at a stub stdio MCP server that exposed one tool
`get_project`, plus `--tools "" --allowed-tools "mcp__video-creater__get_project"`:

```
INIT {"tools": ["StructuredOutput", "mcp__video-creater__get_project"],
      "mcp_servers": [{"name": "video-creater", "status": "connected"}],
      "model": "claude-haiku-4-5-20251001", "permissionMode": "dontAsk", "apiKeySource": "none"}
TOOL_USE mcp__video-creater__get_project {}
TOOL_RESULT [{"type": "text", "text": "{\"projectName\":\"Demo Reel\",\"clipCount\":3}"}]
TOOL_USE StructuredOutput {"projectName": "Demo Reel", "clipCount": 3}
RESULT {"subtype":"success","is_error":false,"stop_reason":"tool_use","num_turns":3,
        "total_cost_usd":0.004579,"structured_output":{"projectName":"Demo Reel","clipCount":3},
        "permission_denials":[],"terminal_reason":"completed"}
```

Conclusions:
- MCP tool names are `mcp__<serverName>__<toolName>`. `--allowed-tools` takes them verbatim.
- `--tools ""` does remove every built-in tool (no Read/Write/Edit/Bash), while leaving
  `StructuredOutput` and the allow-listed MCP tools. That is exactly the read-only surface the
  app wants.
- `mcp_servers[].status` in the init frame is a usable per-turn readiness signal.
- `--mcp-config` is **not** honoured by the `claude mcp list` subcommand — that subcommand
  showed this machine's user-scoped servers even with `--mcp-config … --strict-mcp-config`.
  MCP wiring can only be confirmed from a session's own `system/init` frame.
- `--permission-mode dontAsk --permission-prompts none` means nothing can block on a prompt;
  anything that would have prompted is denied and listed in `result.permission_denials`.

### 2.5 Sessions: resume, continuation, persistence

```
  -c, --continue                        Continue the most recent conversation in
                                        the current directory
  -r, --resume [value]                  Resume a conversation by session ID, or
                                        open interactive picker with optional
                                        search term
  --session-id <uuid>                   Use a specific session ID for the
                                        conversation (must be a valid UUID)
  --fork-session                        When resuming, create a new session ID
                                        instead of reusing the original (use
                                        with --resume or --continue)
  --no-session-persistence              Disable session persistence - sessions
                                        will not be saved to disk and cannot be
                                        resumed (only works with --print)
```

Verified two-process round trip, both processes `-p --output-format json`:

```
TURN1 --session-id 0406a3dc-…  prompt "Remember the code word BANANA."
      {"session_id":"0406a3dc-…","structured_output":{"ok":true},"total_cost_usd":0.004156}
TURN2 --resume 0406a3dc-…      prompt "What was the code word?"
      {"session_id":"0406a3dc-…","structured_output":{"word":"BANANA"},"total_cost_usd":0.002155}
```

Facts established:
- The caller may choose the session UUID up front (`--session-id`), so the app can mint the id
  and store it, exactly as it stores `codex_thread_id`.
- `--resume <uuid>` in a *fresh process* recovers the full history. Conversation state is a
  file, not a live process: `~/.claude/projects/<slugified-cwd>/<session-id>.jsonl`.
- The `--json-schema` value may differ between turns of the same session.
- `--session-id` cannot be combined with `--resume`/`--continue` unless `--fork-session` is
  also given (per `--help`).
- `--no-session-persistence` is the right flag for tests and probes.

### 2.6 Model selection

```
  --model <model>                       Model for the current session. Provide
                                        an alias for the latest model (e.g.
                                        'fable', 'opus', or 'sonnet') or a
                                        model's full name (e.g.
                                        'claude-fable-5').
  --fallback-model <model>              Enable automatic fallback to specified
                                        model(s) when the default model is
                                        overloaded or not available. Accepts a
                                        comma-separated list to try each in
                                        order. ...
  --effort <level>                      Effort level for the current session
                                        (low, medium, high, xhigh, max)
```

Aliases (`opus`, `sonnet`, `haiku`, `fable`) resolve server-side to the current generation, so
the app never has to hard-code a dated model id. Observed: `--model haiku` resolved to
`claude-haiku-4-5-20251001`, reported in the init frame as `model` and in
`result.modelUsage.<dated id>.canonicalModel` as `claude-haiku-4-5`. Current first-party ids,
from the bundled `claude-api` docs skill (cached 2026-06-24): `claude-opus-5` ($5/$25 per MTok,
1M context), `claude-sonnet-5` ($2/$10, 1M), `claude-haiku-4-5` ($1/$5, 200K),
`claude-fable-5-1` ($10/$50, 1M). Those ids are for the Messages API; the CLI wants the alias
or the same string via `--model`.

### 2.7 Auth

```
$ claude auth status
{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty",
 "projectsDirectory":"/home/olhapi/.claude/projects","configDirectory":"/home/olhapi/.claude",
 "email":"oleh@olhapi.com","orgId":"…","orgName":"…","subscriptionType":"max"}
```

This one command is a complete, **zero-token** readiness probe: JSON on stdout, `loggedIn`,
`authMethod`, `subscriptionType`. Other findings:

- A child process inherits the login with no extra work: credentials live in
  `~/.claude/.credentials.json` (mode 0600) on Linux, or the macOS keychain. The init frame
  reports `apiKeySource: "none"` when OAuth is in use.
- `ANTHROPIC_API_KEY` is the alternative; it then shows up as the `apiKeySource`.
- `--bare` deliberately refuses OAuth: *"Anthropic auth is strictly ANTHROPIC_API_KEY or
  apiKeyHelper via --settings (OAuth and keychain are never read)"*. Verified: with no API key
  in the environment, `claude --bare -p …` printed `Not logged in · Please run /login` and
  **exited 1**, with a full stream in which the last frame was
  `{"type":"result","is_error":true,"result":"Not logged in · Please run /login",
  "terminal_reason":"api_error","total_cost_usd":0}` and the assistant frame carried
  `"error":"authentication_failed"`. So `--bare` is the wrong flag for a subscription user;
  `--safe-mode` is the right isolation flag.
- Failure classification rule for the backend: **branch on the `result` frame
  (`is_error`, `terminal_reason`, `error`), not on the exit code alone.** The unauthenticated
  run above exited 1 *and* printed a well-formed error result; a successful run exits 0.

### 2.8 Cancellation

Two mechanisms, both usable:
1. **Kill the process.** Verified: `SIGTERM` to a mid-turn `claude -p` ends it with exit 143;
   the stream stops after the first `assistant` frame with **no `result` frame**. "Stream ended
   without a result frame" is therefore a clean, unambiguous cancelled signal.
2. **Control protocol.** In `--input-format stream-json` mode either side can send
   `{"type":"control_request","request_id":"<unique>","request":{"subtype":"interrupt"}}`
   (envelope and `SDKControlInterruptRequest` quoted from the SDK's `sdk.d.ts`); the CLI
   advertises `interrupt_receipt_v1`, `interrupt_cancel_queued_v1`, `msg_lifecycle_v1` in the
   init frame's `capabilities`. This is only needed for a long-lived process.

### 2.9 Other flags worth knowing

- `--add-dir <dirs...>` widens file-tool access; irrelevant while `--tools ""` is set.
- `--max-budget-usd <amount>` caps API spend per print-mode run. Cheap insurance.
- `--settings <file-or-json>` injects settings without touching the user's files.
- `claude mcp serve` would expose Claude Code itself as an MCP server — not what the app wants.
- `claude doctor` reads settings without a trust prompt; a diagnostic aid, not a readiness probe.

## 3. The Claude Agent SDK

| Fact | Value |
| --- | --- |
| Package | `@anthropic-ai/claude-agent-sdk` |
| Latest version (2026-09-18) | `0.3.276`, `dist-tags.latest` and `.next` both |
| Declared licence | `"SEE LICENSE IN README.md"` |
| `LICENSE.md`, verbatim | `© Anthropic PBC. All rights reserved. Use is subject to the Legal Agreements outlined here: https://code.claude.com/docs/en/legal-and-compliance.` |
| Repo | https://github.com/anthropics/claude-agent-sdk-typescript |
| Tarball size | 1.4 MB (JS + `.d.ts` only) |
| Bundled Claude Code | `"claudeCodeVersion": "2.1.276"` |
| Peer deps | `@anthropic-ai/sdk >=0.93.0`, `@modelcontextprotocol/sdk ^1.29.0`, `zod ^4.0.0` |
| Optional deps | eight per-platform binaries: `@anthropic-ai/claude-agent-sdk-{linux-x64,linux-arm64,linux-x64-musl,linux-arm64-musl,darwin-x64,darwin-arm64,win32-x64,win32-arm64}`, each pinned `0.3.276` |
| Sibling | `@anthropic-ai/claude-code` `2.1.276`, same `"SEE LICENSE IN README.md"` |

So the SDK is a **thin wrapper that spawns the same Claude Code binary** (it exposes
`pathToClaudeCodeExecutable`, `executable?: 'bun' | 'deno' | 'node'`, `executableArgs`), and its
option names map one-to-one onto the CLI flags above: `systemPrompt`, `mcpServers`,
`allowedTools`, `disallowedTools`, `permissionMode`, `resume`, `forkSession`, `settingSources`,
`strictMcpConfig`, `outputFormat`, `includePartialMessages`, `maxBudgetUsd`, `canUseTool`.

Anthropic's own guidance, from the Agent SDK overview
(https://code.claude.com/docs/en/agent-sdk/overview):

> The SDK is available as a library for Python and TypeScript only. To drive the same agent loop
> from another language, [run the CLI as a subprocess](/docs/en/headless) with the `-p` flag and
> `--output-format json`.

Video Creater's agent backend is Rust. That sentence is the whole decision: **drive the CLI.**
Adding the SDK would mean adding a Node sidecar (this repo ships no Node runtime), shipping a
proprietary npm package, and inheriting the SDK's API-key-first auth posture — for a wrapper
around flags the Rust process can pass itself.

### 3.1 Licence and packaging verdict

Primary sources:
- https://code.claude.com/docs/en/legal-and-compliance
- https://code.claude.com/docs/en/agent-sdk/overview (the "License and terms" section)

Quoted, from the legal page, under *"Can customers offer Claude Code in their products?"*:

> Unless we've mutually agreed otherwise, preinstalling or running Claude Code in your products
> or services (e.g. in hosted sandboxes or other agent infrastructure) requires agreeing to our
> Commercial Terms of Service and complying with the conditions below:
>
> * **The Claude Code binary must not be modified.** … customers may not remove, disable, or
>   restrict any authentication method built into it …
> * **Customers may not pay for, resell, or intermediate Claude usage on their end users'
>   behalf.** Each end user must authenticate with their own Anthropic API key, Claude
>   subscription plan credentials, or 3P inference provider credential …

And, on auth:

> Anthropic does not permit third-party developers to offer Claude.ai login into their own
> applications, or to route requests through Free, Pro, or Max plan credentials on behalf of
> their users. Moreover, developers may not collect, store, or intermediate Claude.ai
> credentials or session tokens — sign-in to a Claude account must complete through Anthropic's
> own flow. … Nor does it prevent an end user from signing in to the unmodified Claude Code
> binary with their own Claude subscription, including where a platform hosts Claude Code …

Applied to this repo's licence policy ("Linux follows the macOS license rule: no GPL
components; LGPL GStreamer, GES, GTK and WebKitGTK are dynamically linked" —
`docs/development/linux.md`, plus the LGPL/MIT/BSD-only convention):

- **Do not bundle** `claude`, `@anthropic-ai/claude-code`, or `@anthropic-ai/claude-agent-sdk`.
  All-rights-reserved terms are neither LGPL, MIT nor BSD; bundling would add a proprietary
  binary to `externalBin` and a provenance row the policy cannot justify, and would drag the
  repo under the Commercial ToS as a redistributor. This is the opposite of `@openai/codex`,
  which is Apache-2.0 (`src-tauri/resources/codex-runtime/provenance.json`) and therefore
  bundleable.
- **Do** invoke the user's own unmodified `claude`, found on `PATH` or at a user-set path, with
  the user's own login. That is precisely the case the legal page carves out, it keeps sign-in
  inside Anthropic's flow, and the app never touches a credential.
- Naming, from the same docs: the product may say in plain text that it uses Claude; it may not
  use "Claude Code" as part of a feature name, or Claude Code-branded visuals. The UI string
  should be **"Claude"** (the backlog/redesign rule already bans the literal `Codex` in
  user-visible strings for the same family of reasons).

## 4. What the app needs from a backend (derived from the Codex implementation)

The contract, read off `src-tauri/src/codex/`:

1. **A preset-free turn that returns a validatable proposal.** In: `CodexConversationEditRequest`
   (`prompt`, `focus`, `created_at`). Out: a `CodexConversationEditProposal`
   (`summary`, `edl`, `project_actions`, `render_review`), which Rust then puts through
   `prepare_codex_conversation_proposal` → validation → `classify_codex_proposal_risk` →
   `codex_proposal_impact` → deterministic `codex-action-N-<sha>` ids.
2. **Hidden adaptive context.** `build_codex_conversation_context` assembles 13 summaries
   (media library, timeline, transcripts, generated assets, template overrides, render reports,
   workflow jobs, export artifacts, export capabilities, project files, internal focus) under
   per-collection caps with 4-tier relevance ranking. This is the turn's real payload; the user
   never sees it. Codex's own echo of it is scrubbed by `without_conversation_user_input`.
3. **Cancel.** `register_codex_turn_cancellation` / `request_codex_turn_cancellation` on a
   process-global registry keyed by project, plus a 180 s deadline
   (`APP_SERVER_TURN_TIMEOUT`). A cancelled turn withholds its proposal and its history entry.
4. **Sessions and history.** `VideoProject::codex_thread_id` holds the live provider thread;
   `agent-sessions.json` (`SplitAgentSession.thread_id`) and
   `app-server-conversations.json` hold the chat list and per-turn records.
5. **No internal ids in user-visible text.** Enforced by fixtures
   (`src/lib/runtime/fixtures/conversation-fixtures.test.ts`, regex over `proposal.summary`) and
   by `src/editor/panels/ai/ai-panel.test.tsx` scanning the rendered DOM.
6. **Risk classification and atomic apply stay in Rust.** `conversation/risk.rs` is a
   fail-closed allowlist; `conversation/apply.rs` re-loads the project under a mutation lease,
   re-prepares, compares `action_ids` and refuses stale or unapproved bundles. A backend cannot
   weaken any of it, because a backend only ever produces the untrusted proposal JSON.

Everything in that list except (3)'s transport plumbing is already backend-independent. The
Codex-specific parts are: the app-server JSON-RPC dialect and its method names, the
`initialize`/`thread/start`/`turn/start` sequence, `model_selection.rs`'s `model/list`, the
bundled-binary resolution, `without_conversation_user_input`, and the field name
`codex_thread_id`.

## 5. How Claude maps onto that contract

| App need | Codex | Claude (verified) |
| --- | --- | --- |
| Turn transport | long-lived `codex app-server --stdio`, JSON-RPC-ish | one `claude -p` process per turn |
| Output schema | `turn/start` `params.outputSchema` | `--json-schema <same JSON>` |
| Proposal read-back | `item/completed` → `proposal_from_value` on `final_text` | `result.structured_output` |
| Hidden context | `turn/start` `params.input[0].text` | the user message (stdin or argv) |
| Developer instructions | `thread/start` `params.developerInstructions` | `--system-prompt` (first turn of the session only, see §2.3) |
| Session id | `project.codex_thread_id` | app-minted `--session-id` UUID, reused via `--resume` |
| Cancel | `turn/interrupt` + terminate | SIGTERM (no `result` frame) or a `control_request` interrupt |
| Read-only tools | separately configured MCP sidecar, user-pasted | `--mcp-config` with the app's own `video-creater-mcp-server`, `--tools ""`, `--allowed-tools mcp__video-creater__<tool>` |
| Sandbox | `sandbox: "read-only"`, `approvalPolicy: "never"` | `--tools ""` + `--permission-mode dontAsk` + `--permission-prompts none` |
| Readiness | `initialize` probe over stdio | `claude auth status` (no tokens) + `claude --version` |
| Model guard | `model/list`, pinned fallback `gpt-5.5` | `--model <alias>` + `--fallback-model`; aliases never go stale |

## 6. Cost model

Measured, all on `--model haiku`, on this host:

| Probe | Cost | What it proved |
| --- | --- | --- |
| default system prompt + `--json-schema` | $0.015924 | structured output works; default prompt is expensive |
| `--system-prompt` + stream-json in/out | $0.003325 | stream frames; 4× cheaper |
| MCP tool + structured output | $0.004579 | `mcp__…` naming, `--tools ""`, init `mcp_servers` |
| session turn 1 | $0.004156 | `--session-id` |
| `--resume` turn 2 | $0.002155 | history survives across processes |
| union-schema proposal | $0.003944 | ~70-variant `anyOf` action schema is viable |
| `--safe-mode` + schema | $0.001911 | customization isolation works in print mode |
| **Total** | **$0.036 (7 turns)** | |

Cancelled and unauthenticated probes cost $0. On a subscription these turns consume five-hour
window utilization rather than dollars (`rate_limit_event` reported 0.08 five-hour, 0.07
seven-day at the time). Implication for the plans: **fixtures and a fake transport do all the
behavioural testing; exactly one real Haiku turn is kept as evidence.**

## 7. Limits hit and open questions

1. **No zero-token way to read the `system/init` frame.** Closing stdin on
   `--input-format stream-json` before sending a message produces no output at all, and holding
   stdin open for 45 s produced none either. MCP-attachment readiness can therefore only be
   verified inside a real turn. Mitigation: the app's readiness check uses `claude auth status`
   plus `claude --version`, and records `mcp_servers[].status` from the turn it actually runs.
2. **`claude mcp list` ignores `--mcp-config`/`--strict-mcp-config`.** Do not use it as a probe.
3. **Not tested against `ANTHROPIC_API_KEY`.** This host is an OAuth (Max) login and setting a
   key would shadow it. The API-key path is documented (`apiKeySource` in the init frame) but
   unverified here; the plan treats it as a second, untested-but-supported auth mode and says so.
4. **`--effort` and thinking depth not swept.** Not needed for a proposal turn; noted as a
   future cost lever.
5. **Schema-size ceiling unknown.** The verified union had 2 variants; the real
   `project_action_schema()` has ~70 and is large. Whether the CLI or the API rejects it is the
   single biggest technical risk, so plan 01 makes "send the real schema to the real CLI" an
   early, explicit task with a documented fallback (a trimmed action subset chosen from the
   turn's focus).
6. **`--json-schema` behaviour on refusal or `max_tokens`** not observed. The backend must treat
   a missing `structured_output` as "no proposal", which is already a first-class app state
   (`noProposalCopy`).
7. **Rate limits.** `result` frames and `rate_limit_event` expose utilization and
   `resetsAt`; the backend should surface a plain "usage limit" reason rather than a raw error,
   the same lesson VC-001 recorded for Codex.
8. **macOS keychain inheritance not verified** (no Mac here). On Linux the credential is a file
   and inheritance is automatic. macOS keychain access from a Tauri-spawned child is listed as a
   macOS-only open item.

## 8. Sources

- The installed CLI: `claude --help`, `claude mcp --help`, `claude auth --help`,
  `claude auth status`, `claude --version` (2.1.270), and the seven real turns above.
- `@anthropic-ai/claude-agent-sdk@0.3.276` tarball: `package.json`, `LICENSE.md`, `sdk.d.ts`.
- https://code.claude.com/docs/en/agent-sdk/overview — surface comparison, the
  "run the CLI as a subprocess" guidance, branding, "License and terms".
- https://code.claude.com/docs/en/legal-and-compliance — Commercial/Consumer ToS, the
  preinstall conditions, the authentication-and-credential-use section.
- Bundled `claude-api` docs skill (cached 2026-06-24) — current model ids and per-MTok pricing.
- This repo: `src-tauri/src/codex/**`, `src-tauri/src/settings/agent.rs`,
  `src-tauri/src/project/split.rs`, `src/lib/project.ts`, `src/editor/store/agent-*.ts`,
  `src/editor/panels/ai/**`, `scripts/build-codex-sidecar.mjs`,
  `src-tauri/resources/codex-runtime/provenance.json`, `docs/development/linux.md`.
