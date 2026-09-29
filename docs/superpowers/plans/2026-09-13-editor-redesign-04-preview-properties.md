# Editor Redesign 04 — Preview and Properties Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Expand each task into bite-sized TDD steps with exact code before executing it.

**Goal:** Build two surfaces.

- **Preview.** Render the real composition. It covers:
  - timeline playback with synchronized media and audio,
  - canonical prepared frames,
  - canvas selection, transform, crop and inline text editing,
  - asset preview mode,
  - transport and fullscreen.
- **Properties panel.** Show contextual tabs for every selection kind except the clip "AI" tab, which plan 05 adds. The panel appears as a docked or overlay panel on desktop and as bottom sheets on mobile.

**Architecture:**
- **Preview model.** `buildTimelinePreviewFrame` (`src/lib/timeline-preview.ts`) is the single frame model. A new DOM compositor renders layers using `previewMotionStyle` and the canvas geometry helpers from `src/lib/preview/canvas-geometry.ts`.
- **Playback clock.** `requestAnimationFrame` advances `playheadSeconds` in the store and drives the media elements. The store holds the playhead; media elements follow it.
- **Canonical frames.** When `projectNeedsCanonicalPreview(project)` is true, the preview calls `prepareProjectPreview` and overlays the returned `frameSequences`.
- **Properties.** Controls keep transient values locally while dragging and commit one project action on release, so each change is one undo step. A pure `selectionKind()` routes the selection to its tab set.

**Tech Stack:** React 19, the Zustand store, `@radix-ui/react-slider`, `@radix-ui/react-toggle-group`, `@radix-ui/react-select`, and `@radix-ui/react-switch`, each added in the commit that first uses it. Also lucide-react and Vitest.

**Spec sections:** Preview, Properties, Mobile Layout.
**Depends on:** plans 01–03.

## Global Constraints

- **Workflow.** Prefix commands with `rtk`, use Conventional Commits, and stage only the named files.
- **Source files.** Keep files under 600 lines. Use tokens only.
- **Accessible names.** Preserve these; the retained scripts need them:
  - region "Preview panel", region "Preview viewport"
  - group "Preview transport", button "Play preview" / "Pause preview", slider "Preview scrubber"
  - `<video>` elements inside "Preview viewport"
- **No timing fields in Properties:** no start, duration, source in or source out anywhere.
- **Undo.** One committed control change is one `applyActions` call. Slider drags commit on pointer up or keyboard commit. Numeric fields commit on Enter or blur.
- **Keyframes.** Every animatable control has a ◇ button. When the playhead has a keyframe for that property, the button is filled and clicking it removes the keyframe (`deleteItemKeyframe`). Otherwise clicking adds one (`upsertItemKeyframe`) at the item-relative playhead time. When keyframes exist, value edits upsert at the playhead instead of changing the static property.
- **Reference.** Consult the pre-cut behavior of `preview-panel.tsx`, `timeline-preview-compositor.tsx`, `source-clip-inspector.tsx`, `caption-inspector.tsx`, `template-inspector.tsx` and `text-overlay-inspector.tsx` with `rtk git show <pre-cut-sha>:<path>`. Find the sha as described in plan 03.

---

## File Map

### Pure modules

| File | Contents |
|---|---|
| `src/lib/preview/selection-kind.ts` | `selectionKind(project, itemIds): "none" \| "visual" \| "audio" \| "text" \| "caption" \| "template" \| "transition" \| "multiple"` and `commonPropertySupport(project, itemIds)` |
| `src/lib/preview/playback-clock.ts` | `advancePlayhead(state, nowMs)`, pure clock math with end-of-timeline stop, and `mediaElementTargetTime(layer, playhead)` |
| `src/lib/properties/visual-properties.ts` | Readers: `visualTransform(item)`, `visualOpacity(item)`, `visualBlendMode(item)`, `visualCrop(item)`, `visualFades(item)`, `colorGrade(item)`, `effects(item)`. Action builders: `transformAction(itemId, transform)`, `opacityAction`, `blendModeAction` (`updateItemProperties`), `cropAction`, `fadesAction` (`updateVisualClipFades`), `colorGradeAction`, `effectsAction` |
| `src/lib/properties/audio-properties.ts` | Readers and builders for `updateAudioVolume`, `updateAudioFades`, and denoise, which combines `updateItemEffects` with `audio.denoise` and `updateItemProperties` with `audioDenoisePreparation`. Mirror the pre-cut `applyAudioDenoise`. |
| `src/lib/properties/text-properties.ts` | Readers for text overlays and builders `editTextItem` and `updateTextOverlayItems` (font, size, color, stroke, background, alignment, animation preset) |
| `src/lib/properties/caption-properties.ts` | Caption group resolution (`captionGroupItems(project, itemId)` by group id). Builders `editCaptionText` and `captionStyleActions(project, itemId, scope: "all" \| "this", properties)` using `captionStyleProperties` for presets |
| `src/lib/properties/template-properties.ts` | Wraps `templateFieldsForItem`, `templateStyleForItem`, `updateTemplateItems` and `updateTemplateOverride` |
| `src/lib/properties/keyframe-actions.ts` | `keyframeStateAtPlayhead(item, property, playhead)` and `toggleKeyframeAction(...)`; a value edit on a keyframed property becomes `upsertItemKeyframe` |
| `src/lib/properties/animation-presets.ts` | Maps `motionPresetCatalog` in/out/loop presets to `setItemKeyframes` actions. Check the pre-cut inspector for how presets were applied and match it. |

### Components

| File | Contents |
|---|---|
| `src/editor/preview/preview-panel.tsx` | Replaces `shell/preview-region.tsx` |
| `src/editor/preview/preview-compositor.tsx` | Visual layers, overlay and text layers, canonical frame overlay, audio layers as hidden `<audio>` |
| `src/editor/preview/media-layer.tsx` | `<video>`/`<img>` per layer with time sync |
| `src/editor/preview/use-playback-clock.ts` | rAF loop, media element sync, stop at end |
| `src/editor/preview/use-canonical-preparation.ts` | Debounced `prepareProjectPreview`; state is idle, preparing, ready or failed |
| `src/editor/preview/canvas-selection.tsx` | Click-to-select via `topmostTimelinePreviewLayerAtPoint`, transform box with scale and rotate handles, crop mode |
| `src/editor/preview/inline-text-editor.tsx` | `contentEditable` overlay; commits `editTextItem` or `editCaptionText` |
| `src/editor/preview/preview-transport.tsx` | Time, previous and next frame, play/pause, scrubber, aspect label, fullscreen |
| `src/editor/preview/asset-preview.tsx` | Asset preview mode with the "Previewing: name · Back to timeline" chip |
| `src/editor/preview/preview-failure.tsx` | In-canvas failure message with Retry |
| `src/editor/properties/properties-panel.tsx` | Replaces `shell/properties-region.tsx`; header with the selection name; routes to tab sets |
| `src/editor/properties/controls/slider-field.tsx` | Slider, numeric field, optional keyframe button, local transient value |
| `src/editor/properties/controls/segmented-field.tsx`, `select-field.tsx`, `switch-field.tsx`, `color-swatches.tsx`, `preset-grid.tsx` | Shared controls |
| `src/editor/properties/controls/keyframe-button.tsx` | ◇ keyframe toggle |
| `src/editor/properties/controls/property-section.tsx` | Section title with a reset button |
| `src/editor/properties/visual-tabs.tsx` | Video · Audio · Speed · Animation |
| `src/editor/properties/audio-tabs.tsx` | Basic · Voice · Speed (Speed only if the backend supports audio speed; see Task 6) |
| `src/editor/properties/text-tabs.tsx` | Text · Style · Position · Animation |
| `src/editor/properties/caption-tabs.tsx` | Text · Style · Position · Animation with the "Applies to" toggle |
| `src/editor/properties/template-tabs.tsx` | Content · Style · Animation · Effects |
| `src/editor/properties/multiple-tabs.tsx` | Common properties with a mixed-value indicator |
| `src/editor/properties/mobile-property-sheets.tsx` | Sheet mapping for the mobile clip tools: Adjust, Volume, Speed, Animation, Effects |

### Modified files

- `src/editor/shell/desktop-layout.tsx` and `mobile-layout.tsx`
- `src/editor/store/playback-slice.ts`: `canonicalPreparation` state, `cropModeItemId`, `editingTextItemId`
- `scripts/preview-playback-performance.mjs` and `scripts/browser-preview-render-qa.mjs`: retarget selectors to the preserved names and verify they run
- `knip.jsonc`

---

### Task 1: Selection kind and property readers/builders (pure)

- [ ] **Tests** in `selection-kind.test.ts` and `visual-properties.test.ts` / `audio-properties.test.ts` / `text-properties.test.ts` / `caption-properties.test.ts` / `template-properties.test.ts` / `keyframe-actions.test.ts` / `animation-presets.test.ts`:
  - Every item kind maps to the right selection kind. Mixed selections map to `multiple`, and an empty selection maps to `none`.
  - `commonPropertySupport` returns opacity only when every selected item is visual.
  - Readers return defaults for missing or invalid property values.
  - Each builder emits the exact action shape from the `ProjectAction` union. Verify field names against `src/lib/project.ts` lines 593–894.
  - Caption style with `all` updates every item in the caption group; `this` updates only the selected cue.
  - Toggling a keyframe on and off at the playhead works. A value edit becomes an upsert when keyframes exist for that property.
  - Denoise builder output matches the pre-cut `applyAudioDenoise` action pair.
- [ ] **Implement and commit:** `feat(properties): add selection kind and property action builders`

### Task 2: Playback clock and preview compositor

- [ ] **`playback-clock.ts` tests:**
  - Advancing by elapsed milliseconds at speed 1.
  - Stopping at the timeline end sets `playing` to false.
  - `mediaElementTargetTime` accounts for `sourceIn` and `timelineItemSpeed`.
- [ ] **`use-playback-clock.ts`:**
  - While `playing` is true, the rAF loop updates `seek(next)`.
  - Each frame, media elements whose `|currentTime - target| > 0.12s` get `currentTime` set.
  - Media elements play or pause following `playing`.
  - Audio layers are hidden `<audio>` elements with volume from `volumeDb` and fades.
- [ ] **`preview-compositor.tsx`:**
  - Computes `buildTimelinePreviewFrame({ timeline, timelines, media, generatedAssets, playheadSeconds })`.
  - Renders layers in order with `previewMotionStyle(layer)`.
  - Media URLs come from `previewUrlForMedia` in `src/lib/media/preview-source.ts`.
- [ ] **`use-canonical-preparation.ts`:**
  - When `projectNeedsCanonicalPreview(project)` is true, debounce 400 ms and call `prepareProjectPreview({ projectDir, project })`.
  - Render the returned frame sequences for covered items as `<img>` sequences selected by playhead.
  - `BackendUnavailableError` means the preview stays DOM-only without an error.
  - Any other error shows `preview-failure.tsx` with Retry.
- [ ] **Tests:**
  - Layers render for the fixture project at t=0 and t=3.
  - A `<video>` exists inside "Preview viewport".
  - The play toggle advances the playhead using a fake rAF.
  - A preparation failure shows Retry, and Retry calls the request again.
- [ ] **Commit:** `feat(preview): render the composition with a playback clock and canonical frames`

### Task 3: Transport, asset preview, fullscreen

- [ ] **`preview-transport.tsx`:**
  - Time label via `formatPreviewCurrentTime`.
  - Previous and next frame buttons step by `1 / renderSettings.fps`.
  - "Play preview" / "Pause preview" button.
  - "Preview scrubber" slider.
  - Aspect label from `renderSettings`.
  - Fullscreen via the `requestFullscreen` API on the preview panel, with the store `fullscreen` flag.
- [ ] **`asset-preview.tsx`:** when `previewSource.kind === "asset"`:
  - Render that media alone with its own transport, using `sourcePreviewStepSeconds`.
  - Show a chip "Previewing: <mediaDisplayName> · Back to timeline".
  - Return to the timeline on Back, on Esc (keymap `editor.clearSelection` handling in the shell hook), or on any timeline pointer interaction via a store subscription in `TimelinePanel`.
- [ ] **Tests:**
  - Transport names are present.
  - Frame step at 24 fps.
  - The scrubber seeks.
  - Asset preview shows the chip, and Back returns to the timeline.
- [ ] **Commit:** `feat(preview): add transport, asset preview mode, and fullscreen`

### Task 4: Canvas selection, transform, crop, inline text

- [ ] **`canvas-selection.tsx`:**
  - **Selection.** A click selects via `topmostTimelinePreviewLayerAtPoint`; a click on empty canvas clears the selection.
  - **Transform box.** The selected visual layer shows a box with corner scale handles and a rotate handle.
  - **Transform commit.** Pointer moves update a local transform via `resizedCanvasTransform` and `normalizeRotationDegrees`. Release commits `updateVisualClipTransform` using `canonicalCanvasTransform`, or `upsertItemKeyframe` when keyframed.
  - **Crop mode.** Entered from the Properties "Edit on canvas" button or by double-clicking a visual layer. Edges drag through `croppedCanvasEdges`. Enter or clicking outside commits `updateVisualClipCrop`; Esc cancels.
- [ ] **`inline-text-editor.tsx`:** double-clicking a text or caption layer opens a `contentEditable` overlay at the layer rect. Enter or blur commits `editTextItem` / `editCaptionText`; Esc cancels.
- [ ] **Tests:**
  - A canvas click selects the topmost layer.
  - A scale drag commits one transform action.
  - The crop commit and cancel paths work.
  - Inline edit commits text, and Esc leaves the project unchanged.
- [ ] **Commit:** `feat(preview): add canvas selection, transform, crop, and inline text editing`

### Task 5: Properties panel shell and shared controls

- [ ] **Primitives.** Add Radix `slider`, `toggle-group`, `select` and `switch` under `src/components/ui/` in the style of plan 02.
- [ ] **Shared controls:**
  - **`slider-field`.** Label, slider and numeric input with formatter and parser. It keeps a transient value, calls `onPreview(value)` while dragging, calls `onCommit(value)` once on release or Enter, and has an optional `keyframe` prop.
  - **Other controls.** `segmented-field`, `select-field`, `switch-field`, `color-swatches`, `preset-grid` (keyboard navigable, `aria-pressed`), and `property-section` (title and reset).
  - **Live preview.** `onPreview` writes a store-local `propertyPreview` override. The compositor applies it without mutating the project, so live feedback costs no undo steps.
- [ ] **`properties-panel.tsx`:**
  - Header shows the selection name (`item.label` or the media display name), or "N items".
  - Tabs come from `selectionKind`.
  - The panel scrolls, and the active tab is remembered per selection kind in the `ui` slice.
- [ ] **Tests:**
  - `slider-field` commits exactly once per drag.
  - The numeric field commits on Enter and rejects invalid input inline.
  - The keyframe button reflects its state.
  - The panel routes each selection kind to its tab names.
- [ ] **Commit:** `feat(properties): add the properties panel and shared controls`

### Task 6: Visual, audio and multiple-selection tabs

- [ ] **Visual tabs:**
  - **Video:**
    - Transform section: scale, position X/Y, rotate, opacity, with keyframe buttons.
    - Blend mode select, using `visualBlendModes`.
    - Crop section with the "Edit on canvas" button.
    - Fade in and out sliders.
    - Look section: presets None/Film/Warm/B&W mapped to `updateItemColorGrade` values, plus a "Color" disclosure with exposure, contrast, saturation and temperature sliders.
    - Effects section listing applied effects with parameter controls from the effect catalog (`listVisualEffectCatalog`) and remove buttons (`updateItemEffects`).
  - **Audio:** shown only when the clip has audio. Volume, fades, denoise.
  - **Speed:** constant speed slider 0.1–8 with presets 0.5×, 1×, 1.5× and 2×, via `updateVisualClipSpeed`. Reverse is omitted unless the Rust model supports it; check with `rtk rg -n "reverse" src-tauri/src/project`.
  - **Animation:** in, out and loop preset grids from `animation-presets.ts`, plus a keyframe summary listing keyframed properties, with "Show in timeline" to toggle the keyframe lane.
- [ ] **Audio tabs:**
  - **Basic:** volume and fades.
  - **Voice:** denoise, "Remove silences…" (opens the review dialog from plan 05; disabled until then with the tooltip "Available in the Audio tab soon"), and speakers (list with rename via `renameProjectSpeaker`).
  - **Speed:** Rust `update_visual_clip_speed` accepts visual clips only. Omit the Speed tab for audio clips and record the spec deviation in the plan 09 docs task.
- [ ] **Multiple tabs:**
  - Common opacity for all visual selections, volume for all audio, and effects intersection.
  - Mixed values show "Mixed" until changed.
  - Applying produces one batch of actions.
- [ ] **Tests:**
  - Each control dispatches the right builder output.
  - Keyframed opacity edits upsert.
  - The audio tab is hidden for silent video.
  - Audio clips have no Speed tab.
  - A multiple-selection opacity change is one batch and one undo step.
- [ ] **Commit:** `feat(properties): add visual, audio, and multiple selection property tabs`

### Task 7: Text, caption and template tabs

- [ ] **Text tabs:**
  - **Text:** content textarea committing `editTextItem`, font select, size, color swatches, stroke, background.
  - **Style:** preset grid from text treatments in `motionTemplateCatalog`.
  - **Position:** alignment segmented control and safe-area snapping toggle.
  - **Animation:** in and out presets.
- [ ] **Caption tabs:**
  - **Text:** the cue text, via `editCaptionText`.
  - **Style:** the "Applies to: All captions / Only this" toggle (default All), preset grid over the three `CaptionStylePreset` values, font, size, highlight swatches, and max words per line.
  - **Position:** lower, center or upper placement.
  - **Animation:** word animation presets from `CaptionWordAnimationPreset`.
  - Style, Position and Animation edits go through `captionStyleActions` using the toggle scope.
  - Max words per line rebuilds the cues with `buildCaptionItems`. This is a destructive regroup, so confirm with a dialog naming the cue count change.
- [ ] **Template tabs:**
  - **Content:** fields from `templateFieldsForItem`, via `updateTemplateItems`.
  - **Style:** style overrides, via `updateTemplateOverride`.
  - **Animation:** motion preset.
  - **Effects.**
- [ ] **Tests:**
  - Caption "All" updates every cue in the group in one batch; "Only this" updates one cue.
  - The regroup confirm dialog blocks without confirmation.
  - Template field edits commit `updateTemplateItems`.
- [ ] **Commit:** `feat(properties): add text, caption, and template property tabs`

### Task 8: Mobile property sheets and layout integration

- [ ] **Layout integration.**
  - Replace `PreviewRegion` and `PropertiesRegion` with `PreviewPanel` and `PropertiesPanel` in both layouts, then delete the placeholder files.
  - On mobile, the clip tools buttons open these sheets:
    - Adjust: the Video tab Transform section.
    - Volume: the Audio tab.
    - Speed: the Speed tab.
    - Animation: the Animation tab.
    - Effects: the Video tab Effects section.
  - These sheets use `BottomSheet` at the compact height.
  - Enable the buttons that plan 03 left disabled.
- [ ] **Retarget scripts.** Update the selectors in `scripts/preview-playback-performance.mjs` and `scripts/browser-preview-render-qa.mjs` if any preserved name changed. Run each script once and record the outcome in the commit body:
  - `rtk pnpm visual:qa:preview-performance`
  - `rtk pnpm visual:qa:preview-render`
- [ ] **knip.** Remove consumed entries from the `knip.jsonc` temporary block.
- [ ] **e2e `e2e/editor-preview-properties.spec.ts`:**
  - Desktop:
    1. Select a clip. Properties appear docked at 1440 px and as an overlay at 1100 px.
    2. Change opacity to 50% with the numeric field. The layer style changes. Undo restores it.
    3. Add a keyframe with ◇. The lane shows a diamond when keyframes are visible.
    4. Double-click a caption on the canvas, edit the text, and press Enter.
    5. Click the Media grid asset (from plan 05; skip until then) or call `previewAsset` via the store test hook.
  - Phone 402×874:
    1. Select a clip.
    2. Open Adjust and change scale.
    3. Close the sheet.
- [ ] **Visual check.** Capture the preview and properties at 1440×900, 1100×800 and 402×874. Read the PNGs.
- [ ] **Gate.** `rtk pnpm verify:frontend` passes.
- [ ] **Commit:** `test(editor): cover preview and properties flows at desktop and phone sizes`

## Acceptance

- The preview plays the timeline with synchronized video and audio, honors transforms, opacity, fades, crop, speed and keyframes, and overlays canonical frames when needed.
- Every Properties tab in the spec exists. The documented deviations are:
  - no audio Speed tab (backend limitation),
  - no Reverse unless supported,
  - no clip AI tab until plan 05.
- Every committed control change is one undo step, and live previews never touch history.
- The preserved accessible names work, and the two preview QA scripts run again.
