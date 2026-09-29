# Agent, MCP, and Skills Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace static Agent & MCP and Skills copy with bounded self-tests, truthful component health, and narrowly scoped skill repair.

**Architecture:** Rust probes app-server, MCP, validator, and skill integrity independently. External processes use timeouts and are terminated after the handshake. Bundled canonical skill content is checksum-addressed and repairs only the three mandatory skill files.

**Tech Stack:** Rust process/stdio APIs, serde_json, SHA-256, Tauri, React, Vitest.

## Global Constraints

- Temporal worker status is not MCP health.
- A missing Codex executable must not mark the validator or MCP server failed.
- Self-tests must not mutate canonical projects.
- Repair must never overwrite `AGENTS.md`, custom skills, or unrelated files.
- Raw protocol responses and paths stay in diagnostics.

---

## Task 1: Add a reusable bounded process probe

**Files:**

- Create: `src-tauri/src/settings/process_probe.rs`
- Modify: `src-tauri/src/settings/mod.rs`
- Test: `src-tauri/src/settings/process_probe.rs`

- [ ] Write failing tests using fixture child processes for success, timeout,
malformed JSON, early exit, and guaranteed termination.

- [ ] Implement a probe API with explicit deadline and output cap:

```rust
pub struct ProcessProbeRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub stdin_lines: Vec<String>,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}
```

The result records exit state, parsed response lines, bounded stderr, elapsed
milliseconds, and whether the child required termination.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings
rtk git commit -m "feat(settings): add bounded process probes"
```

## Task 2: Implement Codex app-server health

**Files:**

- Modify: `src-tauri/src/codex/app_server.rs`
- Create: `src-tauri/src/settings/agent.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Test: `src-tauri/src/settings/agent.rs`

- [ ] Extract executable resolution and initialize handshake so production
turns and health checks share request/response decoding.

- [ ] Write failing tests for:

- ready initialize response;
- missing executable;
- incompatible protocol;
- timeout;
- launch failure;
- process terminated after the probe.

- [ ] Implement `probe_codex_app_server` using
`codex app-server --stdio`, a five-second default timeout, and
`build_app_server_initialize_request`.

- [ ] Map diagnostic codes:

- `agent.codex.missing`
- `agent.codex.incompatible`
- `agent.codex.timeout`
- `agent.codex.launchFailed`
- `agent.codex.ready`

- [ ] Add `run_agent_component_self_test(component_id)` returning a shared
Settings operation.

- [ ] Commit:

```bash
rtk git add src-tauri/src/codex/app_server.rs src-tauri/src/settings
rtk git commit -m "feat(settings): probe codex app server"
```

## Task 3: Bundle and probe the MCP server

**Files:**

- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `scripts/build-macos-release.mjs`
- Modify: `src-tauri/src/codex/mcp_server.rs`
- Modify: `src-tauri/src/settings/agent.rs`
- Create: `src-tauri/src/settings/fixtures.rs`
- Test: `src-tauri/src/settings/agent.rs`

- [ ] Add `binaries/video-creater-mcp-server` to `externalBin` and release
verification/signature evidence.

- [ ] Materialize a minimal schema-v2 project fixture in a temporary directory;
do not use a user project.

- [ ] Launch the resolved bundled executable with
`["--project-dir", fixture.path().to_string_lossy().as_ref()]`, where `fixture`
is the temporary schema-v2 project created by the test helper.

Send JSON-RPC `initialize`, validate protocol version, server name, resources,
and tools capabilities, then terminate.

- [ ] Write failing tests for missing binary, malformed response, protocol
mismatch, timeout, and success.

- [ ] Add project-aware configuration state without launching against the
project. When `active_project_dir` is absent, return the action `Open a
project`. When present, generate copyable client configuration containing the
bundled executable path and exact project directory.

- [ ] Commit:

```bash
rtk git add src-tauri/tauri.conf.json src-tauri/Cargo.toml scripts/build-macos-release.mjs src-tauri/src/codex src-tauri/src/settings
rtk git commit -m "feat(settings): bundle and verify mcp server"
```

## Task 4: Add deterministic proposal-validator health

**Files:**

- Modify: `src-tauri/src/settings/agent.rs`
- Modify: `src-tauri/src/codex/proposal.rs`
- Test: `src-tauri/src/settings/agent.rs`

- [ ] Build one valid EDL-first fixture and one invalid fixture with a visual
layer outside selected duration.

- [ ] Health is Ready only when the valid proposal passes and the invalid
proposal is rejected with the expected stable issue code.

- [ ] The probe must call `validate_codex_edit_proposal` directly and must not
write project actions.

- [ ] Add provenance for validator schema version and fixture version.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings/agent.rs src-tauri/src/codex/proposal.rs
rtk git commit -m "feat(settings): self test proposal validation"
```

## Task 5: Verify mandatory skills and compiled context

**Files:**

- Create: `src-tauri/src/settings/skills.rs`
- Modify: `src-tauri/src/codex/context.rs`
- Modify: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Test: `src-tauri/src/settings/skills.rs`

- [ ] Define three canonical entries:

```rust
SkillDefinition {
    id: "video-creater-video-pipeline",
    label: "Edit planning and render pipeline",
    relative_path: ".agents/skills/video-creater-video-pipeline/SKILL.md",
    bundled_content: include_str!("../../../.agents/skills/video-creater-video-pipeline/SKILL.md"),
}
```

Repeat for graphics and visuals.

- [ ] Write failing tests for:

- missing file;
- checksum match;
- differing file;
- prompt bundle contains all mandatory skill headings;
- repair backs up differing content;
- repair never touches `AGENTS.md` or a custom skill.

- [ ] Return item provenance with checksum, path, bundled checksum, load state,
and prompt-inclusion state.

- [ ] Implement `repair_bundled_skills(skill_ids)`:

1. validate IDs against the fixed catalog;
2. present affected paths to the frontend before confirmation;
3. write `.backup-20260716T120000Z` beside a differing file in deterministic
   tests and `.backup-{Utc::now().format("%Y%m%dT%H%M%SZ")}` in production;
4. write a temporary file and rename atomically;
5. rerun verification.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings/skills.rs src-tauri/src/codex
rtk git commit -m "feat(settings): verify mandatory skills"
```

## Task 6: Build Agent & MCP and Skills pages

**Files:**

- Create: `src/components/settings/agent-mcp-settings.tsx`
- Create: `src/components/settings/agent-mcp-settings.test.tsx`
- Create: `src/components/settings/skills-settings.tsx`
- Create: `src/components/settings/skills-settings.test.tsx`
- Create: `src/lib/settings/agent.ts`
- Create: `src/lib/settings/skills.ts`
- Modify: `src/components/settings/model-settings.tsx`

- [ ] Write Agent tests proving three separate rows, independent states,
individual self-test actions, no Temporal copy, diagnostics disclosure, and
project-aware MCP configuration.

- [ ] Write Skills tests proving friendly labels, raw IDs only in diagnostics,
Verify action, conditional Repair action, confirmation content, and focus
restoration.

- [ ] Use shared operation rendering. Self-tests and repair must show queued,
running, succeeded, failed, and interrupted states without console-only errors.

- [ ] Run:

```bash
rtk pnpm test -- src/components/settings/agent-mcp-settings.test.tsx src/components/settings/skills-settings.test.tsx
rtk pnpm lint
rtk cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1
```

- [ ] Commit:

```bash
rtk git add src/components/settings src/lib/settings
rtk git commit -m "feat(settings): operationalize agent and skills"
```

## Task 7: Slice verification

- [ ] In the Tauri app prove:

1. app-server self-test reports exact missing/incompatible/ready state;
2. MCP self-test uses the bundled executable;
3. MCP configuration changes when a project is opened;
4. validator self-test remains ready independently;
5. skill verification shows friendly names;
6. a deliberately altered mandatory skill offers a scoped repair and backup.

- [ ] Run `rtk pnpm verify`.
