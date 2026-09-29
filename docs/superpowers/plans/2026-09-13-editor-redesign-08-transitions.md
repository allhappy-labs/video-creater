# Editor Redesign 08 — Clip Transitions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Expand each task into bite-sized TDD steps with exact code before executing it. Rust tasks must follow `src-tauri/tests/project_action.rs` conventions.

**Goal:** Add four real clip-to-clip transitions: crossfade, dip to black, dip to white, and wipe. They work end to end:

- project model and actions (Rust and TypeScript),
- validation and maintenance under edits,
- GES rendering, including audio crossfade and the missing GES audio fade support,
- DOM preview and canonical preview sampling,
- NLE XML export,
- Codex/MCP schema and the safe-action allowlist,
- Effects tab tiles, timeline badges, the Properties panel, and the context menu.

**Architecture:**

- **Storage.** A transition lives on the track: `TimelineTrack.transitions: Vec<TimelineTransition>`, with serde default empty. It references two adjacent items and is centered on their cut.
- **Canonical timeline.** It never overlaps clips, so `validate_no_increased_timeline_overlap` and ripple logic stay untouched.
- **Handles.** A transition needs `duration/2` of unused source media after the left item's `sourceOut` and before the right item's `sourceIn`.
- **Render.** The render plan expands both clips into their handles, which creates the overlap only in the GES timeline.
- **Preview.** Preview and canonical sampling compute the same transition window.
- **macOS export.** AVFoundation rejects plans that contain transitions, so the existing selection falls back to GES.

**Tech Stack:** Rust 1.87, serde, gstreamer-editing-services (GES) bindings, the existing `frame_compositor` and `precompose` modules, TypeScript, React, Vitest, Playwright.

**Spec sections:** Backend Additions §2 Transitions, Timeline (Transitions), Left Tabs → Effects, Properties (Transition), Acceptance flow 7.
**Depends on:** plans 01–07.

## Global Constraints

- Prefix commands with `rtk`. Use Conventional Commits and stage only the named files.
- **Keep Rust and TypeScript in lockstep.** Every new action lands in the same commit as all of these:
  - the Rust enum variant,
  - its TS union member,
  - its `applyProjectActionLocally` case,
  - its Codex schema entry and MCP tool support.
- **Allowed kinds.** `kind` is `"crossfade" | "dipToBlack" | "dipToWhite" | "wipe"`. The wire form is camelCase.
- **Duration bounds.** Between 1 frame and 5 s. `max = min(5, 2 * leftHandle, 2 * rightHandle, leftItem.duration, rightItem.duration)`.
- **Adjacency.** `|left.start + left.duration - right.start| ≤ 1 / fps`, and both items are on the same track. Items must be visual (`video_clip`, `image_clip`, `generated_clip`) or `audio_clip`. Mixed visual and audio pairs are invalid.
- **Validation.** Returns user-facing messages, for example: "Not enough unused media after <left label> for a 1.0s transition. Maximum is 0.4s."
- **Maintenance.** Any action that moves, trims, resizes, splits, removes or ripples an item referenced by a transition must, in the same action application:
  - drop the transition when adjacency no longer holds,
  - or clamp its duration to the new maximum when adjacency still holds.

  No separate user action is required.
- **Render and preview parity.** Transition preview and render must match within the thresholds of the existing preview-render comparison tooling.
- **GES API.** Verify every GES and GStreamer property name against the bindings in `src-tauri/Cargo.lock`, using `cargo doc` or source, before relying on it. Plan-prescribed names are unverified until tested.

---

## File Map

### Rust

- **`src-tauri/src/project/model.rs`**
  - `TimelineTransition { id, left_item_id, right_item_id, kind: TransitionKind, duration_seconds }` and `enum TransitionKind { Crossfade, DipToBlack, DipToWhite, Wipe }`, both `rename_all = "camelCase"`.
  - `TimelineTrack.transitions: Vec<TimelineTransition>` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- **`src-tauri/src/project/transitions.rs`** (new)
  - `validate_transition(project, track, transition) -> Result<(), ProjectActionError>`
  - `transition_max_duration(project, track, left, right) -> f64`
  - `maintain_transitions(project: &mut VideoProject)`, which drops or clamps invalid transitions. It is called after every item-mutating action inside `apply_project_action` before `recalculate_duration`.
- **`src-tauri/src/project/action.rs`**
  - New variants: `AddTransition { track_id, transition }`, `UpdateTransition { track_id, transition_id, kind: Option<TransitionKind>, duration_seconds: Option<f64> }`, `RemoveTransition { track_id, transition_id }`.
  - Match arms and error variants.
  - A `maintain_transitions` call in the post-apply section.
- **`src-tauri/src/project/split.rs`**: confirm that split-folder serialization round-trips `transitions`. Add a test.
- **`src-tauri/src/render_pipeline/project_export.rs`**
  - `RenderTransition { kind, start_seconds, duration_seconds, left_clip_index, right_clip_index }` on the render plan.
  - `build_project_render_plan_with_range` extends the left clip's `duration` and `source_out` by `d/2`, and the right clip's `start` and `source_in` by `-d/2`, only for rendering.
  - `render_clip_properties` allowlist entries.
  - Nested timeline expansion carries transitions through.
- **`src-tauri/src/render_pipeline/gstreamer_backend.rs`**
  - Place both clips of a transition pair on the same GES layer, with the layer's `auto-transition` enabled. GES then creates a `GESTransitionClip`. Set its video transition type:
    - `crossfade` for Crossfade,
    - a SMPTE wipe, for example `bar-wipe-lr`, for Wipe.
  - **Dips:** keep the default crossfade overlap disabled for the pair, meaning separate layers. Animate each clip's alpha with `apply_visual_alpha_envelope` through 0 at the cut. Add a solid color source clip (`GESTestClip` with a black or white pattern) beneath both clips for `d`.
  - **Audio:** equal-power crossfade volume control points for audio pairs, and for the audio streams of video pairs.
  - **Fades:** `apply_audio_clip_properties` gains `fadeInSeconds` / `fadeOutSeconds` volume envelopes. This closes the existing GES audio fade gap.
- **`src-tauri/src/render_pipeline/avfoundation_backend.rs`**: `reject_unsupported_video_properties` or `build_request` rejects plans with transitions, with a clear error. Selection then falls back to GES. Add a test that asserts fallback selection.
- **Preview sampling.** In `src-tauri/src/precompose/flatten.rs` and `src-tauri/src/frame_compositor/*`, the canonical frame sampling honors transitions using the same window math:
  - crossfade alpha,
  - dip alpha through a solid,
  - wipe mask left to right.
- **`src-tauri/src/project/nle_export.rs`**
  - XMEML: `<transitionitem>` with Cross Dissolve for crossfade, Dip to Color for dips.
  - FCPXML: `<transition>` in the spine.
  - Wipe exports as a cut, with a `nle_clip_limitation_note` entry.
- **`src-tauri/src/codex/app_server.rs`**: supported types prompt list, action schemas, schema test.
- **`src-tauri/src/codex/tools.rs`**: tool descriptors, for example `video_creater.add_transition`, and dispatch.
- **`src-tauri/src/codex/conversation.rs`**: add the three actions to the safe allowlist.

**Tests**
- `src-tauri/tests/project_action.rs`
- `src-tauri/tests/render_pipeline.rs` (plan expansion)
- a `ges-render`-gated transition render test
- `src-tauri/tests/project_nle_export.rs`
- `src-tauri/tests/codex_conversation.rs` (allowlist)
- `src-tauri/tests/codex_app_server.rs` (schema)

### TypeScript

- **`src/lib/timeline.ts`**: `TimelineTransition`, `TransitionKind`, and `TimelineTrack.transitions?`.
- **`src/lib/project.ts`**: union members for `addTransition`, `updateTransition` and `removeTransition`; local application; local maintenance mirroring Rust in `src/lib/timeline-ops/transitions.ts`.
- **`src/lib/timeline-ops/transitions.ts`** (new): `transitionMaxDuration`, `validateTransition`, `maintainTransitions`, `cutsOnTrack(track)` (adjacent pairs), `nearestCut(timeline, seconds, trackId?)`, `transitionWindow(transition, track)`.
- **`src/lib/timeline-preview.ts`**: `buildTimelinePreviewFrame` includes both items during a transition window and adds `transition: { kind, progress }` to their layers. `previewMotionForItem` applies crossfade and dip opacity. Wipe uses `clipPath: inset(...)` via `previewMotionStyle`.
- **UI**
  - `src/editor/panels/effects/transitions-view.tsx`: adds a "Transitions" chip and four tiles with animated previews.
  - `src/editor/timeline/transition-badge.tsx`: badge centered on the cut, with draggable edges for duration.
  - `src/editor/properties/transition-tabs.tsx`: type select and duration slider showing the max.
  - `src/editor/timeline/timeline-context-menu.tsx`: an "Add transition" submenu on cuts and adjacent clips.
  - Timeline selection supports `selectedTransitionId`; `selectionKind` returns `"transition"`.

---

### Task 1: Model and validation (Rust)

- [ ] **Tests** in `src-tauri/tests/project_action.rs`, section "transitions":
  - The serde round trip of a track with transitions uses camelCase and omits the field when empty.
  - `AddTransition` succeeds for adjacent video clips with enough handles.
  - Adjacency rejections:
    - a gap larger than 1 frame,
    - items on different tracks,
    - a mixed audio and video pair,
    - a duplicate transition on the same cut.
  - Handle and duration rejections:
    - short handles report the maximum duration in the message,
    - a duration of 0 or above 5 is rejected.
  - `UpdateTransition` changes kind and clamps nothing when valid. `RemoveTransition` removes it.
  - The wire shape matches `serde_json::from_value(json!({"type":"addTransition", ...}))`.
- [ ] **Implement** the model, `transitions.rs` validation, and the three action variants.
- [ ] **Run:** `rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action transition -- --test-threads=1`
- [ ] **Commit:** `feat(project): add clip transitions to the project model and actions`

### Task 2: Maintenance under edits (Rust)

- [ ] **Tests:**
  - Moving the right item away drops the transition.
  - Trimming the left item's `sourceOut` so fewer handles remain clamps the duration.
  - Splitting the left item re-targets to the new right half. The transition stays between the new adjacent pair: the left half's successor is the split-off right part.
  - Removing either item drops it.
  - `rippleDeleteRanges` that keeps adjacency keeps the transition.
  - Nested timeline decompose carries transitions.
  - Duplicate timeline copies transitions with remapped item ids.
- [ ] **Implement** `maintain_transitions` and call it after item-mutating actions. Make `createTimeline { duplicateActive }` remap ids.
- [ ] **Run** the tests. Also run the full `--test project_action` suite to confirm no regressions.
- [ ] **Commit:** `feat(project): keep transitions valid across timeline edits`

### Task 3: TypeScript mirror and local application

- [ ] **Tests** in `src/lib/timeline-ops/transitions.test.ts` and `src/lib/project.test.ts`: the same cases as Rust Tasks 1–2, using equivalent fixtures so behavior matches. `cutsOnTrack` and `nearestCut` are also covered.
- [ ] **Implement** the types, union members, `applyProjectActionLocally` cases, and `maintainTransitions` after item mutations in the local applier.
- [ ] **Run:** `rtk pnpm vitest run src/lib/timeline-ops/transitions.test.ts src/lib/project.test.ts && rtk pnpm lint`
- [ ] **Commit:** `feat(lib): mirror transition actions and maintenance in TypeScript`

### Task 4: Render plan expansion and AVFoundation fallback

- [ ] **Tests** in `src-tauri/tests/render_pipeline.rs`:
  - The plan for a 1 s crossfade extends the left clip by 0.5 s (`source_out` +0.5) and the right clip by -0.5 s (`start` and `source_in` -0.5).
  - `RenderTransition` has the correct indices and start time.
  - The canonical project is unchanged.
  - Range renders that cut through a transition still include both clips.
  - The AVFoundation `build_request` errors for plans with transitions, and export selection chooses GES. Assert through the selection helper, or refactor the `use_avfoundation` boolean at `project_export.rs:1581` into a testable fn `select_render_backend(...)`.
- [ ] **Implement.**
- [ ] **Commit:** `feat(render): expand transition handles in render plans and route them to GES`

### Task 5: GES rendering (video, audio, fades)

- [ ] **Confirm the GES API first.** Confirm the API available in the pinned bindings:
  - the layer `auto-transition` property,
  - `ges::TransitionClip` and its `vtype` or equivalent,
  - SMPTE transition type names,
  - `GESTestClip` pattern for solid colors,
  - control binding for the audio volume envelope.

  Record the verified names in the commit body.
- [ ] **Tests** (`ges-render` gated `[[test]]` in `Cargo.toml`, following the existing gated tests):
  - Render a 2-clip, 24 fps fixture for each kind: two solid-color sources, red and blue, each 2 s with 1 s of handles.
  - **Crossfade:** the midpoint frame is purple-ish, meaning both channel means sit between the sources within tolerance.
  - **Dip to black:** the midpoint frame is near black.
  - **Wipe:** the left half is blue and the right half is red at the midpoint, or reversed per the chosen direction. Assert the direction.
  - **Audio crossfade:** RMS at the midpoint is within 3 dB of the steady-state RMS (equal power).
  - **Audio fades:** audio clip fade-in starts silent. This is a new assertion covering the fixed fade gap.
- [ ] **Implement** in `gstreamer_backend.rs`.
- [ ] **Run:** `rtk cargo test --manifest-path src-tauri/Cargo.toml --features ges-render --test <gated-test-name> -- --test-threads=1`
- [ ] **Commit:** `feat(render): render crossfade, dip, and wipe transitions with GES`

### Task 6: Preview and canonical sampling parity

- [ ] **TS tests** in `src/lib/timeline-preview.test.ts`:
  - At the transition midpoint the frame contains both layers.
  - Crossfade progress is 0.5, giving opacities 0.5 and 0.5.
  - A dip has opacity 0 at the cut, with a solid layer.
  - Wipe produces a `clipPath` inset of 50%.
  - Outside the window only one layer is present.
  - The DOM compositor test (plan 04) renders both layers.
- [ ] **Rust tests** for precompose and frame compositor sampling at the same timestamps, with the same expectations.
- [ ] **Implement both.**
- [ ] **Parity check.** Create a fixture project with one transition of each kind under `src-tauri/tests/fixtures/` (or the location the existing comparison tooling uses). Run `rtk pnpm visual:qa:preview-render` and `rtk pnpm visual:qa:compare-preview-render` against it. Both must pass the default thresholds, and the report path goes in the commit body.
- [ ] **Commit:** `feat(preview): preview transitions with render parity`

### Task 7: NLE export, Codex schema, MCP tools, safe allowlist

- [ ] **Tests:**
  - **XMEML:** contains a `<transitionitem>` with the correct start and end frames for crossfade and dip.
  - **FCPXML:** contains a `<transition>` with the correct offset and duration.
  - **Wipe:** exported as a cut, with a limitation note.
  - **Codex schema:** accepts the three actions, and the schema test at `app_server.rs` near L4238 is updated.
  - **MCP tools:** `list_codex_local_tools` includes the transition tools, and calling them applies actions.
  - **Conversation risk:** a proposal containing only `addTransition` is classified `safe`.
- [ ] **Implement.**
- [ ] **Run:**
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export --test codex_app_server --test codex_mcp_server --test codex_conversation -- --test-threads=1`
- [ ] **Commit:** `feat(codex): let agents and NLE exports use clip transitions`

### Task 8: Editor UI

- [ ] **Effects tab "Transitions" view:**
  - Four tiles with animated CSS previews.
  - Dropping a tile on a cut, or on the timeline near one within 12 px, adds the transition with a default duration of `min(0.5, max)`.
  - The `+` button adds it to the selected cut, or to the cut nearest the playhead on the selected track.
  - When no cut qualifies, the button is disabled with the tooltip "Place two clips next to each other first".
  - When handles are insufficient, the transition is added at the maximum duration and a toast says "Shortened to 0.4s — not enough unused media".
  - When the maximum is 0, the add is blocked with the validation message.
- [ ] **Timeline badge.**
  - Centered on the cut, `aria-label` "<Kind> transition, <duration>".
  - Clicking it selects the transition, which opens the Properties Transition tab.
  - Dragging its edges changes duration symmetrically, clamped to the maximum, with a live tooltip. It commits `updateTransition` on release.
  - Delete removes it.
- [ ] **Properties Transition tab:** type segmented control, and a duration slider with its max label.
- [ ] **Context menu:** "Add transition ▸ Crossfade / Dip to black / Dip to white / Wipe" on cuts and adjacent clips.
- [ ] **Mobile:** tapping a cut badge selects it. The clip tools bar shows Type, Duration, and Delete.
- [ ] **Tests:**
  - Drop on a cut adds exactly one transition.
  - The shortened toast appears.
  - A badge drag commits one update.
  - Properties edits commit.
  - The context menu adds the transition.
  - Delete removes it.
  - An undo restores the transition after a move dropped it. Maintenance is part of the same action, so this is one undo step.
- [ ] **Commit:** `feat(editor): add transitions from the Effects tab and edit them on the timeline`

### Task 9: e2e and full verification

- [ ] **`e2e/editor-transitions.spec.ts`**, desktop 1440×900 and phone 402×874:
  1. Open the sample project.
  2. Ensure two adjacent clips exist. Split one clip if needed, since the sample media has handles after trimming.
  3. Effects → Transitions → drag or `+` Crossfade onto the cut.
  4. The badge appears.
  5. Properties duration set to 1.0 s updates the badge label.
  6. Undo removes the transition.
- [ ] **Gates:**
  - `rtk pnpm verify:frontend`
  - `rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action --test render_pipeline --test project_nle_export --test codex_conversation -- --test-threads=1`
  - the `ges-render` gated transition test
- [ ] **Native check.** In `rtk pnpm dev` on Linux, add each transition kind to a real project, export MP4, and inspect frames at the transition midpoints. On macOS, confirm the MP4 export used GES by checking `render_report.command.program`. Record both in the commit body.
- [ ] **Commit:** `test(transitions): cover transitions end to end`

## Acceptance

- The four transitions work across all layers: model, validation, maintenance, GES render (video and audio), preview parity, NLE export, agent tools, and UI.
- The canonical timeline never contains overlapping clips.
- GES applies audio fades.
- Spec acceptance flow 7 passes at both viewports.
