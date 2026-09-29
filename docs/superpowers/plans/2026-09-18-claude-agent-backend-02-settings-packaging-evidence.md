# Claude Agent Backend 02 — Settings, Readiness, Docs and Evidence Implementation Plan

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

**Goal:** Let a user choose and trust an agent backend.

- A persisted `agentBackend` preference (`Automatic` / `Codex` / `Claude`) defaulting to
  Automatic, which resolves to whichever agent is ready. **Amended 2026-09-18: Automatic
  prefers Claude when both are ready.**
- A zero-token `agent.claude` readiness component with plain messages for "not installed",
  "not signed in", "signed in", "using an API key" and "didn't start". **Amended 2026-09-18:
  "signed in" is split into "signed in with your Claude subscription" and "signed in, but not
  with a Claude subscription", and the sign-in instruction points at `/login` rather than at an
  API key.**
- A new **Advanced → Agent** settings section holding the backend select, the Claude model
  select, an optional executable path, and both readiness rows.
- Plain-language failure copy in the AI tab for the Claude-specific failures, and a deep link
  from `MissingAgentState` to the new section.
- Docs and the backlog row.

**Spec:** `docs/superpowers/specs/2026-09-18-claude-agent-backend-design.md`, decisions 4 and
11–16.
**Research of record:** `docs/research/2026-09-18-claude-agent-backend.md` (the `claude auth
status` JSON, the verified unauthenticated failure shape, the licence quotes).
**Backlog row:** VC-029.
**Depends on:** plan 01 task 2 (`AgentBackendKind`, `resolve_agent_backend`) for task 1, and
plan 01 task 4 (`resolve_claude_executable`) for task 2. Tasks 3–6 depend on tasks 1 and 2.

**Tech Stack:** Rust (Tauri 2, serde, the existing `AppPreferencesStore` and
`run_process_probe`), React 19 + Zustand, Vitest, Playwright.

## Global Constraints

- **Commands.** Prefix every shell command with `rtk`. `rg` isn't installed, so use
  `rtk grep -rn`. Run long commands in the foreground.
- **Commits.** Conventional Commits, with the trailer
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`. Stage only the task's
  files. Never push.
- **Cargo environment.** `{CARGO_ENV}` = `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
- **No tokens.** Every task in this plan is zero-cost. `claude --version` and
  `claude auth status` make no API call. If a step seems to need a turn, it belongs in plan 01.
- **Rust/TS lockstep.** `AppPreferencesV2` is `deny_unknown_fields`, so a new preference field
  must land in Rust and TS **in the same commit**, together with
  `src/lib/settings-visual-qa-fixtures.ts`.
- **UI styling.** Tokens only. No hex colors, no `white/` or `black/` class fragments. Copy the
  existing Execution `<select>` in `advanced-settings.tsx` rather than inventing a control.
- **Naming.** Settings may say "Claude". Nothing may say "Claude Code" as a feature name, no
  Anthropic logo, no Claude Code-styled visuals (Anthropic's branding guidance, quoted in the
  research note). The existing ban on the literal `Codex` in `src/editor/**` user-visible
  strings still applies; this plan adds its strings under `src/components/settings/**`, where
  "Codex" is already used.
- **Licence policy.** Nothing bundled. No `@anthropic-ai/*` dependency. No `externalBin` entry,
  no provenance row, no `REQUIRED_PAYLOAD` row, no macOS codesign block. If a step seems to need
  one, the design decision was misread.
- **Evidence.** Never claim evidence you didn't observe.

## File Map

### Modified — Rust
- `src-tauri/src/settings/preferences.rs`: `AgentBackendPreference` enum, the field on
  `AppPreferencesV2`, its default, and the field on `AppPreferencesPatch`.
- `src-tauri/src/settings/agent.rs`: `CLAUDE_COMPONENT_ID`, `probe_claude_cli`,
  `probe_claude_cli_with`, the `agent.claude` diagnostic codes, and the self-test arm.
- `src-tauri/src/settings/health.rs`: the `agent.claude` row in `agent_category_health`.

### Modified — TypeScript
- `src/lib/app-settings.ts`: the `AgentBackend` union, the `AppPreferences` field, the default.
- `src/lib/settings-visual-qa-fixtures.ts`: the new field on the fixture preferences object.
- `src/lib/settings/target.ts`: `"agent"` added to the advanced `item` union, plus its label.
- `src/components/settings/advanced-settings.tsx`: the new Agent section, above the MCP section.
- `src/editor/panels/ai/missing-agent-state.tsx`: the deep link target and the copy.
- `src/editor/store/agent-turn-actions.ts`: the two new failure patterns.

### New
- `src/components/settings/agent-backend-settings.tsx` and its `.test.tsx`.

### Docs
- `docs/development/agents.md` (new, or a new section in `docs/development/linux.md` if that file
  is the better home — check first).
- `docs/product-backlog.md`: the VC-029 row moves from `planned` to `implemented`.
- `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md`: one evidence line.

---

## Task 1 — The `agentBackend` preference, Rust and TS in lockstep

**Owns:** `src-tauri/src/settings/preferences.rs`, `src/lib/app-settings.ts`,
`src/lib/settings-visual-qa-fixtures.ts`.

`generationExecutionBackend` is the exact existing template; copy its shape in all four Rust
spots and all three TS spots.

- [ ] Write the failing Rust tests first, in `preferences.rs`'s `mod tests`:
  - a default `AppPreferencesV2` has `agent_backend == AgentBackendPreference::Automatic`;
  - a stored JSON without the key deserializes to `Automatic` (`#[serde(default)]`);
  - `{"agentBackend":"claude"}` deserializes to `Claude` and re-serializes to the same camelCase
    string;
  - an unknown value is a deserialize error, not a silent default;
  - `AppPreferencesPatch { agent_backend: Some(Claude), .. }` applied by
    `AppPreferencesStore::update` round-trips through a temp dir and back.
- [ ] Add `pub enum AgentBackendPreference { Automatic, Codex, Claude }` next to `GenerationExecutionBackend`, with the same derives and `#[serde(rename_all = "camelCase")]`.
- [ ] Add the field to `AppPreferencesV2`, to `impl Default`, and to `AppPreferencesPatch`. Do **not** bump `APP_PREFERENCES_SCHEMA_VERSION`; the field is additive and defaulted. Do not add it to `LegacyAppPreferencesV1` — there is nothing to migrate.
- [ ] Add `type AgentBackend = "automatic" | "codex" | "claude";` to `src/lib/app-settings.ts`, the `agentBackend: AgentBackend` field on `AppPreferences`, and `agentBackend: "automatic"` to `defaultAppPreferences`. `AppPreferencesPatch` and `AppPreferenceIntent` are derived and need no edit.
- [ ] Add `agentBackend: "automatic"` to the full preferences object in `src/lib/settings-visual-qa-fixtures.ts` — the object must stay a complete `AppPreferences` or the fixture fails to type-check.
- [ ] Map `AgentBackendPreference` to plan 01's `Option<AgentBackendKind>` in one small function next to `resolve_agent_backend`'s caller: `Automatic → None`, `Codex → Some(Codex)`, `Claude → Some(Claude)`. Test the three arms.
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib settings::preferences -- --test-threads=1`
- [ ] `rtk pnpm lint && rtk pnpm test`

**Commit:** `feat(settings): add an agent backend preference defaulting to automatic`

---

## Task 2 — The `agent.claude` readiness component

**Owns:** `src-tauri/src/settings/agent.rs`, `src-tauri/src/settings/health.rs`.

Two zero-token probes, both through the existing `run_process_probe`:
`claude --version` and `claude auth status`. The `auth status` JSON was verified in research §2.7.

- [ ] Write the failing tests first, in `agent.rs`'s `mod tests`, against a **stub executable**
      written to a temp dir (never the real binary) so the suite stays hermetic and free:

  | Stub behaviour | Expected `SettingsComponentHealth` |
  | --- | --- |
  | prints `2.1.270 (Claude Code)`; `auth status` prints `{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max","email":"a@b.c"}` | `Ready`, summary names the version and the email, `diagnostic_code: Some("agent.claude.ready")` |
  | version ok; `auth status` prints `{"loggedIn":false}`, no `ANTHROPIC_API_KEY` | `ActionRequired`, summary "Claude is installed but not signed in. Run `claude auth login` in a terminal, or set an Anthropic API key.", code `agent.claude.notLoggedIn`, `action_id: Some("agent.claude.selfTest")` — **amended 2026-09-18 to "Sign in to Claude with your subscription: run `claude` in a terminal and use /login."** |
  | version ok; `auth status` prints `{"loggedIn":false}`; `ANTHROPIC_API_KEY` set in the probe env | `Ready`, summary "Claude will use the Anthropic API key from the environment.", code `agent.claude.apiKey` — **amended 2026-09-18 to name the subscription as the alternative** |
  | no executable anywhere | `NotConfigured`, summary "Claude isn't installed. Install it from claude.com, then reopen settings.", code `agent.claude.missing` |
  | executable exists but exits non-zero on `--version` | `Failed`, code `agent.claude.launchFailed`, `diagnostic_detail` carries the captured stderr tail |
  | `auth status` prints non-JSON | `Failed`, code `agent.claude.malformedStatus` |

  Also assert `provenance` records the resolved executable path, as the Codex component does.
- [ ] Implement `probe_claude_cli()` and a testable `probe_claude_cli_with(executable: &Path, timeout: Duration, api_key_present: bool)`. Reuse `CODEX_PROBE_TIMEOUT`'s 5 s value as `CLAUDE_PROBE_TIMEOUT`, and the same 64 KiB stdout/stderr caps. Executable resolution comes from plan 01's `resolve_claude_executable`, including the optional user-set path.
- [ ] Add the `agent.claude` row to `agent_category_health` in `health.rs`, after `agent.codex`. Update the existing category test's expected item list and ordering.
- [ ] Add the `agent.claude.selfTest` arm to `run_agent_component_self_test`, re-running both probes. Assert in a test that the arm exists and returns a `SettingsOperation`.
- [ ] Assert in a test that the whole category stays `Ready` when **either** agent is ready — an app with only Claude installed must not show a broken Agent category. This is the row that encodes spec decision 4's "defaults to whichever is available".
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib settings::agent -- --test-threads=1`
- [ ] `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib settings::health -- --test-threads=1`
- [ ] Note in the run notes: `settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe` is a known environment-only failure without a staged MCP sidecar. Report it as observed, not as a regression.

**Commit:** `feat(settings): report Claude CLI readiness without spending tokens`

**Parallel with:** task 1. Different files.

---

## Task 3 — The Advanced → Agent settings section

**Owns:** `src/components/settings/agent-backend-settings.tsx` (new), its `.test.tsx` (new),
`src/components/settings/advanced-settings.tsx` (modify), `src/lib/settings/target.ts` (modify).

- [ ] Add `"agent"` to `AppSettingsTarget`'s advanced `item` union and an
      `appSettingsTargetLabel` entry `"Agent settings"`. Update the target-label test.
- [ ] Write the failing component tests first, in `agent-backend-settings.test.tsx`:
  - the section renders `data-settings-target="advanced:agent"`;
  - the backend select shows exactly `Automatic`, `Codex`, `Claude`, reflects the passed
    preference, and calls `onChange({ agentBackend: "claude" })` on selection;
  - with `agentBackend: "automatic"` and only Claude ready, a resolved line reads
    "Automatic is using Claude." — and with only Codex ready, "Automatic is using Codex.";
  - with `agentBackend: "claude"` and Claude not ready, a warning line reads
    "Claude isn't ready, so turns will use Codex." (and the mirror case);
  - with neither ready, "No agent is available. Install Claude or Codex.";
  - the Claude model select shows `Sonnet`, `Haiku`, `Opus`, defaults to Sonnet, and is disabled
    when the resolved backend is Codex;
  - the executable path field is optional, trims whitespace, and shows the resolved path from the
    readiness row's `provenance` as placeholder text;
  - both readiness rows render their summary and their action button, and the action calls the
    self-test;
  - no hex color and no `white/`/`black/` fragment appears in the file (the repo's source-quality
    gate also checks this).
- [ ] Implement the component. Copy the Execution select's markup and tokens verbatim from
      `advanced-settings.tsx` L155–176. Keep the file under 300 lines.
- [ ] Render `<AgentBackendSettings … />` in `advanced-settings.tsx` immediately **above**
      `<AgentMcpSettings />`, so the order reads: Execution, Agent, MCP client configuration,
      System Health, Recovery.
- [ ] `rtk pnpm test src/components/settings/agent-backend-settings.test.tsx`
- [ ] `rtk pnpm lint && rtk pnpm check:source-quality`

**Commit:** `feat(settings): add an Agent section with backend choice and readiness`

---

## Task 4 — AI tab: failure copy and the settings deep link

**Owns:** `src/editor/panels/ai/missing-agent-state.tsx`,
`src/editor/store/agent-turn-actions.ts`, and the existing tests that cover them.

- [ ] Write the failing tests first:
  - `MissingAgentState` links to `{ category: "advanced", item: "agent" }`, not `"mcp"`;
  - its copy says "No AI agent is available." and "Open Agent settings", and names no product;
  - a rejected turn whose message matches the new `usageLimitPattern` classifies as
    `agentUnavailable` with the copy "The AI agent's usage limit is reached. Try again later.";
  - a rejected turn whose message matches `notSignedInPattern` classifies as `agentUnavailable`
    with "The AI agent isn't signed in. Open Agent settings to fix it.";
  - the existing `interruptedPattern`, `staleApplyPattern`, `reviewRequiredPattern` and
    `alreadyActivePattern` behaviours are unchanged;
  - `src/editor/panels/ai/ai-panel.test.tsx`'s "never renders internal ids" test still passes.
- [ ] Implement. Add the two patterns next to the existing ones; match on the stable wording plan
      01's `AgentTurnError::{UsageLimit, NotAuthenticated}` map to, not on a provider's raw text.
- [ ] `rtk grep -rn "Claude\|Codex" src/editor` must return nothing after this task.
- [ ] `rtk pnpm test src/editor/panels/ai && rtk pnpm test src/editor/store`

**Commit:** `feat(ai): point an unavailable agent at Agent settings with plain reasons`

---

## Task 5 — Browser flow coverage

**Owns:** `e2e/` (one new spec or an addition to the existing settings spec — check which exists
first with `rtk ls e2e`).

- [ ] Write the failing Playwright flow: open App settings → Advanced, find the Agent section by
      its `data-settings-target`, switch the backend select to Claude, reload, and assert the
      selection persisted. Use the existing fixture transport; no real agent is involved.
- [ ] Add a second assertion: with the fixture reporting no agent ready, the AI tab's composer
      shows the unavailable state and its button opens Advanced → Agent.
- [ ] `rtk pnpm test:browser`

**Commit:** `test(e2e): cover choosing an agent backend in settings`

---

## Task 6 — Docs and the backlog row

**Owns:** `docs/development/agents.md` (new), `docs/product-backlog.md` (one row),
`docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md` (one line).

- [ ] `rtk git pull --rebase` before touching `docs/product-backlog.md`; other work lands rows
      there.
- [ ] Write `docs/development/agents.md`: the two supported backends, why Claude is not bundled
      (quote the two conditions from Anthropic's legal page and this repo's licence rule), the
      exact flags the Claude transport passes and why each one is there, the read-only MCP tool
      allowlist, how readiness is probed without spending tokens, how sessions map, and how to
      re-run the retained real turn. Link the research note and the spec. Keep it under 200 lines.
- [ ] Update the VC-029 row in `docs/product-backlog.md`: status `planned` → `implemented`, and a
      "Next action" that states exactly what is still unproven — a packaged native run, the
      `ANTHROPIC_API_KEY` auth mode, and macOS keychain inheritance. Bump `Last updated:`.
      **Amended 2026-09-18: the `ANTHROPIC_API_KEY` mode is no longer an open item.**
- [ ] Add one line to the editor-redesign hand-off recording the retained real turn's path and
      cost.
- [ ] `rtk pnpm test:source-quality` (the docs and policy gate).

**Commit:** `docs(agents): document the Claude backend, its flags and its licence stance`

---

## Parallelization Notes

- Tasks 1 and 2 are independent and can run at the same time; both are prerequisites for task 3.
- Task 4 depends only on task 3's `AppSettingsTarget` change; once that has landed, tasks 4 and 5
  can run together.
- Task 6 runs last, so the backlog row describes what actually landed.
- This whole plan can run beside plan 01 tasks 5–10, as long as plan 01 task 2 has landed.

## Acceptance

Done when all of the following were observed and reported:

1. `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib settings:: -- --test-threads=1` passes, with the count, and with the known MCP-sidecar environment failure reported as observed rather than as a regression.
2. `rtk pnpm verify:frontend` passes.
3. A stub-executable readiness test exists for each of the six rows in task 2's table, and each asserts the exact user-facing sentence.
4. Switching the preference to Claude and back persists across a reload in the Playwright flow.
5. With only Claude ready, the Agent settings category is `Ready` and the resolved line says Automatic is using Claude. With neither ready, the AI tab shows the unavailable state and its button opens Advanced → Agent.
6. `rtk grep -rn "Claude\|Codex" src/editor` returns nothing.
7. `rtk grep -rn "anthropic" package.json src-tauri/tauri.conf.json src-tauri/tauri.linux.conf.json scripts/build-linux-release.mjs scripts/build-macos-release.mjs` returns nothing: nothing was bundled and no packaging manifest gained a row.
8. `docs/development/agents.md` exists, and the VC-029 backlog row states what is still unproven.
9. Everything not run is reported as not run, with the reason — at minimum: the packaged `.deb` run and macOS keychain inheritance by a Tauri-spawned child. (The `ANTHROPIC_API_KEY` auth mode was closed on 2026-09-18.)
