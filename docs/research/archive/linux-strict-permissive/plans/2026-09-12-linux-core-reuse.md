# Linux Core Reuse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** Complete in `e24a3896`; scoped independent spec and quality review
accepted the task. The result proves a locked, default-feature-disabled musl
consumer can persist and reopen the canonical fixture. It does not qualify the
dependency closure or prove a Linux editor, package, or Mac build.

**Goal:** Prove that the existing Rust project/storage core can build and execute under musl without pulling in the desktop UI stack.

**Architecture:** Make the existing dialog plugin belong to the existing `app-runtime` feature, then exercise the real library through a standalone development probe with default features disabled. No project model is copied and no unavailable subsystem is replaced with a stub. Existing desktop defaults continue to enable the plugin.

**Tech Stack:** Existing Rust library, Cargo features, standalone locked x86_64-unknown-linux-musl probe.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- No LGPL application-linked/loaded library exemption, including system libraries.
- Preserve existing desktop default features and behavior; no claimed macOS build or package verification on Ubuntu.
- No production media backend replacement before its separate codec/container gates pass.
- Use the actual canonical Rust model/storage APIs, not copied schemas or mock backends.
- Sol implementation/review, TDD, pinned probe lockfile, no new production dependency or broad refactor.
- Do not certify all library dependencies or a Linux editor from a project-file roundtrip.
- Preserve unrelated work and earlier ignored artifacts; no system installation, mounts, network listeners, push, or merge.

## Baseline evidence and completed result

`cargo tree --manifest-path src-tauri/Cargo.toml --locked -p video-creater --no-default-features --target x86_64-unknown-linux-musl --edges normal --invert tauri-plugin-dialog`
showed `tauri-plugin-dialog 2.7.1` as a direct dependency of the library before
`e24a3896`.
Its usage belongs to the existing desktop executable; `src/lib.rs` does not
include the desktop entrypoint or `native_menu` module. This is the concrete
isolation failure that the completed task repaired.

The standalone locked normal musl graph now excludes Tauri, the dialog plugin,
GTK, WebKit, and GStreamer while the existing default feature list and default
dialog ownership remain enabled. Five locked musl tests passed, and the real CLI
saved and reopened the complete typed fixture. The Rustup target inputs and full
linked crate/source closure remain unqualified, and native macOS checks were not
run on this Ubuntu host.

### Task 1: Isolate desktop dialog dependency and execute real project persistence

**Files:**
- Modify: `src-tauri/Cargo.toml` — only dialog dependency feature ownership.
- Create: `native/linux-core-probe/Cargo.toml`, `Cargo.lock`, `src/main.rs`, `README.md`.
- Modify only if Cargo requires metadata refresh: `src-tauri/Cargo.lock`; no opportunistic upgrades.

**Interfaces:** Use `video_creater_lib::project::fixtures::sample_project()`,
`project::storage::save_project(&Path, &VideoProject)` and
`project::storage::load_project(&Path)`. Probe accepts exactly one new output
directory, refuses an existing directory, saves/reopens the sample and compares
the complete typed model, prints a clear narrow success only when equal, and exits
nonzero on I/O/schema/mismatch errors. It does not launch app helpers or write a
user project. Tests use private temporary paths and remove only their own files.

- [x] **Step 1:** Capture the current locked normal target graph demonstrating the
  unexpected dialog/Tauri stack with default features disabled. Create the standalone
  probe manifest pointing to the actual library with `default-features = false`.
  Record the failing musl build or failing no-desktop dependency assertion before
  changing the application manifest.
- [x] **Step 2:** Change exactly the existing feature/dependency entries:

  ```toml
  app-runtime = ["dep:tauri", "dep:tauri-plugin-dialog"]
  tauri-plugin-dialog = { version = "2", optional = true }
  ```

  Keep the existing default feature list and all other production dependencies.
  Verify the non-default normal musl graph excludes tauri-plugin-dialog, Tauri,
  GTK/WebKit and GStreamer; verify the default desktop graph still enables the
  dialog plugin. Cargo graph inspection is not native macOS verification.
- [x] **Step 3:** Write RED tests for the probe's observable new-directory roundtrip
  and pre-existing-directory refusal before implementing its orchestration:

  ```rust
  let original = video_creater_lib::project::fixtures::sample_project();
  video_creater_lib::project::storage::save_project(&output, &original)?;
  let reopened = video_creater_lib::project::storage::load_project(&output)?;
  assert_eq!(reopened, original);
  ```

  The actual test must exercise the probe entry function as well as verify the
  persisted model. An existing directory with a sentinel must remain untouched.
  Unknown/missing CLI arguments must exit nonzero without creating project files.
- [x] **Step 4:** Implement the small probe using those existing APIs, with explicit
  Linux-proof limitations. Do not duplicate project structs or introduce dummy
  runtime modules. If the real library cannot compile after the feature fix, stop
  at the exact failure and ask the controller for a scoped repair plan.
- [x] **Step 5:** Run locked probe tests, release musl build, and the real CLI against
  a new ignored output path. Retain the generated project, binary hash, ELF output,
  target normal dependency tree, and final link map. Report any known nonpermissive
  or unclassified static input; no license-eligible or cross-platform roundtrip claim.
- [x] **Step 6:** Document exact build/test/run commands and observed scope. Run
  `git diff --check`, commit only task files, and obtain independent Sol spec and
  quality review. Relevant existing macOS package checks remain unverified here.
