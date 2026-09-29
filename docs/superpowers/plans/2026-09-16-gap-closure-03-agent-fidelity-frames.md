# Gap Closure 03 — Agent Fidelity and Linux Result Frames Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Each task lists the files it owns, its exact test commands and its commit message.

**Goal:** Close three gaps from the editor redesign on this Linux host, each backed by a test or a retained run:
- **VC-023, result frames on Linux.** `capture_canonical_preview_frame_in_split_project_folder` works on Linux, so AI result cards show post-apply frames there. It uses the Rust canonical frame sampler (Decision 8).
- **VC-022, agent session and undo fidelity** (Decision 16):
  - Each conversation turn records its chat session in the backend, and the editor uses that session instead of inferring it.
  - Undo through the Codex MCP tools cancels the batch's running generations, as editor Undo does.
  - Progress on generated assets the batch didn't create no longer blocks its Undo.
  - Undo snapshots are stored compressed under a retained-size cap. Older files still read.
- **VC-026, agent context size.** `src-tauri/src/codex/context.rs` is split into modules under 600 lines, with no behavior change and the same tests (Decision 19).

**Architecture:**
- **Linux capture.** Today `render_prepared_preview_frame_to_split_project_folder_with_lease` (`src-tauri/src/render_pipeline/project_export.rs`) always renders a one-frame ProRes clip and refuses anything but `avfoundation-native`. On non-macOS targets it now calls a new child module, `project_export/canonical_capture.rs`. That module:
  1. registers a render attempt and records the `captureCanonicalPreviewFrame` job;
  2. prepares the project exactly as a render does (`expand_project_nested_timelines_for_render`, `prepare_project_for_render_cancellable`);
  3. samples the video tracks with `precompose::render_canonical_frame_rgba`;
  4. draws the graphics layers (captions, overlays, HyperFrames scenes, effect layers) for that frame on top, in GES layer order;
  5. writes `renders/<jobId>/preview-qa/preview-frames/preview-0001.png`, `canonical-preview-frame.json`, `report.json` and `render.log`;
  6. attaches the project render report and completes the job.

  macOS keeps the AVFoundation path unchanged.
- **Session per turn.** `SplitAppServerConversationEntry` gains `session_id`. `record_app_server_conversation_turn` resolves or creates the session before it writes the history entry. The editor's `sessionHistoryEntries` filters by `entry.sessionId` when an entry has one, and falls back to today's thread/time inference only for legacy entries.
- **MCP undo.** `video_creater.undo_agent_edit` calls `undo_codex_conversation_edit`, the editor's Undo path: it cancels same-process runs and fal.ai/Replicate requests, and keeps the legacy error strings. `video_creater.apply_project_actions` records a hashed batch entry, like conversation applies, instead of a full `after` snapshot. Without that, a running generation's status would block MCP Undo.

  The MCP sidecar is a separate process and can't reach the app's in-process cancellation registry. A small watcher in the app's in-process generation runner therefore cancels a run once its job has disappeared from the project.
- **Other generations' progress.** A new hash rule, version 4, also ignores progress fields (status, outputs, provider input URLs, created-at, and unplaced completion media) of *background* generated assets. These are assets that existed before the batch and whose progress fields the batch itself didn't change. Undo carries their current progress into the restored project. Placing a background generation's output on the timeline is still a timeline edit, so it still conflicts.
- **Compact snapshots.** `SplitAgentEditHistoryEntry.before` and the legacy `after` become `AgentProjectSnapshot`, which is stored as deflate plus base64 JSON. Reads still accept a plain `VideoProject` object. History keeps at most 20 entries and at most `MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES` of compressed snapshot bytes, and the newest entry is always kept.

**Tech Stack:** Rust (Tauri 2 backend, GStreamer/GES runtime, serde, `flate2` + `base64`), React 19 + Zustand frontend, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`, Decisions 2, 8, 16 and 19, and workstream 03.
**Backlog rows:** VC-022, VC-023 and VC-026 in `docs/product-backlog.md`.
**Depends on:** nothing from workstreams 01, 02, 04 or 05. Workstream 06 records the packaged native run of these flows.

## Global Constraints

- **Commands.**
  - Prefix every shell command with `rtk`. `rg` isn't installed, so use `rtk grep -rn`.
  - Run long commands in the foreground and wait for them to finish.
  - Tool calls time out at 10 minutes, so run large Rust suites one `--test` target at a time.
- **Commits.**
  - Use Conventional Commits. End every commit body with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  - Stage only the files named in the task (`rtk git add <paths>`). Never use `git add -A` or `git add .`.
  - Never push.
- **File size.**
  - Every new source file stays under 600 lines, tests included.
  - Several existing files are already far over 600: `main.rs`, `project/split.rs`, `codex/tools.rs`, `render_pipeline/project_export.rs`, `src/lib/project.ts`. They get only minimal wiring. New logic goes into new modules.
  - Check sizes with `rtk wc -l <files>` before each commit.
- **UI styling.** Tokens only. No hex colors and no `white/` or `black/` class fragments. This plan has no planned UI changes. If one turns out to be necessary, the rule applies.
- **Rust/TS lockstep.** This plan adds no project actions. If a task finds it needs one, it lands in one commit together with the TS union member, `applyProjectActionLocally`, the Codex schema, MCP tool support and the risk classification (safe, like existing clip edits).
- **License policy.**
  - LGPL GStreamer and WebKitGTK are fine. Nothing GPL may be added or linked.
  - The only new crate is `flate2` 1.1.9 (MIT OR Apache-2.0), with backend `miniz_oxide` (MIT OR Zlib OR Apache-2.0). Both are already in `Cargo.lock` through `zip`.
- **Cargo environment.** Commands below use two placeholders. Replace them literally, because shell state doesn't persist between calls.
  - `{CARGO_ENV}` = `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
  - `{GES_ENV}` = `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder`

  Every `cargo` command needs `{CARGO_ENV}`. GES-backed tests (`render_transitions_ges`, `project_export`, capture tests) also need `{GES_ENV}`.
- **Runtime preflight** (before any GES test):
  - Run `rtk ls -L $HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a/lib`. The runtime's symlinks point into `/tmp/vc-deb-root`.
  - If they dangle, run `rtk pnpm build:linux-media-runtime --output $HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`.
  - If the decoder binary is missing, run `rtk pnpm build:compatibility-decoder:linux:dev`, which also copies it to `src-tauri/target/debug/`.
- **Evidence.** Never claim evidence you didn't observe. A skipped, filtered-out or environment-blocked test is reported as not run, with the reason. One known environment-only lib failure isn't a regression, and must be reported as observed: `settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe`, which needs a staged MCP sidecar.
- **Behavior boundaries.**
  - macOS capture stays on AVFoundation and is untouched.
  - Editor Undo semantics stay as they are: Temporal runs aren't cancelled.
  - The `ProjectAgentUndoOutcome` wire shape doesn't change.

## File Map

### VC-026: context split
- `src-tauri/src/codex/context.rs` (modify). This becomes the hub, about 330 lines. It keeps the `use` block, the `MAX_CONTEXT_*` constants, `ProjectSkillBundle`, `VideoEditContext`, `CodexContextError`, `load_project_skill_bundle`, `build_codex_developer_instructions`, `build_video_edit_context`, `build_video_edit_context_with_project_dir`, `build_video_edit_context_with_project_dir_optional`, `CodexConversationContext`, `build_codex_conversation_context` and `read_skill`. It also gains the `mod` declarations and the private `use` of submodule items. The public API is unchanged.
- New submodules under `src-tauri/src/codex/context/`, all created in this plan:
  - `relevance.rs`: `TIER_*`, `ContextReferences`, `ConversationRelevance`, `active_timeline_items`, `generated_output_count`, `select_ranked`, `select_newest` and `conversation_focus_summary`. Currently L292–671.
  - `project_files.rs`: `project_files_summary`, the template override summaries and the label helpers `empty_context_label`, `string_map_context_label` and `json_map_context_label`. Currently L672–851.
  - `history.rs`: generated asset, render report, workflow job and export artifact summaries, `export_capabilities_summary`, `NATIVE_DELIVERY_EXPORT_GUIDANCE`, the status and code helpers, `option_context_label` and `list_context_label`. Currently L852–1207.
  - `timeline.rs`: `TimelineSelection`, the timeline summaries and the item/source/property/kind code helpers, including `compact_context_text`. Currently L1208–1392.
  - `media.rs`: the media library summaries, folder paths, `media_kind_code`, `generated_asset_status_code`, `canonical_prefix`, `timeline_item_count`, `push_truncation_marker` and `transcript_excerpt`. Currently L1393–1572.
  - `tests.rs`: the first five `#[test]` functions, moved verbatim (L1595–1953).
  - `evidence_tests.rs`: the last four `#[test]` functions, moved verbatim (L1954–2207).

### VC-023: Linux result frames
- `src-tauri/src/render_pipeline/project_export/canonical_capture.rs` (new): `capture_canonical_frame_with_sampler(...)`, the capture-local source normalization and the report and evidence writers.
- `src-tauri/src/render_pipeline/project_export/canonical_capture_graphics.rs` (new): the one-frame graphics layer render and the straight-alpha "over" composite.
- `src-tauri/src/render_pipeline/project_export.rs` (modify): `mod canonical_capture;`, the `#[cfg(not(target_os = "macos"))]` dispatch in `render_prepared_preview_frame_to_split_project_folder_with_lease`, and the doc comment.
- `src-tauri/tests/render_transitions_ges/result_frames.rs` (new) and `src-tauri/tests/render_transitions_ges.rs` (modify, `#[path]` module registration).
- `src/lib/project.ts` (modify, doc comment on `captureCanonicalPreviewFrameInSplitProjectFolder` only).

### VC-022: agent fidelity
- `src-tauri/src/project/split.rs` (modify):
  - `SplitAppServerConversationEntry.session_id`;
  - session resolution order in `record_app_server_conversation_turn`;
  - `record_agent_project_action_batch` writes hashed entries;
  - the `SplitAgentEditHistoryEntry` field types and doc comments;
  - new `mod` declarations and re-exports.
- `src-tauri/src/project/split/agent_batch.rs` (modify): the shared hashed entry builder, `legacy_agent_undo_refusal`, and the retained-size budget call. Its inline tests move to `agent_batch_tests.rs` (new) if the file would pass 600 lines.
- `src-tauri/src/project/split/agent_undo_content.rs` (modify): the version dispatch in `entry_matches_project` and `restored_agent_snapshot`.
- `src-tauri/src/project/split/agent_undo_background.rs` (new): the version 4 background progress rule and its carry-forward.
- `src-tauri/src/project/split/agent_history_snapshot.rs` (new): `AgentProjectSnapshot`, the compressed and legacy serde forms, and `retain_agent_edit_history_budget`.
- `src-tauri/src/codex/tools.rs` (modify): `undo_agent_edit_payload`.
- `src-tauri/src/generation/orphan_watch.rs` (new), `src-tauri/src/generation/mod.rs` (modify, `pub mod orphan_watch;`) and `src-tauri/src/main.rs` (modify, `run_generate_media_in_process` wiring only).
- `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` (modify): `flate2 = "1.1.9"`.
- New test modules:
  - `src-tauri/tests/codex_conversation/session_history.rs`
  - `src-tauri/tests/codex_conversation/undo_background_generations.rs`
  - `src-tauri/tests/codex_conversation/history_storage.rs`
  - `src-tauri/tests/codex_mcp_server/undo_generations.rs`

  They are registered in `src-tauri/tests/codex_conversation/main.rs` and `src-tauri/tests/codex_mcp_server.rs` (both modified).
- Frontend (all modified except where noted):
  - `src/lib/project.ts`: `AppServerConversationEntry.sessionId`
  - `src/editor/store/agent-session-actions.ts` and its `.test.ts`
  - `src/lib/runtime/fixtures/conversation-fixture-sessions.ts`
  - `src/lib/runtime/fixtures/conversation-fixtures.test.ts`
  - `src/lib/runtime/fixtures/conversation-fixture-undo.ts` and `src/lib/runtime/fixtures/conversation-fixtures.ts`: version 4 parity
  - `src/lib/runtime/fixtures/conversation-fixture-undo.test.ts` (new)

### Not touched
`knip.jsonc` and `package.json` aren't expected to change. If a task finds it must change them, that task becomes sequential with every other task.

## Task Order and Parallelization

| Task | Gap | Owns (shared files in bold) | Runs after |
| --- | --- | --- | --- |
| 1 | VC-026 | `codex/context.rs`, `codex/context/*` | none (lane A) |
| 2 | VC-023 | **`project_export.rs`**, `project_export/canonical_capture.rs`, `tests/render_transitions_ges.rs`, `tests/render_transitions_ges/result_frames.rs` | none (lane B) |
| 3 | VC-023 | `project_export/canonical_capture*.rs`, `result_frames.rs` | 2 |
| 4 | VC-023 | **`src/lib/project.ts`** (doc comment) | 3, and after 6 (shared `project.ts`) |
| 5 | VC-022 | **`split.rs`**, `tests/codex_conversation/main.rs`, `session_history.rs` | none (lane C) |
| 6 | VC-022 | **`src/lib/project.ts`**, `agent-session-actions.ts`(+test), `conversation-fixture-sessions.ts`, `conversation-fixtures.test.ts` | 5 |
| 7 | VC-022 | **`split.rs`**, **`agent_batch.rs`**, `codex/tools.rs`, `tests/codex_mcp_server.rs`, `tests/codex_mcp_server/undo_generations.rs` | 5 |
| 8 | VC-022 | **`main.rs`**, `generation/mod.rs`, `generation/orphan_watch.rs` | none (lane D); never at the same time as another plan's `main.rs` task |
| 9 | VC-022 | **`split.rs`**, **`agent_batch.rs`**, `agent_undo_content.rs`, `agent_undo_background.rs`, `tests/codex_conversation/main.rs`, `undo_background_generations.rs`, `conversation-fixture-undo.ts`(+new test), `conversation-fixtures.ts` | 7 |
| 10 | VC-022 | **`split.rs`**, **`agent_batch.rs`**, `agent_undo_content.rs`, `agent_history_snapshot.rs`, `Cargo.toml`, `Cargo.lock`, `tests/codex_conversation/main.rs`, `history_storage.rs` | 9 |
| 11 | all | none (verification) | 1–10 |
| 12 | all | `docs/product-backlog.md` | 11 |

- **Parallel lanes.** Lanes A (1), B (2 → 3), C (5 → 7 → 9 → 10, with 6 branching after 5) and D (8) can run in parallel. Task 4 waits for both 3 and 6.
- **Sequential files.**
  - `src-tauri/src/project/split.rs`, `agent_batch.rs` and `tests/codex_conversation/main.rs` belong to lane C and change strictly in the order 5 → 7 → 9 → 10.
  - `src/lib/project.ts` changes in Task 6, then Task 4.
  - `src-tauri/src/main.rs` changes only in Task 8. Other workstreams (for example 01's FIFO executor) also edit `main.rs`, so an orchestrator must never run Task 8 at the same time as them.
  - `Cargo.toml` and `Cargo.lock` change only in Task 10.
- **Cargo in parallel.** Parallel lanes edit different files, but a shared working tree would compile other lanes' unfinished edits. Run each parallel lane in its own git worktree, with `CARGO_TARGET_DIR=/home/olhapi/projects/video-creater/src-tauri/target` so dependency builds are reused, and cherry-pick lane commits onto `main` in table order. Otherwise run the tasks one at a time in the main tree.

---

### Task 1: Split `codex/context.rs` into modules (VC-026)

**Files:**
- Modify `src-tauri/src/codex/context.rs`.
- Create `src-tauri/src/codex/context/{relevance,project_files,history,timeline,media,tests,evidence_tests}.rs`.

**Tests:** the existing tests only. None are added, removed or edited, apart from moving their bodies verbatim.

- [ ] **Baseline.** Record the test list and the results before touching anything:
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib codex::context -- --list`. Expect 9 tests; save the names.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib codex::context -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation context -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1`
- [ ] **Behavior snapshot (scratch, never committed).** Add a temporary `#[test]` at the end of `src-tauri/tests/codex_conversation/context.rs`. Using the fixtures already in that file, it writes `format!("{:#?}", build_codex_conversation_context(...))` and `format!("{:#?}", build_video_edit_context(...))` for a focused project and for an over-cap project to `/tmp/vc-context-before.txt`. Run it with `--nocapture`, then `rtk git checkout src-tauri/tests/codex_conversation/context.rs`.
- [ ] **Move code, one module at a time.**
  - For each submodule in the File Map, cut the listed items verbatim into the new file and add `mod <name>;` in `context.rs`.
  - Items used only inside their new module stay private. Items used by the hub or a sibling become `pub(super)`. Nothing becomes `pub`.
  - Submodules reach hub constants and types through `use super::{...}` or `use super::*`, since child modules can see their parent's private items.
  - Keep `#[cfg(target_os = ...)]` attributes on `NATIVE_DELIVERY_EXPORT_GUIDANCE`.
  - Run `{CARGO_ENV} rtk cargo check --manifest-path src-tauri/Cargo.toml --lib` after each module.
- [ ] **Move tests.**
  - Replace the inline `#[cfg(test)] mod tests { ... }` with `#[cfg(test)] mod tests;` and `#[cfg(test)] mod evidence_tests;`.
  - Each file starts with the original test module's `use` block (`use super::*;` plus the listed `crate::project::model` imports, `serde_json::json`, `BTreeMap` and `Path`). Move the test functions verbatim.
  - Remove imports the compiler reports as unused in each file. Change nothing else.
- [ ] **Sizes.** `rtk wc -l src-tauri/src/codex/context.rs src-tauri/src/codex/context/*.rs`. Every file must be under 600 lines. If one isn't, move whole helper functions into the sibling module that uses them most, and record the move.
- [ ] **Verify.**
  - The `--list` output shows the same 9 test names. Only the module path of the four moved to `evidence_tests` changes.
  - The three baseline commands pass with the same counts.
  - Re-add the scratch snapshot test, write `/tmp/vc-context-after.txt`, and `rtk diff /tmp/vc-context-before.txt /tmp/vc-context-after.txt` shows no difference. Remove the scratch test again.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server -- --test-threads=1`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  - `{CARGO_ENV} rtk cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features mcp-server --bin video-creater-mcp-server -- -D warnings`
- [ ] **Commit:** `refactor(codex): split agent context into focused modules`. The body lists the modules with their line counts and states "no behavior change: same 9 context tests, identical context debug output".

### Task 2: Linux canonical frame capture for video tracks (VC-023)

**Files:**
- Create `src-tauri/src/render_pipeline/project_export/canonical_capture.rs`.
- Modify `src-tauri/src/render_pipeline/project_export.rs`.
- Test: create `src-tauri/tests/render_transitions_ges/result_frames.rs` and modify `src-tauri/tests/render_transitions_ges.rs` (add `#[path = "render_transitions_ges/result_frames.rs"] mod result_frames;`).

- [ ] **Verification: sampler coverage (scratch, not committed).** Confirm what the code suggests before relying on it. Write a throwaway GES test in `result_frames.rs` built on `parity::fixture_dir()` and `parity::load_fixture()`, then record the results in the commit body:
  1. `render_canonical_frame_rgba` on the fixture at 1.0 s returns `width*height*4` bytes that match the warm segment colour at the centre.
  2. A `Caption` or `Overlay` item on a non-video track isn't drawn, because the sampler iterates only `TrackKind::Video`.
  3. An active video-track item whose media is `MediaKind::Generated` with `fps: Some(_)` fails with `precompose.flatten.mediaKind`.
  4. `MediaKind::Image` fails the same way.
  5. A `TimelineSource::Generated` placeholder fails with `precompose.flatten.source`.
  6. `build_project_webm_render_plan` rejects an uncompleted `Generated` source ("Generated render asset is not completed.") and resolves a completed one to its first output media.

  If any observation differs, stop and update this task's normalization rules before implementing.
- [ ] **Failing test: capture writes a sampler frame and records bookkeeping.** In `result_frames.rs`, `capture_writes_the_canonical_frame_and_records_its_job`:
  - Set up the fixture with `fixture_dir()`, `load_fixture()` and `save_split_project(dir, &project)`.
  - Call `render_prepared_preview_frame_to_split_project_folder(dir, 2.0, "agent-result-frame-test", "2026-09-16T10:00:00Z")`.
  - Assert:
    - `preview_frame == "renders/agent-result-frame-test/preview-qa/preview-frames/preview-0001.png"` and the file decodes (`image::open`) to the project `width × height`;
    - its RGBA equals `render_canonical_frame_rgba(dir, &prepared_project, 2.0, None)` exactly, where `prepared_project` comes from `prepare_project_for_render`;
    - `evidence_report` JSON has `"backend": "rust-canonical-sampler"`, `"status": "captured"` and `"previewFrame"` set to the frame path;
    - `render_report.command.program == "rust-canonical-sampler"`, and `renders/agent-result-frame-test/report.json` and `render.log` exist;
    - the returned and reloaded project contain job `agent-result-frame-test` with kind `captureCanonicalPreviewFrame`, status `Completed` and a `render-attempt/` run id, plus a project render report with that id and status `Completed`;
    - `validate_split_project(dir)?.ok` is true, and the project's content (`agent_undo_comparable_content(&loaded, &[])`) equals the saved project's.
  - Copy the PNG and the evidence JSON to `output/linux-result-frames/video-tracks/`.

  Run `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges result_frames -- --test-threads=1`. It fails today with `canonicalPreview.backend`.
- [ ] **Failing test: refusals are plain and leave no running job.** `capture_refuses_image_clips_and_pending_generations_plainly`:
  - An image clip at the playhead fails with path `canonicalPreview.unsupportedSource`, and its message names image clips.
  - A pending generation placeholder at the playhead fails with the same path and a message that names the pending generation.
  - After each failure the job exists with status `Failed`, and no `preview-0001.png` exists.
  - A playhead at or after the timeline end still fails with `playheadSeconds`, as before, and records no job.
- [ ] **Implement `canonical_capture.rs`.** Keep it under 600 lines.
  ```rust
  /// Linux (non-macOS) canonical capture: the Rust sampler instead of a one-frame native render.
  pub(super) fn capture_canonical_frame_with_sampler(
      project_dir: &Path,
      source_project: &VideoProject,
      playhead_seconds: f64,
      frame_end_seconds: f64,
      job: JobSummary,
      updated_at: &str,
      lease: &StorageMutationLease,
  ) -> PipelineResult<PreparedPreviewFrameResult>
  ```
  1. `register_project_render_attempt(project_dir, &source_project.id, &job.id, None)`. Apply `render_job_start_actions(source_project, job, updated_at, &attempt.attempt_id)` through `apply_project_actions_to_split_project`, mapping errors with `render_project_action_error`.
  2. `expand_project_nested_timelines_for_render`, then `prepare_project_for_render_cancellable(project_dir, &expanded, None)`.
  3. Capture-local normalization on a clone, never saved. For each enabled `TrackKind::Video` item active at `[playhead, frame_end)`:
     - a `Generated` source whose asset is `Completed` is rewritten to `Media { media_id: first output }`;
     - an uncompleted `Generated` source, a `Text` source, an image media (`MediaKind::Image`, or `Generated` without `fps`) or a remaining `Timeline` source fails with `PipelineErrorCode::RenderBackendFailed`, path `canonicalPreview.unsupportedSource`, message "Result frames on Linux can't show image clips, text sources or unfinished generations yet." and fix "Open the viewer to check this moment.";
     - `Generated` media with `fps: Some(_)` is given `kind = MediaKind::Video` in the clone.
  4. `render_canonical_frame_rgba(project_dir, &normalized, playhead_seconds, None)`.
  5. Write the PNG with `image::RgbaImage::from_raw(...).save(...)` after `validate_render_project_write_path`. Then write `render.log` (one plain line naming the sampler and the playhead), `report.json` (a `RenderReport` with `command: CommandSpec::new("rust-canonical-sampler")`, `streams` video only, the frame and evidence in `artifacts`, `summary.container`/`video_codec` `"png"`, and requested and actual size) and `canonical-preview-frame.json` (the same schema as the macOS evidence, with `"backend": "rust-canonical-sampler"` and `"sourceOutput"` set to the PNG path).
  6. Apply `AttachRenderReport` (a `ProjectRenderReport` with `status: Completed`, `output_path` set to the PNG, `duration_seconds = frame_end - playhead`, checks `duration: Skipped`, `streams: Passed` and `captionAlignment: Skipped`, `log_path` and `created_at: updated_at`) and `UpdateJobStatus { status: Completed, run_id: Some(attempt_id) }` in one `apply_project_actions_to_split_project` call.
  7. On any error after step 1, apply `UpdateJobStatus { status: Failed }` (best effort) and return the original error.
  8. Return `PreparedPreviewFrameResult { project, playhead_seconds, preview_frame, source_output: preview_frame.clone(), evidence_report, render_report, project_render_report }`.
- [ ] **Wire the dispatch** in `project_export.rs`:
  - Add `mod canonical_capture;` next to the existing module items.
  - In `render_prepared_preview_frame_to_split_project_folder_with_lease`, after the playhead, duration and fps checks and after the `job` is built, add `#[cfg(not(target_os = "macos"))] return canonical_capture::capture_canonical_frame_with_sampler(project_dir, &source_project, playhead_seconds, frame_end_seconds, job, updated_at, lease);`. Gate the rest of the function body (the attempt, the ProRes render, the extraction) with `#[cfg(target_os = "macos")]` so no unused-code warnings appear.
  - Update both doc comments ("macOS: one-frame native render; other platforms: the Rust canonical frame sampler").
- [ ] **Verify.**
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges -- --test-threads=1`: all earlier tests plus the new ones pass.
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1`: 78 or more pass.
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib render_pipeline::project_export -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater preview_render_comparison -- --test-threads=1`. The comparison command also captures through this path. Report the observed count; zero matches is a valid result and must be stated.
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`
  - `rtk wc -l src-tauri/src/render_pipeline/project_export/canonical_capture.rs`
- [ ] **Commit:** `feat(linux): capture agent result frames with the canonical frame sampler`. The body records the coverage observations from step 1.

### Task 3: Draw graphics and generated video in Linux result frames (VC-023)

**Files:**
- Create `src-tauri/src/render_pipeline/project_export/canonical_capture_graphics.rs`.
- Modify `src-tauri/src/render_pipeline/project_export/canonical_capture.rs` and `src-tauri/src/render_pipeline/project_export.rs` (`mod canonical_capture_graphics;` only, if declared there. Prefer declaring it inside `canonical_capture.rs` as a child module).
- Test: modify `src-tauri/tests/render_transitions_ges/result_frames.rs`.

- [ ] **Verification: GES graphics order.**
  - Read `build_ges_timeline` in `src-tauri/src/render_pipeline/gstreamer_backend.rs`. It creates graphics layers before source layers, with `graphics[len-1]` closest to the viewer, so all graphics draw above all video and later graphics draw above earlier ones.
  - Read `build_project_graphics_render_layers` in `project_export.rs`. It sorts by `timeline_start`, then id.
  - Read `graphics_frame_path` in `gstreamer_backend.rs`. It expands `frames_pattern` from `%06d` down to `%d`.

  If any of these differ from this summary, adapt the composite order to what GES does, and note it in the commit body.
- [ ] **Failing test: burned-in caption drawn over video.** `capture_draws_captions_over_the_sampled_video`:
  - Add a caption item (a `TrackKind::Caption` track, `TimelineItemKind::Caption`, and caption properties shaped like an existing caption fixture; find one with `rtk grep -rn "TimelineItemKind::Caption" src-tauri/tests | head`) covering 1.5–2.5 s, with `render_settings.captions = CaptionRenderMode::BurnIn`.
  - Capture at 2.0 s.
  - Assert the PNG differs from the pure sampler frame inside the caption's bounding region and is identical outside it, then copy it to `output/linux-result-frames/caption/`.
  - Add `capture_matches_the_ges_render_with_a_caption`, which renders the same project with GES using `build_project_webm_render_plan` and `render` from `fixtures.rs`, as `parity.rs` does. It compares the capture with GES frame 48 through `scripts/compare-preview-render-frames.mjs`, with threshold `0.01` and channel threshold `16`. Evidence goes to `output/linux-result-frames/caption-parity/`. If text anti-aliasing exceeds `0.01`, don't raise the threshold silently: record the observed ratio in the commit body and use the `project_export_nested_effect_appkit` policy (`0.08` / `18`) with that note.
- [ ] **Failing test: completed generated video output.** `capture_draws_a_completed_generated_video_clip`:
  - Replace clip 0's media with a `MediaKind::Generated` asset with `fps: Some(24.0)` that points at `media/warm.webm`.
  - Capture at 1.0 s and assert the PNG equals the sampler frame of the same project with that media's kind set to `Video`.
- [ ] **Implement `canonical_capture_graphics.rs`:**
  - `render_capture_graphics(project_dir, prepared_expanded_project, playhead, frame_end, render_dir) -> PipelineResult<Vec<(PathBuf, GraphicsReportEntry)>>` calls the parent's `build_project_graphics_render_layers(project, width, height, fps, Some((playhead, frame_end)))` and `render_project_graphics_layers(&layers, project_dir, project_dir.join(render_dir).join("graphics"), None)`, then picks frame 0 of each manifest with the same pattern expansion as GES. Child modules can use the parent's private items. If a helper lives in `gstreamer_backend.rs` and is private there, copy the ten-line pattern expansion instead of widening that file's API.
  - `composite_over(canvas: &mut [u8], layer: &[u8])` does straight-alpha sRGB source-over per pixel. Add a unit test in the same file: opaque over, transparent over, and 50 % over with expected bytes.
  - `canonical_capture.rs` calls it after sampling and composites each layer frame in index order. It adds `RenderGraphicsReport` entries (renderer and template ids from the manifest) to `render_report.graphics`, and the graphics frame paths to `artifacts`.
  - Remove the `Generated`-video refusal that Task 2 left in place, if any. The normalization already maps it.
- [ ] **Verify.**
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges result_frames -- --test-threads=1`
  - The full `render_transitions_ges` target.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib canonical_capture -- --test-threads=1`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`
  - `rtk wc -l` on both capture modules.
- [ ] **Commit:** `feat(linux): draw graphics and generated video in captured result frames`

### Task 4: Capture docs and AI result frame regression (VC-023)

**Files:** modify `src/lib/project.ts` (doc comment above `captureCanonicalPreviewFrameInSplitProjectFolder` only).
**Runs after:** Tasks 3 and 6.

- [ ] Replace "Renders one frame of the saved split project's primary timeline through the export path." with: "Captures one frame of the saved split project's primary timeline: a one-frame native render on macOS, the Rust canonical frame sampler with graphics overlays elsewhere. Image clips, text sources and unfinished generations can't be captured on Linux yet, and the call rejects them."
- [ ] **Verify.** The frontend contract is unchanged, and these runs are the regression evidence for result cards:
  - `rtk pnpm vitest run src/lib/project.test.ts src/editor/panels/ai/ai-cards.test.tsx src/lib/runtime/fixtures/conversation-fixtures.test.ts`
  - `rtk pnpm exec playwright test e2e/editor-ai.spec.ts`: the "auto-apply on" flow asserts a decoded result frame image at both viewports.
  - `rtk pnpm lint`
- [ ] **Commit:** `docs(project): describe canonical frame capture on Linux and macOS`

### Task 5: Record each turn's chat session in the backend (VC-022)

**Files:**
- Modify `src-tauri/src/project/split.rs`.
- Test: create `src-tauri/tests/codex_conversation/session_history.rs` and modify `src-tauri/tests/codex_conversation/main.rs` (`mod session_history;`).

- [ ] **Failing tests** in `session_history.rs`. Use `support::saved_split_project`, `record_app_server_conversation_turn`, `apply_agent_session_action` (`AgentSessionAction::Create` / `Select`), `load_app_server_conversation_history` and `load_agent_session_manifest`.
  1. `a_turn_records_the_session_it_was_filed_under`: the first turn on thread-1 creates `agent-session-1`, and the entry's `session_id` is `Some("agent-session-1")`.
  2. `a_turn_in_a_new_active_chat_records_that_chat`: create and select session B (`threadId: None`), then record a turn on thread-2. The entry has B's id, and B's `thread_id` becomes thread-2.
  3. `a_turn_on_an_earlier_thread_records_that_threads_session`: with B active, a turn on thread-1 records session A, which matches the current filing rule (thread first).
  4. `legacy_entries_without_a_session_still_load`: write `context/app-server-conversations.json` with one entry lacking `sessionId`. It loads with `session_id: None`, and re-serializing omits the key.

  Run `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation session_history -- --test-threads=1`. It doesn't compile yet, because the field is missing.
- [ ] **Implement.**
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub session_id: Option<String>` to `SplitAppServerConversationEntry`, with a doc comment: "The chat session this turn was filed under. Entries written before 2026-09-16 have none."
  - In `record_app_server_conversation_turn`, read the session manifest and resolve or create the session index before pushing the history entry. The logic moves; it doesn't change. Set `session_id` from it. Keep the write order: history file first, then manifest.
- [ ] **Verify.**
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`: 70 or more pass.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::split -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --test-threads=1`: the Temporal worker records turns too.
  - `rtk pnpm rust:fmt`
- [ ] **Commit:** `feat(agent): record each conversation turn's chat session`

### Task 6: Load chat history by recorded session (VC-022)

**Files (all modified):**
- `src/lib/project.ts` (`AppServerConversationEntry`)
- `src/editor/store/agent-session-actions.ts` and `src/editor/store/agent-session-actions.test.ts`
- `src/lib/runtime/fixtures/conversation-fixture-sessions.ts`
- `src/lib/runtime/fixtures/conversation-fixtures.test.ts`

- [ ] **Failing tests.**
  - `agent-session-actions.test.ts`, "assigns history by recorded session, and infers only for entries without one":
    - an entry with `sessionId: "s-2"` and a thread matching `s-1` is listed under `s-2` only;
    - an entry whose `sessionId` names a deleted session appears under that session and not under the active one;
    - an entry without `sessionId` keeps the existing thread-then-creation-window rule;
    - `sessionId === null` input (no sessions) still returns all entries.
  - `conversation-fixtures.test.ts`, extend "keeps chats and turn history in an in-memory manifest": after two turns in two chats, each history entry carries the `sessionId` of the chat the turn was filed under.

  Run `rtk pnpm vitest run src/editor/store/agent-session-actions.test.ts src/lib/runtime/fixtures/conversation-fixtures.test.ts`.
- [ ] **Implement.**
  - `AppServerConversationEntry` gets `sessionId?: string | null;` with the same doc comment as Rust.
  - Change the doc comment and body of `sessionHistoryEntries`:
    ```ts
    return entries.filter((entry) =>
      typeof entry.sessionId === "string" ? entry.sessionId === session.id : inferredMatch(entry),
    );
    ```
    `inferredMatch` is today's thread or creation-window rule, unchanged.
  - The fixture's `recordTurn` files the turn first, then pushes the history entry with `sessionId: target.id`.
- [ ] **Verify.**
  - `rtk pnpm vitest run src/editor/store src/lib/runtime/fixtures src/lib/project.test.ts`
  - `rtk pnpm lint`
  - `rtk pnpm check:unused`
  - `rtk pnpm exec playwright test e2e/editor-ai.spec.ts`
- [ ] **Commit:** `feat(agent): load chat history by the recorded session`

### Task 7: MCP agent edits record hashed batches, and MCP Undo cancels generations (VC-022)

**Files:**
- Modify `src-tauri/src/codex/tools.rs`, `src-tauri/src/project/split.rs` and `src-tauri/src/project/split/agent_batch.rs`. Create `src-tauri/src/project/split/agent_batch_tests.rs` only if `agent_batch.rs` would pass 600 lines.
- Test: create `src-tauri/tests/codex_mcp_server/undo_generations.rs` and modify `src-tauri/tests/codex_mcp_server.rs` (`#[path = "codex_mcp_server/undo_generations.rs"] mod undo_generations;`).

- [ ] **Verification.**
  - `rtk grep -rn "call_codex_local_tool\|handle_tools_call" src-tauri/src/codex/mcp_server.rs` confirms the sidecar dispatches `video_creater.undo_agent_edit` through `call_codex_local_tool` in its own process. That process boundary is why Task 8 exists.
  - `rtk grep -rn "undo_codex_conversation_edit" src-tauri/src` shows it isn't feature-gated, so it builds for `--no-default-features --features mcp-server`.
- [ ] **Failing tests** in `undo_generations.rs`:
  1. `mcp_apply_records_a_hashed_batch_entry`: `video_creater.apply_project_actions` with a `recordGeneratedAsset` (queued, provider `fal.ai`) and a `recordJob` (`generate_media`, queued, `startRequest.input.assetId`). The written `context/agent-history.json` entry has `afterContentHash`, the current `afterContentHashVersion` (3), `addedGeneratedAssetIds` naming the asset, `addedBookkeepingIds.jobs` naming the job, and no `after`.
  2. `mcp_undo_cancels_the_in_process_run_of_a_removed_generation`:
     - after test 1, mark the job running (`UpdateJobStatus`) and the asset running (`UpdateGeneratedAssetStatus`) through `apply_project_actions_to_split_project`, which isn't an agent write;
     - `register_generation_cancellation(project.id, job_id)`;
     - call `video_creater.undo_agent_edit`;
     - assert `payload.undone == true`, `payload.removedGeneratedAssetIds == [asset]`, `payload.warnings == []`, `run.token().is_cancelled()`, and that the asset and job are gone from the reloaded project.
  3. `mcp_undo_keeps_its_legacy_refusal_errors`: an empty history still errors with "no agent edit history", and a manual label edit still errors with "project changed after that batch". The existing tests at `codex_mcp_server.rs` L861–L940 must also still pass unchanged.
  4. `mcp_undo_reports_a_provider_cancel_failure_as_a_warning`: record a `providerRequest` with provider `fal.ai` and a `cancelUrl` on the running job, then call the new seam `video_creater_lib::codex::tools::undo_agent_edit_payload_with(project_dir, &mut |_| Err("provider is offline".to_string()))` directly. The payload has one warning containing "provider is offline".

  Run `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server undo -- --test-threads=1`.
- [ ] **Implement.**
  - **`agent_batch.rs`.** Extract a `pub(super) fn batch_history_entry(entry_id, before: &VideoProject, after: &VideoProject, action_count, action_ids, session) -> Result<SplitAgentEditHistoryEntry, SplitProjectError>` from `apply_batch_with_hook`. It computes the hash, the version, the added bookkeeping ids and the added generated asset ids, and sets `after: None`. `apply_batch_with_hook` uses it.
  - **Also in `agent_batch.rs`.** Add `pub fn legacy_agent_undo_refusal(outcome: &ProjectAgentUndoOutcome) -> Option<SplitProjectError>`, which returns the two legacy messages, and use it inside `undo_latest_agent_project_action`.
  - **`split.rs`.** `record_agent_project_action_batch` builds its entry with `batch_history_entry(format!("agent-edit-{}", len + 1), &before, &after, action_count, Vec::new(), None)`. Re-export `legacy_agent_undo_refusal`.
  - **`tools.rs`.** Three pieces share one mapping:
    - The existing `undo_agent_edit_payload(args)` calls `undo_codex_conversation_edit(Path::new(&args.project_dir), None)`, which uses the real provider cancel.
    - A new seam, `pub fn undo_agent_edit_payload_with(project_dir: &Path, cancel_provider: &mut dyn FnMut(&JobProviderRequest) -> Result<(), String>) -> Result<Value, CodexLocalToolError>`, calls `undo_codex_conversation_edit_with(project_dir, None, cancel_provider)`. Document it as the test seam, like `undo_codex_conversation_edit_with`.
    - A private `undo_outcome_payload(outcome)` maps the outcome for both. On `Undone`, the payload is `{ undone, entryId, actionCount, remainingAgentHistory, writeReport, warnings, removedGeneratedAssetIds }`. On refusals, it returns `CodexLocalToolError::ProjectActionApplication` with the message from `legacy_agent_undo_refusal(&outcome)`.
  - **Size.** If `agent_batch.rs` passes 600 lines, move its `#[cfg(test)] mod tests` verbatim to `agent_batch_tests.rs` via `#[cfg(test)] #[path = "agent_batch_tests.rs"] mod tests;`.
- [ ] **Verify.**
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server -- --test-threads=1`: 317 or more pass.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::split -- --test-threads=1`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`: all three configurations, including the MCP sidecar.
  - `rtk wc -l src-tauri/src/project/split/agent_batch.rs`
- [ ] **Commit:** `fix(mcp): record hashed agent batches and cancel generations on MCP undo`

### Task 8: Stop in-app generation runs whose job an out-of-process Undo removed (VC-022)

**Files:**
- Create `src-tauri/src/generation/orphan_watch.rs`.
- Modify `src-tauri/src/generation/mod.rs` and `src-tauri/src/main.rs` (`run_generate_media_in_process` only).
**Exclusive:** `main.rs`. Don't run in parallel with another plan's `main.rs` task.

- [ ] **Failing unit tests** in `orphan_watch.rs`, with an injected `job_recorded: impl Fn() -> Result<bool, String> + Send + 'static` and a millisecond interval:
  1. `a_removed_job_requests_cancellation_once`: the closure returns `Ok(true)` twice, then `Ok(false)`. The registered token (`register_generation_cancellation("orphan-p", "orphan-j")`) becomes cancelled within one second.
  2. `a_read_error_never_cancels`: the closure always returns `Err`, and after 50 ms the token isn't cancelled.
  3. `dropping_the_guard_stops_watching`: drop the guard, then flip the closure to `Ok(false)`, and the token stays uncancelled.

  Run `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib generation::orphan_watch -- --test-threads=1`.
- [ ] **Implement.**
  ```rust
  /// Watches a running in-process generation's job. When an agent Undo from another process (the
  /// MCP sidecar) removes the job, this process's cancellation registry is the only way to stop it.
  pub fn watch_generation_job(project_id: &str, job_id: &str, interval: Duration,
      job_recorded: impl Fn() -> Result<bool, String> + Send + 'static) -> GenerationJobWatch
  ```
  - A thread sleeps in `interval` steps, using a `(Mutex<bool>, Condvar)` stop signal so drop wakes it at once.
  - On `Ok(false)` it calls `request_generation_cancellation(project_id, job_id)` once and exits.
  - It ignores `Err`.
  - `GenerationJobWatch::drop` signals stop and joins.
- [ ] **Wire it** in `run_generate_media_in_process` (`main.rs`), right after `cancellation_guard` is created: `let job_watch = watch_generation_job(&workflow_input.project_id, &workflow_input.job_id, Duration::from_secs(5), move || load_split_project(&watched_dir).map(|p| p.jobs.iter().any(|j| j.id == watched_job)).map_err(|e| e.to_string()));`. Move `job_watch` into the `spawn_blocking` closure next to `_cancellation_guard`, so it lives exactly as long as the run.
- [ ] **Verify.**
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib generation:: -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --bin video-creater generate_media_in_process -- --test-threads=1`: includes `cancel_generate_media_in_process_works_before_provider_metadata_and_is_idempotent`.
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test direct_audio_generation_cancellation -- --test-threads=1`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`
- [ ] **Commit:** `fix(generation): stop in-app runs whose job an agent undo removed`

### Task 9: Other generations' progress doesn't block agent Undo (VC-022)

**Files:**
- Create `src-tauri/src/project/split/agent_undo_background.rs`.
- Modify `src-tauri/src/project/split/agent_undo_content.rs`, `src-tauri/src/project/split/agent_batch.rs` and `src-tauri/src/project/split.rs` (the `mod` line, the doc comments on `after_content_hash_version`, and the new entry field).
- Tests: create `src-tauri/tests/codex_conversation/undo_background_generations.rs` and modify `src-tauri/tests/codex_conversation/main.rs`.
- Fixture parity: modify `src/lib/runtime/fixtures/conversation-fixture-undo.ts` and `src/lib/runtime/fixtures/conversation-fixtures.ts`, and create `src/lib/runtime/fixtures/conversation-fixture-undo.test.ts`.

**Rule (hash version 4):**
- `background_generated_asset_ids` are the ids of generated assets present in both `before` and `after` of the batch whose progress fields (`status`, `outputs`, `references.provider_input_urls`, `created_at`) are equal in `before` and `after`.
- For those assets, comparison and hashing ignore:
  - the progress fields;
  - library media that `completion_added` recorded for their outputs *and* that no timeline clip uses.
- Undo writes `before`, with each background asset's current progress fields copied in, and those unused completion media appended in current order.
- A background asset that was deleted or edited in any other field, or whose output was placed on a timeline, still conflicts.
- Version 3 and older entries keep their rules unchanged.

- [ ] **Failing tests** in `undo_background_generations.rs`. Reuse the `generation_bundle`-style JSON from `undo_generations.rs`, copied into this file; don't import it from there.
  1. `an_earlier_generations_progress_doesnt_block_a_later_batch`:
     - Batch 1 records generation G.
     - A user write (not an agent write) marks G running.
     - Batch 2 applies a safe volume change.
     - A user write completes G without placement (`completeGeneratedAsset` with no `replacement`).
     - Undo batch 2 is `Undone`. The reloaded project has batch 2's volume reverted, G `Completed` with its outputs, and G's output media still in the library.
  2. `placing_an_earlier_generations_output_still_conflicts`: as test 1, but completion replaces G's placeholder. Undo batch 2 returns `Conflict` with the unchanged `PROJECT_CHANGED` message.
  3. `a_batch_that_changes_another_generations_status_restores_it`: batch 2 itself contains `updateGeneratedAssetStatus` for G (queued → cancelled). The entry's `backgroundGeneratedAssetIds` excludes G, and Undo restores G to queued.
  4. `version_3_entries_keep_their_rule`: rewrite the entry to version 3 (the hash from `agent_project_content_hash(&after, &added)`, without `backgroundGeneratedAssetIds`). G's progress after batch 2 now conflicts, as before.

  Run `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation undo_background -- --test-threads=1`.
- [ ] **Implement.**
  - **`agent_undo_background.rs`:**
    - `pub(super) fn background_generated_asset_ids(before, after) -> Vec<String>`
    - `pub(super) fn background_comparable_content(content: &mut VideoProject, source: &VideoProject, ids: &[String])`: clears the progress fields and removes unused completion media for `ids`, reusing `completion_added` (make it `pub(super)` in `agent_undo_content.rs`).
    - `pub(super) fn carry_background_progress(restored: &mut VideoProject, current: &VideoProject, ids: &[String])`
    - Unit tests for each function.
  - **`agent_undo_content.rs`:**
    - `AGENT_CONTENT_HASH_VERSION = 4`. Add `GENERATION_SCOPED_HASH_VERSION = 3`.
    - Add `agent_project_content_hash_with_background(project, batch_ids, background_ids)`. The existing `agent_project_content_hash` keeps its signature and version 3 meaning.
    - `entry_matches_project` dispatches `Some(4)` to the new hash and `Some(3)` to the old one.
    - `restored_agent_snapshot` calls `carry_background_progress` for version 4 entries.
  - **`split.rs`.** `SplitAgentEditHistoryEntry` gains `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub background_generated_asset_ids: Vec<String>`. Update the version doc comment to list 4, 3, 2 and none.
  - **`agent_batch.rs`.** `batch_history_entry` fills `background_generated_asset_ids` and hashes with version 4.
  - **Fixture parity.**
    - `undoContent(project, batchIds, backgroundIds = [])` applies the same clearing.
    - `restoredProject` carries progress for `records.backgroundGeneratedAssetIds ?? []`.
    - `conversation-fixtures.ts` records `backgroundGeneratedAssetIds` when it applies.
    - `conversation-fixture-undo.test.ts` covers test 1 and test 2 of this task against the fixture functions.
- [ ] **Verify.**
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server undo -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::split -- --test-threads=1`
  - `rtk pnpm vitest run src/lib/runtime/fixtures`
  - `rtk pnpm check:unused`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`
  - `rtk wc -l src-tauri/src/project/split/agent_undo_*.rs src-tauri/src/project/split/agent_batch.rs src/lib/runtime/fixtures/conversation-fixture-undo.ts`
- [ ] **Commit:** `fix(agent): let other generations' progress through agent undo`

### Task 10: Compressed undo snapshots with a retained-size cap (VC-022)

**Files:**
- Create `src-tauri/src/project/split/agent_history_snapshot.rs`.
- Modify `src-tauri/src/project/split.rs`, `src-tauri/src/project/split/agent_batch.rs`, `src-tauri/src/project/split/agent_undo_content.rs`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`.
- Tests: create `src-tauri/tests/codex_conversation/history_storage.rs` and modify `src-tauri/tests/codex_conversation/main.rs`.

**Stored form** of `before` and the legacy `after`:
```json
{ "encoding": "deflate-base64", "jsonBytes": 183422, "data": "<base64 of raw-deflate VideoProject JSON>" }
```
- Reads accept this object, or a plain `VideoProject` object (older files).
- Writes always use the compressed form.
- `SPLIT_AGENT_EDIT_HISTORY_SCHEMA_VERSION` stays `1`. Reading new files with an older app build isn't supported, and the commit body states this.

- [ ] **License and dependency check.**
  - `rtk grep -n "^license" $HOME/.cargo/registry/src/*/flate2-1.1.9/Cargo.toml $HOME/.cargo/registry/src/*/miniz_oxide-0.8.9/Cargo.toml` shows "MIT OR Apache-2.0" and "MIT OR Zlib OR Apache-2.0".
  - Add `flate2 = { version = "1.1.9", default-features = false, features = ["rust_backend"] }` to `[dependencies]`.
  - Confirm that the feature name exists in `$HOME/.cargo/registry/src/*/flate2-1.1.9/Cargo.toml` before relying on it.
  - `{CARGO_ENV} rtk cargo tree --manifest-path src-tauri/Cargo.toml -i flate2 -e normal` shows no new C or GPL backend.
- [ ] **Failing unit tests** in `agent_history_snapshot.rs`:
  1. `a_snapshot_round_trips_through_its_compressed_form`: `sample_project()` gives equal `project()` after serde.
  2. `a_legacy_plain_project_still_reads`
  3. `a_corrupt_or_mismatched_snapshot_is_a_json_error`: bad base64, bad deflate, or a `jsonBytes` mismatch each produce `SplitProjectError::Json`.
  4. `the_budget_drops_oldest_entries_but_keeps_the_newest`: with a 1 KiB cap and three entries, only the newest remains even if it alone is over the cap. With 25 small entries, 20 remain.
- [ ] **Failing integration tests** in `history_storage.rs`:
  1. `applied_batches_store_compressed_snapshots`: after an apply, `agent_history_json(...)["entries"][0]["before"]["encoding"] == "deflate-base64"`, and Undo restores the project exactly as before.
  2. `a_history_file_with_plain_snapshots_still_undoes`: write the legacy full-snapshot file shape (as `undo.rs` L175–L195 does), and Undo succeeds.
  3. `a_large_history_stays_under_the_retained_size_cap`: a project with a 50,000-word transcript and 20 applies. The sum of `data.len()` over entries is ≤ `MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES`. Also record the file size before and after compression in the test output with `eprintln!`.

  Run `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation history_storage -- --test-threads=1`.
- [ ] **Implement.**
  - **`agent_history_snapshot.rs`:**
    - `pub struct AgentProjectSnapshot` with `#[serde(try_from = "StoredSnapshot", into = "StoredSnapshot")]` holds the compressed bytes and `json_bytes`.
    - `pub fn from_project(&VideoProject) -> Result<Self, SplitProjectError>` and `pub fn project(&self) -> Result<VideoProject, SplitProjectError>`.
    - `pub fn stored_bytes(&self) -> usize`.
    - `StoredSnapshot` is `#[serde(untagged)] enum { Compressed(CompressedSnapshot /* deny_unknown_fields */), Legacy(Box<VideoProject>) }`. A legacy read is compressed on conversion.
    - `pub(super) const MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES: usize = 32 * 1024 * 1024;`
    - `pub(super) fn retain_agent_edit_history_budget(entries: &mut Vec<SplitAgentEditHistoryEntry>, max_bytes: usize)` enforces both caps. It replaces both `drain(0..excess)` blocks. Move `MAX_AGENT_EDIT_HISTORY_ENTRIES` there.
  - **`split.rs`.** `before: AgentProjectSnapshot`, `after: Option<AgentProjectSnapshot>`. Add `mod agent_history_snapshot;` and re-export `AgentProjectSnapshot`.
  - **`agent_batch.rs` and `agent_undo_content.rs`.** Construct with `AgentProjectSnapshot::from_project(&before)?`. `restored_agent_snapshot` and `entry_matches_project` decode with `.project()?`, so `restored_agent_snapshot` returns `Result`. `SplitAgentBookkeepingIds::added_between` receives decoded projects.
- [ ] **Verify.**
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::split -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_conversation -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_mcp_server undo -- --test-threads=1`
  - `{CARGO_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_split -- --test-threads=1`
  - `rtk pnpm rust:fmt`
  - `{CARGO_ENV} rtk pnpm rust:clippy`
  - `rtk wc -l src-tauri/src/project/split/*.rs`
- [ ] **Commit:** `perf(agent): store compressed undo snapshots under a size cap`. The body gives the observed size reduction from test 3.

### Task 11: Full verification gate

**Files:** none. Fixes found here get their own `fix(...)` commits, which stage only the files they change.

- [ ] Runtime preflight, as in Global Constraints.
- [ ] `rtk pnpm rust:fmt` and `{CARGO_ENV} rtk pnpm rust:clippy`.
- [ ] **Rust suites**, one command each, recording pass, fail and ignored counts exactly as observed:
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1`
  - `{CARGO_ENV} {GES_ENV} rtk cargo test --manifest-path src-tauri/Cargo.toml --bins -- --test-threads=1`
  - `--test` targets (same prefix): `codex_conversation`, `codex_mcp_server`, `codex_app_server`, `project_split`, `project_action`, `project_export`, `render_transitions_ges`, `render_pipeline`, `temporal_workflows`, `one_click_edit`, `direct_audio_generation_cancellation`
- [ ] `rtk pnpm verify:frontend`.
- [ ] **Sizes.** Every file this plan created or split is under 600 lines. Check with `rtk wc -l` on each new file listed in the File Map.
- [ ] **Evidence.** `rtk ls output/linux-result-frames/*` lists the PNG and JSON files the capture tests retained. Open one PNG with the Read tool and describe what it shows.
- [ ] Record the results in the Task 12 commit body. Don't write a report file.

### Task 12: Update the backlog rows this plan closed

**Files:** modify `docs/product-backlog.md`.
**Runs after:** Task 11 passed. Only change a row whose proof was observed in Task 11.

- [ ] **VC-023.** Status `implemented`. The packaged desktop app run belongs to workstream 06, so the row isn't `verified` until that run is recorded. Next action: "Record a native Linux desktop run of an AI result card with frames (workstream 06). Linux capture uses the canonical frame sampler with graphics overlays (`result_frames` tests, `output/linux-result-frames/`); image clips, text sources and unfinished generations are refused and show the card fallback."
- [ ] **VC-022.** Status `implemented`. Next action: "Record a native Codex app-server and MCP undo run (workstream 06). Turns record their chat session; MCP Undo cancels in-process runs (including runs in the app process, through the job watch) and fal.ai/Replicate requests but not Temporal runs; other generations' progress no longer blocks Undo, though placing their output still does; undo snapshots are compressed under a 32 MiB retained cap."
- [ ] **VC-026.** Status `verified`. Next action: "None. `codex/context.rs` is split into modules under 600 lines with the same 9 tests." If any module is still ≥ 600 lines, use `implemented` and name that module instead.
- [ ] Set "Last updated" to the commit date.
- [ ] **Commit:** `docs(backlog): record agent fidelity and Linux result frame closure`. The body contains the Task 11 counts.

## Acceptance

| Gap | Behavior | Proof |
| --- | --- | --- |
| VC-023 | Linux capture writes the sampler frame, records and completes its job, attaches a render report, and keeps the project valid | `render_transitions_ges::result_frames::capture_writes_the_canonical_frame_and_records_its_job`, with evidence in `output/linux-result-frames/video-tracks/` |
| VC-023 | Captions and overlays are drawn over video in GES order, and match a GES render | `capture_draws_captions_over_the_sampled_video`, `capture_matches_the_ges_render_with_a_caption` (`output/linux-result-frames/caption-parity/`) |
| VC-023 | Completed generated video outputs are drawn; image clips, text sources and unfinished generations are refused plainly and leave a failed job | `capture_draws_a_completed_generated_video_clip`, `capture_refuses_image_clips_and_pending_generations_plainly` |
| VC-023 | AI result cards show frames from the capture command | `e2e/editor-ai.spec.ts` (decoded result image, both viewports), `ai-cards.test.tsx`. The native desktop run is recorded by workstream 06 |
| VC-022 | Each turn records its chat session; the editor lists history by that session and infers only for legacy entries | `codex_conversation::session_history::*`, `agent-session-actions.test.ts`, `conversation-fixtures.test.ts` |
| VC-022 | MCP apply records hashed batches, and MCP Undo cancels the removed generations' in-process runs and provider requests while keeping its error contract | `codex_mcp_server::undo_generations::*` and the existing MCP undo tests |
| VC-022 | In-app runs stop when an out-of-process Undo removed their job | `generation::orphan_watch` unit tests and the `run_generate_media_in_process` wiring |
| VC-022 | Progress of generations a batch didn't create doesn't block its Undo; placement still conflicts; version 3 entries keep their rule | `codex_conversation::undo_background_generations::*`, `conversation-fixture-undo.test.ts` |
| VC-022 | Snapshots are stored compressed, older plain snapshots still undo, and history stays under 20 entries and 32 MiB | `agent_history_snapshot` unit tests, `codex_conversation::history_storage::*` |
| VC-026 | `codex/context.rs` is split into modules under 600 lines with no behavior change | the same 9 `codex::context` lib tests, the `codex_conversation context`, `codex_app_server` and `codex_mcp_server` targets passing, identical context debug output, `wc -l` |
| All | Nothing regressed | Task 11 counts, `verify:frontend`, clippy on all three configurations |
