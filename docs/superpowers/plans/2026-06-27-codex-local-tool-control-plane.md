# Codex Local Tool Control Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a discoverable Rust-owned local tool control plane for Codex/Palmier parity.

**Architecture:** Add a focused `codex::tools` module that owns tool descriptors, schemas, bounded query results, and validation-only calls. Expose the same dispatcher through Tauri commands so future MCP stdio wrapping can reuse the contract without changing semantics.

**Tech Stack:** Rust, Tauri commands, serde/serde_json, existing `ProjectAction` and `CodexEditProposal` validation.

---

### Task 1: Red Tests For The Local Tool Manifest

**Files:**
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing manifest coverage**

Add tests that import `list_codex_local_tools` and assert the stable tool names and categories:

```rust
#[test]
fn local_tool_manifest_exposes_palmier_parity_control_plane() {
    let tools = list_codex_local_tools();
    let names = tools.iter().map(|tool| tool.name.as_str()).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "video_creater.project_context",
            "video_creater.timeline",
            "video_creater.media_library",
            "video_creater.generation_defaults",
            "video_creater.validate_project_actions",
            "video_creater.validate_codex_edit_proposal",
        ]
    );
    assert!(tools.iter().any(|tool| tool.category == "query"));
    assert!(tools.iter().any(|tool| tool.category == "generation"));
    assert!(tools.iter().any(|tool| tool.category == "edit_validation"));
    assert!(tools.iter().all(|tool| tool.input_schema["type"] == json!("object")));
}
```

- [ ] **Step 2: Run test and verify red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_manifest_exposes_palmier_parity_control_plane`

Expected: compile failure because `video_creater_lib::codex::tools` does not exist.

### Task 2: Implement Manifest

**Files:**
- Create: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/src/codex/mod.rs`

- [ ] **Step 1: Add descriptor types and manifest**

Implement `CodexLocalToolDescriptor` and `list_codex_local_tools()` with six stable tool descriptors and JSON object input schemas.

- [ ] **Step 2: Export module**

Add `pub mod tools;` to `src-tauri/src/codex/mod.rs`.

- [ ] **Step 3: Run manifest test and verify green**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_manifest_exposes_palmier_parity_control_plane`

Expected: PASS.

### Task 3: Red Tests For Query And Generation Tools

**Files:**
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Add dispatcher coverage**

Add tests for `video_creater.project_context`, `video_creater.timeline`, `video_creater.media_library`, `video_creater.generation_defaults`, and an unknown tool error.

- [ ] **Step 2: Run tests and verify red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_`

Expected: compile failure or failing tests because dispatcher functions do not exist yet.

### Task 4: Implement Query And Generation Tools

**Files:**
- Modify: `src-tauri/src/codex/tools.rs`

- [ ] **Step 1: Add call result and error types**

Implement `CodexLocalToolCallResult`, `CodexLocalToolError`, and `call_codex_local_tool`.

- [ ] **Step 2: Add bounded query payloads**

Implement project context, timeline, and media-library payload builders with stable JSON keys.

- [ ] **Step 3: Add generation defaults**

Use selected media when `selectedMediaId` is present and valid; otherwise return conservative defaults.

- [ ] **Step 4: Run query tests and verify green**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_`

Expected: query/generation tests pass.

### Task 5: Red Tests For Validation Tools

**Files:**
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Add validation coverage**

Add tests for valid and invalid `video_creater.validate_project_actions`, plus valid `video_creater.validate_codex_edit_proposal`.

- [ ] **Step 2: Run tests and verify red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_validate`

Expected: validation tools return unknown or unsupported.

### Task 6: Implement Validation Tools

**Files:**
- Modify: `src-tauri/src/codex/tools.rs`

- [ ] **Step 1: Validate project actions on a clone**

Deserialize `actions`, apply each action to a cloned project, return `valid`, `actionCount`, and project counts.

- [ ] **Step 2: Validate Codex edit proposals**

Deserialize `request` and `proposal`, call `validate_codex_edit_proposal`, and return `valid`, `clipCount`, and `durationSeconds`.

- [ ] **Step 3: Run validation tests and verify green**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_validate`

Expected: PASS.

### Task 7: Expose Tauri Commands

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add command wrappers**

Add `list_codex_local_tools_command` and `call_codex_local_tool_command` wrappers around the Rust API.

- [ ] **Step 2: Register commands**

Add wrappers to `tauri::generate_handler!`.

- [ ] **Step 3: Run Rust tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server local_tool_`

Expected: PASS.

### Task 8: Final Verification

**Files:**
- Verify only

- [ ] **Step 1: Run focused Rust test suite**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server`

Expected: PASS.

- [ ] **Step 2: Check formatting**

Run: `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check`

Expected: PASS.
