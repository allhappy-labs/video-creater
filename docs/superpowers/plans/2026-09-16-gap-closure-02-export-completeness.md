# Gap Closure 02 — Export Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Each task lists the files it owns, its test commands and its commit message. Expand the code inside a step before executing it; do not change the contracts named here without recording why in the commit body.

**Goal:** Close VC-019 and VC-020 on Linux.

- **VC-019, export destination and output settings.** The Export popover's Save to folder, file name and frame-rate fields reach the backend, and Master quality is enabled. Master is the highest-quality encode of the chosen format (design decision 6). The inputs cover the in-process `render_media_to_split_project_folder` command and the Temporal export start request. An existing file is never overwritten; the export takes the next free name.
- **VC-020, export records and names.** In-process video exports record an export artifact. The job records its export settings (quality included), so Retry after a restart restores them. "Save range as media" names the new media after the timeline and range instead of `output`.
- **Reveal (design decision 7).** "Show in folder" may reveal a recorded absolute export artifact outside the project folder. It still refuses every unrecorded path.

**Architecture:**
- **Options.** `ExportRenderOptions` gains an optional frame-rate override (`fps`) and an encode tier (`ExportEncodeTier::Standard | Master`). Master is always `RenderQuality::Final` plus `encode_tier: Master`. It is not a third `RenderQuality`, which has about 65 match sites and drives AVFoundation profile and availability semantics. `RenderPlan` carries `encode_tier`. The frame-rate override is applied to the render's working copy of the project before preparation, so precompose, graphics and the GES plan all agree.
- **Master encoders.** GES encoders read a tier-aware quality ladder (`effective_quality_settings_for_tier`). On Linux, H.264 is OpenH264 and H.265 is VA-API; neither has a CRF or preset, so Master uses the "equivalent bitrate ladder" branch of decision 6 plus the encoder's best-effort knobs. WebM (VP8) uses its slowest good-quality settings. ProRes is unchanged, and Master is refused for it.
- **Destination.** A new pure module, `project::export_destination`, is shared by the in-process renderer and the Temporal `WriteExportArtifact` activity. It does four things:
  - validates the file name and target folder;
  - picks a collision-free name (`Name.mp4`, then `Name (2).mp4`, and so on);
  - materializes the rendered file without clobbering: a hard link when possible, otherwise a staged copy and a no-clobber link or rename;
  - builds the `ProjectExportArtifact`.

  The default folder is the project's `exports/` directory, recorded project-relative. A user-chosen folder outside the project is recorded as an absolute path. A folder inside the project but outside `exports/` is refused.
- **Job record.** `JobSummary` gains `export_settings: Option<JobExportSettings>`: the options, plus the output file name and folder. The in-process renderer writes it at job start. The editor writes it on Temporal jobs. Retry presets read it first.
- **Reveal.** `export_reveal::reveal_recorded_export_with` keeps the project-relative rule. It adds one branch: an absolute `artifactPath` that equals a recorded absolute `exportArtifacts[].path`, is not a symlink, and still exists. Action validation accepts an absolute artifact path only when the artifact's job recorded that folder in `export_settings`, so an agent-recorded artifact can't point anywhere.
- **Save range naming.** `import_media_to_project` gains an optional `names` map (source path to display name). The export service names the range `<project or timeline name> <mm:ss>–<mm:ss>`.

**Tech Stack:** Rust (Tauri 2 commands, GStreamer/GES via gstreamer-rs, temporalio-sdk 0.4), React 19, Zustand, Radix, Vitest, Playwright.

**Design:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md` (decisions 2, 6 and 7 are binding here).
**Depends on:** nothing in workstreams 01, 03, 04 or 05 functionally. See Cross-workstream coordination for textual conflicts.
**Evidence consumer:** workstream 06 (native Linux evidence) runs after this plan.

## Global Constraints

- **Shell.** Prefix every shell command with `rtk`; environment assignments may precede it (`TAURI_CONFIG=… rtk cargo test …`). `rg` is not installed; use `rtk grep -rn`. Never run `grep -r` over `src-tauri` without narrowing to `src-tauri/src` or `src-tauri/tests`, because `src-tauri/target` is huge. Run long commands (cargo, Playwright, `verify:frontend`) in the foreground.
- **Commits.** Use Conventional Commits, and end every commit message with the trailer
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  Stage only the files the task names (`rtk git add <paths>`), never `git add -A`. Never push.
- **File size.** Keep every file under 600 lines; `scripts/editor-source-policy.test.ts` enforces this for `src/editor`. Put new Rust logic in new modules rather than growing `project_export.rs` (5,067 lines), `workflows/mod.rs` (16,617) or `main.rs` (13,465) beyond the wiring they need.
- **UI styling.** Use design tokens only: no hex colors and no `white/` or `black/` class fragments.
- **Rust/TS lockstep.** This plan adds no project action. Two contracts still change in lockstep:
  - A Tauri command signature change and its `src/lib/project.ts` wrapper land in one commit (Task 10).
  - `JobSummary` / `ProjectJobSummary`, `ProjectExportArtifact` path rules, and `ProjectMediaRenderResult` change in Rust first (Tasks 6–8) and in TS in Task 10, before any TS consumer uses them.
- **License policy.** LGPL GStreamer/GES and WebKitGTK are OK; nothing GPL. Master must not introduce `x264enc`, `x265enc` or any GPL plugin. Use only the reviewed factories in `src-tauri/src/render_pipeline/output_profile.rs` (`openh264enc`, `vah265enc`, `vp8enc`, `avenc_prores_ks`). The encoder policy check `auto_selected_encoder_policy_error` must keep passing.
- **Cargo environment.** Every cargo command in this plan uses
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'` and `--test-threads=1`.
  The shorthand `$CARGO_ENV` below stands for that assignment. Write it out in full, because shell state doesn't persist between agent calls.
- **GES-gated tests.** These need the staged, license-filtered runtime:
  - `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`.
  - Its symlinks point into `/tmp/vc-deb-root`. If `test -e "$VIDEO_CREATER_RENDER_RUNTIME_ROOT/plugins/libgstopenh264.so"` fails, rebuild with `rtk pnpm build:linux-media-runtime --output "$VIDEO_CREATER_RENDER_RUNTIME_ROOT"`.
  - The `project_export` renders also need `VIDEO_CREATER_COMPATIBILITY_DECODER=$PWD/src-tauri/target/debug/video-creater-compatibility-decoder`. Build it with `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo build --manifest-path src-tauri/Cargo.toml -p video-creater-compatibility-decoder`.
  - The shorthand `$GES_ENV` below stands for both variables.
- **Evidence honesty.** Never claim evidence that wasn't observed (design decision 2). A GES test that returns early because a profile is unavailable, such as `render_export_profile_with_fixture` when `vah265enc` has no VA-API device, is "not run", not "passed". Record which tests actually rendered, with the `cargo test` output lines, in the commit body of Task 16.
- **Verify uncertain APIs first.** Where a step depends on a GStreamer property name, verify it against the staged runtime before relying on it (Task 5, step 1). No `gst-inspect-1.0` is installed on this host; the verification is a GES-gated Rust test.

### Cross-workstream coordination

- **`src-tauri/src/project/model.rs` `JobSummary`.** Workstream 01 adds `progress`. If it has landed, add `export_settings` beside it, and reuse its literal-site updates where the same struct literals change.
- **`src-tauri/src/main.rs`.** Workstream 01 moves `import_media_to_project` and the apply/save commands onto a per-project FIFO executor. Task 10 edits `render_media_to_split_project_folder`, `build_temporal_export_media_start_request` and `import_media_to_project`. Rebase onto whichever lands first; keep the argument additions and do not reintroduce synchronous execution.
- **`src-tauri/src/codex/context.rs`.** Workstream 03 splits this file (VC-026). Task 6 touches only one `JobSummary` literal in it (about L1866). If the split has landed, edit the literal in its new module.
- **`src/lib/project.ts`, `knip.jsonc`, `package.json`.** Only Task 10 edits `project.ts` in this plan. Nothing here edits `package.json`. `knip.jsonc` is edited only in Task 15, and only if knip flags something.

---

## File Map

### Rust — new
- **`src-tauri/src/project/export_destination.rs`** (new, under 600 lines):
  - `ExportOutputRequest { file_name: String, directory: Option<String> }` (serde camelCase)
  - `ExportDestinationError` (thiserror, plain-word messages)
  - `validate_export_file_name(name, extension) -> Result<String, ExportDestinationError>`: the stem without a duplicate extension
  - `resolve_export_destination(project_dir, output, extension) -> Result<ExportDestination, _>`, where `ExportDestination { directory: PathBuf, stem: String, extension: String, inside_project_exports: bool }`
  - `ExportDestination::free_path(&self) -> Result<PathBuf, _>`: `Name.ext`, then `Name (2).ext` … `Name (999).ext`
  - `materialize_export_output(request: ExportMaterialization<'_>) -> Result<MaterializedExport, _>`
  - `MaterializedExport { recorded_path: String, absolute_path: PathBuf }`
  - `export_artifact_contract(profile) -> Option<(ProjectExportArtifactKind, &'static str /*ext*/, &'static str /*mime*/)>`
  - `export_artifact_for(job_id, profile, recorded_path, created_at) -> ProjectExportArtifact`
  - unit tests

### Rust — modified
- **`src-tauri/src/edit/render_plan.rs`:** new `ExportEncodeTier { #[default] Standard, Master }` (serde camelCase, `is_standard()`), and `RenderPlan.encode_tier` (`#[serde(default, skip_serializing_if = "ExportEncodeTier::is_standard")]`).
- **`src-tauri/src/project/export_options.rs`:**
  - `ExportRenderOptions` gains `fps: Option<f64>` and `encode_tier: ExportEncodeTier`, both serde-defaulted. The `Eq` derive is dropped, since `f64` isn't `Eq`; the containers only derive `PartialEq`, which was verified.
  - New methods `with_fps`, `with_encode_tier` and `effective_fps(project_fps)`.
  - New error variants `InvalidFrameRate`, `MasterNeedsFinalQuality`, `MasterUnsupportedProfile`.
  - New `JobExportSettings { #[serde(flatten)] options: ExportRenderOptions, output: Option<ExportOutputRequest> }`.
- **`src-tauri/src/project/mod.rs`:** adds `pub mod export_destination;`.
- **`src-tauri/src/render_pipeline/quality.rs`:** new `effective_quality_settings_for_tier` and `master_video_bitrate_kbps`. `effective_quality_settings` stays as the Standard wrapper.
- **`src-tauri/src/render_pipeline/gstreamer_backend.rs`:**
  - tier-aware `encoding_profile`, `quality_video_element_properties`, `webm_encoder_settings` and `configure_platform_encoder` (effort enum);
  - `--encode-tier=master` in `build_command` only for Master, so the goldens without transitions stay unchanged;
  - the test hooks `encoding_profile_summary_for_test` / `webm_encoding_profile_summary_for_test` unchanged in signature.
- **`src-tauri/src/render_pipeline/project_export.rs`:**
  - the fps override on the working project copy;
  - `encode_tier` and the effective fps in `build_project_render_plan_with_range`;
  - new `pub struct MediaExportRequest<'a>` and `pub fn render_media_export_to_split_project_folder`;
  - `output` threaded through `MediaRenderRequest`, `ProjectMediaRenderRequest` and `StartedProjectMediaRender`;
  - materialization and `RecordExportArtifact` in the completion batch;
  - `ProjectMediaRenderResult.export_artifact: Option<ProjectExportArtifact>` (`skip_serializing_if = "Option::is_none"`);
  - the `JobSummary` literal (about L948).
- **`src-tauri/src/project/model.rs`:** `JobSummary.export_settings`.
- **`src-tauri/src/project/action.rs`:**
  - `validate_job_summary` validates `export_settings`;
  - `validate_export_artifact` accepts absolute paths tied to the job's recorded folder;
  - new errors;
  - the `JobSummary` literals.
- **`src-tauri/src/project/split.rs`:** `validate_export_artifact` (about L7772) accepts recorded absolute paths; the `JobSummary` literals.
- **`src-tauri/src/project/export_reveal.rs`:** the absolute recorded-artifact branch and new messages; tests.
- **`src-tauri/src/project/import.rs`:** new `import_media_files_with_names(project_dir, project, sources, names: &BTreeMap<PathBuf, String>)`; `import_media_files` delegates to it with an empty map; tests.
- **`src-tauri/src/workflows/mod.rs`:**
  - `temporal_render_options_from_input` (about L4615) parses `fps` and `encodeTier`;
  - `temporal_export_media_start_request_with_options_and_overwrite` (about L8034) writes `fps`, `encodeTier` and `destination`;
  - `temporal_export_media_workflow_activity_plan_value` (about L4116) passes them into `buildRenderPlanInput`, `renderMediaInput`, `validate*Input` and `writeExportArtifactBaseInput`;
  - `temporal_export_media_write_artifact_activity_value` (about L3790) writes to the destination through `export_destination`;
  - the `RenderPlan` and `JobSummary` literals.
- **`src-tauri/src/main.rs`:**
  - `render_media_to_split_project_folder` (about L3860) gains `fps: Option<f64>`, `encode_tier: Option<ExportEncodeTier>` and `output: Option<ExportOutputRequest>`, and calls `render_media_export_to_split_project_folder`;
  - `build_temporal_export_media_start_request` (about L2752) gains the same three;
  - `import_media_to_project` (about L2137) gains `names: Option<BTreeMap<String, String>>`;
  - tests in `mod tests`.
- **`JobSummary` literal sites** (compile-driven; find them with `rtk grep -rn "provider_request: None" src-tauri/src src-tauri/tests`):
  - `workflows/mod.rs`, `render_pipeline/project_export.rs`, `render_pipeline/codex_e2e.rs`, `codex/context.rs`
  - `project/split/agent_undo_content.rs`, `project/split.rs`, `project/action.rs`
  - `bin/video-creater-compatibility-evidence.rs`, `bin/video-creater-native-export-evidence.rs`
  - `tests/codex_app_server.rs`, `tests/codex_conversation/context.rs`, `tests/codex_conversation/undo_bookkeeping.rs`
  - `tests/project_split.rs`, `tests/project_export_prores_appkit.rs`, `tests/project_export_nested_effect_appkit.rs`
- **`RenderPlan` literal sites** (compile-driven; find them with `rtk grep -rn "RenderPlan {" src-tauri/src src-tauri/tests`):
  - `workflows/mod.rs`, `render_pipeline/gstreamer_transitions_tests.rs`, `render_pipeline/gstreamer_backend.rs`, `render_pipeline/project_export.rs`
  - `render_pipeline/combined_e2e.rs`, `render_pipeline/avfoundation_backend.rs`, `render_pipeline/proposal.rs`, `edit/render_plan.rs`
  - `tests/render_pipeline/transition_fixtures.rs`, `tests/one_click_edit.rs`, `tests/render_transitions_ges/fixtures.rs`, `tests/render_pipeline.rs`

### Rust tests — modified
- `src-tauri/tests/project_export.rs`: GES renders with an output folder, collisions, fps override and Master.
- `src-tauri/tests/render_pipeline.rs`: Master encoding-profile summaries and encoder property verification.
- `src-tauri/tests/temporal_workflows.rs`: start request, plan and writer destination.
- `src-tauri/tests/project_action.rs`: export settings validation and absolute artifact rules.
- `src-tauri/tests/project_split.rs`: an absolute artifact survives save, load and validate.

### TypeScript — new
- **`src/lib/export/export-naming.ts`** and its test:
  - `exportFileNameProblem(name): string | null`, the same rules and words as `validate_export_file_name`;
  - `exportFileLabel(name, extension)`;
  - `artifactFileName(path)`;
  - `saveRangeMediaName(project, range)`.
- **`e2e/editor-export-destination.spec.ts`:** folder, fps, Master, collision naming and Retry preset.
- **`e2e/editor-save-range.spec.ts`:** saved range media name.

### TypeScript — modified
- **`src/lib/project.ts`** and `src/lib/project.test.ts`:
  - `ExportEncodeTier`, `ExportOutput`, `JobExportSettings`;
  - `ProjectJobSummary.exportSettings?`, `ProjectMediaRenderResult.exportArtifact?`;
  - `renderMediaToSplitProjectFolder` input `fps?` / `encodeTier?` / `output?`;
  - `ExportMediaStartInput` with the same three;
  - `importMediaToProject` input `names?`.
- **`src/lib/export/export-plan.ts`** and its test: `ExportChoices.fps` / `.directory`, `exportFrameRateOptions`, Master availability, the plan's `render` / `temporal` fields, `exportChoicesFromPreset` from job settings, and Master size estimates.
- **`src/lib/export/profiles.ts`** and its test: `inProcessExportLabel(profile, quality, encodeTier?)`.
- **`src/lib/jobs/activity-records.ts`** and its test: the export artifact path wins over the render report output path.
- **`src/lib/jobs/task-records.ts`** and its test: the export label reads `exportSettings`.
- **`src/editor/store/ui-slice.ts`:** `ExportPreset.settings`.
- **`src/editor/store/jobs-slice.ts`** and its test: `exportPresetFromJob` reads `exportSettings` first.
- **`src/editor/services/export-service.ts`** and its test: sends `fps`, `encodeTier` and `output`; records `exportSettings` on Temporal jobs; toasts from the recorded artifact; names saved ranges.
- **`src/editor/overlays/export-options.tsx`, `export-popover.tsx`** and `export-popover.test.tsx`: Save to chooser, File name hint, Frame rate select, Master enabled.
- **`src/lib/runtime/adapters/tauri-dialog.ts`** and its test: `fixtureExportDirectoryChooserOperation`.
- **`src/lib/runtime/fixture-bootstrap.ts`:** registers the chooser as unavailable by default.
- **`src/lib/runtime/fixtures/export-fixtures.ts`** and its test: output, artifact, collisions and the directory chooser.
- **`src/lib/runtime/fixtures/project-fixtures.ts`** and its test: `names` on import.
- **`e2e/editor-export-tasks.spec.ts`:** the reveal path and toast title change.
- **`docs/product-backlog.md`:** Task 17.

---

## Task Order and Parallelism

| Wave | Tasks (parallel within a wave) | Must follow |
| --- | --- | --- |
| A | 1, 2, 3, 4 | — |
| B | 5, 6 | 5 after 1; 6 after 1 and 2 |
| C | 7 | 6 |
| D | 8, 9 | 2, 6, 7 (8 also after 5, which shares no files but must land before the Master render test) |
| E | 10 | 3, 8, 9 — the only task touching `main.rs` and `src/lib/project.ts` |
| F | 11 | 4, 10 |
| G | 12, 13 | 11 |
| H | 14 | 12, 13 |
| I | 15 | 14 — the only task that may touch `knip.jsonc` |
| J | 16 | all code tasks |
| K | 17 | 16 |

Tasks within a wave own disjoint files. Tasks 1, 6 and 9 all touch `workflows/mod.rs`, and Tasks 1, 6 and 8 touch `project_export.rs`, so those run in separate waves.

---

### Task 1: Encode tier and frame-rate override in export options and render plans (Rust)

**Owns:**
- `src-tauri/src/edit/render_plan.rs`
- `src-tauri/src/project/export_options.rs`
- `src-tauri/src/render_pipeline/quality.rs`
- `src-tauri/src/render_pipeline/project_export.rs`: the plan building and fps override only
- the `RenderPlan` literal sites listed in the File Map
- `src-tauri/tests/project_export.rs`: non-GES plan tests only

**Steps:**
- [ ] **Failing unit tests** (in `export_options.rs` `#[cfg(test)] mod tests`, new):
  - `ExportRenderOptions::new(Mp4H264, Final, 1920, 1080)?.with_fps(Some(29.97))` keeps 29.97.
  - `with_fps(Some(0.0))`, `Some(f64::NAN)` and `Some(240.5)` fail with `InvalidFrameRate`. The allowed range is 1–120 inclusive.
  - `with_encode_tier(Master)` on Draft fails with `MasterNeedsFinalQuality`, and on `ProResMov` with `MasterUnsupportedProfile`.
  - `effective_fps(30.0)` returns the override or 30.
  - Serde: `{"profile":"mp4H264","quality":"final","width":2,"height":2}` deserializes with `fps: None, encode_tier: Standard`, and serializing it back omits `fps` and `encodeTier`.
- [ ] **Failing quality tests** (in `quality.rs`, new `#[cfg(test)]` module):
  - `effective_quality_settings_for_tier(Final, Master, 1920, 1080, 30.0)` gives `video_bitrate_kbps == Some(12_000)` and `speed_hint == "master-quality"`.
  - At 3840×2160, 60 fps: `Some(48_000)`. The ladder is `(final × 2).clamp(12_000, 60_000)`.
  - The Standard tier equals `effective_quality_settings`.
- [ ] **Failing plan test** (`tests/project_export.rs`, new): `project_media_render_plan_applies_frame_rate_override_and_encode_tier`.
  - Call `build_project_media_render_plan_with_options` with options `with_fps(Some(30.0))` and `with_encode_tier(Master)` on the 24 fps `sample_project()`.
  - Assert `plan.fps == 30.0` and `plan.encode_tier == ExportEncodeTier::Master`.
  - Confirm the helper is exported with `rtk grep -n "pub fn build_project_media_render_plan_with_options" src-tauri/src/render_pipeline/project_export.rs`; the Temporal build activity calls it.
- [ ] **Run and expect failures to compile or fail:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- export_options quality --test-threads=1`
- [ ] **Implement:**
  - `ExportEncodeTier` in `render_plan.rs`.
  - The new fields, builders and errors, with error texts in plain words:
    - "Choose a frame rate between 1 and 120 fps."
    - "Master quality needs Final quality."
    - "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality."
  - `RenderPlan.encode_tier`, and update every literal site to `encode_tier: ExportEncodeTier::Standard` unless it is built from options.
  - In `build_project_render_plan_with_range`: `fps: options.effective_fps(project.render_settings.fps)` and `encode_tier: options.encode_tier`.
  - In `render_project_media_after_job_started`, before `expand_project_nested_timelines_for_render`: when `options.and_then(|o| o.fps)` is `Some`, render from a clone with `render_settings.fps` replaced. Use that clone for `preflight_render_storage`, preparation, graphics layers and the plan.
  - The one `ExportRenderOptions { .. }` literal in `render_prepared_preview_frame_to_split_project_folder_with_lease` gets `fps: None, encode_tier: Standard`.
- [ ] **Run:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- export_options quality --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export project_media_render_plan -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo check --manifest-path src-tauri/Cargo.toml --all-targets`

  Expect PASS and a clean check, including the `transition_backends` golden test: `rtk cargo test … --test render_pipeline plans_without_transitions -- --test-threads=1`.
- [ ] **Commit:** `feat(export): add frame-rate override and Master encode tier to export options`

### Task 2: Export destination resolver and materializer (Rust, new module)

**Owns:** `src-tauri/src/project/export_destination.rs` (new) and `src-tauri/src/project/mod.rs`.

**Steps:**
- [ ] **Failing unit tests** (inside the new module):
  - **File names.**
    - `validate_export_file_name("Edison intro", "mp4")` gives `"Edison intro"`.
    - `"Edison intro.MP4"` gives `"Edison intro"`, so no double extension.
    - `""`, `"  "`, `"a/b"`, `"a\\b"`, `"."`, `".."`, `".hidden"`, a name with `\u{0}` or `\n`, and a 181-byte name are each refused with a plain message such as "Export names can't contain slashes.".
  - **Destinations.**
    - `resolve_export_destination(project, {fileName, directory: None}, "mp4")` targets `<project>/exports`, with `inside_project_exports == true`.
    - An absolute directory outside the project is accepted.
    - A relative directory is refused.
    - A missing directory is refused with "The export folder no longer exists.".
    - `<project>/media` and `<project>` itself are refused with "Choose a folder outside the project, or the project's exports folder.".
    - `<project>/exports/sub` is accepted.
  - **Collision-free names.**
    - With `Name.mp4` present, `free_path` returns `Name (2).mp4`; with `Name (2).mp4` also present, `Name (3).mp4`.
    - Exhausting 999 names fails.
  - **Materialization.**
    - `materialize_export_output` into the project's `exports/` records `exports/Name.mp4`. On Unix the file shares the source inode (`std::os::unix::fs::MetadataExt::ino`).
    - Into an outside tempdir it records the absolute path.
    - With `LinkPolicy::CopyOnly`, a test-only field that forces the copy fallback, the bytes are identical and no `.<name>.<job>.partial` staging file remains.
    - It never overwrites an existing file, even when that file appears between `free_path` and the link: create the file inside a test hook and assert the result is `Name (2).mp4`.
    - An empty source file is refused.
  - **Contracts.**
    - `export_artifact_contract(Mp4H264)` gives `(Mp4, "mp4", "video/mp4")`, `ProResMov` gives `(Mov, "mov", "video/quicktime")`, `Webm` gives `(Webm, "webm", "video/webm")`, and `PalmierProject` gives `None`.
    - A cross-check test asserts the extensions equal `crate::render_pipeline::output_profile::gstreamer_output_profile_target(profile).extension`.
    - `export_artifact_for` sets `schema_version: 1`, `id == job_id`, `job_id: Some(job_id)` and the format string `mp4H264` etc. These are the same values the Temporal writer records (`workflows/mod.rs` about L3945).
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::export_destination -- --test-threads=1`
- [ ] **Implement:**
  - **Materialization order.**
    1. `fs::hard_link(source, candidate)`. On `AlreadyExists`, take the next candidate.
    2. On any other error (`EXDEV`, or a filesystem without links), copy to `<dir>/.<stem>.<job>.partial`, check that the length matches, then `fs::hard_link(staging, candidate)`, retrying the next candidate on `AlreadyExists`.
    3. If linking the staging file is unsupported, `rename` only after `!candidate.exists()`. The race window on link-less filesystems is documented in a code comment.
    4. Remove the staging file in every exit path.
  - **Recorded path.** Project-relative (`exports/...`, forward slashes) when the target is under the canonical `<project>/exports`; otherwise the absolute path as a string.
- [ ] **Run and expect PASS:** the same command, then `rtk cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings` with the cargo env.
- [ ] **Commit:** `feat(export): resolve export destinations without overwriting existing files`

### Task 3: Named media imports (Rust)

**Owns:** `src-tauri/src/project/import.rs`.

**Steps:**
- [ ] **Failing tests** in `import.rs` `#[cfg(test)] mod tests` (existing module, about L398):
  - `import_media_files_with_names` given `{ <abs>/renders/save-range-1/output.webm: "Edison Restoration Demo 00:04–00:09" }` records media `name == "Edison Restoration Demo 00:04–00:09"`.
  - The relative path is `media/<id>-edison-restoration-demo-00-04-00-09.webm`, with the stem from `sanitize_file_stem` of the name.
  - A source without a map entry keeps today's stem name.
  - A blank name falls back to the stem.
  - A name over 120 characters is truncated on a char boundary.
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::import -- --test-threads=1`
- [ ] **Implement.** Add `import_media_files_with_names`. Thread an `Option<&str>` display name into `copy_media_asset`. `import_media_files` calls the new function with an empty map, so its callers stay untouched: `codex/tools.rs`, `workflows/mod.rs`, `project/split.rs` and the bins.
- [ ] **Run and expect PASS:** the same command.
- [ ] **Commit:** `feat(media): name imported media from an optional display name`

### Task 4: Export naming helpers (TypeScript, pure)

**Owns:** `src/lib/export/export-naming.ts` and `src/lib/export/export-naming.test.ts` (both new).

**Steps:**
- [ ] **Failing tests:**
  - `exportFileNameProblem` returns null for "Edison intro". For `""`, `"a/b"`, `"a\\b"`, `"."`, `".."`, `".hidden"`, control characters and 181 UTF-8 bytes it returns the exact Rust messages from Task 2. Copy them from the Task 2 constants and keep them identical.
  - `exportFileLabel("Edison intro.mp4", "mp4")` gives `"Edison intro.mp4"`, and `exportFileLabel("Edison intro", "mp4")` gives `"Edison intro.mp4"`.
  - `artifactFileName("/home/me/Movies/Edison intro (2).mp4")` gives `"Edison intro (2).mp4"`, and `"exports/a.mp4"` gives `"a.mp4"`.
  - `saveRangeMediaName`:
    - A project with no or one `timelines` entry gives `"<project.name> 00:04–00:09"`, using `formatDurationLabel` from `src/lib/format.ts` and an en dash.
    - With two or more timelines it uses the active timeline's `name`, found with `activeTimelineId ?? "main"`.
    - A blank name falls back to "Timeline range".
- [ ] **Run and expect FAIL:** `rtk pnpm vitest run src/lib/export/export-naming.test.ts`
- [ ] **Implement** the module; it has no React or store imports.
- [ ] **Run and expect PASS:** the same command.
- [ ] **Commit:** `feat(export): add export file and saved range naming helpers`

### Task 5: Master encoder settings in GStreamer/GES (Rust)

**Owns:** `src-tauri/src/render_pipeline/gstreamer_backend.rs` and `src-tauri/tests/render_pipeline.rs`. **Follows:** Task 1.

**Steps:**
- [ ] **Verify the encoder property names on the staged runtime before relying on them.**
  - Add a GES-gated test to `tests/render_pipeline.rs`: `#[cfg(all(feature = "ges-render", target_os = "linux"))] linux_master_encoder_properties_exist_in_the_curated_runtime`.
  - It initializes through `start_render_process_runtime` (already used by `tests/project_export.rs`; import it the same way) and uses a new `#[doc(hidden)] pub fn encoder_property_names_for_test(factory: &str) -> PipelineResult<Option<Vec<String>>>` in `gstreamer_backend.rs`. That hook creates the element with `gst::ElementFactory::make(factory)` under `gstreamer_runtime_guard` / `init_gstreamer` and returns its property names, or `None` when the factory is missing.
  - Assert these names:
    - `openh264enc`: `bitrate`, `max-bitrate`, `complexity`, `rate-control`
    - `vp8enc`: `target-bitrate`, `deadline`, `cpu-used`, `auto-alt-ref`, `lag-in-frames`
    - `vah265enc`: `bitrate`, `target-usage`, `rate-control`, only when the factory exists; otherwise print `vah265enc not present: not verified`.
  - Run: `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline linux_master_encoder_properties -- --test-threads=1 --nocapture`
  - **If a name is absent,** drop that setting from the Master design below and say so in the commit body. Don't guess alternatives.
- [ ] **Failing profile tests** in `tests/render_pipeline.rs`, next to `gstreamer_ges_encoding_profile_preserves_webm_codec_across_quality_settings` (about L3595):
  - `gstreamer_ges_master_webm_profile_uses_the_master_ladder`: a Final 1920×1080 at 60 fps plan with `encode_tier = Master` gives `target_bitrate_bps == 48_000_000`, `deadline == 1_000_000` and `cpu_used == 0`.
  - `#[cfg(all(feature = "ges-render", not(target_os = "macos")))] linux_master_h264_profile_doubles_the_final_bitrate`: `encoding_profile_summary_for_test(&plan, Mp4H264)` gives `video_factory == "openh264enc"`, Final `video_bitrate_kbps == 6_000` and Master `12_000` at 1920×1080 30 fps, with `!realtime`.
  - `gstreamer_ges_command_records_master_encode_tier_only_for_master`: `build_command` args contain `--encode-tier=master` for Master and no `--encode-tier` argument for Standard.
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline master -- --test-threads=1`
- [ ] **Implement:**
  - Replace the `realtime: bool` in `render_built_timeline` and `configure_platform_encoder` with an `EncoderEffort { Realtime, Final, Master }` derived from `effective_quality_settings_for_tier(...).speed_hint`.
  - `quality_video_element_properties` takes the tier-aware settings; the bitrate comes from the ladder.
  - In `configure_platform_encoder`, add Master-only settings guarded by `find_property`, which is the existing pattern:
    - `openh264enc`: `max-bitrate` = 1.5 × bitrate, in bits per second like `bitrate`, set with `set_property` as a `u32`; `complexity=high`.
    - `vah264enc` / `vah265enc`: `target-usage=1` when present.
    - `vp8enc`: `cpu-used=0`, `auto-alt-ref=true`, `lag-in-frames=25`.
  - `webm_encoder_settings` gains a Master branch: `profile_name: "video-creater-master-web"`, the ladder bitrate, `deadline 1_000_000`, `cpu_used 0`.
  - `avenc_prores_ks` is unchanged, because Task 1 refuses Master for ProRes.
  - **AVFoundation (macOS)** uses presets without a bitrate; leave `avfoundation_backend.rs` untouched and note in a code comment that Master equals Final on the native exporter. It can't be verified on this host.
- [ ] **Run and expect PASS:** the same command, then the whole target to check for regressions:
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1`
- [ ] **Commit:** `feat(render): encode Master exports with the highest-quality encoder settings`. The body lists the verified property names and any setting dropped.

### Task 6: Record export settings on jobs (Rust)

**Owns:**
- `src-tauri/src/project/model.rs`
- `src-tauri/src/project/export_options.rs` (`JobExportSettings`)
- `src-tauri/src/project/action.rs` (`validate_job_summary`)
- every `JobSummary` literal site in the File Map
- `src-tauri/tests/project_action.rs`: the job tests

**Follows:** Tasks 1 and 2.

**Steps:**
- [ ] **Failing tests** (`tests/project_action.rs`):
  - `record_job_accepts_valid_export_settings`: a `render_draft` job with `export_settings: Some(JobExportSettings { options: Mp4H264/Final/1920×1080/fps 25/Master, output: Some({fileName: "Edison intro", directory: None}) })` is recorded and round-trips through `serde_json`. The JSON key is `exportSettings`, with `encodeTier: "master"` and `fps: 25.0`.
  - `record_job_rejects_invalid_export_settings`: each of these fails with an error that names the rule, and leaves the project unchanged:
    - a file name with a slash;
    - fps 0;
    - Master with Draft;
    - Master with ProRes;
    - a relative `directory`.
  - `job_summary_without_export_settings_serializes_unchanged`: no `exportSettings` key.
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action record_job -- --test-threads=1`
- [ ] **Implement:**
  - Add the field with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  - Validate in `validate_job_summary`:
    - rebuild the options through `ExportRenderOptions::new(..).with_fps(..).with_encode_tier(..)`;
    - `validate_export_file_name`;
    - `directory`, when present, must be absolute. Existence is checked at write time, not here, so reloads of old projects stay valid.
  - Add `export_settings: None` to every literal. Use `rtk cargo check --all-targets` as the driver.
- [ ] **Run and expect PASS:**
  - the same test command;
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo check --manifest-path src-tauri/Cargo.toml --all-targets`;
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --test-threads=1`.
- [ ] **Commit:** `feat(jobs): record export settings on export jobs`

### Task 7: Absolute export artifacts and restricted reveal (Rust)

**Owns:**
- `src-tauri/src/project/action.rs` (`validate_export_artifact`)
- `src-tauri/src/project/split.rs` (`validate_export_artifact`)
- `src-tauri/src/project/export_reveal.rs`
- `src-tauri/tests/project_action.rs`: the artifact tests
- `src-tauri/tests/project_split.rs`

**Follows:** Task 6.

**Steps:**
- [ ] **Failing action tests** (`tests/project_action.rs`, near `record_export_artifact_rejects_paths_outside_exports_folder` at about L772):
  - `record_export_artifact_accepts_an_absolute_path_in_the_jobs_chosen_folder`: the job `export-mp4H264-1` has `export_settings.output.directory = "/tmp/vc-exports"`, and the artifact path is `/tmp/vc-exports/Edison intro (2).mp4`. The folder needn't exist, because action validation is lexical.
  - `record_export_artifact_rejects_an_absolute_path_the_job_did_not_choose`, with four variants: no job, a job without settings, a different directory, and a nested subdirectory. The error message is "export artifact path must stay under exports/ or in the folder its export chose".
  - `record_export_artifact_rejects_absolute_paths_with_parent_components`: `/tmp/vc-exports/../etc/x.mp4`.
  - The existing tests keep passing, including the `renders/` rejection and the mp4/mov/palmier contracts.
- [ ] **Failing split test** (`tests/project_split.rs`, new): `split_project_keeps_a_recorded_absolute_export_artifact`.
  - Save a project with that job and artifact, reload it, and run the split validation.
  - Validate with `validate_split_project` (`src-tauri/src/project/split.rs` about L3779), as the file's existing validation tests do.
  - Assert no issue mentions the artifact path.
- [ ] **Failing reveal tests** (`export_reveal.rs` tests module):
  - `reveal_export_artifact_reveals_a_recorded_absolute_artifact_outside_the_project`: an outside tempdir file recorded as an absolute artifact is revealed at its canonical path.
  - `reveal_export_artifact_refuses_an_unrecorded_absolute_path_outside_the_project`: a sibling file in the same outside folder gets `UNRECORDED_EXPORT_MESSAGE`.
  - `reveal_export_artifact_refuses_a_recorded_absolute_symlink` (`#[cfg(unix)]`): the message is `UNRECORDED_EXPORT_MESSAGE`.
  - `reveal_export_artifact_reports_a_missing_absolute_artifact_plainly`: "This file is no longer in the folder it was exported to.".
  - The existing tests keep passing, notably `reveal_export_artifact_refuses_parent_traversal_and_paths_outside_the_project`. Its `outside.path()` case is unrecorded, so it stays refused.
- [ ] **Run and expect FAIL:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::export_reveal -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action record_export_artifact -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split split_project_keeps -- --test-threads=1`
- [ ] **Implement:**
  - **Action validation.** An absolute path must have only `RootDir`/`Prefix`/`Normal` components, and its parent must equal the recorded `export_settings.output.directory` of the job named by `artifact.job_id`. Otherwise keep the existing relative rules. The profile contract (`validate_export_artifact_profile_contract`) applies to both.
  - **Split validation.** Skip `validate_project_relative_path_value` for absolute paths. Add a lexical check with no `..`, and issue text "Export artifact paths outside the project must be absolute without parent components.".
  - **Reveal.** In `reveal_recorded_export_with`, when `artifact_path` is absolute and `project_relative` returns `None`:
    1. Find a recorded `export_artifacts[].path` that is absolute and lexically equal. Render report paths are never absolute. None found: `UNRECORDED_EXPORT_MESSAGE`.
    2. `symlink_metadata` missing: the new missing message. A symlink: `UNRECORDED_EXPORT_MESSAGE`.
    3. Canonicalize and call `reveal`.
  - Update the doc comments on `recorded_export_paths` and on `revealExportArtifactInSplitProjectFolder` (TS doc in Task 10).
- [ ] **Run and expect PASS:** the three commands, plus `--bin video-creater reveal_export_artifact` (the existing `main.rs` test, about L12379).
- [ ] **Commit:** `feat(export): record and reveal exports saved outside the project folder`

### Task 8: In-process export output, artifact and settings (Rust)

**Owns:** `src-tauri/src/render_pipeline/project_export.rs` (render request plumbing and completion) and `src-tauri/tests/project_export.rs` (GES export tests). **Follows:** Tasks 2, 5, 6 and 7.

**Steps:**
- [ ] **Failing non-GES test:** `media_export_refuses_a_destination_inside_the_project_before_rendering`.
  - `render_media_export_to_split_project_folder` with `output: {fileName: "x", directory: Some(<project>/media)}` fails with path `export.output` and the Task 2 message.
  - No job is recorded, and no `renders/<job>` directory exists. The destination check runs before `register_project_render_attempt`.
- [ ] **Failing GES tests** in `tests/project_export.rs`. Follow `render_export_profile_with_fixture` (about L677): call `start_render_process_runtime`, generate the 320×180, 24 fps, 4 s fixture, and return early with an explicit `eprintln!("… not run: {reason}")` when a profile is unavailable.
  1. `in_process_export_records_a_named_artifact_in_the_project_exports_folder` (MP4 H.264):
     - `result.export_artifact.path == "exports/Edison intro.mp4"`;
     - the file shares the render output's inode on Unix;
     - the reloaded project has the artifact and the job's `export_settings` (profile, quality, output);
     - the job status is `Completed`.
  2. `in_process_export_to_a_chosen_folder_never_overwrites`:
     - an outside tempdir already holds `Edison intro.mp4` with the bytes `b"keep"`;
     - the artifact path is `<dir>/Edison intro (2).mp4`, absolute;
     - the original still reads `b"keep"`;
     - a second export records `(3)`.
  3. `in_process_export_applies_the_frame_rate_override`: options `with_fps(Some(30.0))` on the 24 fps project. `probe_media_with_gstreamer(output, …)` reports `video.fps` within 0.01 of 30.0. Import `probe_media_with_gstreamer` as `tests/render_pipeline.rs` does.
  4. `in_process_master_export_renders_h264_and_webm`: for `Mp4H264` and `Webm` with Master, the render succeeds, `render_report.command.args` contains `--encode-tier=master`, validation passes, and the job settings record `encodeTier: master`. If the `vp8enc` alternate-reference settings from Task 5 make `validate_rendered_media` or webmmux fail, remove `auto-alt-ref`/`lag-in-frames` in `gstreamer_backend.rs`, amend Task 5's evidence in this commit's body, and re-run.
  5. `in_process_export_leaves_no_artifact_when_cancelled`: follow the existing cancellation tests. Find them with `rtk grep -n "cancel" src-tauri/tests/project_export.rs`, and pick the one that cancels a registered attempt. A cancelled export records no artifact and leaves no file in the chosen folder.
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a VIDEO_CREATER_COMPATIBILITY_DECODER=$PWD/src-tauri/target/debug/video-creater-compatibility-decoder rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- in_process_ media_export_refuses --test-threads=1 --nocapture`
- [ ] **Implement:**
  - `pub struct MediaExportRequest<'a> { project_dir, project_id, options, job, updated_at, run_id, range_seconds, timeline_id, output: Option<&'a ExportOutputRequest> }`.
  - `pub fn render_media_export_to_split_project_folder(request)`:
    1. Resolve the destination first; it only validates here.
    2. Set `job.export_settings = Some(JobExportSettings { options, output: output.cloned() })` when `output` is `Some` and the job has none.
    3. Register the attempt and take the leases exactly like `render_media_to_split_project_folder_for_timeline`, whose body now delegates with `output: None`.
  - Thread `output` through `MediaRenderRequest`, `ProjectMediaRenderRequest` (WebM path: `None`) and `StartedProjectMediaRender`.
  - In `render_project_media_after_job_started`, after `cancellation.begin_completion()` succeeds:
    1. Materialize via `materialize_export_output`, with the profile from `export_profile`, job id and `updated_at`.
    2. Push `ProjectAction::RecordExportArtifact { artifact }` into the completion batch after `AttachRenderReport`.
    3. If that batch write fails, remove `materialized.absolute_path` (best effort) before returning the error.
  - Map `ExportDestinationError` to `PipelineError::new(PipelineErrorCode::PipelineInputInvalid, "export.output", <message>, "Choose another export folder or name, then export again.")`.
  - `media_result_from_write` gains the `export_artifact` argument.
  - `render_media_to_split_project_folder` (non-timeline, used by Temporal `RenderMedia` and the evidence bins) keeps `output: None`, so Temporal still materializes in `WriteExportArtifact`.
- [ ] **Run and expect PASS:** the same command. Then run the whole target for regressions, noting which render tests printed "not run":
  `… rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1`
- [ ] **Commit:** `feat(export): save in-process exports to a named file and record the export artifact`

### Task 9: Temporal export destination, frame rate and Master (Rust)

**Owns:** `src-tauri/src/workflows/mod.rs` and `src-tauri/tests/temporal_workflows.rs`. **Follows:** Tasks 2, 6 and 7. May run in parallel with Task 8.

**Steps:**
- [ ] **Failing tests** in `tests/temporal_workflows.rs`, next to `temporal_export_media_start_request_serializes_policy_checked_payload_without_secret` (about L2220) and the writer tests (about L3249–L3390):
  - `temporal_export_media_start_request_carries_fps_encode_tier_and_destination`:
    - options `with_fps(Some(25.0))` and Master, and `ExportOutputRequest { file_name: "Edison intro", directory: Some("/tmp/vc-exports") }`;
    - `input.fps == 25.0`, `input.encodeTier == "master"`, `input.destination == {"fileName": "Edison intro", "directory": "/tmp/vc-exports"}`;
    - with Standard options and no destination, no `fps`, `encodeTier` or `destination` keys appear.
    - Pass the output through the new function `temporal_export_media_start_request_with_output(project_id, project_dir, job_id, options, output_path, output: Option<&ExportOutputRequest>)`. The existing `…_with_options` delegates with `None`.
  - `temporal_render_options_from_input_reads_fps_and_encode_tier`: parses them, and rejects `encodeTier: master` with `quality: draft` as `TemporalWorkflowInputError::Render` carrying the Task 1 message.
  - `temporal_export_media_workflow_activity_plan_passes_export_settings_through`:
    - `buildRenderPlanInput`, `renderMediaInput` and `validateRenderedMediaInput` carry `fps` and `encodeTier`;
    - `writeExportArtifactBaseInput` also carries `destination`;
    - the legacy `outputPath` checks still apply.
  - `temporal_export_media_writer_saves_to_the_chosen_folder_without_overwriting`:
    - follow `temporal_export_media_writer_materializes_validated_webm_without_secondary_encoder` for the fabricated validated render;
    - with `destination` pointing at a tempdir that already holds `Edison intro.webm`, the output is `artifactPath == "<dir>/Edison intro (2).webm"` and `writtenPath` equals it;
    - the reloaded project has an absolute artifact whose job carries `export_settings`. Record the job with settings first, as the editor will.
  - `temporal_export_media_writer_defaults_destination_to_project_exports`: `destination: {fileName: "Edison intro"}` with no directory gives `exports/Edison intro.webm`.
  - The existing tests keep passing, including `temporal_export_media_writer_respects_overwrite_for_validated_h264_copy` (legacy `outputPath` with no destination).
- [ ] **Run and expect FAIL:**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- temporal_export_media temporal_render_options --test-threads=1`
- [ ] **Implement:**
  - Parse the options with the Task 1 builders.
  - In the start request, write `fps` and `encodeTier` only when they aren't the defaults, and `destination` only when an output is given.
  - In the plan value, add them to each input object.
  - In the writer, when `destination` is present, replace the `output_absolute` / staging / rename block with `export_destination::materialize_export_output` from the validated render output. Keep the byte-identical check for the copy path by comparing lengths plus the existing `files_have_identical_bytes`. Record `export_artifact_for(...)` with the `recorded_path`, and return `artifactPath` / `writtenPath` from it.
  - Without `destination`, keep the current behavior exactly.
  - The workflow driver needs no change. It copies `writeExportArtifactBaseInput` verbatim (about L9453). `temporal_export_media_workflow_activity_plan_value` runs inside the `VideoCreaterExportMediaWorkflow` body (about L9356), so only add fields to its JSON. Don't add, remove or reorder activities, because in-flight runs must replay deterministically. Say so in the commit body.
- [ ] **Run and expect PASS:** the same command, then the whole target:
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --test-threads=1`
- [ ] **Commit:** `feat(workflows): export Temporal renders to a chosen folder with frame rate and Master`

### Task 10: Command contract, Rust and TypeScript together (sequential)

**Owns:** `src-tauri/src/main.rs`, `src/lib/project.ts` and `src/lib/project.test.ts`. **Follows:** Tasks 3, 8 and 9.

**Steps:**
- [ ] **Failing TS tests** in `src/lib/project.test.ts`, next to the existing command assertions at about L138, L673, L736 and L1040:
  - `renderMediaToSplitProjectFolder({ …, fps: 25, encodeTier: "master", output: { fileName: "Edison intro", directory: "/tmp/vc-exports" } })` invokes `render_media_to_split_project_folder` with those keys.
  - `buildTemporalExportMediaStartRequest` forwards the same three.
  - `importMediaToProject({ …, names: { "/p/renders/x/output.webm": "Edison 00:04–00:09" } })` forwards `names`.
- [ ] **Failing Rust test** in `main.rs` `mod tests`: `import_media_to_project_names_media_from_the_names_map`. Save a split project and write a small `.png` source (import accepts images). Call `super::import_media_to_project(dir, project, vec![src], Some(map))` and assert the media name. If workstream 01 made the command async, use its test pattern.
- [ ] **Run and expect FAIL:**
  - `rtk pnpm vitest run src/lib/project.test.ts`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater import_media_to_project -- --test-threads=1`
- [ ] **Implement, Rust.**
  - **`render_media_to_split_project_folder`.**
    - New args `fps: Option<f64>`, `encode_tier: Option<ExportEncodeTier>`, `output: Option<ExportOutputRequest>`.
    - Build the options with `ExportRenderOptions::new(..)?.with_fps(fps)?.with_encode_tier(encode_tier.unwrap_or_default())?`.
    - Call `render_media_export_to_split_project_folder`.
  - **`build_temporal_export_media_start_request`.** The same three args, calling `temporal_export_media_start_request_with_output`.
  - **`import_media_to_project`.** `names: Option<BTreeMap<String, String>>`, mapped to `PathBuf` keys, calling `import_media_files_with_names`.
  - **Registration.** The `invoke_handler` list (about L7171) is unchanged because the command names are unchanged.
- [ ] **Implement, TS.**
  - Add `export type ExportEncodeTier = "standard" | "master"`.
  - Add `export interface ExportOutput { fileName: string; directory?: string | null }`.
  - Add `export interface JobExportSettings { profile: Exclude<ExportProfile, "palmierProject">; quality: ExportQuality; width: number; height: number; fps?: number | null; encodeTier?: ExportEncodeTier; output?: ExportOutput | null }`.
  - Add the optional fields on `ProjectJobSummary`, `ProjectMediaRenderResult`, `renderMediaToSplitProjectFolder`'s input, `ExportMediaStartInput` and `importMediaToProject`'s input.
  - Update the `revealExportArtifactInSplitProjectFolder` doc comment for recorded absolute artifacts.
- [ ] **Run and expect PASS:**
  - both commands above;
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater -- --test-threads=1`;
  - `rtk pnpm lint`.
- [ ] **Commit:** `feat(export): pass export folder, file name, frame rate and Master to export commands`

### Task 11: Export plan with folder, frame rate and Master (TypeScript)

**Owns:** `src/lib/export/export-plan.ts` and `src/lib/export/export-plan.test.ts`. **Follows:** Tasks 4 and 10.

**Steps:**
- [ ] **Failing tests**, updating the existing ones:
  - **Choices.** `ExportChoices` gains `fps: number | null` (null means the timeline rate) and `directory: string | null` (null means the project `exports/` folder). `defaultExportChoices` returns `fps: null, directory: null`.
  - **MP4 1080p High** gives:
    - `render: { projectDir, projectId, profile: "mp4H264", quality: "final", width: 1920, height: 1080, jobId, output: { fileName: "Edison intro", directory: null } }`, with no `fps` or `encodeTier` keys when they are the defaults;
    - `temporal` with the same `output`.
  - **Master** gives `quality: "final"`, `encodeTier: "master"` in `render` and `temporal`, and `codecLabel` stays "H.264".
    - `exportChoiceOptions(...).quality` Master is enabled for MP4 and WebM when Final is available.
    - For ProRes it is disabled with "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality.".
    - When Final is unavailable, Master takes Final's reason.
  - **Frame rate.**
    - `exportFrameRateOptions(timelineFps)` gives `[{ value: null, label: "Timeline (30 fps)" }, 23.976, 24, 25, 29.97, 30, 50, 59.94, 60]`, with labels like "29.97 fps".
    - Choosing 25 on a 30 fps timeline gives `plan.fps === 25` and `render.fps === 25`.
    - Choosing the timeline rate sends no `fps`.
    - Draft with fps null on a 30 fps timeline gives `plan.fps === 24` and `render.fps === 24`. The backend used to render Draft at the timeline rate despite the summary; the explicit rate makes the summary true.
  - **Name.** A name with a slash is blocked with `exportFileNameProblem`'s message.
  - **Size.** `estimatedExportSize` for Master is greater than High and follows the Task 1 ladder: `(final × 2)` clamped to 12,000–60,000 kbps.
  - **Presets.**
    - `exportChoicesFromPreset(base, { profile, quality, settings })` with `settings` (a `JobExportSettings`) restores format, codec, quality (`final` plus `master` gives "master"), resolution from the short side (720 / 1080 / 2160), `fps`, name (`output.fileName`) and `directory`.
    - Without `settings` the behavior is today's.
  - **Retry.** `replanExport` keeps `output`, `fps` and `encodeTier`.
- [ ] **Run and expect FAIL:** `rtk pnpm vitest run src/lib/export/export-plan.test.ts`
- [ ] **Implement.** Remove `masterUnavailableReason`. `backendQuality("master")` is `"final"`, and a new `encodeTierFor(choice)` returns the tier.
- [ ] **Run and expect PASS:** the same command, then `rtk pnpm lint`.
- [ ] **Commit:** `feat(export): plan exports with a folder, frame rate and Master quality`

### Task 12: Retry presets and task artifacts from recorded settings (TypeScript)

**Owns:**
- `src/lib/export/profiles.ts` and `src/lib/export/profiles.test.ts`
- `src/lib/jobs/activity-records.ts` and `src/lib/jobs/activity-records.test.ts`
- `src/lib/jobs/task-records.ts` and `src/lib/jobs/task-records.test.ts`
- `src/editor/store/ui-slice.ts`
- `src/editor/store/jobs-slice.ts` and `src/editor/store/jobs-slice.test.ts`

**Follows:** Task 11. May run in parallel with Task 13.

**Steps:**
- [ ] **Failing tests:**
  - **`profiles.test.ts`.** `inProcessExportLabel("mp4H264", "final", "master")` gives "H.264 Master". Without the tier, today's labels.
  - **`activity-records.test.ts`.** A job with both a render report (`renders/j/output.mp4`) and an export artifact (`exports/Edison intro.mp4`) gets `outputPath === "exports/Edison intro.mp4"`. A render-only job keeps the report path.
  - **`task-records.test.ts`.** An in-process `render_draft` job whose id matches `export-mp4H264-…` and which carries `exportSettings` (Master) is labelled "H.264 Master export". A completed row's `artifactPath` is the recorded artifact, which may be absolute.
  - **`jobs-slice.test.ts`** (extend "re-runs a stored export plan … else opens the Export popover preset from the job", about L333). A job with `exportSettings` and no stored plan opens `exportPopover.preset` with `settings` equal to the job's settings. The existing expectations gain `settings: null`.
- [ ] **Run and expect FAIL:**
  `rtk pnpm vitest run src/lib/export/profiles.test.ts src/lib/jobs/activity-records.test.ts src/lib/jobs/task-records.test.ts src/editor/store/jobs-slice.test.ts`
- [ ] **Implement:**
  - `ExportPreset` gains `readonly settings: JobExportSettings | null`.
  - `exportPresetFromJob` reads `job.exportSettings`, then the start request `encodeTier` / `fps` / `destination` inputs for Temporal jobs recorded before Task 13, then today's fallbacks.
  - `exportLabel` prefers `exportSettings`.
  - `buildActivityJobRecords` puts `exportArtifact?.path` before `renderReport?.outputPath`.
- [ ] **Run and expect PASS:** the same command.
- [ ] **Commit:** `feat(jobs): restore export settings on Retry and show the saved export file`

### Task 13: Export service (TypeScript)

**Owns:** `src/editor/services/export-service.ts` and `src/editor/services/export-service.test.ts`. **Follows:** Task 11. May run in parallel with Task 12.

**Steps:**
- [ ] **Failing tests:**
  - **In process.**
    - `exportVideo(plan)` sends `render_media_to_split_project_folder` with `output: { fileName: "Edison intro", directory: null }`, plus `fps` and `encodeTier` when the plan has them.
    - On completion, the toast title is `Exported ${artifactFileName(result.exportArtifact.path)}`, for example "Exported Edison intro (2).mp4".
    - "Show in folder" reveals `result.exportArtifact.path`, including absolute paths.
    - Without `exportArtifact`, as from an older backend, it falls back to the render report path as today.
  - **Temporal.**
    - `build_temporal_export_media_start_request` receives `fps`, `encodeTier` and `output`.
    - The recorded job (`recordJob`) carries `exportSettings`.
    - When polling merges the completed job and an artifact whose `jobId` matches, the toast uses that artifact's file name and path.
    - Without an artifact the toast is "Exported <name>" and reveals `plan.outputPath`, as today.
  - **Save range.**
    - `saveRangeAsMedia({ startSeconds: 4, endSeconds: 9 })` imports with `names: { [absolute render output]: "Edison Restoration Demo 00:04–00:09" }` on the single-timeline fixture project.
    - The toast reads "Saved Edison Restoration Demo 00:04–00:09 to Media".
    - The media name must come from the `names` map, not from the `import_media_to_project` handler's default stem. Assert it in the mocked handler.
- [ ] **Run and expect FAIL:** `rtk pnpm vitest run src/editor/services/export-service.test.ts`
- [ ] **Implement.** Change `ExportOutcome` to resolve the artifact lazily by job id in `settleAwaited`. `renderedOutputPath` prefers `result.exportArtifact?.path`. Build `exportSettings` from `plan`. Keep the file under 600 lines; move helpers to `src/lib/export/export-naming.ts` only when they are pure.
- [ ] **Run and expect PASS:** the same command, then `rtk pnpm lint`.
- [ ] **Commit:** `feat(export): save exports under their chosen name and name saved ranges`

### Task 14: Export popover — Save to, frame rate and Master (TypeScript UI)

**Owns:**
- `src/editor/overlays/export-options.tsx`
- `src/editor/overlays/export-popover.tsx` and `src/editor/overlays/export-popover.test.tsx`
- `src/lib/runtime/adapters/tauri-dialog.ts` and `src/lib/runtime/adapters/tauri-dialog.test.ts`
- `src/lib/runtime/fixture-bootstrap.ts`

**Follows:** Tasks 12 and 13.

**Steps:**
- [ ] **Failing tests:**
  - **`tauri-dialog.test.ts`.**
    - In the DEV fixture runtime, `chooseExportDirectory("/p/exports")` sends `fixtureExportDirectoryChooserOperation` (`"open_export_directory_dialog"`) with `{ defaultPath }` and resolves its answer.
    - The desktop behavior tests (about L69–L87) are unchanged.
  - **`export-popover.test.tsx`.**
    - **Save to.** It shows `${projectDir}/exports` for both backends; the old expectation `${projectDir}/renders` changes. "Choose export folder" is enabled.
      - Clicking it with a `chooseExportDirectory` mock resolving "/home/me/Movies" shows that folder.
      - "Export video" then sends `output.directory: "/home/me/Movies"`.
      - Resolving null keeps the folder.
      - In the browser runtime (`useRuntimeMode() === "browser"`) the button is `aria-disabled` with "Choosing a folder needs the desktop app.".
    - **File name hint.** A hint under Name reads "Saves as Edison intro.mp4". A name with a slash shows the blocked reason and keeps "Export video" `aria-disabled`.
    - **Frame rate.** Advanced has a "Frame rate" select (`src/components/ui/select.tsx`) listing `exportFrameRateOptions`. Choosing "25 fps" changes the summary to `H.264 · 25 fps · ≈ …` and the render input to `fps: 25`.
    - **Master.** The "Master" radio has no `aria-disabled` for MP4 and WebM. Choosing it updates the summary size, and the export sends `encodeTier: "master"`. With ProRes chosen, Master is disabled with the Task 11 reason. Replace the old "Master quality isn't available in this build yet." assertion.
    - **Preset.** A preset with `settings` restores the folder, fps, Master and the name.
- [ ] **Run and expect FAIL:**
  `rtk pnpm vitest run src/lib/runtime/adapters/tauri-dialog.test.ts src/editor/overlays/export-popover.test.tsx`
- [ ] **Implement:**
  - Remove `exportFolderUnavailableReason`.
  - `ExportOptions` gets the props `projectExportsFolder`, `folderChooser: { disabledReason: string | null; choose(): void }` and `frameRateOptions`.
  - Keep the tokens-only classes, matching the existing `rounded-control`, `bg-raised`, `text-muted-foreground` and `text-dim`.
  - `fixture-bootstrap.ts` adds the new operation to `recognizedUnavailableOperations`.
  - Keep `export-options.tsx` under 600 lines. If it grows past about 300, split the Advanced block into `src/editor/overlays/export-advanced-options.tsx` (new) and add it to this task's owned files.
- [ ] **Run and expect PASS:** the same command, then:
  - `rtk pnpm vitest run src/editor`
  - `rtk pnpm lint`
  - `rtk pnpm test:source-quality`
  - `rtk grep -rnE "#[0-9a-fA-F]{3,8}\b|(white|black)/" src/editor/overlays/export-options.tsx src/editor/overlays/export-popover.tsx`, which must print nothing
- [ ] **Commit:** `feat(export): choose the export folder, file name, frame rate and Master quality`

### Task 15: Fixtures and browser end-to-end coverage

**Owns:**
- `src/lib/runtime/fixtures/export-fixtures.ts` and `src/lib/runtime/fixtures/export-fixtures.test.ts`
- `src/lib/runtime/fixtures/project-fixtures.ts` and `src/lib/runtime/fixtures/project-fixtures.test.ts`
- `e2e/editor-export-tasks.spec.ts`
- `e2e/editor-export-destination.spec.ts` (new)
- `e2e/editor-save-range.spec.ts` (new)
- `knip.jsonc`, only if knip flags something

**Follows:** Task 14.

**Steps:**
- [ ] **Failing fixture tests:**
  - **`export-fixtures.test.ts`.**
    - `render_media_to_split_project_folder` with `output: { fileName: "Edison intro", directory: null }` resolves, after `exportFixtureRenderPolls` reloads, with `exportArtifact.path === "exports/Edison intro.mp4"`. The artifact is recorded in the project, and the job has `exportSettings`.
    - A second identical export records `exports/Edison intro (2).mp4`. The fixture keeps a per-folder set of names, mirroring the Rust collision rule.
    - With `directory: "/tmp/video-creater-exports"` the artifact path is absolute, and `reveal_export_artifact_in_split_project_folder` accepts it (task fixture handler).
    - `open_export_directory_dialog` answers `"/tmp/video-creater-exports"`.
    - Without `output` (save range) no artifact is recorded.
  - **`project-fixtures.test.ts`.** `import_media_to_project` honors `names`.
- [ ] **Run and expect FAIL:**
  `rtk pnpm vitest run src/lib/runtime/fixtures/export-fixtures.test.ts src/lib/runtime/fixtures/project-fixtures.test.ts`
- [ ] **Implement** the fixture handlers.
- [ ] **Update `e2e/editor-export-tasks.spec.ts`:**
  - The toast filter becomes "Exported Edison Restoration Demo.mp4".
  - The reveal expectation becomes `{ projectDir, artifactPath: "exports/Edison Restoration Demo.mp4" }`.
  - The details "Render report" still names `renders/export-mp4H264-…/output.mp4`.
  - The row label becomes "H.264 Final export" if Task 12 changed it for in-process jobs; confirm it in the fixture run.
- [ ] **New `e2e/editor-export-destination.spec.ts`,** desktop 1440×900 and phone 402×874, `openSampleEditor(page, { exportFixture: true })`:
  1. Open Export. Save to shows `/tmp/video-creater-editor-project/exports`. "Choose export folder" then shows `/tmp/video-creater-exports`.
  2. Advanced → Frame rate "25 fps", and the summary shows `· 25 fps ·`. Choose Master; it is not disabled.
  3. Export video. The toast "Exported Edison Restoration Demo.mp4" appears, and "Show in folder" records `artifactPath: "/tmp/video-creater-exports/Edison Restoration Demo.mp4"`.
  4. Export again with the same settings. The toast reads "Exported Edison Restoration Demo (2).mp4".
  5. Screenshot `output/playwright/editor-export-destination/<viewport>-export-popover.png` and read the PNG.

  The fixture reports ProRes as unavailable, so Master's ProRes reason is covered by `export-plan.test.ts` and `export-popover.test.tsx`, not by this spec.
- [ ] **New `e2e/editor-save-range.spec.ts`,** desktop 1440×900, `openAcceptanceEditor` from `e2e/support/editor.ts`:
  1. Select the first clip with `timelineClips(page).first().click()`, then right-click it.
  2. Choose "Save range as media".
  3. After the render fixture completes, the toast reads `Saved Edison Restoration Demo 00:00–<mm:ss> to Media`, built from the clip span.
  4. The Media grid contains an item with that name.

  Confirm the sample's first clip span and project name from `src/lib/sample-project.ts` before hard-coding the time.
- [ ] **Run and expect PASS:**
  - `rtk pnpm vitest run src/lib/runtime/fixtures`
  - `rtk pnpm exec playwright test e2e/editor-export-tasks.spec.ts e2e/editor-export-destination.spec.ts e2e/editor-save-range.spec.ts e2e/editor-export.spec.ts`
  - `rtk pnpm check:unused`. Edit `knip.jsonc` only if knip reports a new unused export that is intentionally public, and justify it in a comment.
- [ ] **Commit:** `test(export): cover export folder, frame rate, Master, collisions and saved range names`

### Task 16: Verification gates and native render evidence

**Owns:** no source files. It creates evidence under `output/gap-closure-02-export/`, which is git-ignored, and fixes only if a gate fails. A fix commit names its files and uses `fix(export): …`.

**Steps:**
- [ ] **Runtime.**
  - `test -e "$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a/plugins/libgstopenh264.so"`; if it fails, `rtk pnpm build:linux-media-runtime --output "$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a"`.
  - Build the decoder: `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo build --manifest-path src-tauri/Cargo.toml -p video-creater-compatibility-decoder`.
- [ ] **Rust suites touched.** Run each in the foreground and save its output with `| tee output/gap-closure-02-export/<target>.log`:
  - `--lib`
  - `--bin video-creater`
  - `--test project_export`
  - `--test render_pipeline`
  - `--test temporal_workflows`
  - `--test project_action`
  - `--test project_split`
  - `--test render_transitions_ges`
  - `--test codex_app_server`
  - `--test codex_conversation`

  Use `TAURI_CONFIG=…`, `VIDEO_CREATER_RENDER_RUNTIME_ROOT=…`, `VIDEO_CREATER_COMPATIBILITY_DECODER=$PWD/src-tauri/target/debug/video-creater-compatibility-decoder` and `-- --test-threads=1`. `codex_app_server` and `codex_conversation` are included because of the `JobSummary` literals.

  The known environment-only lib failure is `settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe` (no staged MCP sidecar). Report it as such, not as a pass.
- [ ] **Evidence summary.** Grep the logs for the Task 5 and Task 8 test names and their `--nocapture` "not run" lines, and record a table in the commit body or `output/gap-closure-02-export/README.md`: test, rendered or not run, reason.
  - H.265 Master is expected "not run" unless `vah265enc` has a VA-API device.
  - `cargo check --target` for macOS is not possible here; the AVFoundation Master behavior is unverified.
- [ ] **Frontend gate.** `rtk pnpm verify:frontend`, which must exit 0. Record the step results.
- [ ] **Policy checks.**
  - `rtk git status --short` shows only intended files.
  - `rtk grep -rn "x264enc\|x265enc" src-tauri/src/render_pipeline` returns nothing new.
  - `rtk pnpm test:source-quality` passes.
- [ ] **Commit.** Only if fixes were needed, as `fix(export): …`. Otherwise there is no commit, and the evidence stays under `output/`.

### Task 17: Update the product backlog

**Owns:** `docs/product-backlog.md`. **Follows:** Task 16. Do this only after its gates passed as observed.

**Steps:**
- [ ] **VC-019** (line about 108). Set the status to `implemented`. The Next action should say what remains unproven at the native boundary and name what the proof is:
  - a Linux desktop smoke export to a chosen folder with Master and a frame-rate override, owned by workstream 06;
  - H.265 Master and macOS AVFoundation Master, which can't be verified on this host;
  - the proof: the GES tests from Task 8, the Temporal writer tests from Task 9, and the e2e specs from Task 15.
- [ ] **VC-020** (line about 109). Set the status to `implemented`. The Next action should cover native Show in folder for an export outside the project in the packaged app (workstream 06). Remove the "Retry … leaves quality at the popover default" and `output.webm` text, which are now fixed.
- [ ] **Header.** Bump "Last updated" to the execution date.
- [ ] **Docs check.** `rtk grep -n "VC-019\|VC-020" docs/product-backlog.md` shows the edited rows only.
- [ ] **Commit:** `docs(backlog): record export destination, Master and export records as implemented`

---

## Acceptance

| Gap | Behavior | Proof |
| --- | --- | --- |
| VC-019 Save to folder | Popover chooser; the backend writes into the chosen folder and never overwrites | `export_destination` unit tests (Task 2); `in_process_export_to_a_chosen_folder_never_overwrites` (Task 8, GES render); `temporal_export_media_writer_saves_to_the_chosen_folder_without_overwriting` (Task 9); `e2e/editor-export-destination.spec.ts` steps 1, 3 and 4 |
| VC-019 file name | Name becomes `<name>.<ext>`; invalid names blocked with the same words in TS and Rust | `validate_export_file_name` tests; `export-naming.test.ts`; `in_process_export_records_a_named_artifact_in_the_project_exports_folder`; popover blocked-name test |
| VC-019 frame rate | Advanced frame-rate select; the render encodes at that rate; Draft truly caps at 24 | `project_media_render_plan_applies_frame_rate_override_and_encode_tier`; `in_process_export_applies_the_frame_rate_override` (probe fps); `export-plan.test.ts` Draft fps; e2e step 2 |
| VC-019 Master | Enabled for MP4 and WebM; highest-quality encoder settings; refused for ProRes | `linux_master_encoder_properties_exist_in_the_curated_runtime`; Master profile summary tests (Task 5); `in_process_master_export_renders_h264_and_webm` (render plus `--encode-tier=master` in the report); popover Master tests; e2e step 2. H.265 and AVFoundation Master are reported as not verified |
| VC-020 in-process artifact | In-process exports record `exportArtifacts` and Show in folder reveals the saved file | Task 8 tests (artifact in the reloaded project); `activity-records.test.ts` precedence; `export-service.test.ts` toast and reveal; `e2e/editor-export-tasks.spec.ts` reveal of `exports/Edison Restoration Demo.mp4` |
| VC-020 Retry quality after restart | Job `exportSettings` restores format, quality (including Master), resolution, fps, name and folder | `record_job_accepts_valid_export_settings`; job settings asserted in Task 8 and Task 9 tests; `jobs-slice.test.ts` preset; `export-plan.test.ts` `exportChoicesFromPreset` |
| VC-020 save-range naming | Saved range media is named `<project or timeline> mm:ss–mm:ss` | `import.rs` named-import tests; `import_media_to_project_names_media_from_the_names_map`; `export-service.test.ts` save range; `e2e/editor-save-range.spec.ts` |
| Decision 7 reveal | Recorded absolute artifacts are revealable; unrecorded paths, symlinks and paths an agent recorded without a matching job folder are refused | `export_reveal.rs` absolute tests; `record_export_artifact_rejects_an_absolute_path_the_job_did_not_choose`; `split_project_keeps_a_recorded_absolute_export_artifact`; e2e step 3 |
| Gates | No regressions | Task 16 logs: the Rust suites listed, `verify:frontend` exit 0, with "not run" items stated |

Native packaged-app evidence (desktop smoke export to a chosen folder, Show in folder in the file manager) belongs to workstream 06 and is not claimed here.
