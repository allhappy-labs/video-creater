# Codex MCP Tool Server Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide full backend MCP/tool-server parity for Video Creater's Codex integration.

**Architecture:** Expand `codex::tools` into a broad Rust-owned control plane, then add a focused `codex::mcp_server` JSON-RPC adapter that maps MCP `tools/list` and `tools/call` to the same dispatcher. A small binary handles stdio and project-dir loading, while Tauri commands continue to reuse the same tool functions.

**Tech Stack:** Rust, serde/serde_json, Tauri commands, split-project storage, existing Temporal start-request builders, JSON-RPC over stdin/stdout.

---

### Task 1: Expanded Tool Surface Tests

**Files:**
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing manifest and tool behavior tests**

Add tests for new tools:

- `video_creater.generated_assets`
- `video_creater.workflow_jobs`
- `video_creater.render_reports`
- `video_creater.export_artifacts`
- `video_creater.export_profiles`
- `video_creater.transcription_readiness`
- `video_creater.build_generate_media_start_request`
- `video_creater.build_codex_edit_start_request`
- `video_creater.build_export_media_start_request`
- `video_creater.build_export_nle_xml_start_request`
- `video_creater.apply_project_actions`

- [ ] **Step 2: Run red tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server mcp_parity_tool`

Expected: failures because new tools do not exist.

### Task 2: Expanded Tool Dispatcher

**Files:**
- Modify: `src-tauri/src/codex/tools.rs`

- [ ] **Step 1: Add descriptor entries and argument structs**

Add tool descriptors and typed args for each new tool.

- [ ] **Step 2: Implement read-only query payloads**

Return bounded JSON for generated assets, jobs, render reports, and export artifacts.

- [ ] **Step 3: Implement operation-prep tools**

Use existing Temporal start-request builders and export-profile/transcription validation helpers.

- [ ] **Step 4: Implement split-project apply tool**

Call `apply_project_actions_to_split_project` using an absolute `projectDir`.

- [ ] **Step 5: Run focused tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server mcp_parity_tool`

Expected: PASS.

### Task 3: MCP Server Tests

**Files:**
- Create: `src-tauri/tests/codex_mcp_server.rs`

- [ ] **Step 1: Write JSON-RPC adapter tests**

Test `initialize`, `tools/list`, `tools/call`, unknown method, and tool error responses against a temp split project.

- [ ] **Step 2: Run red tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server`

Expected: compile failure because `codex::mcp_server` does not exist.

### Task 4: MCP Server Library And Binary

**Files:**
- Create: `src-tauri/src/codex/mcp_server.rs`
- Create: `src-tauri/src/bin/video-creater-mcp-server.rs`
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Implement JSON-RPC handlers**

Add `handle_mcp_request(project, request)` and response helpers.

- [ ] **Step 2: Implement stdio loop**

Read newline-delimited JSON-RPC requests from stdin, load the split project for each request, and write one JSON response per line.

- [ ] **Step 3: Register binary**

Add `video-creater-mcp-server` bin to Cargo.

- [ ] **Step 4: Run MCP server tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server`

Expected: PASS.

### Task 5: Final Verification

**Files:**
- Verify only

- [ ] **Step 1: Format check**

Run: `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check`

Expected: PASS.

- [ ] **Step 2: Full Rust suite**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS.
