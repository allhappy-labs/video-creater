# Editor Redesign 03 — Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Before executing a task, expand it into bite-sized TDD steps with exact code against the store and lib APIs as they exist after plans 01–02. Do not change contracts listed here without updating this plan.

**Goal:** Replace the timeline placeholder with the full CapCut-style timeline at both layouts:

- one-row toolbar
- dynamic tracks with icon-and-name headers
- clips with filmstrip and waveform visuals
- drag, trim and snap
- multi-select and marquee
- keyframe lane under the selected clip
- overview bar
- dead-air shading
- multiple timelines and nested sequences
- full context menu
- timeline keyboard shortcuts
- touch interactions with a centered playhead on mobile

**Architecture:**

- **Pure logic.** Timeline behavior lives in `src/lib/timeline-ops/*` (from plan 01) and in the existing `src/lib/timeline*.ts` modules:
  - `evaluateTimelineMove` / `evaluateTimelineResize`
  - `resolveTimelineSnap`
  - `timelineViewportWindow` / `adaptiveTimelineTicks`
  - `createTimelineFrameCoalescer`
  - `planProjectRippleTrim`
- **Commands.** New pure planners produce `ProjectAction[]`. A thin command layer, `src/editor/timeline/timeline-commands.ts`, reads the store and calls `applyActions`.
- **Rendering.** Components render from store selectors. Viewport windowing keeps long projects fast.
- **Interactions.** Pointer interactions evaluate through the frame coalescer and commit one action on release, which is one undo step.

**Tech Stack:** React 19, Zustand store from plan 02, `@radix-ui/react-context-menu`, lucide-react, Vitest, Playwright.

**Spec sections:** Timeline, Keyboard, Mobile Layout, Accessibility.
**Depends on:** plans 01 and 02.

## Global Constraints

- **Commands:** prefix every shell command with `rtk`. Use Conventional Commits and stage only the named files.
- **File size:** no file in `src/editor/**` may exceed 600 lines.
- **Colors:** tokens only. Clip fills use `bg-clip-video|text|caption|audio|graphics`, and keyframes use `--keyframe`.
- **Keep these accessible names:**
  - region "Timeline canvas"
  - toolbar "Timeline tools"
  - button "Undo" and "Redo"
  - each track exposed as `listbox` named by its display name, for example "Video 1"
  - each clip as `option` named "<label>, <start timecode>, <duration>"
- **Undo granularity:** every committed interaction is exactly one `applyActions` call. Live drag and resize state is local and never written to the store project.
- **No timing fields in Properties.** Timing is edited only here: through drag, the tooltips, and the "Set duration…" dialog.
- **Shortcuts:** use the plan 01 keymap ids (`timeline.*`, `navigation.*`, `playback.toggle`). Do not hard-code key strings in components.
- **knip:** delete each entry in the temporary `knip.jsonc` block as soon as this plan consumes it.
- **Reference implementation:** the pre-cut behavior is available with `rtk git show <pre-cut-sha>:src/components/workspace/timeline-editor.tsx`. The pre-cut sha is the parent of the plan 02 hard-cut commit; find it with `rtk git log --format=%H -1 --grep "remove the legacy editor workspace"`, then append `^`. Use it to confirm edge-case behavior, not as code to copy wholesale.

---

## File Map

### New pure modules (`src/lib/timeline-ops/`)

| File | Contents |
|---|---|
| `track-bands.ts` | `trackBand(kind): "above" \| "main" \| "below"`; `orderedTracksByBand(timeline)`; `trackDisplayNames(timeline): Map<trackId, string>` producing "Video 1", "Text 1", "Captions", "Graphics 1" and "Audio 1" |
| `dynamic-tracks.ts` | `planDropTarget(input): DropTargetPlan` and `emptyTrackRemovals(before, after): ProjectAction \| null` |
| `clip-commands.ts` | Pure planners for split, delete, ripple delete, gap delete, nudge, duplicate, paste, paste insert, set duration, speed, link and unlink, and decompose. Each returns `ProjectAction[]` or a `{ blocked: reason }` result. |
| `clipboard.ts` | `TimelineClipboard` type; `copyItems(timeline, ids)`; `pastePlan(timeline, clipboard, playhead, mode)` |
| `marquee.ts` | `itemsInMarquee(geometry, rect)` |
| `dead-air.ts` | `deadAirRangesForItem(project, item)`, which maps `project.mediaSilenceRanges` through the item's source range and speed |
| `multi-timeline.ts` | Planners for `createTimeline`, `renameTimeline`, `deleteTimeline` and `setActiveTimeline`, plus `nestedSequenceItems(project)` |

### New components (`src/editor/timeline/`)

| File | Role |
|---|---|
| `timeline-panel.tsx` | Replaces `shell/timeline-region.tsx`: toolbar, then tracks area, then overview |
| `timeline-toolbar.tsx` | Selector, tools, edit buttons, snap, keyframes and zoom |
| `timeline-selector.tsx` | Dropdown: switch, new, duplicate, rename (dialog), delete (confirm dialog), nested sequences section |
| `timeline-ruler.tsx` | `adaptiveTimelineTicks`; click and drag to seek |
| `track-header.tsx` | Icon, short name (double-click to rename via `renameTrack` if supported, otherwise read-only), visibility or mute, lock |
| `track-lane.tsx` | Listbox of clips inside the viewport window; drop indicator; dead-air hatch |
| `timeline-clip.tsx` | Clip body, label chip, trim handles, generation progress and failure mark |
| `clip-filmstrip.tsx` | Filmstrip thumbnails via `cacheTimelineFilmstripInSplitProjectFolder`; falls back to a solid fill |
| `clip-waveform.tsx` | SVG waveform from `waveformPeaks` (lib automation) |
| `playhead.tsx` | Desktop playhead plus the mobile fixed-center variant |
| `keyframe-lane.tsx` | Lane property dropdown and draggable diamonds |
| `overview-bar.tsx` | Density bar and draggable viewport window (lib navigation `normalizeWindow` and `interactionWindow`) |
| `timeline-context-menu.tsx` | Radix context menu with the full action set |
| `set-duration-dialog.tsx` | Timecode input with validation |
| `speed-dialog.tsx` | Speed input (visual clips only) |
| `use-timeline-geometry.ts` | Pixels per second from zoom, track row geometry, scroll state, viewport window |
| `use-clip-interactions.ts` | Pointer move and trim for clips and handles, via evaluator, snap and coalescer |
| `use-timeline-drop.ts` | Drag-and-drop of media, templates and backgrounds from panels, via `planDropTarget` |
| `use-timeline-shortcuts.ts` | Timeline-scope keymap handling while the timeline has focus |
| `use-touch-timeline.ts` | Mobile: centered playhead scroll sync, pinch zoom, long-press context menu |
| `timeline-commands.ts` | Store-bound wrappers: `splitAtPlayhead()`, `deleteSelection()`, and so on |
| `drag-data.ts` | MIME type `application/x-video-creater-asset` and a typed payload `{ kind: "media" \| "template" \| "background", id }` |

Each component has a colocated `*.test.tsx`.

### Modified files

- `src/editor/shell/desktop-layout.tsx` and `mobile-layout.tsx` render `TimelinePanel`. Delete `shell/timeline-region.tsx`.
- `src/editor/store/selection-slice.ts` gains `clipboard`, `selectedTrackId` and `marquee`.
- `src/editor/store/timeline-view-slice.ts` gains `tool: "select" | "blade"`, `scrollLeft`, `laneProperty`.
- `src/components/ui/context-menu.tsx` is new: the Radix primitive in the same style as `dropdown-menu.tsx`.
- `knip.jsonc`
- `e2e/editor-shell.spec.ts` becomes `e2e/editor-timeline.spec.ts`, a new spec.

---

## Contracts

```ts
// src/lib/timeline-ops/dynamic-tracks.ts
export type AssetKind = "video" | "image" | "audio" | "text" | "caption" | "template" | "background" | "lottie";
export interface DropTargetInput {
  readonly timeline: Timeline;
  readonly assetKind: AssetKind;
  /** Track under the pointer, or null when between tracks / outside. */
  readonly hoveredTrackId: string | null;
  /** Insert position when between tracks: index in band-ordered tracks. */
  readonly insertIndex: number | null;
  readonly startSeconds: number;
  readonly durationSeconds: number;
  readonly newTrackId: string; // caller-supplied deterministic id
}
export type DropTargetPlan =
  | { readonly kind: "existing"; readonly trackId: string }
  | {
      readonly kind: "create";
      readonly createTrack: Extract<ProjectAction, { type: "createTrack" }>;
      /** createTrack can only insert after a track or append; when the new track must sit above
       * the first stored track, apply this reorder in the same batch right after createTrack. */
      readonly reorderTrack: Extract<ProjectAction, { type: "reorderTrack" }> | null;
      readonly trackId: string;
    }
  | { readonly kind: "invalid"; readonly reason: string };
export function planDropTarget(input: DropTargetInput): DropTargetPlan;
export function emptyTrackRemovals(before: Timeline, after: Timeline): Extract<ProjectAction, { type: "removeTracks" }> | null;
```

**Rules:**

- **Allowed kinds.** A hovered track is used only when `itemAllowedOnTrack(canonicalItemKind, track.kind)` holds and the evaluator reports no collision at the drop time. Otherwise, a track is created in the correct band next to the hovered one.
- **Band order.** Tracks are ordered within their band. Graphics, overlay and caption tracks sit above the main video band. Audio sits below it.
- **Removing empty tracks.** Only tracks that became empty through this edit are removed. A track that was empty before the edit stays, unless it was auto-created and never used. There is no such case, because tracks are only created on drop.
- **Single undo step.** Commands append `emptyTrackRemovals` to the same `applyActions` batch.
- **Precondition.** `removeTracks` has no TypeScript mirror today; it is Rust-only (see the plan 02 research notes). Task 1 adds it to the `ProjectAction` union and to `applyProjectActionLocally`, with a matching test.

```ts
// src/lib/timeline-ops/clip-commands.ts
export type CommandResult = { readonly actions: ProjectAction[] } | { readonly blocked: string };
export function splitAtPlayhead(project: VideoProject, itemIds: readonly string[], playheadSeconds: number, newId: (itemId: string) => string): CommandResult;
export function deleteItems(project: VideoProject, itemIds: readonly string[]): CommandResult;
export function rippleDeleteItems(project: VideoProject, itemIds: readonly string[]): CommandResult; // rippleDeleteRanges on sync-locked tracks
export function deleteGapAt(project: VideoProject, trackId: string, seconds: number): CommandResult;   // uses timelineGapAtSeconds
export function nudgeItems(project: VideoProject, itemIds: readonly string[], frames: number): CommandResult; // fps from renderSettings
export function setItemDuration(project: VideoProject, itemId: string, durationSeconds: number): CommandResult; // resizeItems + evaluator
export function setItemSpeed(project: VideoProject, itemId: string, speed: number): CommandResult; // updateVisualClipSpeed, 0.1–8, visual only
export function linkItems(project: VideoProject, itemIds: readonly string[], linkGroupId: string): CommandResult;
export function unlinkItems(project: VideoProject, itemIds: readonly string[]): CommandResult;
export function decomposeNested(project: VideoProject, itemId: string): CommandResult;
```

Each `blocked` string is user-facing copy, for example "Move the playhead over the selected clip to split." It is shown in the disabled tooltip and in `lastError`.

---

### Task 1: Track bands, names, dynamic track planning (pure)

- [ ] **Tests** in `track-bands.test.ts` and `dynamic-tracks.test.ts`:
  - **Bands and names.** Band assignment for every `TrackKind`. Display names number per kind in band order ("Video 1", "Video 2", "Audio 1"). A single caption track is named "Captions" and a second one "Captions 2".
  - **Drop onto an existing track.** A video drop onto a compatible, free track returns `existing`. A drop that collides returns `create` in the same band, directly above the hovered track.
  - **Drops that create a track.**
    - Text dropped while hovering a video track creates a track in the above band.
    - Audio dropped between two video tracks creates a track at the top of the below band.
  - **Invalid drops.** A drop onto a locked track is invalid, with reason "Track is locked".
  - **Empty-track removal.**
    - Returns `null` when nothing becomes empty.
    - Removes only the tracks that became empty.
    - Never removes the last video track, the main band minimum.
- [ ] **Precondition task.** Add `ProjectActionRemoveTracks { type: "removeTracks"; trackIds: string[] }` to `src/lib/project.ts`, matching the Rust serde shape in `src-tauri/src/project/action.rs` near line 250. Add a `removeTracks` case to `applyProjectActionLocally` and a `project.test.ts` case.
- [ ] **Implement, run the tests, then commit:** `feat(timeline): plan dynamic track targets and empty track removal`

### Task 2: Clip command planners and clipboard (pure)

- [ ] **Tests** in `clip-commands.test.ts` and `clipboard.test.ts`. Build fixtures with `fixtureProject()` plus explicit tracks.
  - **Split.** Split inside, at a boundary (blocked), and across multi-selection on linked items (both split).
  - **Delete and ripple.** Delete removes items and appends empty-track removal. Ripple delete produces `rippleDeleteRanges` covering the selection union on sync-locked tracks. Gap delete finds the gap under the playhead, and is blocked when there is no gap.
  - **Nudge.** Nudge is ±1 frame at 24 and 30 fps. A collision is blocked with the evaluator reason.
  - **Duration and speed.**
    - Duration is clamped by source duration.
    - Speed is rejected for audio clips with "Speed is available for video and image clips".
    - Speed outside 0.1–8 is blocked.
  - **Link.** Link and unlink work on 2+ items.
  - **Decompose.** Decompose applies only to `source.type === "timeline"` items.
  - **Clipboard.** Copy stores relative offsets. Paste at the playhead onto the original tracks creates new ids via an injected `newId`. Paste insert uses `insertItems` and ripples later items.
- [ ] **Implement, run the tests, then commit:** `feat(timeline): add pure clip command planners and clipboard`

### Task 3: Dead air, marquee, multi-timeline planners (pure)

- [ ] **Tests:**
  - `deadAirRangesForItem` maps source silence through a trimmed item at speed 1.5 and clips ranges to the item bounds.
  - `itemsInMarquee` handles partial overlap, zero-size rects and rows off-screen.
  - Multi-timeline planners emit `createTimeline` with `duplicateActive`, `renameTimeline`, `deleteTimeline` and `setActiveTimeline`. Deleting the only timeline is blocked.
  - `nestedSequenceItems` lists items whose source is another timeline.
- [ ] **Implement and commit:** `feat(timeline): add dead-air, marquee, and multi-timeline planners`

### Task 4: Store additions and timeline commands

- [ ] **Store additions.**
  - Selection slice: `clipboard: TimelineClipboard | null`, `selectedTrackId`, `marquee`.
  - Timeline-view slice: `tool`, `scrollLeft`, `laneProperty`, with persistence for `tool` and `laneProperty` only.
- [ ] **Commands.** `timeline-commands.ts` exposes `useTimelineCommands()`. It returns memoized functions that:
  - read the store,
  - call the pure planners,
  - call `applyActions`, or set `lastError` when blocked,
  - update selection afterwards (split selects the right half; paste selects the pasted items).
- [ ] **Tests** with a real store and the backend client mocked unavailable (local application):
  - split, then undo, restores the project;
  - delete removes the emptied track and one undo restores both;
  - a blocked command sets `lastError` and leaves history unchanged.
- [ ] **Commit:** `feat(timeline): bind timeline commands to the editor store`

### Task 5: Geometry, ruler, playhead, overview

- [ ] **`use-timeline-geometry.ts`:**
  - `pixelsPerSecond = basePps * zoomPercent / 100`, with `basePps = 80`.
  - Header column 118 px on desktop, 0 on mobile.
  - Track row heights: main video 58, other 34, keyframe lane 40.
  - `timelineViewportWindow` for visible items, with `overscanViewports = 1`.
  - `fitZoom()` computes the zoom that fits the duration in the viewport.
- [ ] **Components:**
  - `timeline-ruler.tsx` uses `adaptiveTimelineTicks`, labels ticks with `formatTimecode`, and seeks on pointer down and drag.
  - `playhead.tsx` is a white line with a cap and follows `playheadSeconds`.
  - `overview-bar.tsx` is 18 px tall, draws clip density per kind color, and has a draggable window that updates `scrollLeft` and zoom.
- [ ] **Tests:**
  - ruler tick labels at zooms 10/100/1000;
  - a ruler click seeks to the expected seconds;
  - an overview drag updates `scrollLeft`;
  - the playhead position matches `playheadSeconds * pps - scrollLeft`.
- [ ] **Commit:** `feat(timeline): render ruler, playhead, and overview bar`

### Task 6: Tracks, headers and clips (static rendering)

- [ ] **`track-header.tsx`.**
  - Kind icons: `Film` (video), `Type` (text or overlay), `Captions`, `Sparkles` (graphics), `Music` (audio).
  - Name from `trackDisplayNames`.
  - Visual tracks get an Eye/EyeOff toggle (`setTrackEnabled`). Audio tracks get Volume2/VolumeX (`setTrackEnabled`). All tracks get Lock/Unlock (`setTrackLocked`).
  - Reordering by dragging the header within its band uses `reorderTrack`.
- [ ] **`track-lane.tsx`** renders `role="listbox"` with `aria-label` set to the display name, and an `option` per visible clip. Arrow Left/Right moves focus between clips, and Enter selects.
- [ ] **`timeline-clip.tsx`:**
  - kind fill and label chip;
  - `clip-filmstrip` for video and image, `clip-waveform` for audio and for video with audio;
  - a dead-air hatch overlay;
  - selected outline and 7 px trim handles (24 px hit area on touch);
  - generation state: a progress fill while running and a red corner mark with tooltip when failed, derived via `generatedAssetForTimelineItem`;
  - no status badges.
- [ ] **`clip-filmstrip.tsx`:**
  - requests `cacheTimelineFilmstripInSplitProjectFolder` with `zoomBucket` and `heightBucket`, at most one request per clip per bucket;
  - renders frames with `backendMediaUrl`;
  - falls back to a solid fill for sample or browser projects, or when the backend is unavailable.
- [ ] **Tests:**
  - listbox and option names;
  - kind classes;
  - handles only on the selected clip;
  - failure mark tooltip text;
  - the filmstrip falls back when the request throws `BackendUnavailableError`;
  - header toggles dispatch the right actions.
- [ ] **Commit:** `feat(timeline): render dynamic tracks, headers, and clips`

### Task 7: Selection, marquee, drag, trim, snap

- [ ] **`use-clip-interactions.ts`.**
  - **Pointer down on a clip:** select the clip. Shift or Mod toggles it in the selection.
  - **Drag:**
    - Each pointer move goes to `createTimelineFrameCoalescer`. `evaluate` calls `evaluateTimelineMove` with snap targets from `timelineSnapTargets(editPoints, playhead)` and `resolveTimelineSnap`. `deliver` updates local ghost state and a tooltip showing start · duration.
    - On release, an `accepted` or `clamped` result commits `moveItems`, plus a created track and `emptyTrackRemovals` when dragging between bands (via `planDropTarget`), as one batch.
    - A `rejected` result shows a red outline at the target, with the reason in the tooltip, and commits nothing.
  - **Trim handles:** `evaluateTimelineResize` commits `trimItems`. With the ripple modifier (Alt), it commits `rippleTrimItem` using `planProjectRippleTrim` for the preview.
- [ ] **Marquee.** Pointer down on empty lane space starts a marquee, which selects via `itemsInMarquee`. A click on empty space clears the selection.
- [ ] **Blade tool.** In blade mode, a click on a clip splits it at the pointer time.
- [ ] **Tests** with jsdom pointer events. Stub `requestFrame` to run synchronously.
  - drag commits one `moveItems`;
  - a snap to the playhead within 8 px;
  - a rejected collision commits nothing and exposes the reason;
  - trimming the left edge commits `trimItems` with an updated `sourceIn`;
  - the marquee selects two clips;
  - the blade splits.
- [ ] **Commit:** `feat(timeline): add clip selection, marquee, drag, trim, and snapping`

### Task 8: Keyframe lane

- [ ] **`keyframe-lane.tsx`.** Visible when `keyframesVisible` is on and exactly one clip is selected. It renders under that clip's track.
- [ ] **Lane property dropdown.** Built from `keyframePropertyConfigsForItem`.
- [ ] **Diamonds.**
  - Positioned with `automationPointPosition`.
  - Dragging a diamond commits `moveItemKeyframe`, and its value when dragged vertically.
  - Double-clicking the lane adds a keyframe with `upsertItemKeyframe` at the pointer time, using `suggestedValue`.
  - Delete or Backspace on a focused diamond commits `deleteItemKeyframe`.
- [ ] **Tests:**
  - the lane is hidden with 0 or 2 selected clips;
  - the dropdown lists the opacity and volume configs;
  - dragging commits `moveItemKeyframe` with the new `atSeconds`;
  - keyboard delete works.
- [ ] **Commit:** `feat(timeline): add the selected clip keyframe lane`

### Task 9: Toolbar, timeline selector, dialogs, context menu

- [ ] **`timeline-toolbar.tsx`**, left to right:
  - `TimelineSelector`
  - separator
  - Select/Blade segmented control
  - separator
  - Split, Delete, Ripple delete, Link/Unlink, Nudge left, Nudge right
  - spacer
  - Snap toggle, Keyframes toggle
  - separator
  - Zoom out, zoom slider (a Radix Slider primitive added now as `src/components/ui/slider.tsx`), Zoom in, Fit

  **Tooltips:** each tooltip is `"<label> (<formatShortcut>)"`. When a control is disabled, the tooltip shows the planner's `blocked` reason instead.
- [ ] **`timeline-selector.tsx`.**
  - Menu items: timelines list (checked = active), "New timeline", "Duplicate timeline", "Rename…" (dialog), "Delete…" (confirm dialog naming the timeline), and a "Nested sequences" section listing `nestedSequenceItems` (selecting one selects the item).
- [ ] **`timeline-context-menu.tsx`** has a clip menu and a lane menu.
  - **Clip menu:**
    - Cut, Copy, Paste, Paste insert, Duplicate
    - Delete, Ripple delete
    - Split at playhead, Set duration…, Speed…
    - Link or Unlink, Decompose nested sequence (nested only)
    - Replace with media… (opens Media tab in replace mode: sets `ui.replaceTargetItemId`, consumed by plan 05)
    - Reveal in Media (`setActiveTab("media")` plus `ui.revealMediaId`)
    - Save range as media (disabled with "Available after export setup" until plan 07 wires it; add `ui` flag)
    - Ask AI about this clip (sets `ui.pendingAgentContext`, consumed by plan 06)
    - Lock track, Hide/Mute track
  - **Lane menu:** Paste, Delete gap, Add track above or below.
  - **Omitted entries:**
    - "Detach audio": `ProjectAction` has no detach action. Verify with `rtk rg -n "detach" src/lib/project.ts src-tauri/src/project/action.rs`. If it is absent, leave the item out and add a backlog note in plan 09's docs task.
    - "Add transition": plan 08 adds it.
- [ ] **`set-duration-dialog.tsx` and `speed-dialog.tsx`.** Timecode or number inputs validated through the planners, with errors shown inline under the input.
- [ ] **Tests:**
  - every toolbar button dispatches its command;
  - disabled tooltips show the blocked reason;
  - the selector creates, renames and deletes, and delete confirms;
  - the context menu lists the full set for a video clip and hides decompose for non-nested clips;
  - Replace, Reveal and Ask AI set their `ui` fields.
- [ ] **Commit:** `feat(timeline): add the timeline toolbar, selector, dialogs, and context menu`

### Task 10: Drop from panels and timeline shortcuts

- [ ] **`drag-data.ts`** defines the MIME type and payload codec. `use-timeline-drop.ts`:
  - computes the hovered track or insert index from the pointer;
  - shows a drop indicator line;
  - on drop, builds the item action via `mediaTimelineAction` (media), `createTemplateOverlayItem` (templates) or `createShaderBackgroundTemplateItem` (backgrounds), placed with `planDropTarget`, and commits one batch.
- [ ] **Add-at-playhead command.** Export `insertAssetAtPlayhead(payload)` for the `+` buttons. Plan 05 uses it.
- [ ] **`use-timeline-shortcuts.ts`.** Active while the focus is inside the "Timeline canvas" region. Maps the keymap timeline scope to commands:
  - V/C switch tools;
  - S or Mod+K split;
  - I/O set range marks (`selection.rangeIn/rangeOut`);
  - Delete/Backspace delete, and Shift+Delete ripple delete;
  - Mod+D/C/X/V and Shift+Mod+V run duplicate, copy, cut, paste and paste insert;
  - `[`/`]` trim to the playhead;
  - arrows nudge;
  - Home/End and PageUp/PageDown navigate;
  - Space toggles playback.
- [ ] **Tests:**
  - a drop creates a track and an item in one undo step;
  - an invalid drop onto a locked track shows the reason and commits nothing;
  - each shortcut id triggers its command;
  - shortcuts are ignored in inputs.
- [ ] **Commit:** `feat(timeline): support asset drops and timeline keyboard shortcuts`

### Task 11: Mobile timeline

- [ ] **`use-touch-timeline.ts`:**
  - **Centered playhead.** The playhead is fixed at 50% width, and scrolling sets `playheadSeconds = (scrollLeft + width/2) / pps`. Playback scrolls the content.
  - **Pinch zoom.** Pinch with two pointers changes `zoomPercent` around the center.
  - **Long press.** A 500 ms long press opens the context menu at the touch point.
  - **Hit areas.** Trim handle hit areas are at least 24 px.
- [ ] **Layout.** `TimelinePanel` in mobile mode hides headers and the toolbar. The mobile clip tools bar covers them.
  - Add a `+` button at the end of the main track that opens the Media sheet.
  - The mobile clip tools bar (plan 02 "Clip tools" toolbar) gets Split, Speed, Volume, Delete and Adjust buttons calling timeline commands. Volume and Adjust open property sheets from plan 04; until then they are disabled with the tooltip "Available soon".
- [ ] **Tests:**
  - scroll updates the playhead;
  - pinch changes zoom;
  - long press opens the menu;
  - the Split clip tool splits.
- [ ] **Commit:** `feat(timeline): add the touch timeline with a centered playhead`

### Task 12: Integration, knip cleanup, e2e

- [ ] **Swap in the panel.** Replace `TimelineRegion` with `TimelinePanel` in both layouts and delete `timeline-region.tsx`.
- [ ] **knip.** Remove the entries this plan consumed from the temporary `knip.jsonc` block. Run `rtk pnpm check:unused`.
- [ ] **`e2e/editor-timeline.spec.ts`**, desktop 1440×900:
  1. Open the sample project.
  2. Select a clip by click; its option has `aria-selected="true"`.
  3. Press S with the playhead inside the clip; the clip count increases by 1.
  4. Press Control+Z (or Meta+Z on macOS); the count is restored.
  5. Drag a clip 100 px right; its start timecode changes.
  6. Open the context menu, choose "Set duration…", enter 2 seconds, and confirm.
  7. The keyboard-only path works: Tab into the timeline, Arrow to a clip, Enter, then S.
- [ ] **Phone 402×874 run:** a tap selects, the Clip tools bar appears, Split works, and scrolling moves the playhead.
- [ ] **Visual check.** Capture 1440×900, 1024×768 and 402×874 into `output/editor-timeline/` and read the PNGs. Check for label truncation only on short clips, visible handles, no overflow, and readable header names.
- [ ] **Gate.** `rtk pnpm verify:frontend` passes.
- [ ] **Commit:** `test(timeline): cover timeline editing flows at desktop and phone sizes`

## Acceptance

- Every spec Timeline bullet is implemented, except "Add transition" (plan 08) and "Save range as media" (plan 07). Both are visible but disabled, with their stated reasons.
- Each interaction produces exactly one undo step, and empty tracks are removed in that same step.
- Long projects (fixture with 500 items) scroll and zoom without rendering off-window clips. The test asserts rendered option count ≤ visible plus overscan.
- There are no timing inputs outside the Set duration dialog.
