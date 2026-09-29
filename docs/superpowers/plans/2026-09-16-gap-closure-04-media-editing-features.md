# Gap Closure 04 — Media Editing Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Each task lists the files it owns. An orchestrator may run tasks in parallel only when their owned files are disjoint and the dependency column allows it (see "Task order and parallelism").

**Goal:** Close the media editing gaps left by the editor redesign, with tests or recorded Linux runs as proof:

- **VC-017 audio clip speed:**
  - a validated `updateAudioClipSpeed` project action;
  - pitch-preserving GES rendering through `scaletempo`;
  - preview `playbackRate` with `preservesPitch`;
  - an audio Properties Speed tab;
  - handle math, and NLE export of retimed audio.
- **VC-018 Detach audio:**
  - a `detachAudio` project action that creates a linked audio clip from a video clip's sound, as one undo step;
  - context menu, mobile clip tool and Audio tab entry points;
  - render and export correctness.
- **Reverse:**
  - a feasibility spike first (design decision 14);
  - then either the full feature, or a recorded deviation with the spike's evidence.
- **VC-015 follow-ups:**
  - speed combined with transitions, render-verified, including the frozen first moments GES showed for sped-up clips;
  - the preview audio crossfade follows clip speed;
  - lower-track clips under a flattened group count as covered in the preview.
- **Lottie handles:** Lottie precompose with transition handles, render-verified with the Lottie worker on Linux (design decision 20).

**Architecture:**
- **Clip speed stays a `speed` property.** It is already read by trim, split, handle, render-plan and NLE code for every clip kind (`src/lib/timeline.ts`, `src/lib/timeline-edit-evaluator.ts`, `src/lib/timeline-ops/transitions.ts`, `src-tauri/src/project/transitions.rs`, `src-tauri/src/render_pipeline/transition_plan.rs`, `src-tauri/src/render_pipeline/project_export.rs`). What is missing:
  - a validated action for audio clips;
  - a GES audio time effect (`apply_audio_clip_properties` in `gstreamer_backend.rs` sizes audio clips by source span and assumes speed 1);
  - preview audio mapping (`buildTimelinePreviewFrame` passes speed `1` for audio layers);
  - UI.
- **New action logic lives in new modules.** It goes in `src-tauri/src/project/audio_edits.rs` and `src-tauri/src/codex/tools/audio_edits.rs`, following `project/transitions.rs` and `codex/tools/transitions.rs`. `action.rs` and `tools.rs` only gain variants, dispatch and registration.
- **Detach audio follows the existing sound model.** Timeline video never sounds:
  - GES adds video clips with `TrackType::VIDEO` only;
  - preview `<video>` is always `muted`;
  - `linked-audio.ts` documents that sound comes from the audio clip sharing `linkGroupId`.

  So `detachAudio` adds an `audio_clip` whose source is the video's own media. `render_audio_media_for_item` already accepts video and generated media. "Muting the video clip's own audio" means moving the clip-level audio properties (`volumeDb` and the `volumeDb` keyframe lane) to the new audio clip and marking the video `audioDetached: true`, so NLE XMEML stops writing source audio for it.
- **Reverse is gated.** Task 12 decides go or no-go with measured evidence. Tasks R1–R4 run only on go; Task R0 runs only on no-go.

**Tech Stack:** Rust (GStreamer 1.24.2 / GES 1.24.2 via `gstreamer-editing-services` 0.25.2), Tauri 2, React 19, Zustand, Vitest, Playwright, Node test runner.

**Spec:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`. Decisions 12, 13, 14, 15, 18 and 20 are binding.
**Depends on:** nothing from workstreams 01–03 or 05. Shared hot files are listed under "Cross-workstream file contention".

## Global Constraints

- **Shell.** Prefix every shell command with `rtk`. `rg` is not installed; use `rtk grep -rn`. Run long commands (cargo, GES renders, runtime verification, Playwright) in the foreground, never in the background.
- **Commits.**
  - Use Conventional Commits. Every commit message ends with the trailer:
    `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
  - Stage only the files the task names, with `rtk git add <paths>`. Never `git add -A` or `git add .`.
  - Never push.
- **File size.** New files stay under 600 lines. Non-test files in `src/editor` must stay under 600 lines (`scripts/editor-source-policy.test.ts`). Already-oversized files (`src/lib/project.ts`, `src-tauri/src/project/action.rs`, `src-tauri/src/codex/tools.rs`, `src-tauri/src/render_pipeline/gstreamer_backend.rs`) only gain variants, dispatch, registration or small call sites. New logic goes in new modules.
- **UI styling.** Use design tokens only (`bg-raised`, `text-dim`, `rounded-control`, `hover:bg-hover`, `ring-ring` and so on). No hex colors and no `white/` or `black/` opacity fragments (`arbitraryHexColorPattern` and `rawOpacityColorPattern` in `scripts/editor-source-policy.test.ts`). User-visible strings never contain "Codex" or "HyperFrames".
- **Rust/TS lockstep.** A new project action lands in one commit containing all of these:
  - the Rust `ProjectAction` variant and its error mapping in `src-tauri/src/project/patch.rs`;
  - the TS `ProjectAction` union member and its `applyProjectActionLocally` case in `src/lib/project.ts`;
  - the Codex structured-output schema: an `*_action_schema()` in `src-tauri/src/codex/app_server.rs`, the prompt's "Supported ProjectAction type values" line, and `expected_project_action_types` in `src-tauri/tests/codex_app_server.rs`;
  - MCP tool support in `src-tauri/src/codex/tools.rs` and `src-tauri/src/codex/tools/audio_edits.rs`;
  - the `Safe` risk classification in `src-tauri/src/codex/conversation/risk.rs` and its TS mirror `safeActionTypes` in `src/lib/runtime/fixtures/conversation-fixture-proposals.ts`.
- **License policy.** LGPL GStreamer, GES and WebKitGTK, dynamically linked, are fine. Nothing GPL. `scaletempo` is in `libgstaudiofx.so` (gst-plugins-good, LGPL). The staged runtime's `manifest.json` inventory lists `{"name":"scaletempo","pluginName":"audiofx","license":"LGPL"}`. Do not use `pitch` from `soundtouch` (the soundtouch library is LGPL, but it is not needed and adds review surface).
- **Cargo environment.** Every cargo command runs with
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`.
- **GES-gated tests** (`--test render_transitions_ges`, and `--lib` tests under `render_pipeline::`/`precompose::` that render) also need:
  - `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`. Its `lib/` symlinks point into `/tmp/vc-deb-root`. If they are dangling, rebuild with `rtk pnpm build:linux-media-runtime --output "$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a"` (foreground; long).
  - For `--test project_export`: `VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder`, built by `rtk pnpm build:compatibility-decoder:linux:dev` if missing.
  - For Lottie preparation: the precompose worker at `src-tauri/target/debug/video-creater-precompose-worker`, built by `rtk pnpm build:precompose-sidecar:dev`. `precompose_worker_path()` finds it as the sibling of the `deps/` test binary, or through `VIDEO_CREATER_PRECOMPOSE_WORKER`.
  - Always pass `-- --test-threads=1`.
- **Command shapes used below** (cwd `/home/olhapi/projects/video-creater`):
  - **Plain cargo:**
    `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml <target> -- --test-threads=1`
  - **GES cargo:** the plain form with `VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a"` added before `rtk`.
  - **Vitest:** `rtk pnpm vitest run <files>`
  - **Playwright:** `rtk pnpm exec playwright test <spec>`
- **Evidence.**
  - Never claim evidence that was not observed. A skipped or unrunnable check is reported as not run, with the missing prerequisite.
  - Render evidence goes under `output/` (git-ignored) and is cited by path in commit bodies.
- **knip.** Every new export needs a production importer; `rtk pnpm check:unused` must pass. Do not add `knip.jsonc` entries. This plan does not touch `knip.jsonc`, `package.json` or `src-tauri/src/main.rs`.
- **API names are verified before use.** Each step that relies on a GStreamer, GES or NLE identifier has a verification sub-step. Record what was observed in the commit body.

---

## Design refinements (made while planning, recorded for review)

1. **"Mute the video clip's own audio" is a property move** (see Architecture). Detach audio is blocked when an `audio_clip` already shares the video's `linkGroupId`. MCP `add_clips` and EDL builds create that pairing (`codex/tools.rs` `creates_linked_audio`, `edit/render_plan.rs`). Block copy: "This clip's sound is already on a linked audio clip."
2. **Audio layers in the preview accept video and generated media.** `buildTimelinePreviewFrame` rejects an `audio_clip` whose media is not `audio` ("Timeline audio item … references video media"), while the render accepts it. Detached audio, and every EDL-built linked audio clip, reference video media, so the preview must accept it too (Task 6).
3. **Speed follows links.** `setItemSpeed` applies the matching speed action and rescaled duration to every linked partner (video ↔ audio), in the same batch. This matches MCP `set_clip_properties`, which already propagates speed (`linked_partner_speed`). Text partners are excluded, as in MCP.
4. **AVFoundation declines retimed audio.** It rejects audio clips with `speed != 1`, so macOS falls back to GES, which preserves pitch. Pitch behavior of `AVMutableComposition.scaleTimeRange` can't be verified on Linux. This mirrors how transitions fall back today (`reject_transitions`).
5. **Denoised audio stops baking varispeed.** `precompose/audio_denoise.rs` `process_wav` currently resamples by speed, which shifts pitch. The denoise intermediate is produced at 1× over the source range, and the prepared clip keeps its `speed`, so GES applies `scaletempo`. `CACHE_VERSION` goes 2 → 3 so old varispeed entries are not reused.
6. **The Reverse spike compares two strategies.** Negative-rate seeks on the curated decoders, and forward decode followed by reversed reassembly using the existing `decode_video_frames_rgba`, `package_png_frames_as_mov` and `decode_to_wav`. It has numeric go/no-go criteria (Task 12).
7. **Codex schema scope.** `updateVisualClipSpeed` is not in the Codex structured-output schema today. This plan adds only the new actions and leaves that pre-existing omission unchanged.
8. **XMEML speed.** XMEML ignores `speed` for every clip today. The plan verifies a public "Time Remap" reference before writing it. Without a verifiable reference, retimed clips get a `speed` limitation note instead (Task 10).

---

## File Map

### Rust — project model and actions
- **`src-tauri/src/project/audio_edits.rs` (new):**
  - `pub(crate) fn update_audio_clip_speed(project, item_id, speed)`
  - `pub(crate) fn detach_audio(project, item_id, audio_item_id, target_track_id, link_group_id)`
  - the pure `detach` helpers
  - Under 600 lines.
- **`src-tauri/src/project/mod.rs`:** `pub mod audio_edits;`
- **`src-tauri/src/project/action.rs`:**
  - variants `UpdateAudioClipSpeed { item_id, speed }` and `DetachAudio { item_id, audio_item_id, target_track_id, link_group_id }`;
  - errors `InvalidAudioClipSpeed(String)`, `NoDetachableAudio(String)`, `AudioAlreadyDetached(String)`;
  - dispatch.
- **`src-tauri/src/project/patch.rs`:** maps the new errors in the validation match near line 161.
- **`src-tauri/tests/project_action/audio_edits.rs` (new)**, plus a `#[path]` module line in `src-tauri/tests/project_action.rs`.

### Rust — agents
- **`src-tauri/src/codex/app_server.rs`:** `update_audio_clip_speed_action_schema()`, `detach_audio_action_schema()`, both registered in `project_action_schema()`, and the prompt type list near line 1826.
- **`src-tauri/src/codex/tools/audio_edits.rs` (new):** `video_creater.detach_audio` descriptor and dispatch.
- **`src-tauri/src/codex/tools.rs`:**
  - `mod audio_edits;` and `tools.extend(audio_edits::audio_edit_tool_descriptors())`;
  - the dispatch arm next to the transition arm (~line 3431);
  - `set_clip_properties` emits `UpdateAudioClipSpeed` for audio clips.
- **`src-tauri/src/codex/conversation/risk.rs`:** both variants join the safe allowlist.
- **Tests:**
  - `src-tauri/tests/codex_app_server.rs`: the type list at line ~350, plus schema assertions.
  - `src-tauri/tests/codex_conversation/prepare.rs`: the safe allowlist test at line ~187.
  - `src-tauri/tests/codex_mcp_server/audio_edits.rs` (new), plus a `#[path]` module line in `src-tauri/tests/codex_mcp_server.rs`.

### Rust — render
- **`src-tauri/src/render_pipeline/gstreamer_audio_speed.rs` (new, `#[cfg(feature = "ges-render")]`):**
  - `audio_speed_for_render_clip`
  - `add_audio_speed_effect` (`scaletempo rate=…`)
  - the measured `GesSourceTiming` for audio
- **`src-tauri/src/render_pipeline/mod.rs`:** `#[cfg(feature = "ges-render")] mod gstreamer_audio_speed;`
- **`src-tauri/src/render_pipeline/gstreamer_backend.rs`:**
  - audio `add_asset` duration from `timelineDurationSeconds`;
  - the effect call in `apply_audio_clip_properties`;
  - `"scaletempo"` pushed onto `required_factories` when any audio clip is retimed;
  - the visual speed fix from Task 5.
- **`src-tauri/src/render_pipeline/avfoundation_backend.rs`:** `reject_unsupported_audio_properties` rejects `speed != 1`, with a unit test next to `rejects_plans_with_video_or_audio_transitions`.
- **`src-tauri/src/render_pipeline/plugin_policy.rs`:** an `AllowedFactoryPolicy` for `scaletempo` (plugin `audiofx`, `GSTREAMER_GOOD_PACKAGES`, `LGPL_COMPATIBLE`).
- **`src-tauri/src/precompose/audio_denoise.rs`:** no varispeed, and `CACHE_VERSION = 3`.
- **Project export and NLE:**
  - `src-tauri/src/project/nle_export.rs`: XMEML speed (Task 10). **Contended with workstream 05.**
- **Tests:**
  - `src-tauri/tests/render_pipeline.rs`: `reviewed_gstreamer_factory_cases` gains `("scaletempo", "audiofx", "GStreamer Good Plug-ins")`, and the array length becomes 34.
  - `src-tauri/tests/render_transitions_ges.rs`: new `#[path]` modules `audio_speed`, `speed`, `detach_audio`, `lottie_handles`.
  - `src-tauri/tests/render_transitions_ges/{audio_speed,speed,detach_audio,lottie_handles}.rs` (new).
  - `src-tauri/tests/render_transitions_ges/media.rs`: `write_video_with_tone`, `dominant_frequency`.
  - `src-tauri/tests/render_transitions_ges/parity.rs`: `segment_colours` becomes `pub(crate)`.
  - `src-tauri/tests/fixtures/transitions/lottie-colour-steps.json` (new).
  - `src-tauri/tests/project_nle_export/audio_edits.rs` (new), plus a `#[path]` line in `src-tauri/tests/project_nle_export.rs`.

### Runtime scripts
- `scripts/build-linux-media-runtime.mjs`: `reviewedRenderFactories.portable` gains `"scaletempo"`.
- `scripts/verify-linux-media-runtime.mjs`: `requiredFactories` gains `"scaletempo"`.
- `scripts/gstreamer-runtime-policy.mjs`: macOS `requiredPlugins` gains `"audiofx"`. This is unverifiable on Linux; say so in the commit body.
- `scripts/build-linux-media-runtime.test.ts`, `scripts/gstreamer-runtime-policy.test.ts`: assertions.

### TypeScript — model, preview, UI
- **`src/lib/project.ts`:** union members `updateAudioClipSpeed` and `detachAudio`, and `applyProjectActionLocally` cases. The detach body is a call into `src/lib/timeline-ops/detach-audio.ts` `applyDetachAudio`.
- **`src/lib/project-audio-actions.test.ts` (new):** TS mirror tests.
- **`src/lib/runtime/fixtures/conversation-fixture-proposals.ts`:** `safeActionTypes`.
- **`src/lib/timeline-ops/detach-audio.ts` (new):**
  - `planDetachAudio(project, itemId, ids)` returns a `CommandResult`;
  - `applyDetachAudio(project, action)` is the pure local apply.
  - Its test is `src/lib/timeline-ops/detach-audio.test.ts` (new).
- **`src/lib/timeline-ops/clip-commands.ts`:** `setItemSpeed` accepts audio clips and linked partners.
- **Preview:**
  - `src/lib/timeline-preview.ts`: audio speed mapping; audio layers accept video/generated media; `playbackRate` on audio and video layers; `flattenedCoverItemIds` on the frame (Task 7).
  - `src/lib/preview/transition-eligibility.ts`: `flattenGroupsAt` computed even without transitions.
  - `src/editor/preview/media-layer.tsx` and `src/editor/preview/use-media-synchronization.ts`: `playbackRate` and `preservesPitch`.
  - `src/editor/preview/canonical-frames.ts`: coverage.
- **Properties and timeline UI:**
  - `src/editor/properties/speed-tab.tsx` (new): `ClipSpeedTab`, moved from `visual-speed-animation.tsx` `VisualSpeedTab`.
  - `src/editor/properties/visual-speed-animation.tsx`, `visual-tabs.tsx`, `audio-tabs.tsx`, `property-tabs.ts`, `mobile-property-sheets.tsx`.
  - `src/editor/timeline/timeline-commands.ts`: `detachAudio(itemId)`.
  - `src/editor/timeline/timeline-context-menu.tsx`: "Detach audio".
  - `src/editor/timeline/mobile-clip-tools.tsx`: "Detach audio" tool.
  - `src/editor/timeline/speed-dialog.tsx`: doc comment only.
- **`e2e/editor-media-editing.spec.ts` (new).**

### Docs
- `docs/research/2026-09-16-reverse-intermediate-spike.md` (new; Task 12).
- `docs/product-backlog.md` (final task).

---

## Task order and parallelism

| Task | Title | Depends on | Owns (exclusive while running) | Parallel with |
| --- | --- | --- | --- | --- |
| 1 | `scaletempo` in the reviewed runtime | — | runtime scripts, `plugin_policy.rs`, `tests/render_pipeline.rs` | 2, 6, 12 |
| 2 | `updateAudioClipSpeed` lockstep | — | `action.rs`, `patch.rs`, `project/mod.rs`, `project/audio_edits.rs`, `app_server.rs`, `tools.rs`, `tools/audio_edits.rs`, `risk.rs`, `project.ts`, the fixture mirror, their tests | 1, 6, 12 |
| 3 | `detachAudio` lockstep | 2 | same files as 2, plus `timeline-ops/detach-audio.ts` | 4, 7, 8 |
| 4 | GES audio speed render | 1, 2 | `gstreamer_backend.rs`, `gstreamer_audio_speed.rs`, `render_pipeline/mod.rs`, `avfoundation_backend.rs`, `audio_denoise.rs`, `tests/render_transitions_ges.rs`, `render_transitions_ges/{audio_speed,media}.rs` | 3, 7, 8 |
| 5 | Speed + transitions render, frozen start fix | 4 | `gstreamer_backend.rs`, `tests/render_transitions_ges.rs`, `render_transitions_ges/{speed,parity}.rs` | 7, 8, 9 |
| 6 | Preview: audio speed, `playbackRate`, audio from video media | — | `timeline-preview.ts` (+test), `media-layer.tsx`, `use-media-synchronization.ts` (+test) | 1, 2, 12 |
| 7 | Preview: flattened-group coverage | 6 | `transition-eligibility.ts` (+test), `timeline-preview.ts`, `canonical-frames.ts` (+test) | 3, 4, 5, 8 |
| 8 | Audio Speed tab and linked speed | 2 | `clip-commands.ts` (+test), `speed-tab.tsx`, `visual-speed-animation.tsx`, `visual-tabs.tsx`, `audio-tabs.tsx` (+test), `property-tabs.ts`, `mobile-property-sheets.tsx` (+test), `properties-panel.test.tsx`, `timeline-context-menu.test.tsx`, `speed-dialog.tsx` | 3, 4, 5, 7 |
| 9 | Detach audio UI | 3, 8 | `timeline-commands.ts`, `timeline-context-menu.tsx` (+test), `mobile-clip-tools.tsx`, `visual-tabs.tsx`, `mobile-layout.test.tsx` | 5, 10 |
| 10 | Detach and retimed audio in render and NLE export | 3, 5 | `tests/render_transitions_ges.rs`, `render_transitions_ges/{detach_audio,media}.rs`, `nle_export.rs`, `tests/project_nle_export.rs`, `project_nle_export/audio_edits.rs` | 9 |
| 11 | Lottie precompose handles render | 10 | `tests/render_transitions_ges.rs`, `render_transitions_ges/lottie_handles.rs`, `tests/fixtures/transitions/lottie-colour-steps.json`, any fix in `precompose/mod.rs` | 9 |
| 12 | Reverse feasibility spike | — | `src-tauri/tests/reverse_intermediate_spike.rs` (uncommitted unless go), `docs/research/2026-09-16-reverse-intermediate-spike.md` | 1, 2, 6 |
| R1–R4 | Reverse (only on go) | 12, 3, 11, 9 | see each task | sequential |
| R0 | Reverse deviation record (only on no-go) | 12 | the research doc | any |
| 13 | Playwright acceptance | 9 (and R3 on go) | `e2e/editor-media-editing.spec.ts` | — |
| 14 | Full verification gate | all | none (fix commits name their files) | — |
| 15 | Backlog update | 14 | `docs/product-backlog.md` | — |

**Sequential chains within this plan:**
- Rust action files: 2 → 3 → R1.
- `gstreamer_backend.rs` and `tests/render_transitions_ges.rs`: 4 → 5 → 10 → 11 → R2/R4.
- `timeline-preview.ts`: 6 → 7 → R3.
- `visual-tabs.tsx`: 8 → 9 → R3.

**Cross-workstream file contention.** The orchestrator must not run these concurrently with the named workstream's tasks:
- **`src/lib/project.ts`** (tasks 2, 3, R1): workstreams 01 (job progress field) and 02 (export inputs).
- **`src-tauri/src/project/action.rs` and `patch.rs`** (2, 3, R1): workstream 01 (job progress through the job-status path).
- **`src-tauri/src/codex/app_server.rs`, `codex/tools.rs`, `codex/conversation/risk.rs`** (2, 3, R1): workstream 03 (agent fidelity). Workstream 03's split of `codex/context.rs` is untouched here on purpose: agent guidance for the new actions goes only in the `app_server.rs` prompt.
- **`src-tauri/src/project/nle_export.rs`** (10): workstream 05. Run Task 10 after workstream 05's `nle_export.rs` tasks land, or rebase onto them.
- This plan never edits `src-tauri/src/main.rs`, `knip.jsonc` or `package.json`.

---

### Task 1: `scaletempo` in the reviewed runtime and plugin policy

**Files:**
- Modify: `scripts/build-linux-media-runtime.mjs`, `scripts/verify-linux-media-runtime.mjs`, `scripts/gstreamer-runtime-policy.mjs`, `src-tauri/src/render_pipeline/plugin_policy.rs`
- Test: `scripts/build-linux-media-runtime.test.ts`, `scripts/gstreamer-runtime-policy.test.ts`, `src-tauri/tests/render_pipeline.rs`

- [ ] **Verify the element on the staged runtime.** Run:
  ```
  rtk python3 -c "import json,os;m=json.load(open(os.path.expanduser('~/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a/manifest.json')));print([f for f in m['inventory']['factories'] if f['name']=='scaletempo'])"
  ```
  - Expect `pluginName: audiofx`, `license: LGPL`.
  - Also confirm `plugins/libgstaudiofx.so` resolves: `rtk ls -laL "$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a/plugins/libgstaudiofx.so"`.
  - If it is absent, add the plugin to the staging allowlist in `scripts/build-linux-media-runtime.mjs` and rebuild with `rtk pnpm build:linux-media-runtime --output <runtime root>` before continuing.
- [ ] **Write failing tests.**
  - In `src-tauri/tests/render_pipeline.rs`, add `("scaletempo", "audiofx", "GStreamer Good Plug-ins")` to `reviewed_gstreamer_factory_cases` (array length 34).
  - In `scripts/build-linux-media-runtime.test.ts`, assert `reviewedRenderFactories.portable.includes("scaletempo")`.
  - In `scripts/gstreamer-runtime-policy.test.ts`, assert `requiredPlugins.includes("audiofx")`.
- [ ] **Run the tests and see them fail.**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline reviewed_gstreamer_factor -- --test-threads=1`
  - `rtk pnpm test:linux-media-runtime`
  - `rtk pnpm test:gstreamer-runtime-policy`
- [ ] **Implement.**
  - Add the `AllowedFactoryPolicy { name: "scaletempo", plugin_names: &["audiofx"], package_families: GSTREAMER_GOOD_PACKAGES, license_predicate: LGPL_COMPATIBLE }` entry to `ALLOWED_FACTORY_POLICIES`.
  - Add `"scaletempo"` to `reviewedRenderFactories.portable` and to `requiredFactories`.
  - Add `"audiofx"` to macOS `requiredPlugins`.
- [ ] **Re-run the three commands (pass).** Then verify the staged runtime really loads it:
  `rtk pnpm verify:linux-media-runtime --runtime "$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a"`
  Expect no `required factory missing: scaletempo`.
- [ ] **Commit:** `build(runtime): review scaletempo for pitch-preserving audio speed`. The body records the manifest line observed, and says the macOS `audiofx` addition is not verified on this host.

### Task 2: `updateAudioClipSpeed` action (Rust/TS lockstep)

**Files:**
- Create: `src-tauri/src/project/audio_edits.rs`, `src-tauri/src/codex/tools/audio_edits.rs`, `src-tauri/tests/project_action/audio_edits.rs`, `src-tauri/tests/codex_mcp_server/audio_edits.rs`, `src/lib/project-audio-actions.test.ts`
- Modify: `src-tauri/src/project/mod.rs`, `src-tauri/src/project/action.rs`, `src-tauri/src/project/patch.rs`, `src-tauri/src/codex/app_server.rs`, `src-tauri/src/codex/tools.rs`, `src-tauri/src/codex/conversation/risk.rs`, `src-tauri/tests/project_action.rs`, `src-tauri/tests/codex_app_server.rs`, `src-tauri/tests/codex_conversation/prepare.rs`, `src-tauri/tests/codex_mcp_server.rs`, `src/lib/project.ts`, `src/lib/runtime/fixtures/conversation-fixture-proposals.ts`

**Action contract** (JSON / TS):
```ts
| { type: "updateAudioClipSpeed"; itemId: string; speed: number }
```
- **Speed.** `speed` must be finite and in `[0.1, 8]`. Otherwise the error is `InvalidAudioClipSpeed(itemId)`.
- **Item.** It must exist (`ItemNotFound`), be `TimelineItemKind::AudioClip` (`NotAudioClip`), and sit on an unlocked track (`TrackLocked`).
- **Stored value.** A speed of `1` removes `properties.speed`; any other value sets it.
- **Duration.** The action does not resize, exactly like `update_visual_clip_speed`. Callers pair it with `resizeItems`.

- [ ] **Rust tests** (`tests/project_action/audio_edits.rs`, with `mod audio_edits;` via `#[path]` in `tests/project_action.rs`):
  - serde round-trip of `{"type":"updateAudioClipSpeed","itemId":"…","speed":2.0}`;
  - sets and clears `speed`;
  - rejects 0.05, 9 and NaN (NaN through a constructed action);
  - rejects a video clip and a locked track;
  - a crossfade between two audio clips re-validates against speed-scaled handles after `UpdateAudioClipSpeed` plus `ResizeItems`. For example, `sourceIn` 1 at speed 2 gives a 0.5 s head handle, so a 1.2 s transition is clamped or dropped by `maintain_transitions`. Assert the observed result.
- [ ] **Codex schema test** (`tests/codex_app_server.rs`):
  - add `"updateAudioClipSpeed"` to `expected_project_action_types`;
  - assert the variant schema `required == ["type","itemId","speed"]`, with `speed` `{ "type":"number","minimum":0.1,"maximum":8 }`;
  - the prompt text contains `updateAudioClipSpeed`.
- [ ] **Risk test** (`tests/codex_conversation/prepare.rs`, `risk_safe_allowlist_covers_local_edits_and_additions`): add `ProjectAction::UpdateAudioClipSpeed { .. }` to the safe list.
- [ ] **MCP test** (`tests/codex_mcp_server/audio_edits.rs`, module line in `tests/codex_mcp_server.rs`):
  - `video_creater.set_clip_properties` with `{ updates: [{ itemId: <audio clip>, speed: 2 }] }` yields an `updateAudioClipSpeed` action (not `updateItemProperties`) in the returned `projectActions`;
  - a visual clip still yields the existing actions (regression);
  - `video_creater.apply_project_actions` with `{"type":"updateAudioClipSpeed",…}` applies to a saved split project, with `save_split_project` / `load_split_project` round-trip.
- [ ] **Run all four and see them fail.**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action --test codex_app_server --test codex_mcp_server --test codex_conversation -- --test-threads=1`
- [ ] **Implement Rust.**
  - `audio_edits.rs::update_audio_clip_speed`, mirroring `update_visual_clip_speed` (`action.rs` ~3282).
  - The variant, error and dispatch in `action.rs`.
  - `patch.rs` error arm.
  - `app_server.rs` schema function, registration and prompt list.
  - `risk.rs` allowlist.
  - `tools.rs` `set_clip_properties`: when the resolved item is `AudioClip`, push `ProjectAction::UpdateAudioClipSpeed` instead of `UpdateItemProperties { set: speed }`. `linked_partner_speed` expansion stays as is; audio partners go through the same branch.
  - `codex/tools/audio_edits.rs` is created now with only the module skeleton and `audio_edit_tool_descriptors()` returning `vec![]`. Task 3 adds the tool. Register it in `tools.rs`, or defer the file to Task 3 if clippy flags the empty vector; record which.
- [ ] **TS tests** (`src/lib/project-audio-actions.test.ts`): the `applyProjectActionLocally` mirror sets and clears `speed` on an `audio_clip`, and throws for a missing item. Run `rtk pnpm vitest run src/lib/project-audio-actions.test.ts` (fail).
- [ ] **Implement TS.**
  - The union member and case in `src/lib/project.ts`, next to `updateVisualClipSpeed` (~line 2908).
  - `"updateAudioClipSpeed"` in `safeActionTypes`.
- [ ] **Verify.**
  - The cargo command above (pass).
  - `rtk pnpm vitest run src/lib/project-audio-actions.test.ts src/lib/runtime/fixtures/conversation-fixtures.test.ts src/lib/project.test.ts`
  - `rtk pnpm lint`
  - `rtk pnpm rust:fmt`
- [ ] **Commit:** `feat(project): add a validated audio clip speed action for editor and agents`

### Task 3: `detachAudio` action (Rust/TS lockstep)

**Files:**
- Create: `src/lib/timeline-ops/detach-audio.ts`, `src/lib/timeline-ops/detach-audio.test.ts`
- Modify: every Task 2 file (`project/audio_edits.rs`, `action.rs`, `patch.rs`, `app_server.rs`, `tools.rs`, `tools/audio_edits.rs`, `risk.rs`, `project.ts`, `conversation-fixture-proposals.ts`) and the Task 2 test files

**Action contract:**
```ts
| { type: "detachAudio"; itemId: string; audioItemId: string; targetTrackId: string; linkGroupId: string }
```
**Rust semantics** (`audio_edits.rs::detach_audio`):
1. **The item.** It must exist and be `VideoClip` with `TimelineSource::Media` whose media kind is `Video` or `Generated`. Otherwise `NoDetachableAudio(itemId)`.
2. **No existing linked audio.** If the item has a `linkGroupId` and any `AudioClip` in the active timeline shares it, the error is `AudioAlreadyDetached(itemId)`.
3. **The target track.** `targetTrackId` must exist (`TrackNotFound`), be `TrackKind::Audio` (`TrackTypeMismatch`), and be unlocked. The item's own track must be unlocked (`TrackLocked`).
4. **Free span.** `[start, start + duration)` must not overlap an item on the target track (`TrackItemOverlap`). This is checked explicitly, because `add_items` would overwrite.
5. **The new item.** `audioItemId` must be unused (`DuplicateItemId`). The new `AudioClip`:
   - `id = audioItemId`, `label = "<label> audio"`;
   - same `start_seconds`, `duration_seconds` and `source`;
   - properties copied: `sourceIn`, `sourceOut`, `speed`, `volumeDb`, `fadeInSeconds`, `fadeOutSeconds` when they are audio fades, the `keyframes.volumeDb` lane, and `effects` whose `effectType` starts with `audio.`;
   - plus `linkGroupId` and `sourceClipType: "audio"`.
6. **The video item.** It gains `linkGroupId` (keeping an existing one, in which case the action's `linkGroupId` must equal it, else `InvalidLinkGroup`; reuse the closest existing error or add one) and `audioDetached: true`. It loses `volumeDb`, `keyframes.volumeDb` and `audio.*` effects. Visual `fadeInSeconds` / `fadeOutSeconds` stay on the video: they are opacity fades.
7. **Verify the fade semantics first.** Before choosing (5) and (6), check how fades are read for video items. Run `rtk grep -rn "fadeInSeconds" src-tauri/src/render_pipeline/gstreamer_backend.rs src/lib/timeline-preview.ts` and confirm video `fadeInSeconds` means opacity. If it does, do not copy fades to the audio clip; start it without fades. Record the decision in the commit body.

- [ ] **Rust tests** (append to `tests/project_action/audio_edits.rs`):
  - creates the linked clip with the copied range and speed, removes `volumeDb` from the video, and sets `audioDetached`;
  - one `apply_project_action` call;
  - `AudioAlreadyDetached` when a linked audio clip exists;
  - `NoDetachableAudio` for an image clip and an audio-only media clip;
  - an overlap on the target track is rejected and leaves the project unchanged;
  - a locked target track and a wrong track kind are rejected;
  - moving the video afterwards with `RippleTrimItem { propagate_linked: true }` also trims the audio (link works).
- [ ] **Codex, risk and MCP tests:**
  - schema `required == ["type","itemId","audioItemId","targetTrackId","linkGroupId"]`;
  - `"detachAudio"` in the expected types and the safe list;
  - MCP tool `video_creater.detach_audio` with `{ itemId }`:
    - it chooses the first unlocked audio track with a free span, else emits `createTrack` for a new audio track first;
    - ids are `<itemId>-audio` (suffix `-2`, `-3` on collision) and `link-<itemId>` (or the existing group);
    - it returns the applied actions;
  - `list_codex_local_tools` lists it with category `mutation`.
- [ ] **TS tests** (`detach-audio.test.ts`):
  - `applyDetachAudio` mirrors every Rust case above with the same messages as `Error` text;
  - `planDetachAudio(project, itemId, { audioItemId, linkGroupId, trackId })`:
    - returns `[detachAudio]` onto a free existing audio track;
    - returns `[createTrack(+reorderTrack), detachAudio]` when every audio track collides (use `planDropTarget` with `assetKind: "audio"`, `insertIndex: null`, `hoveredTrackId: <first audio track>`, and `newTrackId(timeline, "audio")`);
    - blocked copy:
      - "Detach audio is available for video clips."
      - "This clip has no sound to detach." (`timelineItemHasAudio` false)
      - "This clip's sound is already on a linked audio clip."
      - "Unlock the track to detach this clip's audio."
- [ ] **Run and see the tests fail.** Cargo command from Task 2 plus `rtk pnpm vitest run src/lib/timeline-ops/detach-audio.test.ts`.
- [ ] **Implement** Rust (`audio_edits.rs`, variant, dispatch, errors, schema, prompt, risk, MCP tool in `codex/tools/audio_edits.rs` with dispatch arm in `tools.rs`) and TS (union member; `applyProjectActionLocally` case delegating to `applyDetachAudio`; `safeActionTypes`).
- [ ] **Verify.** Task 2's cargo and Vitest commands plus `src/lib/timeline-ops/detach-audio.test.ts`, then `rtk pnpm lint` and `rtk pnpm rust:fmt`.
- [ ] **Commit:** `feat(project): detach a video clip's sound into a linked audio clip`

### Task 4: GES renders retimed audio with pitch preserved

**Files:**
- Create: `src-tauri/src/render_pipeline/gstreamer_audio_speed.rs`, `src-tauri/tests/render_transitions_ges/audio_speed.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`, `src-tauri/src/render_pipeline/gstreamer_backend.rs`, `src-tauri/src/render_pipeline/avfoundation_backend.rs`, `src-tauri/src/precompose/audio_denoise.rs`, `src-tauri/tests/render_transitions_ges.rs`, `src-tauri/tests/render_transitions_ges/media.rs`

- [ ] **Verify the GES time-effect API** (test-first, in `audio_speed.rs`): `ges_effect_is_a_time_effect_for_scaletempo_rate`. Build `ges::Effect::new("scaletempo rate=2.0")`, add it to an audio `UriClip` from `write_tone`, and assert `ges::prelude::BaseEffectExt::is_time_effect(&effect)` is true. `libges-1.0.so.0` contains the string `scaletempo`, so a registered rate property is expected. If this fails, stop and record: the fallback is to pre-render a tempo-changed intermediate via `scaletempo` in a plain pipeline, which needs a design note.
- [ ] **Failing render tests** (`audio_speed.rs`, fixtures from `fixtures.rs`):
  - **`retimed_audio_keeps_pitch_and_timeline_length`.**
    - Setup: `tone-a` (440 Hz, 4 s) as an audio clip at 0–1.5 s with `sourceIn` 0.5, `sourceOut` 3.5, speed 2, over a 1.5 s red video clip.
    - Assert the decoded output audio duration is 1.5 s ±1 frame.
    - Assert `dominant_frequency` (new helper in `media.rs`: zero-crossing count / 2 / seconds over a steady window) is 440 Hz ±3%, not 880 Hz.
  - **`retimed_audio_fades_land_in_timeline_time`.** `fadeInSeconds` 0.5 at speed 2: RMS over the first 20 ms < 0.02 and at 0.25 s ≈ half steady (±25%), measured like `audio_fade_project`.
  - **`retimed_audio_crossfade_uses_speed_scaled_handles`.**
    - Setup: `tone-a` speed 2 (0–2 s, `sourceIn` 1.0 → `sourceOut` 5.0 would exceed 4 s media, so use duration 1.25 s: `sourceIn` 1, `sourceOut` 3.5) crossfading 0.5 s into `tone-b` at speed 1. Adjust values until `AddTransition` accepts; the error message reports the maximum.
    - Assert midpoint RMS within 0.5 dB of steady, and no silence gap over the window (min 10 ms RMS > 0.1).
  - **Structure.** In `structure.rs`'s `summary()` output for a retimed audio clip, the audio clip line lists a `scaletempo` effect. Only projects with retimed audio change; `timeline_without_transitions.golden.txt` stays byte-identical.
  - **Timing verification.** Bind a volume envelope with a step at clip-local 0.5 s on a speed-2 clip with `sourceIn` 1. Find where the step lands in output RMS, and derive the internal-time mapping (the visual `videorate` measured `inpoint / s + t * s`; confirm or correct it for `scaletempo`). Encode the observed mapping in `GesSourceTiming` for audio and cite the numbers in the commit body.
- [ ] **Run the tests and see them fail.**
  `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges audio_speed -- --test-threads=1`
- [ ] **Implement.**
  - **`gstreamer_audio_speed.rs`:**
    - `audio_speed_for_render_clip(clip, index) -> PipelineResult<Option<f64>>`, with the `0.1..=8` check and the error path `renderPlan.audioClips[{index}].properties.speed`;
    - `add_audio_speed_effect(ges_clip, speed, index)`, using `ges::Effect::new(&format!("scaletempo rate={speed:.6}"))` and mirroring `add_visual_effect`'s error shape.
  - **`gstreamer_backend.rs` audio loop (~line 950):**
    - `duration` becomes `render_clip_number_property(clip, "timelineDurationSeconds").unwrap_or(source_out - source_in)`, validated positive;
    - `apply_audio_clip_properties` attaches the speed effect before binding envelopes, and passes `GesSourceTiming { source_in, speed }` using the measured mapping;
    - `render_with_ges` pushes `"scaletempo"` to `required_factories` when any audio clip has `speed != 1`.
  - **`avfoundation_backend.rs` `reject_unsupported_audio_properties`:** `speed` other than 1 returns `"AVFoundation export does not retime audio clips."` with the hint to render with GStreamer. Add the unit test `rejects_retimed_audio_clips` next to `rejects_plans_with_video_or_audio_transitions`.
  - **`audio_denoise.rs`:**
    - `CACHE_VERSION = 3`;
    - `process_wav` stops dividing by `speed` (`output_frames` from the source span; `source_position` uses rate 1);
    - the prepared item keeps its original `speed` (remove the `speed = 1.0` insert) and `sourceOut = manifest.duration_seconds`;
    - update `wav_fixture_preserves_trim_and_speed_timing_while_reducing_noise` to assert the source-span duration;
    - keep `speed_micros` in the fingerprint.
- [ ] **Verify.**
  - The GES command above, then the whole target: `... --test render_transitions_ges -- --test-threads=1`. Expect the prior 20 tests plus the new ones.
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --lib avfoundation_backend precompose::audio_denoise -- --test-threads=1`. If cargo rejects two filters, run them separately.
- [ ] **Commit:** `feat(render): render retimed audio clips with pitch-preserving scaletempo`. The body records the measured frequency, duration and timing mapping.

### Task 5: Speed combined with transitions, and the frozen start of sped-up clips

**Files:**
- Create: `src-tauri/tests/render_transitions_ges/speed.rs`
- Modify: `src-tauri/tests/render_transitions_ges.rs`, `src-tauri/tests/render_transitions_ges/parity.rs` (`segment_colours` becomes `pub(crate)`), `src-tauri/src/render_pipeline/gstreamer_backend.rs` (the fix)

- [ ] **Reproduce first** (`speed.rs`). Name the test `sped_up_clip_starts_moving_on_its_first_frame`.
  - Setup: `write_segmented_video` media (14 segments × 7 frames, `segment_colours(true)`), with one clip at 0–1.5 s, `sourceIn` 1.0, speed 2 (`sourceOut` 4.0 fits the ~4.08 s medium), no transitions.
  - Decode the GES output with `decode_frames`. For output frames 0..12, assert each frame's centre colour equals the canonical frame from `render_canonical_frame_rgba` at the same time (channel diff ≤ 16). No run of more than `ceil(7 / 2)` identical consecutive frames is allowed while source time advances.
  - Run with the GES command, filter `speed`. **Record the observed result.**
    - **If the frames freeze:** capture which frames freeze and for how long (for example, "frames 0–11 repeat source 1.0 s"). Continue to the fix.
    - **If they don't freeze:** say so in the commit body. Keep the test as the regression and skip the fix step.
- [ ] **Parity with transitions** (`speed.rs`), named `speed_changes_render_like_canonical_frames_through_transitions`. Build a project with warm and cool segmented media:
  - clip A: 0–2 s, speed 1.5, `sourceIn` 0.5, `sourceOut` 3.5;
  - clip B: 2–4 s, speed 0.5, `sourceIn` 1.0, `sourceOut` 2.0;
  - a 0.5 s crossfade, then a separate pair with a 0.5 s dip to black. Adjust the durations to the maximum `AddTransition` reports if needed.

  Call `assert_render_parity("speed-transitions", dir, &project, &project, &transition_sample_times(&[2.0]))` plus one frame 0.1 s into each clip. Evidence goes under `output/transition-preview-parity/speed-transitions/`.
- [ ] **Run both and see the failure** (or record that they pass).
- [ ] **Fix the frozen start**, only if it was reproduced.
  - **Verify the API first.** `ges::prelude::ClipExt::internal_time_from_timeline_time` and `timeline_time_from_internal_time` exist in `gstreamer-editing-services` 0.25.2 (`src/auto/clip.rs:181` and `:223`).
  - **Diagnose.** After `add_asset` plus the `videorate rate=s` effect, log `ges_clip.inpoint()`, `ges_clip.duration_limit()`, and `internal_time_from_timeline_time(core_source, start + k/fps)` for k = 0..12.
  - **Candidate causes, to confirm against the log:**
    - (a) the in-point is interpreted pre-effect, so the source seeks to `sourceIn` but `videorate` holds the first buffer until timestamps reach `sourceIn / s`;
    - (b) the effect was added after `add_asset` fixed `max-duration`.
  - **Candidate fixes:**
    - set the in-point after adding the time effect;
    - or set the in-point to the value `internal_time_from_timeline_time` reports for the desired source time;
    - or replace the `videorate` time effect with a prepared retimed intermediate.
  - **Rules for the fix.**
    - Keep the `GesSourceTiming` alpha envelope mapping consistent. Re-run `render_transitions_ges` fades and keyframe tests.
    - Any change to the mapping must update the doc comment on `GesSourceTiming`, with the new measurement.
- [ ] **Verify.**
  - The whole `render_transitions_ges` target (GES command).
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1`
  - Regenerate and confirm the three existing parity reports still pass, `output/transition-preview-parity/{every-kind,flattened-transitions,lut-handles}/preview-comparison.json` with `status: passed`, by reading the JSON files.
- [ ] **Commit:**
  - If a fix was needed: `fix(render): start sped-up GES clips at their source in-point and verify speed with transitions`.
  - Otherwise: `test(render): verify clip speed through transitions against canonical frames`.

  The body lists the observed freeze (or its absence) and the mismatch ratios.

### Task 6: Preview plays retimed audio, and audio from video media

**Files:**
- Create: `src/editor/preview/use-media-synchronization.test.tsx`
- Modify: `src/lib/timeline-preview.ts`, `src/lib/timeline-preview.test.ts`, `src/editor/preview/media-layer.tsx`, `src/editor/preview/use-media-synchronization.ts`

- [ ] **Failing tests** in `timeline-preview.test.ts`:
  - **Retimed audio.** An `audio_clip` at speed 2 with `sourceIn` 2 has `sourceTimeSeconds` `2 + local * 2` and `playbackRate: 2` on its audio layer.
  - **Crossfade follows speed.** In the tail handle of an audio crossfade (use `crossfade` / `sourceClip` from `src/editor/timeline/transition-test-project.ts`, or `src/lib/timeline-ops/transition-fixtures.ts`), `sourceTimeSeconds = sourceOut + (t - end) * speed` for a speed-2 outgoing audio clip, and `sourceIn - (start - t) * speed` in the incoming head.
  - **Audio from video media.** An `audio_clip` whose media kind is `video` or `generated` builds an audio layer, and no longer reports "references video media".
  - **Visual layers.** A visual layer at speed 1.5 carries `playbackRate: 1.5`; speed 1 carries `playbackRate: 1`.
- [ ] **Failing hook tests** in `use-media-synchronization.test.tsx`: render a tiny component with an `<audio>` ref.
  - `playbackRate` 2 sets `element.playbackRate = 2` and `element.preservesPitch = true`, and `webkitPreservesPitch = true` when that property exists on the element.
  - Changing to 1 resets the rate.
  - Seeks still use the drift rule.
- [ ] **Run** `rtk pnpm vitest run src/lib/timeline-preview.test.ts src/editor/preview/use-media-synchronization.test.tsx` (fail).
- [ ] **Implement.**
  - **In `buildTimelinePreviewFrame`:**
    - the audio branch computes `const playbackSpeed = previewPlaybackSpeedForItem(item)`;
    - pass it to `transitionHandleSourceSeconds` and `previewSourceTimeForItem` instead of `1`;
    - accept `asset.kind` `audio`, `video` or `generated`;
    - add `playbackRate` to `TimelinePreviewAudioLayer` and `TimelinePreviewLayer`.
  - **In `useMediaSynchronization`:** add an optional `playbackRate`. Apply it and `preservesPitch` (plus `webkitPreservesPitch` when present) in `synchronizeClock`.
  - **In `media-layer.tsx`:** pass `layer.playbackRate` for video and audio layers.
  - **Verify on WebKitGTK:** `preservesPitch` support can't be proven in jsdom. Note in the commit body that native WebKitGTK behavior is unverified until workstream 06.
- [ ] **Verify.**
  - `rtk pnpm vitest run src/lib/timeline-preview.test.ts src/editor/preview`
  - `rtk pnpm lint`
- [ ] **Commit:** `feat(preview): play retimed audio at clip speed with pitch preserved`

### Task 7: Preview counts lower-track clips under a flattened group as covered

**Files:**
- Modify: `src/lib/preview/transition-eligibility.ts`, `src/lib/preview/transition-eligibility.test.ts`, `src/lib/timeline-preview.ts`, `src/lib/timeline-preview.test.ts`, `src/editor/preview/canonical-frames.ts`, `src/editor/preview/canonical-frames.test.ts`

- [ ] **Failing tests.**
  - **`transition-eligibility.test.ts`.** `flattenGroupsAt(timeline, sources, frameSeconds, playheadSeconds)`:
    - returns the active groups (`start`, `end`, `topTrackIndex`) for a screen-blend clip on track 1 over a plain clip on track 0, with no transitions (the current early return skips this case);
    - groups widen over flattenable transitions exactly as `transitionEligibility` does;
    - `transitionEligibility` results are unchanged. Keep every existing assertion.
  - **`timeline-preview.test.ts`.** The frame's `flattenedCoverItemIds` lists the plain lower-track clip id (track index ≤ `topTrackIndex`, active at the playhead inside the group). It is absent outside the group span and for tracks above the top track.
  - **`canonical-frames.test.ts`.** `canonicalCoverageItemIds` includes those ids when an active `flatten-…` prepared layer exists, and not otherwise (mirror the existing "covers both clips of a transition baked into a flattened composite" test).
- [ ] **Run** `rtk pnpm vitest run src/lib/preview/transition-eligibility.test.ts src/lib/timeline-preview.test.ts src/editor/preview/canonical-frames.test.ts` (fail).
- [ ] **Implement.**
  - Factor group planning out of `transitionEligibility` into `flattenGroups(timeline, sources, frameSeconds)`, called by both.
  - Rich spans come from enabled video tracks, as today. Transitions only widen.
  - Mirror Rust `collect_dependencies`: tracks `≤ top_track_index`, enabled, video kind, and item active span overlapping the group.
  - Export `flattenGroupsAt`.
  - `buildTimelinePreviewFrame` sets `flattenedCoverItemIds` when non-empty.
  - `canonicalCoverageItemIds` adds them inside its existing `flatten-` branch.
  - Update the doc comments that cite Rust names.
- [ ] **Verify** with the same Vitest command plus `rtk pnpm vitest run src/editor/preview`.
- [ ] **Commit:** `fix(preview): leave lower-track clips inside a flattened group to its prepared frames`

### Task 8: Audio Speed tab, and speed that follows linked clips

**Files:**
- Create: `src/editor/properties/speed-tab.tsx`
- Modify:
  - `src/lib/timeline-ops/clip-commands.ts`, `src/lib/timeline-ops/clip-commands.test.ts`
  - `src/lib/properties/visual-properties.ts`: doc comment on `speedActions` only
  - `src/editor/properties/visual-speed-animation.tsx`: remove `VisualSpeedTab`
  - `src/editor/properties/visual-tabs.tsx`
  - `src/editor/properties/audio-tabs.tsx`, `src/editor/properties/audio-tabs.test.tsx`
  - `src/editor/properties/property-tabs.ts`, `src/editor/properties/properties-panel.test.tsx`
  - `src/editor/properties/mobile-property-sheets.tsx`, `src/editor/properties/mobile-property-sheets.test.tsx`
  - `src/editor/timeline/timeline-context-menu.test.tsx`
  - `src/editor/timeline/speed-dialog.tsx`: comment only

- [ ] **Failing tests.**
  - **`clip-commands.test.ts`:**
    - `setItemSpeed(project, "music", 2)` returns `[updateAudioClipSpeed, resizeItems]` with duration `d/2`. This replaces the "Speed is available for video and image clips." assertion at line ~478.
    - A video clip linked to an audio clip returns speed and resize actions for both in one result.
    - A collision on the partner's track blocks with `"… Make room after this clip first."`.
    - Text overlays and captions are still blocked with "Speed is available for video, image and audio clips."
  - **`audio-tabs.test.tsx`:** replace "has Basic and Voice tabs and no Speed tab" with Basic, Voice and Speed tabs. The Speed preset 2× commits `updateAudioClipSpeed` plus `resizeItems`.
  - **`properties-panel.test.tsx`:** the audio tab list includes "Speed".
  - **`mobile-property-sheets.test.tsx`:** `tools("audio")` includes `"Speed:speed"`.
  - **`timeline-context-menu.test.tsx`** (~line 225): Speed… is enabled for an audio clip.
- [ ] **Run** `rtk pnpm vitest run src/lib/timeline-ops/clip-commands.test.ts src/editor/properties src/editor/timeline/timeline-context-menu.test.tsx` (fail).
- [ ] **Implement.**
  - **`setItemSpeed`:**
    - accept `isVisualOpacityItem(item) || item.kind === "audio_clip"`;
    - choose `updateVisualClipSpeed` or `updateAudioClipSpeed` by kind;
    - collect partners sharing `stringProperty(item, "linkGroupId")` that are visual or audio clips, not text;
    - evaluate each partner's `evaluateTimelineResize` for `duration * partnerSpeed / speed`;
    - return all actions, or the first block.
  - **`speed-tab.tsx`:** `ClipSpeedTab`, moved verbatim from `VisualSpeedTab`.
  - `visual-tabs.tsx` and `audio-tabs.tsx` render it for tab `speed`.
  - `propertyTabsForKind("audio")` adds `tab("speed", "Speed")`, and its doc comment is updated.
  - `toolTabsByKind.audio` becomes `[["speed","speed"],["volume","basic"],["ai","ai"]]`.
  - Update the `AudioTabBody` doc comment.
- [ ] **Verify.**
  - The same Vitest command.
  - `rtk pnpm lint`
  - `rtk node --test scripts/editor-source-policy.test.ts` (file sizes, tokens)
- [ ] **Commit:** `feat(editor): change audio clip speed from Properties, clip tools and the timeline`

### Task 9: Detach audio in the timeline, clip tools and Audio tab

**Files:**
- Modify: `src/editor/timeline/timeline-commands.ts`, `src/editor/timeline/timeline-commands.test.tsx`, `src/editor/timeline/timeline-context-menu.tsx`, `src/editor/timeline/timeline-context-menu.test.tsx`, `src/editor/timeline/mobile-clip-tools.tsx`, `src/editor/shell/mobile-layout.test.tsx`, `src/editor/properties/visual-tabs.tsx`, `src/editor/properties/visual-tabs.test.tsx`

- [ ] **Failing tests.**
  - **`timeline-commands.test.tsx`.** `commands.detachAudio("item")`:
    - applies one `applyActions` batch built by `planDetachAudio` with fresh ids (`link-<uuid>`, `<itemId>-audio` made unique with `duplicateTimelineItemId`, `newTrackId(timeline, "audio")`);
    - selects the new audio clip;
    - one Undo restores the project (store history length +1 only).
  - **`timeline-context-menu.test.tsx`.** A video clip's menu shows "Detach audio" after "Unlink/Link":
    - it is enabled for a video clip with audio;
    - it is disabled with the tooltip "This clip's sound is already on a linked audio clip." when linked;
    - it is not shown for audio or text clips.
  - **`mobile-layout.test.tsx`.** The "Clip tools" toolbar for a selected video clip has a "Detach audio" button that calls the command.
  - **`visual-tabs.test.tsx`.** The Audio tab's "no linked audio clip" state shows a "Detach audio" button (tokens-only classes, same style as "Retry denoise"), and clicking it detaches. The copy becomes "This clip's sound isn't on its own audio clip yet."
- [ ] **Run** `rtk pnpm vitest run src/editor/timeline src/editor/shell/mobile-layout.test.tsx src/editor/properties/visual-tabs.test.tsx` (fail).
- [ ] **Implement:**
  - `TimelineCommands.detachAudio(itemId)`, using `run(result, (actions) => [<audioItemId>])`;
  - the `MenuAction` in `ClipMenuItems` (reason from `blockedReason(planDetachAudio(...))`, rendered only for `item.kind === "video_clip"`);
  - a `ClipTool` in `mobile-clip-tools.tsx` for video selections, using an `AudioLines` icon from `lucide-react` (verify the icon export exists: `rtk grep -n "AudioLines" node_modules/lucide-react/dist/lucide-react.d.ts`, else use `Music`);
  - the button in `VisualAudioTab`.
- [ ] **Verify.**
  - The Vitest command.
  - `rtk pnpm lint`
  - `rtk node --test scripts/editor-source-policy.test.ts`
  - Check that `timeline-context-menu.tsx` stays under 600 lines.
- [ ] **Commit:** `feat(editor): detach a video clip's audio from the timeline, clip tools and Audio tab`

### Task 10: Detached and retimed audio in render and NLE export

**Files:**
- Create: `src-tauri/tests/render_transitions_ges/detach_audio.rs`, `src-tauri/tests/project_nle_export/audio_edits.rs`
- Modify: `src-tauri/tests/render_transitions_ges.rs`, `src-tauri/tests/render_transitions_ges/media.rs` (`write_video_with_tone`), `src-tauri/tests/project_nle_export.rs`, `src-tauri/src/project/nle_export.rs`

- [ ] **Failing render tests** (`detach_audio.rs`):
  - **Plan contents.** The media is `write_video_with_tone` (VP8 red plus a 440 Hz Opus tone in one WebM, 4 s) and the project is a single video clip. `build_project_webm_render_plan` has 0 audio clips before detaching. After `apply_project_action(DetachAudio …)` it has 1, with `source_path` = the video file, the same `source_in`/`source_out`, and `timelineDurationSeconds` equal to the clip.
  - **Rendered sound.** The GES render of the detached project has steady RMS > 0.3 over 1–3 s and dominant frequency 440 Hz ±3%. The render before detaching has RMS < 0.01 (no sound: documents the model).
  - **Detach plus speed 2.** Output length and pitch as in Task 4.
- [ ] **Failing NLE tests** (`project_nle_export/audio_edits.rs`):
  - **DaVinci FCPXML, retimed audio.** A retimed audio clip at speed 2 writes `<timeMap frameSampling="floor">` inside its audio `asset-clip`, and the `start` is scaled like `exports_davinci_fcpxml_with_retimed_clip_time_map`.
  - **Detached audio, both formats.** It is exported exactly once:
    - XMEML: one audio `clipitem` for the audio clip, and no `<masterclipid>` source-audio duplicate for the video, because `audioDetached` excludes it in `xmeml_has_source_audio`;
    - FCPXML: one audio `asset-clip`.
  - **XMEML, retimed clips:** see the verification step below.
- [ ] **Verify the XMEML retime representation before writing it.** Look for a public, citable FCP7 XML reference for the Time Remap filter (`effectid` `timeremap`, parameters `speed`, `reverse`, `variablespeed`, `frameblending`). A reference qualifies if it is Apple's "Final Cut Pro XML" documentation or the xmeml DTD (`xmeml.dtd` version 4/5), fetched with WebFetch.
  - **With a reference:** write `<filter><effect><name>Time Remap</name><effectid>timeremap</effectid>…<parameter><parameterid>speed</parameterid><value>200</value>…` for retimed video and audio clipitems, test the exact XML, and cite the reference URL in the commit body.
  - **Without one:** add `"speed"` to `nle_clip_limitation_note` for `premiereXmeml` when `clip_is_retimed(clip)`, and test that the note appears.
  - **Either way,** projects without speed export byte-identical XML. The golden files `src-tauri/tests/fixtures/nle_export/adjacent-clips-without-transitions.{xml,fcpxml}` are unchanged.
- [ ] **Run.** The GES command with filter `detach_audio`, and `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export -- --test-threads=1` (fail where expected).
- [ ] **Implement** the `nle_export.rs` changes: `xmeml_has_source_audio` returns false when `audioDetached == true`, plus the verified XMEML retime output or note. Fix any render-plan gap the tests expose in `project_export.rs` (none expected: `render_audio_media_for_item` accepts video media), and add that file to the commit only if changed.
- [ ] **Verify.**
  - The whole `render_transitions_ges` target.
  - `--test project_nle_export`.
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1`
- [ ] **Commit:** `feat(export): render and export detached and retimed audio clips`

### Task 11: Lottie precompose with transition handles, render-verified on Linux

**Files:**
- Create: `src-tauri/tests/render_transitions_ges/lottie_handles.rs`, `src-tauri/tests/fixtures/transitions/lottie-colour-steps.json`
- Modify: `src-tauri/tests/render_transitions_ges.rs`. Also `src-tauri/src/precompose/mod.rs`, only if a defect is found.

- [ ] **Build the worker.** `rtk pnpm build:precompose-sidecar:dev` (foreground). Confirm `src-tauri/target/debug/video-creater-precompose-worker` exists.
  - If the build fails (for example ThorVG, bindgen or clang), record the error.
  - Retry once with `VIDEO_CREATER_PRECOMPOSE_WORKER=/tmp/vc-deb-root/usr/bin/video-creater-precompose-worker` (the release worker from the `.deb` staging tree). A protocol mismatch fails loudly.
  - Record which worker ran.
- [ ] **Author the fixture.** A 64×64, 24 fps, 4 s (96 frames) Lottie JSON: one shape layer, a full-frame rectangle whose fill colour steps every 7 frames through hold keyframes (`"h": 1`) on `c`, cycling through distinct colours. Verify it renders by preparing one Lottie clip and decoding frames 0, 7 and 14 of the intermediate with `decode_video_frames_rgba`. Each must show its step colour (channel diff ≤ 8).
- [ ] **Failing tests** (`lottie_handles.rs`). Media: the fixture copied into the project as `MediaKind::Lottie`, plus the warm/cool segmented videos.
  - **`lottie_clip_keeps_both_transition_handles`.**
    - Setup: cool video 0–2 s → Lottie clip 2–4 s (`sourceIn` 1, `sourceOut` 3) → warm video 4–6 s, with a 1 s crossfade and a 1 s dip to black.
    - After `prepare_project_for_render`:
      - the Lottie item's media id starts with `precompose-`;
      - `sourceIn == 0.5`;
      - the prepared media duration is 3.0 s;
      - `plan_clip_transitions(&prepared.project)` still has 2 transitions of 1 s.
    - The handle frame at prepared 0.25 s equals the Lottie source at 0.75 s (the colour-step index).
  - **`lottie_handles_render_like_canonical_frames`.** `assert_render_parity("lottie-handles", dir, &prepared.project, &prepared.project, &transition_sample_times(&[2.0, 4.0]))`. Evidence goes to `output/transition-preview-parity/lottie-handles/`.
  - **`sped_up_lottie_clip_keeps_handles`.** The same setup at speed 1.5 (`sourceOut = sourceIn + 2 * 1.5`). The prepared intermediate covers `head + duration + tail` timeline seconds, with source start `sourceIn - head * 1.5` (`prepare_lottie_task`). Parity passes at the two cuts.
  - **Missing worker.** If `precompose_worker_path` resolves to a missing file, the tests `panic!` with "Build the Lottie worker with `pnpm build:precompose-sidecar:dev` or set VIDEO_CREATER_PRECOMPOSE_WORKER." They never skip silently.
- [ ] **Run** the GES command with filter `lottie_handles`. Record the pass or the failure detail.
- [ ] **Fix only real defects** in `precompose/mod.rs` `prepare_lottie_task` (handle span or source start), with the failing assertion cited.
- [ ] **Verify** with the whole `render_transitions_ges` target, and read `output/transition-preview-parity/lottie-handles/preview-comparison.json` (`status: passed`).
- [ ] **Commit:** `test(render): verify Lottie precompose transition handles with the Linux worker`. If a defect was fixed, use `fix(precompose): …`. The body lists the worker path used and the mismatch ratios.

### Task 12: Reverse feasibility spike (decision 14)

**Files:**
- Create: `docs/research/2026-09-16-reverse-intermediate-spike.md`, and `src-tauri/tests/reverse_intermediate_spike.rs`. The harness is committed only on go; on no-go its source is quoted in the doc.

- [ ] **Harness** (`reverse_intermediate_spike.rs`, `#![cfg(feature = "ges-render")]`, runtime from `start_render_process_runtime`). Media, all generated with `gst::parse::launch` pipelines like `render_transitions_ges/media.rs`:
  - (i) a segmented VP8 WebM (14 × 7 frames);
  - (ii) the same content as H.264 MP4 via `openh264enc ! h264parse ! mp4mux`;
  - (iii) a 30 s 1920×1080 24 fps H.264 MP4 plus AAC (`avenc_aac`) for cost;
  - (iv) an Opus tone sweep (the frequency rises over time, so reversal is measurable).
- [ ] **Strategy A: negative-rate seeks.**
  - Pipeline: `filesrc ! decodebin ! videoconvert ! appsink` (and the audio equivalent).
  - Seek with `rate = -1.0`, flags `FLUSH | ACCURATE`, start `sourceIn`, stop `sourceOut`.
  - Record per decoder (`vp8dec`, `avdec_h264`, `openh264dec`, `opusdec`, `avdec_aac`):
    - whether frames arrive in strictly descending PTS order;
    - frames delivered vs expected;
    - duplicates, and wall time.
- [ ] **Strategy B: forward decode and reversed reassembly.**
  - **Video:**
    - `decode_video_frames_rgba` forward over `[sourceIn, sourceOut]` into PNGs on disk;
    - rename in reverse order;
    - `package_png_frames_as_mov` into a MOV;
    - also try `avenc_prores_ks` for (iii). `require_allowed_factories` is `pub(crate)`, so verify the factory from the integration test with `gst::ElementFactory::find("avenc_prores_ks")` plus the public `video_creater_lib::render_pipeline::plugin_policy::evaluate_gstreamer_factory` on a `GstFactoryInfo` built from its plugin name, package and license.
  - **Audio:** `precompose::decode_to_wav` (it is `pub(crate)`: call it from a `--lib` test module instead, or duplicate its `decodebin ! audioconvert ! audioresample ! wavenc` pipeline in the harness), then reverse the sample frames with `hound` and write WAV.
  - **Verify the result:**
    - the reversed output decodes, and the segment colour order is exactly reversed;
    - zero dropped or duplicated frames;
    - the tone sweep falls instead of rising;
    - GES renders the reversed intermediate as a clip with a 0.5 s crossfade on both sides (use `assert_render_parity`-style checks);
    - record wall time and intermediate size for (iii).
- [ ] **Go/no-go criteria.** Write them into the doc before running, then fill in the numbers. It is **go** only if all of these hold:
  1. At least one strategy yields exactly reversed video and audio for (i), (ii) and (iv).
  2. GES renders that intermediate with transitions, with parity mismatch ratio ≤ 0.01.
  3. For (iii), preparation takes ≤ 120 s wall time on this host, and the intermediate is ≤ 4 GB with PNG-MOV or ≤ 1 GB with ProRes.
  4. The approach uses only reviewed LGPL/BSD factories on Linux, and the macOS path has a named equivalent (`vtenc_prores` or PNG-MOV), even though it is unverified here.

  Otherwise it is **no-go**.
- [ ] **Run** `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --test reverse_intermediate_spike -- --test-threads=1 --nocapture` in the foreground. Paste the observed numbers into the doc.
- [ ] **Doc content:**
  - the date, runtime id, GStreamer version;
  - the commands;
  - a per-strategy table of the numbers;
  - the criteria with pass or fail each;
  - the decision.
- [ ] **Commit.**
  - **Go:** `docs(research): record the reverse intermediate spike` (doc plus harness).
  - **No-go:** the same subject, with the doc only.

### Task R0 (only on no-go): keep Reverse as a documented deviation

- [ ] **Leave Reverse UI absent.** Confirm the existing comment in `src/editor/properties/visual-speed-animation.tsx` (or `speed-tab.tsx` after Task 8) cites the spike doc path. If the comment is changed, include that file in the commit.
- [ ] **The backlog row is added in Task 15.**
- [ ] **Commit** (only if a comment changed): `docs(editor): cite the reverse spike where Reverse is omitted`

### Task R1 (go only): reverse model and action lockstep

**Files:**
- The Task 2 lockstep file set.
- Pure source-mapping helpers in new files:
  - `src-tauri/src/project/reverse.rs`, with `pub mod reverse;` in `project/mod.rs`;
  - `src/lib/timeline-ops/reverse.ts`, with test `reverse.test.ts`.
- Call-site edits in every place that maps timeline time to source time. Each was verified to exist:
  - Rust: `project/action.rs` `item_properties_for_subrange`, `project/transitions.rs` `item_source_window` and the handle functions, `render_pipeline/transition_plan.rs` `extend_item`, the `render_pipeline/project_export.rs` range overlap mapping (~lines 2176–2181 and 2289–2290), and `project/nle_export.rs` source ranges (a limitation note: "reverse").
  - TS: `src/lib/timeline.ts` (~lines 311–391), `src/lib/timeline-edit-evaluator.ts` (~296–375), `src/lib/timeline-ops/transitions.ts` (~91–112), `src/lib/preview/transition-frame.ts` `transitionHandleSourceSeconds`, and `src/lib/timeline-preview.ts` `previewSourceTimeForItem`.

**Contract:** `{ type: "updateClipReverse"; itemId: string; reverse: boolean }`.
- It applies to `video_clip` with video media and to `audio_clip`.
- `reverse: true` sets `properties.reverse = true`; false removes it.
- Reversed mapping:
  - local `t` → source `sourceOut - t * speed`;
  - head handle `(mediaDuration - sourceOut) / speed`;
  - tail handle `sourceIn / speed`;
  - splitting keeps the left part's `sourceOut`.
- `setItemSpeed`-style linked propagation applies in the UI command (R3), not in the action.

- [ ] **Tests first**, Rust and TS mirrored with identical numbers: mapping, handles, split, trim-left, trim-right, transitions max duration, and the range render overlap.
- [ ] **Implement** helpers and call sites. Add schema, prompt, risk (`Safe`), the MCP `set_clip_properties` `reverse` field, and the TS union member.
- [ ] **Verify:**
  - Task 2's cargo command, plus `--test project_split --test render_pipeline --test project_nle_export`;
  - `rtk pnpm vitest run src/lib`;
  - `rtk pnpm lint`.
- [ ] **Commit:** `feat(project): add reversed clip playback to the project model and agents`

### Task R2 (go only): reversed intermediates in render preparation

**Files:**
- Create: `src-tauri/src/precompose/reverse.rs`, `src-tauri/tests/render_transitions_ges/reverse.rs`
- Modify: `src-tauri/src/precompose/mod.rs` (module and call), `src-tauri/src/precompose/planner.rs` (`PreparationReason::Reverse`), `src-tauri/src/precompose/cache.rs` (a `ReverseFingerprint` struct, if the existing fingerprint helpers need one), `src-tauri/tests/render_transitions_ges.rs`

- [ ] **Tests first.** The spike's winning strategy, as a production preparation step:
  - the reversed video intermediate covers head + duration + tail;
  - the prepared item is `reverse`-free, with `sourceIn = head`;
  - reversed audio clips get a reversed WAV intermediate (audio tracks are planned like `audio_denoise::collect_tasks`);
  - the cache is reused on the second run;
  - GES parity passes through a crossfade (`assert_render_parity("reverse-handles", …)`);
  - the tone sweep falls.
- [ ] **Implement,** then run the GES command for `render_transitions_ges` and `--lib precompose`.
- [ ] **Commit:** `feat(render): render reversed clips from cached reversed intermediates`

### Task R3 (go only): reverse in preview and UI

**Files:**
- Modify: `src/lib/timeline-preview.ts` (reversed layers need canonical preparation), `src/editor/preview/compositor-model.ts` and `src/editor/preview/timeline-preview.tsx` (reversed audio plays the prepared audio intermediate from the ready `PreparedProjectPreview.project` media via `previewUrlForMedia`, else it is silent with the issue "Reversed audio is preparing."), `src/editor/properties/speed-tab.tsx` (a "Reverse" `SwitchField`), `src/editor/timeline/timeline-context-menu.tsx` ("Reverse" / "Play forward"), and their tests.

- [ ] **Tests first:**
  - the switch commits `updateClipReverse` for the clip and its linked partner in one batch;
  - a reversed layer has `canonicalPreparationRequired: true`;
  - the compositor uses prepared frames;
  - the reversed audio layer URL is the prepared media path.
- [ ] **Implement and verify:**
  - `rtk pnpm vitest run src/lib src/editor`
  - `rtk pnpm lint`
  - `rtk node --test scripts/editor-source-policy.test.ts`
- [ ] **Commit:** `feat(editor): reverse clips from the Speed tab and timeline`

### Task R4 (go only): render-verify reverse end to end

- [ ] Add the Reverse steps to `e2e/editor-media-editing.spec.ts` (Task 13).
- [ ] Re-run the GES `render_transitions_ges` target and the parity reports. Record the ratios.
- [ ] **Commit:** `test(render): verify reversed clips through transitions and speed`. This commit is tests only; stage only the changed test files.

### Task 13: Playwright acceptance for media editing

**Files:**
- Create: `e2e/editor-media-editing.spec.ts`

Sample project: Video 1 holds "Opening clip" (0–4 s) and "Restored Edison alternate" (4–8 s); Audio 1 holds "Music bed" (0–4 s). Use `openSampleEditor` from `e2e/support/editor-fixture.ts` and `acceptanceViewports` from `e2e/support/editor.ts`.

- [ ] **Desktop 1440×900:**
  1. Select "Music bed". The Properties panel has a Speed tab. Choose 2×, and the clip's `aria-label` duration becomes 00:00:02.
  2. Press Meta+Z, and the duration is back to 00:00:04. That is one undo step.
  3. Right-click "Opening clip" and choose "Detach audio". A new "Opening clip audio" option appears on a new audio track, because Music bed collides.
  4. Right-click "Opening clip" again. "Detach audio" is disabled with the tooltip "This clip's sound is already on a linked audio clip."
  5. Meta+Z removes the audio clip and the new track in one step.
  6. On go only: Speed tab → Reverse on, and the clip shows the reversed state.
- [ ] **Phone 402×874:**
  - Select "Opening clip". The Clip tools have "Detach audio"; tap it, and the audio clip exists.
  - Select "Music bed". The Clip tools have "Speed".
- [ ] **Run** `rtk pnpm exec playwright test e2e/editor-media-editing.spec.ts`. Then capture screenshots of the audio Speed tab and the context menu at both sizes to `output/editor-media-editing/`, and read the PNGs.
- [ ] **Commit:** `test(editor): cover audio speed and detach audio at desktop and phone sizes`

### Task 14: Full verification gate

No planned commit. A fix commit names only its files, with `fix(<scope>): …`.

- [ ] `rtk pnpm verify:frontend`. It must exit 0. Record each step's counts, as in the hand-off table.
- [ ] Rust, each in the foreground:
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action --test codex_app_server --test codex_mcp_server --test codex_conversation --test project_nle_export --test project_split --test render_pipeline -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" VIDEO_CREATER_COMPATIBILITY_DECODER=/home/olhapi/projects/video-creater/src-tauri/target/debug/video-creater-compatibility-decoder rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a" rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1`. The known environment-only failure `settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe` may remain. Name it; don't count it as a pass.
  - `rtk pnpm rust:fmt`, then `rtk pnpm rust:clippy`.
  - `rtk pnpm test:linux-media-runtime`, `rtk pnpm test:gstreamer-runtime-policy`, `rtk pnpm test:source-quality`.
- [ ] **Read the parity reports** under `output/transition-preview-parity/` (every-kind, flattened-transitions, lut-handles, speed-transitions, lottie-handles, and reverse-handles on go). All must have `status: passed`. Record the maximum mismatch ratios.
- [ ] **Check the worktree.** `rtk git status --short` is clean apart from git-ignored `output/`.

### Task 15: Update the backlog rows (only after Task 14 passed)

**Files:**
- Modify: `docs/product-backlog.md`

- [ ] **Edit only after Task 14's observed results.**
  - **VC-017:** status `implemented`. Next action: "Retain native WebKitGTK and packaged evidence that preview audio keeps pitch at non-1× speed (workstream 06); macOS AVFoundation retimed audio falls back to GES and the macOS `audiofx` plugin addition is unverified." Cite the commits of Tasks 1, 2, 4, 6 and 8.
  - **VC-018:** status `implemented`. Next action: "Retain packaged native evidence of Detach audio and its single Undo (workstream 06)." Cite Tasks 3, 9 and 10.
  - **VC-015:**
    - Remove the three follow-ups this plan closed, citing Tasks 5, 6 and 7 and the parity report paths.
    - Keep the status `implemented`.
    - Keep any follow-up that failed or was not run, stated as not run.
  - **Reverse.**
    - **On go:** no row change. The hand-off deviation is updated by workstream 06.
    - **On no-go:** add a row after the last ID. Take the next free `VC-0NN` at execution time (`rtk grep -n "^| VC-" docs/product-backlog.md`), with P2, `blocked`, "Reverse clip playback", user outcome "Play a clip backwards in preview and render.", inspiration Original, and next action "Reversed intermediates failed the feasibility criteria on the Linux runtime; see `docs/research/2026-09-16-reverse-intermediate-spike.md`."
  - Update "Last updated" to the execution date.
- [ ] **Verify** `rtk pnpm check:source-quality` (docs policy, if any) and `rtk git diff docs/product-backlog.md`. The diff contains only these rows and the date.
- [ ] **Commit:** `docs(backlog): close audio speed, detach audio and transition speed follow-ups`

---

## Acceptance

| Gap | Proof |
| --- | --- |
| VC-017 action and agents | Task 2 tests: `project_action` `audio_edits`, `codex_app_server` schema and type list, `codex_conversation` safe allowlist, `codex_mcp_server` `audio_edits`, `src/lib/project-audio-actions.test.ts` |
| VC-017 pitch-preserving render | Task 1 `verify:linux-media-runtime` shows `scaletempo` loaded from the LGPL `audiofx` plugin. Task 4 GES tests: 440 Hz ±3% at speed 2, timeline length, fades, crossfade; AVFoundation rejection unit test |
| VC-017 preview | Task 6 `timeline-preview.test.ts` (audio mapping at speed, audio from video media) and `use-media-synchronization.test.tsx` (`playbackRate`, `preservesPitch`). Native WebKitGTK pitch is recorded as not run here |
| VC-017 UI | Task 8 tests (audio Speed tab, clip tool, context menu, linked speed) and Task 13 Playwright steps 1–2 |
| VC-017 handles and NLE | Task 2 transition revalidation test; Task 4 retimed crossfade render; Task 10 FCPXML `timeMap` for audio, and XMEML Time Remap or a limitation note |
| VC-018 action | Task 3 Rust and TS mirror tests (one action, linked, properties moved, all blocks) |
| VC-018 UI and one undo step | Task 9 tests (menu, clip tool, Audio tab, one history entry) and Task 13 steps 3–5 plus phone |
| VC-018 render and export | Task 10: render plan audio clip from video media; GES output RMS and 440 Hz after detaching and silence before; single audio in XMEML and FCPXML |
| Reverse | Task 12 doc with measured criteria. On go: R1–R4 tests, the `reverse-handles` parity report and Task 13 step 6. On no-go: the doc plus the Task 15 backlog row |
| VC-015 speed + transitions | Task 5 `speed.rs`: first-frame motion (frozen start reproduced and fixed, or recorded as not reproduced) and the `speed-transitions` parity report `status: passed` |
| VC-015 preview audio crossfade follows speed | Task 6 handle-mapping test for retimed audio crossfades |
| VC-015 flattened-group coverage | Task 7 tests in `transition-eligibility`, `timeline-preview` and `canonical-frames` |
| Lottie handles (decision 20) | Task 11 `lottie_handles.rs` with the worker path recorded, and `output/transition-preview-parity/lottie-handles/preview-comparison.json` `status: passed` |
| No regressions | Task 14 gate: `verify:frontend` exit 0, the named cargo targets, GES targets, clippy and fmt, all observed |

## Risks that may block a gap

- **The frozen start may come from GES time-effect semantics that can't be fixed in place.** Task 5 then needs a retimed intermediate. That is a larger change: stop and record it rather than shipping a workaround.
- **`scaletempo` timing under GES may not match `videorate`'s measured `inpoint / s + t * s`.** Task 4 measures it before binding envelopes.
- **The Lottie worker may not build on this host** (ThorVG/bindgen toolchain). The fallback is the `.deb` release worker in `/tmp/vc-deb-root`, which may be protocol-incompatible. In that case Task 11 records the blocked build, and decision 20 stays open.
- **WebKitGTK `preservesPitch` support is unverified.** Only workstream 06's native run can prove the Linux preview pitch.
- **An XMEML Time Remap reference may not be verifiable.** Retimed clips then carry a limitation note instead of real speed in Premiere XML.
- **`nle_export.rs`, `project.ts`, `action.rs`, `app_server.rs` and `tools.rs` are contended** with workstreams 01, 02, 03 and 05. Serialize, or expect rebase conflicts.
- **Reverse touches every source-mapping site in Rust and TS.** R1 is the largest task. A partial implementation must not ship; the whole R chain lands or none of it does.
