# Linux X11 Probe Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the successful static-musl X11 protocol proof reproducible from checked-in source, with one bounded verification command and retained evidence.

**Architecture:** Promote the existing throwaway probe into a standalone development crate outside the Tauri workspace. A Node runner starts a private Xvfb using `-displayfd`, executes the probe, collects ELF/process evidence, and always cleans up its own processes. This is a development acceptance harness, not the production editor or an app runtime dependency.

**Tech Stack:** Rust, pinned x11rb 0.13.2 with default features disabled, x86_64-unknown-linux-musl; Node built-ins; development Xvfb and readelf.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- No LGPL dependencies, including libraries linked/loaded by application-owned processes.
- Independent OS services are permitted; private Xvfb is a development service.
- No system installation, privileged operations, host mounts, TCP listeners, or changes to existing display sessions.
- Preserve existing ignored spike artifacts and macOS/Tauri behavior.
- All new dependencies stay isolated from the application workspace and package manifest.
- Sol implementation and independent review, TDD for new behavior, Conventional Commits.
- An ELF/static proof is not a license audit, hardware test, or packaged-release result.

## Scope

The existing source is `output/linux-host-spike/src/main.rs`, with its manifest and
lockfile alongside it. Read the corresponding X11 research report before copying.
Promote only source, locked dependency metadata, and reproducible commands. Do not
copy binaries, screenshots, captures, source registries, or downloaded services.
Wayland/audio probes and native editor design remain separate work.

### Task 1: Reproducible static X11 probe and bounded runner

**Files:**
- Create: `native/linux-host-probe/Cargo.toml`, `Cargo.lock`, `src/main.rs`.
- Create: `native/linux-host-probe/README.md` — prerequisites, commands, exact scope.
- Create: `scripts/verify-linux-host-probe.mjs` — orchestration and evidence only.
- Create: `scripts/verify-linux-host-probe.test.ts` — validation and cleanup failures.

**Interfaces:** The Rust command retains `--state-dir DIR --open FILE` and `--probe DIR`.
Runner CLI is `node scripts/verify-linux-host-probe.mjs --binary ABSOLUTE_FILE --output NEW_DIRECTORY`.
Exit 0 only after static ELF checks, live mappings, exact pixel/input/file checks and
client exit 0; nonzero for failed/missing prerequisites. Never print a license-pass claim.
Output includes raw readelf output, binary SHA256, process maps, separate child logs,
probe state/capture and a JSON summary with explicit synthetic/provenance limitations.

- [x] **Step 1:** Copy the probe source/manifest/lockfile into the standalone crate,
  retaining exact x11rb pin/features and standalone `[workspace]`. Rename only the
  local package entry in the lockfile. Run the original build with `--locked` before
  changing behavior to establish a reproducible baseline.
- [x] **Step 2:** Write regression tests before fixing two observed weaknesses:
  frame color arithmetic must not overflow after repeated input, and probe pixel
  comparisons must assert the expected cyan/orange/cyan sequence. Extract a small
  `accent_pixel(interaction: u8) -> u32`/pixel-buffer function if needed and test the
  actual buffer location across all 256 interaction states, not only a helper result.

  ```rust
  // With the extracted frame builder used by draw(), this regression must run
  // in debug mode and never panic for any representable interaction state.
  for interaction in 0..=u8::MAX {
      let frame = frame_pixels(interaction, "fixture");
      let offset = (80 * 640 + 55) * 4;
      let actual = u32::from_le_bytes(frame[offset..offset + 4].try_into().unwrap());
      assert_eq!(actual, if interaction % 2 == 0 { 0x002dd4bf } else { 0x00fb923c });
  }
  ```

- [x] **Step 3:** Fix overflow using bounded/wrapping arithmetic; require the known
  Xvfb image depth/format or fail clearly. Replace fixed 120ms input sleeps with
  polling for the expected pixel under a fixed deadline. Check image reply length
  before indexing. Keep CPU pixels and synthetic X11 events; do not add a UI toolkit.
- [x] **Step 4:** Write runner tests RED before implementation. A pre-existing output
  directory containing a sentinel must be rejected and preserved. Missing binary
  must fail without launching a display. A fake Xvfb executable that never reports
  readiness must time out and be terminated; use a private temporary PATH fixture
  only in the test process. Assert no process survives and unrelated output remains.
  The runner may export `verifyHostProbe({binary, output, timeoutMs})` for focused
  bounded tests, while CLI uses a fixed reasonable timeout of 30 seconds per phase.
- [x] **Step 5:** Implement the Node runner with argument-array child spawning and
  no shell execution. Refuse pre-existing output; check Linux/x86_64 and binary file;
  create a known text fixture and separate logs. Spawn `Xvfb -displayfd 3 -screen 0
  800x600x24 -nolisten tcp`; pass its allocated DISPLAY only to owned clients.
  Capture `readelf -lW` and `readelf -dW`; require ELF x86_64 with no INTERP, NEEDED,
  RPATH or RUNPATH, retaining output. While ready, save `/proc/PID/maps` and reject
  file-backed mappings other than the canonical probe executable. Run the `--probe`
  mode, verify file-read evidence/expected pixel sequence/window destruction and
  clean exit. In `finally`, terminate and reap only children spawned by this run;
  never delete an X socket manually or touch other processes.
- [x] **Step 6:** Run focused Node tests and Rust tests. Build release musl `--locked`,
  then run the real runner against private Xvfb with a fresh output directory. Record
  source commit, tool versions, binary hash, command/results and limitations in the
  ignored task report. Do not overwrite earlier spike artifacts or evidence.
- [x] **Step 7:** Document exact build/run commands and that Xvfb/readelf/Cargo/Node
  are development prerequisites, not app payload requirements. State supported
  synthetic visual format, input limits, toolchain provenance gap and real-desktop
  work still required. Run `git diff --check`, commit only task files, obtain Sol
  spec and quality review, and resolve findings through the implementer.

## Execution record

Controller records task status after fresh implementation and review evidence.

Completed in `c33e97ae` with reviewed signal-cleanup repair `c2c1437b`.
Initial review reproduced orphaned children after targeted CLI termination;
SIGINT/SIGTERM now reach bounded cleanup with exit130/143. Independent scoped
re-review found the defect resolved and no collateral findings. Controller
combined Node gate passed36/36; Rust probe test passed1/1 and actual private
Xvfb smoke passed. These are development-probe results, not a Linux editor release.
