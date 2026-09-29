# Gap Closure 06 — Native Linux Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Every step names its files, its command and its expected observation.

**Goal:** Prove the redesigned editor on this Linux host with native runs recorded under `output/gap-closure-06/`:
- an audited packaged `.deb`, and the desktop smoke run against the app extracted from it;
- the Temporal smoke steps (`--temporal`) against a local Temporal dev server;
- the MCP sidecar self-tests;
- the Secret Service credential step with a private GNOME Keyring (`--keyring-root`);
- the audio denoise step, re-run with its evidence retained;
- a GTK-level Ctrl+Z check and a native menu Edit → Undo check, driven by `xdotool`;
- AI flows 1 (safe edit auto-applies with result frames, Show changes, Undo) and 2 (review edit, then Dismiss) through the real Codex app-server, with result frames observed on Linux.

The hand-off and backlog rows VC-001 and VC-025 are then updated with observed results only.

**Architecture:**
- **Smoke script split.** `scripts/linux-desktop-smoke.mjs` is 847 lines. It is split into side-effect-free modules (driver, selectors, step runner, retention, tools, Temporal, native menu, agent flows) so every file stays under 600 lines and the pure parts are unit tested with `node --test`.
- **Honest step statuses.** The step runner records `passed`, `failed` or `skipped` (with a reason). Today the Secret Service step records `passed` when it was skipped. `evidence.json` gains a `summary` and a `runContext` (app kind, commit, package hash, `VIDEO_CREATER_*` overrides, `--only` patterns).
- **Retained evidence.** Files the checks rely on (render pipeline reports, denoise manifests, result-frame PNGs) are copied from the temporary project into the run's output folder, so the evidence outlives `/tmp`.
- **New opt-in flags.** `--native-menu --xdotool-root <dir> [--gst-tools-root <dir>]`, `--agent-flows`, `--release-report <report.json>`. When a flag is absent, its steps are recorded as `skipped` with the flag named in the reason.
- **Native runs.** The runs share Xvfb display `:94`, WebDriver port 4444, `/tmp/video-creater-editor-project` and the app preferences, so they run one at a time.

**Tech Stack:** Node 24 (`node --test`, type-stripped `.test.ts`), WebDriver through `tauri-driver` + `WebKitWebDriver`, Xvfb, `dbus-run-session`, GNOME Keyring, `xdotool`, the Temporal CLI dev server, Tauri 2.11 `.deb` bundling, Rust `cargo test`.

**Spec:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md` (workstream 06; Decisions 2, 3, 4 and 5 bind this plan).
**Depends on:** gap-closure plans 01–05 (`docs/superpowers/plans/2026-09-16-gap-closure-0[1-5]-*.md`) have landed on `main`. Do not start Track C before they have.

## Design refinements (recorded for review)

1. **Temporal steps run against the debug build, not the package.** Linux release builds deliberately leave out the `temporal-worker` feature: `RELEASE_FEATURES` in `scripts/build-linux-release.mjs` is `app-runtime,custom-protocol,ges-render,gpu-render,graphics-render`, and `scripts/linux-release-policy.test.ts` asserts `doesNotMatch(RELEASE_FEATURES, /coreml|temporal/)`. The packaged app therefore starts no workflows, and the packaged `video-creater-temporal-worker` is built without its `main`. The Temporal evidence is labelled `appKind: "debug"`. Everything else runs against the package.
2. **The Codex run uses the app's bundled sidecar with the existing auth.** The app always spawns `video-creater-codex` next to its executable (`bundled_codex_app_server_command` in `src-tauri/src/codex/app_server.rs`, used by `start_codex_conversation_edit_for_project` in `src-tauri/src/main.rs`). That sidecar is the pinned `@openai/codex` 0.141.0. No setting points the app at the installed `~/.local/bin/codex` (0.154.0). The run uses the bundled sidecar with the default `CODEX_HOME` (`~/.codex`), where `video-creater-codex login status` already reports "Logged in using ChatGPT". The installed CLI is used only for the prerequisite auth check. Credentials are never copied: a copied ChatGPT refresh token that rotates could sign the user out.
3. **Flow 2 uses a review edit that costs nothing.** The prompt asks to remove a whole existing track. `classify_codex_proposal_risk` (`src-tauri/src/codex/conversation/risk.rs`) always sends `RemoveTracks` to review (`removesTracks`), and dismissing it spends no provider credit. This follows Decision 3 ("one review edit that is dismissed") without needing provider credentials.
4. **GNOME Keyring is GPL-2.0-or-later.** It is used only as an out-of-process, test-only D-Bus service in a private `dbus-run-session`. It is never linked into the app or shipped (the `.deb` only `Recommends` it). Decision 5's binding rule, that nothing GPL is linked into the app, holds. `xdotool` (BSD-3-Clause) and `gstreamer1.0-tools` (LGPL) fit the rule's permissive list.
5. **The package runs extracted, not installed.** `dpkg-deb -x` into `/tmp/vc-deb-root-06` needs no sudo. Tauri resolves resources at `<exe dir>/../lib/Video Creater` (`resource_dir_from` in tauri-utils 2.9.2, the version in `src-tauri/Cargo.lock`). The render runtime materializes under `VIDEO_CREATER_APP_SUPPORT_DIR/render-runtime/linux-<fingerprint>`. The fingerprint hashes only the manifest (`runtime_fingerprint` in `src-tauri/src/render_runtime.rs`), so without isolation the packaged app could reuse an existing runtime whose symlinks point into the older `/tmp/vc-deb-root`. The packaged runs set `VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke-06/app-support`, and `/tmp/vc-deb-root` (used by the GES test runtime) is left untouched.

## Global Constraints

- **Commands.** Prefix every shell command with `rtk`; environment assignments go before `rtk` (for example `TAURI_CONFIG=… rtk cargo test …`). `rtk` passes unknown commands through (`rtk dbus-run-session -- …` works).
- **Commits.**
  - Use Conventional Commits, and end every message with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  - Stage only the files the task names (`rtk git add <paths>`, never `-A` or `.`).
  - Never push.
- **File size and UI styling.**
  - Keep every source file under 600 lines. Check with `rtk wc -l <files>`.
  - UI code uses theme tokens only: no hex colors, no `white/` or `black/` fragments. This plan changes no UI; the rule applies to any product fix made under the Track C fix rule.
- **Rust/TS lockstep.** A new project action lands in one commit with:
  - the TS union member,
  - `applyProjectActionLocally`,
  - the Codex schema,
  - MCP tool support,
  - the risk classification.

  This plan adds no actions. The rule applies to any product fix.
- **License policy.** LGPL GStreamer, GES, GTK and WebKitGTK are fine, dynamically linked; nothing GPL is linked into the app. Test-only tools live under `/tmp/vc-smoke-tools`, extracted from Ubuntu `.deb` files without sudo.
- **Cargo.**
  - Cargo test and check commands use `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'` and `--test-threads=1`.
  - Exception: the debug app build for the Temporal run (Task B3) omits `TAURI_CONFIG`, so `tauri-build` copies the staged sidecars next to the binary.
- **GES and export tests.**
  - GES-gated tests need `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`. Its symlinks point into `/tmp/vc-deb-root`. If that directory is missing, or its links don't resolve (for example after workstream 04 changed the runtime manifest), rebuild with `rtk pnpm build:linux-media-runtime --output /tmp/vc-smoke-06/render-runtime` and use that path instead.
  - `project_export` tests also need `VIDEO_CREATER_COMPATIBILITY_DECODER=src-tauri/target/debug/video-creater-compatibility-decoder`.
  - Never set `VIDEO_CREATER_RENDER_RUNTIME_ROOT` for the packaged runs.
- **Long commands.**
  - Run long commands in the foreground with the maximum tool timeout (600000 ms).
  - If the timeout kills a command, re-run the same command. Cargo and the Linux build cache (`~/.cache/video-creater-linux-build`) resume from the finished units.
  - Keep each smoke run small enough to fit, using `--only`.
- **Evidence.**
  - Never claim evidence that wasn't observed. A step that is skipped, filtered out, or not run is reported as not run, with the missing prerequisite.
  - Every native result cites its output folder and the `evidence.json` step status.
  - `output/` is git-ignored: evidence is recorded in the hand-off, not committed.
- **Scope of code changes.** Only the smoke tooling and docs change, except under the Track C fix rule (Task C0).

---

## File Map

### Smoke tooling (Track A)
- **`scripts/linux-desktop-smoke.mjs`** (modify): entry point only. It parses options, starts the processes (Xvfb, Vite, Temporal, tauri-driver, keyring), creates the session, runs the core steps, and calls the module step groups. It writes `evidence.json` and restores preferences.
- **`scripts/linux-desktop-smoke-driver.mjs`** (new): `createDriver({ port })`, returning `webdriver`, `screenshot(outDir, name)`, `invoke`, `execute`, `find`, `click`, `attribute`, `pressKeys`, `type` and `keys`; plus `poll`, `sleep` and `waitForPort`. The helper bodies move out of the current lines 54–241.
- **`scripts/linux-desktop-smoke-selectors.mjs`** (new): `projectHome`, `editorWorkspace`, `exportButton`, `exportPopover`, `editorMenu`, `backgroundTasks`, `settingsTab`, `editorTab`, `exportChoice`, and the AI selectors added in Task A5.
- **`scripts/linux-desktop-smoke-steps.mjs`** (new, pure):
  - `parseSmokeOptions(argv)`
  - `createStepRunner({ evidence, only, onFailure, log })`, whose actions may return `skipped(reason)`
  - `skipped(reason)`
  - `summarizeSteps(steps)`
  - `smokeRunContext({ options, env, commit, releaseReport })`
  - `exitCodeFor(evidence)`
- **`scripts/linux-desktop-smoke-retain.mjs`** (new, pure file helpers): `retainProjectFiles(projectDir, relativePaths, destinationDir)`, `newExportArtifacts(before, after, kind)` and `resolveArtifactPath(projectDir, path)`.
- **`scripts/linux-desktop-smoke-tools.mjs`** (new, pure):
  - `extractedToolEnvironment(root, env, extra)`: `LD_LIBRARY_PATH` set to `<root>/usr/lib/x86_64-linux-gnu`, prepended to the existing value
  - `extractedToolPath(root, name)`
  - `xScreenshotArgs(display, path)`: `gst-launch-1.0` argv for `ximagesrc … ! videoconvert ! pngenc ! filesink`
- **`scripts/linux-desktop-smoke-temporal.mjs`** (new): `runTemporalSteps(context)`, holding the five steps moved from the current lines 674–813.
- **`scripts/linux-desktop-smoke-native-menu.mjs`** (new): `runNativeMenuSteps(context)`.
- **`scripts/linux-desktop-smoke-agent.mjs`** (new): `runAgentFlowSteps(context)` plus the pure `timelineFingerprint(project)`, `agentFlowPrompts(project)` and `resultFramePngs(projectDir, sinceMs)`.
- **Tests** (new): `scripts/linux-desktop-smoke-steps.test.ts`, `scripts/linux-desktop-smoke-retain.test.ts`, `scripts/linux-desktop-smoke-tools.test.ts`, `scripts/linux-desktop-smoke-agent.test.ts`.
- **`scripts/linux-desktop-smoke-selectors.test.ts`** (modify): scans every `scripts/linux-desktop-smoke*.mjs`, adds `data-testid` patterns, and asserts every smoke module is under 600 lines.
- **`scripts/linux-desktop-smoke-denoise.mjs` and `scripts/linux-desktop-smoke-denoise.test.ts`:** unchanged.
- **`package.json`** (modify): append the four new test files to `test:source-quality`.
- **`docs/development/linux.md`** and **`docs/development/runtime-and-verification.md`** (modify): the new flags, tool roots and evidence layout.

### Environment only (Track B, no tracked files)
These are git-ignored or outside the repo:
- `/tmp/vc-smoke-tools/{cargo/bin/tauri-driver, root/usr/bin/WebKitWebDriver}` (both exist today)
- `/tmp/vc-smoke-tools/{keyring-root, xdotool-root, gst-tools-root, temporal}`
- `/tmp/vc-deb-root-06`
- `/tmp/vc-smoke-06/app-support`
- `output/linux-release/<commit12>/report.json`
- `output/gap-closure-06/rust-evidence/*.log`

### Docs (Track D)
- `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md` (modify)
- `docs/product-backlog.md` (modify: VC-001, VC-025, `Last updated`, and the VC-001 acceptance paragraph)

## Parallelization

| Track | Tasks | Order | Owns |
| --- | --- | --- | --- |
| A — smoke tooling | A1 → A2 → A3 → A4 → A5 → A6 | Sequential: all of them touch `scripts/linux-desktop-smoke.mjs`, and A2–A5 touch `package.json` | `scripts/linux-desktop-smoke*`, `package.json`; A6: `docs/development/*.md` |
| B — environment and build | B1 → B2 → B3 | Sequential inside the track (B2 and B3 share `src-tauri/target`). **B can run in parallel with A**, since A changes nothing under `src/` or `src-tauri/`. | no tracked files |
| C — native runs | C1 → C2 → C3 → C4 | After A6 and B3. Sequential (shared display, port, sample project and preferences). | `output/gap-closure-06/*`; fixes follow Task C0 |
| D — docs | D1, D2 | After C4. D1 and D2 may run in parallel (different files). | the hand-off; `docs/product-backlog.md` |

No task in this plan touches `src-tauri/src/main.rs`, `src/lib/project.ts` or `knip.jsonc`. A fix under Task C0 that needs one of them runs alone, after the other writers.

---

## Track A — smoke tooling

### Task A1: Split the smoke script into modules (no behavior change)

**Files:**
- modify `scripts/linux-desktop-smoke.mjs` and `scripts/linux-desktop-smoke-selectors.test.ts`
- create `scripts/linux-desktop-smoke-driver.mjs`, `scripts/linux-desktop-smoke-selectors.mjs` and `scripts/linux-desktop-smoke-temporal.mjs`

- [ ] **Failing test.** In `linux-desktop-smoke-selectors.test.ts`:
  - Read every `scripts/linux-desktop-smoke*.mjs` (via `readdirSync("scripts")`, filtered by `/^linux-desktop-smoke.*\.mjs$/`) instead of only the entry script, for both existing tests.
  - Add `test("every Linux desktop smoke module stays under 600 lines")`.
  - Run `rtk node --test scripts/linux-desktop-smoke-selectors.test.ts`. Expected: the new test fails, reporting `scripts/linux-desktop-smoke.mjs` at 847 lines.
- [ ] **Move the driver helpers.**
  - Move `start`-independent helpers into `linux-desktop-smoke-driver.mjs`: `waitForPort`, `sleep`, `webdriver`, `screenshot`, `invoke`, `execute`, `find`, `click`, `attribute`, `keys`, `pressKeys`, `type` and `poll`.
  - Expose them through `createDriver({ port: 4444 })` with a mutable `session` set by `driver.startSession(capabilities)`.
  - `screenshot` takes `outDir` from the driver options.
  - The module has no top-level side effects.
- [ ] **Move the selector constants and builders** (current lines 183–191) into `linux-desktop-smoke-selectors.mjs`. Also move `waitForEditor`, `openSampleEditor`, `openExportPopover`, `backgroundTasksLabel` and `openSettings` there as `(driver) => …` functions, and import them into the entry script.
- [ ] **Move the Temporal steps.** Move the five steps after `if (!process.argv.includes("--temporal")) return;` into `runTemporalSteps({ driver, step, projectPath, outDir })` in `linux-desktop-smoke-temporal.mjs`. Keep their step names byte-for-byte.
- [ ] **Verify:**
  - `rtk node --test scripts/linux-desktop-smoke-selectors.test.ts scripts/linux-desktop-smoke-denoise.test.ts` passes.
  - `rtk node --check scripts/linux-desktop-smoke.mjs` passes.
  - `rtk pnpm check:tooling-source` passes.
  - `rtk wc -l scripts/linux-desktop-smoke*.mjs`: each file is under 600 lines.
  - `rtk git diff --stat` shows only the five files.
- [ ] **Commit:** `refactor(smoke): split the Linux desktop smoke into modules`

### Task A2: Record skipped steps, a run summary and the run context

**Files:**
- create `scripts/linux-desktop-smoke-steps.mjs` and `scripts/linux-desktop-smoke-steps.test.ts`
- modify `scripts/linux-desktop-smoke.mjs`, `scripts/linux-desktop-smoke-temporal.mjs` and `package.json`

- [ ] **Failing tests** in `linux-desktop-smoke-steps.test.ts`:
  - **`parseSmokeOptions`:**
    - Defaults match today's values: app `src-tauri/target/debug/video-creater`, display `:94`, out `output/linux-desktop-smoke`.
    - `--only "a, b"` becomes `["a","b"]`.
    - Boolean flags parse: `--dev-server`, `--fake-audio`, `--temporal`, `--native-menu` and `--agent-flows`.
    - Value flags parse: `--keyring-root`, `--temporal-cli`, `--temporal-worker`, `--xdotool-root`, `--gst-tools-root` and `--release-report`.
    - `--native-menu` without `--xdotool-root` throws `"--native-menu needs --xdotool-root"`.
    - A leading `--` (from `pnpm … --`) is ignored.
  - **`createStepRunner`:**
    - A resolved action records `passed` with `ms` and `detail`.
    - A thrown error records `failed` with `error` and calls `onFailure(name)` for the screenshot.
    - An action returning `skipped("no --keyring-root")` records `{ status: "skipped", reason }`.
    - A name filtered out by `only` records nothing and returns `undefined`, except `"project home renders"`.
  - **`summarizeSteps`** returns `{ passed, failed, skipped, total }`.
  - **`exitCodeFor`** is 1 when any step failed or `fatal` is set, and 0 otherwise. Skipped steps don't fail the run.
  - **`smokeRunContext`:**
    - `appKind` is `"debug"` when the app path contains `/target/debug/` or `/target/release/`, and `"packaged"` otherwise.
    - It copies only `VIDEO_CREATER_*` variables plus `DBUS_SESSION_BUS_ADDRESS` presence as a boolean. It never copies other variables.
    - It reads `commit`, `package.path` and `package.sha256` from a release report object when one is given.
- [ ] **Run** `rtk node --test scripts/linux-desktop-smoke-steps.test.ts`. Expected: fails because the module is missing.
- [ ] **Implement** `linux-desktop-smoke-steps.mjs`, then wire it into the entry script:
  - Replace the inline `option()`, `onlySteps` and `step()` with it.
  - Write `evidence.runContext`, where `commit` comes from `spawnSync("git", ["rev-parse", "HEAD"])` and the release report JSON from `--release-report`.
  - Write `evidence.summary`.
  - Set the exit code with `exitCodeFor`.
  - The Secret Service step returns `skipped("run with --keyring-root <extracted gnome-keyring root>")` instead of `{ skipped: … }`.
  - When `--temporal` is absent, record the five Temporal step names as `skipped("run with --temporal")`, so a non-Temporal run never looks as if it covered them.
- [ ] **`package.json`:** append `scripts/linux-desktop-smoke-steps.test.ts` to `test:source-quality`.
- [ ] **Verify:**
  - `rtk node --test scripts/linux-desktop-smoke-steps.test.ts scripts/linux-desktop-smoke-selectors.test.ts` passes.
  - `rtk pnpm test:source-quality` passes.
  - `rtk pnpm check:unused` passes.
  - `rtk wc -l scripts/linux-desktop-smoke*.mjs`: each is under 600.
- [ ] **Commit:** `feat(smoke): record skipped steps and the run context in Linux smoke evidence`

### Task A3: Retain evidence files and follow the landed export records

**Files:**
- create `scripts/linux-desktop-smoke-retain.mjs` and `scripts/linux-desktop-smoke-retain.test.ts`
- modify `scripts/linux-desktop-smoke.mjs`, `scripts/linux-desktop-smoke-temporal.mjs` and `package.json`

- [ ] **Verify the landed behavior before changing any step**, and note the findings in the commit body.
  - `rtk git log --oneline -- src/editor/overlays/export-popover.tsx src/editor/services/export-service.ts src/lib/export src/lib/jobs src-tauri/src/render_pipeline | head -30`
  - Read the File Map of `docs/superpowers/plans/2026-09-16-gap-closure-02-*.md` and confirm:
    - (a) in-process video exports now record a `ProjectExportArtifact` (`src-tauri/src/project/model.rs`) with `jobId`;
    - (b) where the default output lands (project `exports/` or `renders/<jobId>/`);
    - (c) whether `path` is absolute or project-relative.
  - `rtk grep -n "complete" src/lib/jobs/task-indicator.ts`: confirm that the Background tasks description still ends in ` complete`, and read how workstream 01 renders progress (for example `· 40%`).
- [ ] **Failing tests** in `linux-desktop-smoke-retain.test.ts`, using temporary directories:
  - `retainProjectFiles` copies listed project-relative files into `destinationDir`, keeping their relative layout, and returns `{ from, to, bytes }`. It rejects `..` or absolute paths outside the project with `"refusing to retain a path outside the project"`, and skips missing files with `{ from, missing: true }`.
  - `newExportArtifacts(before, after, "mp4")` returns only the artifacts whose `id` is new and whose `kind` matches.
  - `resolveArtifactPath(projectDir, path)` keeps absolute paths and joins relative ones.
- [ ] **Implement** the module, then update the steps:
  - **"export the sample to MP4 H.264 from the export popover":**
    - Snapshot `exportArtifacts` before clicking "Export video".
    - Poll `load_split_project_from_folder` until `newExportArtifacts(…, "mp4")` returns one whose resolved path exists with size > 0, failing early on `window.__vcInvokeFailures` as today.
    - Keep the render report check through `renders/<artifact.jobId>/pipeline-report.json` when that file exists.
    - While polling, sample `backgroundTasksLabel()` and record the distinct labels as `detail.progressLabels`. This is informational: no pass or fail on progress.
    - Retain `renders/<jobId>/pipeline-report.json` into `<out>/export-mp4/`.
  - **"audio denoise renders through the Linux DeepFilterNet3 helper":** after `audioDenoiseEvidence` succeeds, retain `renders/<jobId>/pipeline-report.json` and every `denoised[].manifest` into `<out>/denoise/`, and return the retained list.
  - **"export MP4 through the Temporal worker":** replace the `export-mp4H264-[a-z0-9]+\.mp4` file-name regex with the same artifact-based discovery. Take the job from `artifact.jobId`, and keep the `job.workflow.runId` assertion.
- [ ] **`package.json`:** append `scripts/linux-desktop-smoke-retain.test.ts` to `test:source-quality`.
- [ ] **Verify:**
  - `rtk node --test scripts/linux-desktop-smoke-retain.test.ts scripts/linux-desktop-smoke-selectors.test.ts` passes.
  - `rtk pnpm test:source-quality` passes.
  - `rtk pnpm check:unused` passes.
  - `rtk wc -l scripts/linux-desktop-smoke*.mjs`: each is under 600.
- [ ] **Commit:** `feat(smoke): retain render and denoise evidence and find exports by their records`

### Task A4: Native menu steps driven by xdotool

**Files:**
- create `scripts/linux-desktop-smoke-tools.mjs`, `scripts/linux-desktop-smoke-tools.test.ts` and `scripts/linux-desktop-smoke-native-menu.mjs`
- modify `scripts/linux-desktop-smoke.mjs` and `package.json`

- [ ] **Failing tests** in `linux-desktop-smoke-tools.test.ts`:
  - `extractedToolEnvironment("/r", { LD_LIBRARY_PATH: "/x" }, { DISPLAY: ":94" })` returns `LD_LIBRARY_PATH: "/r/usr/lib/x86_64-linux-gnu:/x"` and `DISPLAY: ":94"`, and it doesn't mutate its input.
  - `extractedToolPath("/r", "xdotool")` is `/r/usr/bin/xdotool`.
  - `xScreenshotArgs(":94", "/o/a.png")` equals `["ximagesrc", "display-name=:94", "use-damage=false", "num-buffers=1", "!", "videoconvert", "!", "pngenc", "!", "filesink", "location=/o/a.png"]`.
- [ ] **Implement** `linux-desktop-smoke-tools.mjs`. Switch the entry script's keyring start to `extractedToolEnvironment(root, process.env, { XDG_DATA_HOME: … })`.
- [ ] **Implement `runNativeMenuSteps({ driver, step, options, outDir })`.** Its tool helpers:
  - `xdotool(...args)`: `spawnSync(extractedToolPath(root, "xdotool"), args, { env: extractedToolEnvironment(root, process.env, { DISPLAY }) })`. It throws with stderr on a non-zero status.
  - `xShot(name)`: runs when `--gst-tools-root` is given, via `gst-launch-1.0` with `xScreenshotArgs`. Otherwise it returns `null`, and the detail records `xScreenshots: "not captured (no --gst-tools-root)"`.
  - Window lookup: `xdotool search --sync --onlyvisible --name "^Video Creater$"`, taking the first id.
- [ ] **Step "native menu: a real Ctrl+Z key press undoes exactly one timeline edit":**
  - Open the sample editor. Pick two clips by `data-item-id`, and delete the first, then the second, with WebDriver clicks and `keys.delete`, as the existing Ctrl+Z step does.
  - Poll the saved project until both ids are gone.
  - Run `xdotool windowfocus --sync <wid>`, then `xdotool key --clearmodifiers ctrl+z`.
  - Poll the saved project. The second clip id returns, and the first stays absent for 3 s after that (a single undo, not two).
  - Record the item ids, the `xShot` paths and WebDriver screenshots.
- [ ] **Step "native menu: Edit → Undo in the GTK menu bar undoes the next edit":**
  - Run `xdotool key F10` and `xShot("menu-01-f10")`. Move to the Edit menu (`Right`), select its first item (`Down` if nothing is highlighted), and take `xShot("menu-02-edit")`. Then press `Return`.
  - Poll the saved project until the first clip id returns.
  - If F10 opens no menu, as observed in the X screenshot or when the project doesn't change within 10 s, fall back to clicking the menu bar: `xdotool mousemove --window <wid> <x> <y> click 1`, with coordinates read from the first X screenshot. Record which method was used.
  - Fail if the project didn't change.
- [ ] **Without `--native-menu`,** record both step names as `skipped("run with --native-menu --xdotool-root <dir>")`.
- [ ] **`package.json`:** append `scripts/linux-desktop-smoke-tools.test.ts` to `test:source-quality`.
- [ ] **Verify:**
  - `rtk node --test scripts/linux-desktop-smoke-tools.test.ts scripts/linux-desktop-smoke-steps.test.ts scripts/linux-desktop-smoke-selectors.test.ts` passes.
  - `rtk pnpm test:source-quality` passes.
  - `rtk pnpm check:unused` passes.
  - `rtk wc -l scripts/linux-desktop-smoke*.mjs`
- [ ] **Commit:** `feat(smoke): check Ctrl+Z and the GTK Edit menu Undo with real X key events`

### Task A5: AI flows 1 and 2 through the real Codex app-server

**Files:**
- create `scripts/linux-desktop-smoke-agent.mjs` and `scripts/linux-desktop-smoke-agent.test.ts`
- modify `scripts/linux-desktop-smoke.mjs`, `scripts/linux-desktop-smoke-selectors.mjs`, `scripts/linux-desktop-smoke-selectors.test.ts` and `package.json`

- [ ] **Verify the landed result-frame layout** from workstream 03: run `rtk grep -rn "preview-frames" src-tauri/src` and read the Linux branch of `capture_canonical_preview_frame_in_split_project_folder` (`src-tauri/src/main.rs`). Confirm that captures land at `renders/<jobId>/preview-qa/preview-frames/*.png`, and adjust `resultFramePngs` if the 03 plan chose another name.
- [ ] **Failing tests** in `linux-desktop-smoke-agent.test.ts`:
  - **`timelineFingerprint(project)`:**
    - It is stable under a changed `contentRevision`.
    - It changes when an item's `startSeconds`, `durationSeconds` or `source` changes, or when an item or track is added or removed.
  - **`agentFlowPrompts(project)`** takes a fixture project with a "Video" track holding two clips and a "Music" track:
    - `safe` names the last visual clip's `label` and asks to trim one second off its end.
    - `review` names a non-empty track by `name` and asks to remove that whole track from the timeline.
    - It throws `"sample project has no clips to edit"` for an empty timeline.
  - **`resultFramePngs(dir, sinceMs)`** returns project-relative `renders/*/preview-qa/preview-frames/*.png` paths newer than `sinceMs`, sorted, and ignores other PNGs.
- [ ] **Selectors.** Add to `linux-desktop-smoke-selectors.mjs`:
  - `aiPanel`, the `AI` tabpanel under `editorWorkspace`
  - `composer`: `//textarea[@aria-label='Describe an edit']`
  - `autoApplySwitch`: `//button[@role='switch' and @aria-label='Auto-apply safe edits']`
  - `appliedCard`: `//article[starts-with(@aria-label, 'Applied to ')]`
  - `reviewCard`: `//article[@aria-label='Needs your review']`
  - `undoneCard`: `//article[@aria-label='Undone']`
  - `resultPreview`: `//*[@role='group' and @aria-label='Result preview']`

  Then extend `labelPatterns` in the selectors test with `/\[data-testid='([^']+)'\]/g` and `/@data-testid='([^']+)'/g`, so `clip-highlight` is checked against `src/`.
- [ ] **Implement `runAgentFlowSteps({ driver, step, outDir })`**, sharing state between steps in a local `flow` object.
  - **Precheck** inside the first step: `get_agent_settings_health` has `agent.codex` in state `ready`. Otherwise fail with the item's `diagnosticCode` and `summary`.
  - **"agent flow 1: a safe edit auto-applies with result frames":**
    - Open the sample editor and click `editorTab("AI")`. Read `aria-checked` on `autoApplySwitch`; if it's off, click it and record `restoreAutoApply: false`.
    - Snapshot `timelineFingerprint` from `load_split_project_from_folder` (`/tmp/video-creater-editor-project`).
    - Type `prompts.safe` into `composer` and press Enter (``).
    - Poll up to 240 s for `appliedCard`, `reviewCard` or `//article[@aria-label="Couldn't validate the edit"]`. A review or failure card fails the step with the card text.
    - Poll up to 120 s until the first `resultPreview//img` inside `appliedCard` has `complete && naturalWidth > 0`. Assert that the text "Preview frames aren't available." is absent.
    - Retain `resultFramePngs(projectDir, startedAt)` into `<out>/agent-flow-1/frames/`, and fail if none exist.
    - Record the prompt, the card's `Facts` text, the frame count and a screenshot.
  - **"agent flow 1: Show changes highlights the changed clips":**
    - If flow 1 didn't apply, return `skipped("flow 1 did not apply")`.
    - Click `appliedCard//button[normalize-space()='Show changes']`.
    - Poll until `document.querySelectorAll("[data-testid='clip-highlight']").length > 0`. Record the count and a screenshot.
  - **"agent flow 1: Undo restores the timeline":**
    - Skip under the same condition as the Show changes step.
    - Click `appliedCard//button[normalize-space()='Undo']`, then wait for `undoneCard`.
    - Poll the saved project until `timelineFingerprint` equals the snapshot. Assert that no `clip-highlight` remains.
  - **"agent flow 2: a review edit waits, and Dismiss leaves the timeline unchanged":**
    - Snapshot the fingerprint, then send `prompts.review`.
    - Poll up to 240 s for `reviewCard` or `appliedCard`. If `appliedCard` appears, click its Undo, wait for `undoneCard`, and fail with `"the agent applied a request expected to need review"`.
    - Record the `Planned changes` text and a screenshot.
    - Assert that the saved fingerprint is unchanged, click `reviewCard//button[normalize-space()='Dismiss']`, and wait for the text "Dismissed".
    - Assert that the fingerprint is unchanged 3 s later.
  - **Finally:** if `restoreAutoApply` is false, click the switch back off.
- [ ] **Without `--agent-flows`,** record the four step names as `skipped("run with --agent-flows")`.
- [ ] **`package.json`:** append `scripts/linux-desktop-smoke-agent.test.ts` to `test:source-quality`.
- [ ] **Verify:**
  - `rtk node --test scripts/linux-desktop-smoke-agent.test.ts scripts/linux-desktop-smoke-selectors.test.ts scripts/linux-desktop-smoke-steps.test.ts` passes.
  - `rtk pnpm test:source-quality` passes.
  - `rtk pnpm check:unused` passes.
  - `rtk pnpm check:tooling-source` passes.
  - `rtk wc -l scripts/linux-desktop-smoke*.mjs`
- [ ] **Commit:** `feat(smoke): run AI flows 1 and 2 against the real Codex app-server`

### Task A6: Document the smoke flags and evidence layout

**Files:** modify `docs/development/linux.md` and `docs/development/runtime-and-verification.md`.

- [ ] **`linux.md` → Verification:**
  - Replace the usage block with the packaged and debug invocations from Tasks C1–C4.
  - Document `--native-menu --xdotool-root [--gst-tools-root]`, `--agent-flows` and `--release-report`.
  - Explain that skipped steps are recorded as `skipped`.
  - Describe the retained files (`export-mp4/`, `denoise/`, `agent-flow-1/frames/`).
  - Explain `VIDEO_CREATER_APP_SUPPORT_DIR` for runs of an extracted package.
  - State that Temporal steps need the debug build, because releases don't include `temporal-worker`.
  - List the tool packages: `xdotool libxdo3`; `gnome-keyring libgck-1-0 libgcr-base-3-1 gcr`; `gstreamer1.0-tools`.
- [ ] **`runtime-and-verification.md` → Editor policy tests:** add that the selector test covers every `scripts/linux-desktop-smoke*.mjs` module and their 600-line limit, and list the four new unit test files.
- [ ] **Verify:** `rtk grep -n "agent-flows\|native-menu\|release-report" docs/development/linux.md` shows the new flags.
- [ ] **Commit:** `docs(linux): document the native smoke flags and retained evidence`

---

## Track B — environment and build (parallel with Track A)

### Task B1: Stage test tools under /tmp/vc-smoke-tools

**Files:** none tracked. Write `output/gap-closure-06/prerequisites/tools.txt`, holding the version and license lines observed.

- [ ] **Existing tools.**
  - `rtk /tmp/vc-smoke-tools/cargo/bin/tauri-driver --help | head -1`
  - `rtk ls -la /tmp/vc-smoke-tools/root/usr/bin/WebKitWebDriver`
  - `rtk dpkg -l libwebkit2gtk-4.1-0` and `rtk apt-cache policy webkit2gtk-driver`: the driver candidate must equal the installed WebKitGTK version (2.52.6-0ubuntu0.24.04.1 on 2026-09-16). If it differs, re-extract `webkit2gtk-driver` into `/tmp/vc-smoke-tools/root`.
  - If either tool is missing, run `rtk cargo install tauri-driver --root /tmp/vc-smoke-tools/cargo`, and/or `rtk apt-get download webkit2gtk-driver` + `rtk dpkg-deb -x webkit2gtk-driver_*.deb /tmp/vc-smoke-tools/root`.
- [ ] **xdotool (BSD-3-Clause).**
  - In `/tmp/vc-smoke-tools/debs`, run `rtk apt-get download xdotool libxdo3`, then `rtk dpkg-deb -x <each> /tmp/vc-smoke-tools/xdotool-root`.
  - Check: `LD_LIBRARY_PATH=/tmp/vc-smoke-tools/xdotool-root/usr/lib/x86_64-linux-gnu rtk /tmp/vc-smoke-tools/xdotool-root/usr/bin/xdotool version`. Record the license line from `usr/share/doc/xdotool/copyright`.
- [ ] **GNOME Keyring (GPL-2.0-or-later, test-only; see refinement 4).**
  - `rtk apt-get download gnome-keyring libgck-1-0 libgcr-base-3-1 gcr`, each extracted into `/tmp/vc-smoke-tools/keyring-root`.
  - `LD_LIBRARY_PATH=/tmp/vc-smoke-tools/keyring-root/usr/lib/x86_64-linux-gnu rtk ldd /tmp/vc-smoke-tools/keyring-root/usr/bin/gnome-keyring-daemon | rtk grep "not found"` must print nothing. Download and extract any library it names.
  - **Verify the provider works before relying on it.** Write this helper outside the repo as `/tmp/vc-smoke-06/keyring-probe.sh`:
    ```bash
    set -eu
    root=/tmp/vc-smoke-tools/keyring-root
    export XDG_DATA_HOME="$(mktemp -d /tmp/vc-keyring-data-XXXXXX)"
    printf smoke-test-password | LD_LIBRARY_PATH="$root/usr/lib/x86_64-linux-gnu" \
      "$root/usr/bin/gnome-keyring-daemon" --unlock --components=secrets --daemonize
    gdbus call --session --dest org.freedesktop.secrets \
      --object-path /org/freedesktop/secrets/aliases/default \
      --method org.freedesktop.DBus.Properties.Get org.freedesktop.Secret.Collection Locked
    ```
    Run `rtk dbus-run-session -- bash /tmp/vc-smoke-06/keyring-probe.sh`. Expected: `(<false>,)`.
  - If the default alias is missing, record the output. Retry with `--login` in place of `--unlock` (same stdin password), and note the working flags. If the flags differ from the smoke's `--unlock --components=secrets --daemonize`, fix `scripts/linux-desktop-smoke.mjs` under Task C0 before C1.
- [ ] **X screenshots (LGPL).**
  - `rtk apt-get download gstreamer1.0-tools`, extracted into `/tmp/vc-smoke-tools/gst-tools-root`.
  - Check with a helper written as `/tmp/vc-smoke-06/x-probe.sh`:
    ```bash
    set -eu
    Xvfb :95 -screen 0 800x600x24 -nolisten tcp & xvfb=$!
    sleep 1
    DISPLAY=:95 /tmp/vc-smoke-tools/gst-tools-root/usr/bin/gst-launch-1.0 ximagesrc display-name=:95 \
      use-damage=false num-buffers=1 ! videoconvert ! pngenc ! filesink location=/tmp/vc-smoke-06/x-probe.png
    kill "$xvfb"
    ```
    Run `rtk bash /tmp/vc-smoke-06/x-probe.sh`, then `rtk ls -la /tmp/vc-smoke-06/x-probe.png`. Expect a non-empty PNG.
  - If the pipeline fails, record the error. Track C then runs without `--gst-tools-root`, and the hand-off states that X screenshots weren't captured.
- [ ] **Temporal CLI (MIT).**
  - If `~/.cache/vc-desktop-tools/temporal/temporal` exists, copy it to `/tmp/vc-smoke-tools/temporal/temporal`.
  - Otherwise download the `linux_amd64` archive from the `temporalio/cli` GitHub releases. Verify the asset name on the release page first.
  - Checks:
    - `rtk /tmp/vc-smoke-tools/temporal/temporal --version` (observed on 2026-09-16: `Temporal CLI 1.8.3 (Server 1.31.2, UI 2.50.1)`).
    - `rtk /tmp/vc-smoke-tools/temporal/temporal server start-dev --help | rtk grep headless` shows `--headless`.
- [ ] **Codex auth (no spend).**
  - `rtk codex login status` for the installed CLI.
  - After Task B2, run `rtk /tmp/vc-deb-root-06/usr/bin/video-creater-codex --version` (expect `codex-cli 0.141.0`) and `rtk /tmp/vc-deb-root-06/usr/bin/video-creater-codex login status` (expect "Logged in using ChatGPT").
  - `rtk grep -E "^model" ~/.codex/config.toml`: record the configured model name, because the bundled 0.141.0 sidecar will use it (see Risks).
- [ ] **Host checks.**
  - `rtk pgrep -a Xvfb`: no process on `:94`.
  - `rtk which bwrap dbus-run-session Xvfb gdbus`
  - `rtk sysctl kernel.apparmor_restrict_unprivileged_userns`, recorded because Codex sandboxing depends on the system `bwrap`.
- [ ] **No commit** (no tracked files change).

### Task B2: Build, audit and extract the packaged .deb

**Files:** none tracked. Outputs: `output/linux-release/<commit12>/report.json` and `/tmp/vc-deb-root-06`.

- [ ] **Preflight:** `rtk pnpm release:linux:preflight`. Expect `{ "status": "ready" }`.
- [ ] **Build:** `rtk pnpm release:linux`, in the foreground; re-run it if the tool timeout stops it. `tauri.linux.conf.json` `beforeBuildCommand` stages every sidecar into `src-tauri/binaries`: precompose worker, audio enhancer, semantic encoder, speech worker, compatibility decoder, Codex and MCP server.
- [ ] **Expected result:** the command prints `"status": "passed"`. `report.json` has `missingPayload: []` and `deniedElfDependencies: []`, and `package.sha256` is present. If the status is `failed`, stop and report the missing payload or denied libraries. Do not run Track C against a failed audit.
- [ ] **Record** `package.path`, `package.sha256`, `commit` and `features` from the report.
- [ ] **Extract.**
  - `rtk rm -rf /tmp/vc-deb-root-06 && rtk dpkg-deb -x <package.path> /tmp/vc-deb-root-06`
  - `rtk ls /tmp/vc-deb-root-06/usr/bin` lists `video-creater`, `video-creater-codex`, `video-creater-mcp-server`, `video-creater-audio-enhance`, `video-creater-speech`, `video-creater-semantic-encoder`, `video-creater-compatibility-decoder` and `video-creater-precompose-worker`.
  - `rtk ls "/tmp/vc-deb-root-06/usr/lib/Video Creater"` lists `codex-runtime`, `render-runtime`, `sample-project` and `speech-runtime`.
- [ ] **Track A commits can't have changed the package.** At the start of Track C, `rtk git diff --stat <report.commit>..HEAD -- src src-tauri index.html package.json pnpm-lock.yaml` may list only `package.json` script lines from Track A. Anything else means the package is stale: rebuild.
- [ ] **No commit.**

### Task B3: Debug build for the Temporal run, and the Rust sidecar and Secret Service tests

**Files:** none tracked. Logs go to `output/gap-closure-06/rust-evidence/`.

- [ ] **Debug build.** With the sidecars staged by B2, run `rtk cargo build --manifest-path src-tauri/Cargo.toml --bin video-creater --bin video-creater-temporal-worker`, without `TAURI_CONFIG`, so `tauri-build` copies `externalBin` and resources. Then run `rtk pnpm build:compatibility-decoder:linux:dev`, which writes `src-tauri/target/debug/video-creater-compatibility-decoder` (a workspace package, not a `--bin` of the app crate).
  - Check: `rtk ls src-tauri/target/debug/video-creater-speech src-tauri/target/debug/video-creater-audio-enhance src-tauri/target/debug/video-creater-codex src-tauri/target/debug/sample-project`.
  - `rtk src-tauri/target/debug/video-creater-temporal-worker --print-manifest | head -5` prints the manifest JSON (the `temporal-worker` feature is on by default).
- [ ] **MCP sidecar probe.** This lib test failed in the hand-off run only because the sidecar wasn't staged:
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk bash -c 'cargo test --manifest-path src-tauri/Cargo.toml --lib settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe -- --exact --test-threads=1 > output/gap-closure-06/rust-evidence/mcp-sidecar-probe.log 2>&1; echo exit=$?'`
  Expected: `exit=0` and `test result: ok. 1 passed`. Read it back with `rtk grep "test result" output/gap-closure-06/rust-evidence/mcp-sidecar-probe.log`.
- [ ] **Secret Service tests.** Write `/tmp/vc-smoke-06/secret-service-tests.sh` outside the repo. It takes `with-provider` or `without-provider`:
  ```bash
  set -u
  cd /home/olhapi/projects/video-creater
  export TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'
  export VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE="com.olhapi.video-creater.settings-acceptance.$(node -e 'console.log(require("crypto").randomBytes(16).toString("hex"))')"
  if [ "$1" = with-provider ]; then
    root=/tmp/vc-smoke-tools/keyring-root
    export XDG_DATA_HOME="$(mktemp -d /tmp/vc-keyring-data-XXXXXX)"
    printf smoke-test-password | LD_LIBRARY_PATH="$root/usr/lib/x86_64-linux-gnu" \
      "$root/usr/bin/gnome-keyring-daemon" --unlock --components=secrets --daemonize
    cargo test --manifest-path src-tauri/Cargo.toml --lib -- --ignored --test-threads=1 \
      provider_credentials::tests::linux_secret_service_replaces_and_deletes_an_isolated_account \
      provider_credentials::tests::linux_locked_secret_service_is_unavailable_never_missing \
      > output/gap-closure-06/rust-evidence/secret-service.log 2>&1
  else
    cargo test --manifest-path src-tauri/Cargo.toml --lib -- --ignored --exact --test-threads=1 \
      provider_credentials::tests::linux_session_bus_without_secret_service_provider_is_unavailable \
      > output/gap-closure-06/rust-evidence/secret-service-missing.log 2>&1
  fi
  echo "exit=$?"
  ```
  (Use the B1 flags if B1 found that `--login` is needed.)
- [ ] **Run with a provider:** `rtk dbus-run-session -- bash /tmp/vc-smoke-06/secret-service-tests.sh with-provider`. Expected: `exit=0`, and `rtk grep "test result" output/gap-closure-06/rust-evidence/secret-service.log` shows `2 passed`.
- [ ] **Run without a provider:** `rtk dbus-run-session -- bash /tmp/vc-smoke-06/secret-service-tests.sh without-provider`. Expected: `exit=0` and `1 passed`.
- [ ] **No commit.**

---

## Track C — native runs (after A6 and B3; sequential)

### Task C0: Fix rule for failures found by native runs

- **Smoke defects** (a wrong selector, timeout or path in the script) are fixed with TDD in the owning `scripts/linux-desktop-smoke*.{mjs,test.ts}` files. Commit each as `fix(smoke): <what>`, then re-run only the affected steps with `--only` into `<out>-rerun-<n>`. Both folders are cited.
- **Product defects** are fixed only when the fix is local and a failing unit or integration test reproduces them first. Commit as `fix(<area>): <what>`, following the Global Constraints (lockstep, tokens, 600 lines, cargo env). Rebuild the package (B2) before re-running packaged steps.
- Otherwise the step is reported as failed, with the observed error and folder, and the gap stays open.
- Never edit `evidence.json` by hand.

### Task C1: Packaged app smoke — core, denoise, Secret Service, MCP self-tests, GPL scan

**Output:** `output/gap-closure-06/packaged-smoke/`

- [ ] **Run** (no `--dev-server`, because the release build embeds the frontend):
  `VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke-06/app-support rtk dbus-run-session -- env -u VIDEO_CREATER_RENDER_RUNTIME_ROOT node scripts/linux-desktop-smoke.mjs --app /tmp/vc-deb-root-06/usr/bin/video-creater --tauri-driver /tmp/vc-smoke-tools/cargo/bin/tauri-driver --native-driver /tmp/vc-smoke-tools/root/usr/bin/WebKitWebDriver --fake-audio --keyring-root /tmp/vc-smoke-tools/keyring-root --release-report output/linux-release/<commit12>/report.json --out output/gap-closure-06/packaged-smoke`
  If the full run exceeds the tool timeout, split it with `--only` into `…/packaged-smoke-part-<n>` runs. Keep `running app processes load no GPL media libraries` in every part.
- [ ] **Read** `rtk json output/gap-closure-06/packaged-smoke/evidence.json` and confirm:
  - `runContext.appKind` is `"packaged"`, and `runContext.package.sha256` matches B2.
  - `summary.failed` is 0, or each failure is handled by C0.
  - **"export the sample to MP4 H.264 from the export popover"** is `passed`, with an artifact path and `progressLabels` recorded.
  - **"audio denoise renders through the Linux DeepFilterNet3 helper"** is `passed`. `detail.denoised[0].noiseRmsAfter < noiseRmsBefore`, and `denoise/` holds the retained `pipeline-report.json` and `manifest.json` (`rtk ls output/gap-closure-06/packaged-smoke/denoise -R`).
  - **"provider credentials round-trip through the Secret Service"** is `passed`, not `skipped`. `listed.configured` is `true` with `source: "keychain"`, and `after.configured` is `false`.
  - **"agent and MCP self-tests pass"** is `passed`, with `agent.mcpServer`, `agent.proposalValidator` and `agent.codex` all `succeeded`.
  - **"running app processes load no GPL media libraries"** is `passed`, and its `inspected` list includes `video-creater` and `WebKitWebProcess`.
  - The native-menu, agent-flow and Temporal steps are `skipped` with their flag reasons. That is expected in this run.
- [ ] **Confirm the runtime came from the package:** `rtk ls /tmp/vc-smoke-06/app-support/render-runtime/` shows one `linux-<32 hex>` directory, and `rtk readlink /tmp/vc-smoke-06/app-support/render-runtime/linux-*/plugins/libgstlibav.so` points into `/tmp/vc-deb-root-06/`.
- [ ] **No commit.**

### Task C2: GTK-level Ctrl+Z and native menu Undo

**Output:** `output/gap-closure-06/native-menu/`

- [ ] **Run:**
  `VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke-06/app-support rtk dbus-run-session -- env -u VIDEO_CREATER_RENDER_RUNTIME_ROOT node scripts/linux-desktop-smoke.mjs --app /tmp/vc-deb-root-06/usr/bin/video-creater --tauri-driver /tmp/vc-smoke-tools/cargo/bin/tauri-driver --native-driver /tmp/vc-smoke-tools/root/usr/bin/WebKitWebDriver --fake-audio --native-menu --xdotool-root /tmp/vc-smoke-tools/xdotool-root --gst-tools-root /tmp/vc-smoke-tools/gst-tools-root --release-report output/linux-release/<commit12>/report.json --only "native menu:,running app processes" --out output/gap-closure-06/native-menu`
  Omit `--gst-tools-root` if B1 recorded that the probe failed.
- [ ] **Confirm:**
  - Both `native menu:` steps are `passed`.
  - The first records one restored id and one still-deleted id.
  - The second records the activation method (`F10` or `menu bar click`).
  - The X screenshots (`menu-01-f10.png`, `menu-02-edit.png`) exist, or the detail says they weren't captured.
- [ ] **Look at the X screenshots** with the Read tool, and record what the Edit menu shows (Undo as its first, enabled item).
- [ ] **No commit.**

### Task C3: AI flows 1 and 2 with the real Codex app-server

**Output:** `output/gap-closure-06/agent-flows/`

- [ ] **Run:**
  `VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke-06/app-support rtk dbus-run-session -- env -u VIDEO_CREATER_RENDER_RUNTIME_ROOT node scripts/linux-desktop-smoke.mjs --app /tmp/vc-deb-root-06/usr/bin/video-creater --tauri-driver /tmp/vc-smoke-tools/cargo/bin/tauri-driver --native-driver /tmp/vc-smoke-tools/root/usr/bin/WebKitWebDriver --fake-audio --agent-flows --release-report output/linux-release/<commit12>/report.json --only "agent flow,running app processes" --out output/gap-closure-06/agent-flows`
- [ ] **Confirm:**
  - The three `agent flow 1:` steps and the `agent flow 2:` step are `passed`.
  - `agent-flow-1/frames/` holds at least one PNG.
  - Flow 1 records its facts text, and flow 2 records its planned-changes text.
- [ ] **Look at the result frames and screenshots** (Read tool) and describe what they show: the trimmed clip's frame, and the review card's planned changes.
- [ ] **If a turn fails:**
  - Record the failure card text or the backend error (for example the 180 s `APP_SERVER_TURN_TIMEOUT`, or a model the 0.141.0 sidecar rejects).
  - A model-classification miss (flow 1 reviewed, or flow 2 applied) may be retried once with the same prompts. If it misses again, the step stays failed.
  - Don't change the user's `~/.codex` configuration or credentials.
- [ ] **No commit.**

### Task C4: Temporal smoke steps (debug build)

**Output:** `output/gap-closure-06/temporal-smoke/`

- [ ] **Models.** `rtk ls ~/.local/share/com.olhapi.video-creater/models`: note whether the transcription and speech models are already installed. The install step then only verifies them.
- [ ] **Run:**
  `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a rtk dbus-run-session -- node scripts/linux-desktop-smoke.mjs --dev-server --app src-tauri/target/debug/video-creater --tauri-driver /tmp/vc-smoke-tools/cargo/bin/tauri-driver --native-driver /tmp/vc-smoke-tools/root/usr/bin/WebKitWebDriver --fake-audio --temporal --temporal-cli /tmp/vc-smoke-tools/temporal/temporal --temporal-worker src-tauri/target/debug/video-creater-temporal-worker --only "render system health,create a project,import H.264/AAC,install the transcription model,select Temporal execution,export MP4 through the Temporal worker,transcribe imported speech,analyze speech,running app processes" --out output/gap-closure-06/temporal-smoke`
  - Use the rebuilt runtime path when the Global Constraints rule applies.
  - If the model download can't finish within one run, first run `--only "install the transcription model"` into `temporal-smoke-models`, repeating until it reports `ready`. Record whether an interrupted download resumed or restarted, then run the rest.
- [ ] **Confirm:**
  - `runContext.appKind` is `"debug"`.
  - "export MP4 through the Temporal worker" is `passed`, with `job.runId` and an artifact path.
  - "transcribe imported speech through the Temporal worker" is `passed`, with `words > 0`.
  - "analyze speech in the desktop process" is `passed`, with `speechRanges > 0`.
  - The Temporal server and worker logs (`temporal.log`, `video-creater-temporal-worker.log`) are in the folder.
- [ ] **No commit.**

---

## Track D — docs (after C4)

### Task D1: Record the native evidence in the hand-off

**Files:** modify `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md`.

- [ ] **Add a section** `## Native Linux evidence — gap closure 06 (<run date>)` after "Native evidence and its limits". It contains:
  - **Build:** the package path, SHA-256, commit, release features, the audit status and `output/linux-release/<commit12>/report.json`.
  - **Runs:** one table per run (C1–C4) with the columns Step | Status (`passed` / `failed` / `skipped` / not run) | Observed detail | Folder. Take the rows from each `evidence.json`. Copy statuses exactly, and give every non-passing row its reason.
  - **Rust evidence:** the three B3 logs, each with its `test result` line.
  - **Observations:** what the X screenshots and result frames showed, in plain words.
- [ ] **Update "Not yet run" and "What the smoke didn't cover"** in the same file. Remove only items that now have a `passed` step, and point to the new section. Keep the macOS ⌘Z check and the macOS visual baselines as pending, with the reason "needs a Mac".
- [ ] **Verify:**
  - Every folder cited exists: `rtk ls <each folder>`.
  - Every status in the doc matches its `evidence.json`: `rtk json <evidence.json>`.
- [ ] **Commit:** `docs(editor): record native Linux evidence for the redesigned editor`

### Task D2: Update backlog rows VC-001 and VC-025 (only after verification)

**Files:** modify `docs/product-backlog.md`.

- [ ] **VC-001 row and its "Current P0 Acceptance Direction" paragraph.**
  - **If C3 passed all four agent-flow steps with retained frames:** set the status to `verified`. State the evidence boundary: "packaged Linux `.deb` (extracted, Ubuntu 24.04 x86_64, Xvfb) with the bundled Codex app-server; macOS packaged evidence is tracked by VC-008". Name the folder `output/gap-closure-06/agent-flows/`. The next action becomes "Retain macOS packaged evidence under VC-008".
  - **Otherwise:** keep `implemented`, and write the failed or not-run steps and their reasons as the next action.
- [ ] **VC-025 row.** Keep `implemented`, because the macOS items remain.
  - Replace the Linux next actions with the observed results: packaged smoke, Temporal steps, MCP self-tests, Secret Service, denoise with retained evidence, GTK Ctrl+Z and Edit → Undo. Cite their folders.
  - The remaining next actions become the macOS ⌘Z single-undo check, the macOS visual baselines, and any Linux step that did not pass (named, with its reason).
  - Remove the stale denoise sentence ("re-run it to record denoise evidence") only if the C1 denoise step passed with retained files.
- [ ] **Set `Last updated`** to the run date.
- [ ] **Verify:** `rtk grep -n "VC-001\|VC-025" docs/product-backlog.md`. Re-read both rows against D1's tables: no claim without a `passed` step.
- [ ] **Commit:** `docs(backlog): record native Linux evidence for VC-001 and VC-025`

---

## Acceptance

| Gap (design workstream 06) | Proof |
| --- | --- |
| Packaged `.deb` with all sidecars | `output/linux-release/<commit12>/report.json`: `status: "passed"`, `missingPayload: []`, `deniedElfDependencies: []`. B2's extraction lists the Codex, MCP server, audio enhancer, speech, semantic encoder, compatibility decoder and precompose sidecars. |
| Desktop smoke against the packaged app | `output/gap-closure-06/packaged-smoke/evidence.json`: `runContext.appKind: "packaged"` with the B2 SHA-256, and core steps `passed` (home, platform, render health, export profiles, project creation, import, sample playback, MP4 export with a recorded artifact, settings, NLE XML, package, webview Ctrl+Z). |
| MCP sidecar self-tests | C1 step "agent and MCP self-tests pass" `passed`, with all three components `succeeded`; `rust-evidence/mcp-sidecar-probe.log` with `1 passed`. |
| Secret Service credentials | C1 step "provider credentials round-trip through the Secret Service" `passed` (not `skipped`); `rust-evidence/secret-service.log` (2 passed) and `secret-service-missing.log` (1 passed). |
| Denoise re-run with recorded evidence | C1 denoise step `passed`, with `noiseRmsAfter < noiseRmsBefore` and retained `denoise/pipeline-report.json` plus the manifest. |
| GTK-level Ctrl+Z native menu check | `output/gap-closure-06/native-menu/evidence.json`: both `native menu:` steps `passed` (single undo from a real X key event; Edit → Undo through the GTK menu bar), with X screenshots or an explicit not-captured note. |
| AI flow 1 (safe edit, frames, Show changes, Undo) | `output/gap-closure-06/agent-flows/evidence.json`: the three `agent flow 1:` steps `passed`, with PNGs in `agent-flow-1/frames/`. |
| AI result frames on Linux | The same run: a loaded result-frame image, no "Preview frames aren't available." fallback, and retained PNGs from `renders/*/preview-qa/preview-frames/`. |
| AI flow 2 (review, then Dismiss) | `agent flow 2:` `passed`, with an unchanged timeline fingerprint before and after Dismiss. |
| Temporal smoke steps | `output/gap-closure-06/temporal-smoke/evidence.json` (`appKind: "debug"`, refinement 1): the Temporal export with `runId`, transcription with words, and speech analysis all `passed`. |
| No GPL libraries loaded | "running app processes load no GPL media libraries" `passed` in every C run. |
| Smoke tooling | `rtk pnpm test:source-quality`, `rtk pnpm check:unused` and `rtk pnpm check:tooling-source` pass; every `scripts/linux-desktop-smoke*.mjs` is under 600 lines; skipped steps are recorded as `skipped`. |
| Hand-off and backlog | D1 and D2 commits cite only `passed` steps as evidence, and list every other step with its reason. |

## Risks

- **Codex turn limits.** Real turns can exceed `APP_SERVER_TURN_TIMEOUT` (180 s). The bundled 0.141.0 sidecar reads `~/.codex/config.toml`, which names a newer model and loads user MCP servers and hooks; a model it rejects fails both flows. This plan doesn't change the user's configuration.
- **Model nondeterminism.** The agent may classify the flow 1 prompt as needing review, or propose a safe ripple delete for flow 2. One retry is allowed; then the step stays failed.
- **Keyboard focus on Xvfb.** Without a window manager, `xdotool windowfocus` may not give GTK keyboard focus, and F10 may not open the GTK menu bar under WebKitGTK. The menu-bar click fallback is recorded. If both fail, the native menu gap stays open.
- **GNOME Keyring default collection.** A private keyring may lack a default collection without a prompter. B1 verifies this before C1 relies on it.
- **Tool timeouts.** The release build and model downloads can exceed the 10-minute foreground limit. Re-runs rely on the cargo and build caches, and on the model store's resume behavior, which B1/C4 record.
- **Changes from 01–05.** Workstream 04 may change the runtime manifest (for example adding `scaletempo`), which invalidates the GES runtime path. Workstreams 01–03 may change export paths, task labels or frame layout. A3 and A5 verify each of these before changing a step, and C0 handles any drift that remains.
- **`/tmp` cleanup.** A cleanup removes the staged tools, `/tmp/vc-deb-root` (the GES test runtime links) and `/tmp/vc-deb-root-06`. Re-run B1/B2 when that happens.
