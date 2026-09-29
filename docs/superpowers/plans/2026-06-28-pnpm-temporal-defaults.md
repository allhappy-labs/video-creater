# pnpm Temporal Defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix pnpm's pre-script build approval failure and make the Temporal worker part of the default Rust feature set.

**Architecture:** Keep the change at the workspace configuration boundary. pnpm gets an explicit reviewed build policy for esbuild, while Cargo defaults opt into the existing Temporal worker feature without removing the feature itself.

**Tech Stack:** pnpm workspace config, npm scripts, Cargo features, Temporal Rust worker.

---

### Task 1: Approve esbuild Builds in pnpm

**Files:**
- Modify: `pnpm-workspace.yaml`

- [ ] **Step 1: Add the reviewed pnpm build policy**

Set the file to:

```yaml
packages:
  - "."

allowBuilds:
  esbuild: true
```

- [ ] **Step 2: Verify pnpm reaches the package script**

Run:

```bash
rtk pnpm check:temporal-worker
```

Expected before Task 2 is either a Cargo feature/script failure or success. The important regression check for this task is that pnpm no longer exits with `ERR_PNPM_IGNORED_BUILDS`.

### Task 2: Make Temporal Worker Default

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`

- [ ] **Step 1: Include Temporal in default Cargo features**

Change the feature block to:

```toml
[features]
default = ["ges-render", "temporal-worker"]
```

Keep the existing `temporal-worker = [...]` feature dependency list unchanged.

- [ ] **Step 2: Make the pnpm script check the default path**

Change the package script to:

```json
"check:temporal-worker": "cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker"
```

- [ ] **Step 3: Verify default worker compilation**

Run:

```bash
rtk pnpm check:temporal-worker
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --test-threads=1
```

Expected: all commands exit 0.

### Task 3: Review Scope

**Files:**
- Review: `git diff -- pnpm-workspace.yaml package.json src-tauri/Cargo.toml docs/superpowers/specs/2026-06-28-pnpm-temporal-defaults-design.md docs/superpowers/plans/2026-06-28-pnpm-temporal-defaults.md`

- [ ] **Step 1: Confirm there are no generated pnpm placeholders**

Run:

```bash
rtk rg -n "set this to true or false|allowBuilds|onlyBuiltDependencies|ignoredBuiltDependencies" pnpm-workspace.yaml package.json
```

Expected: only `allowBuilds` and `esbuild: true` appear in `pnpm-workspace.yaml`; no placeholder text remains.
