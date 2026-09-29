# Editor Redesign 05 — Left Tabs (Media, Audio, Text, Captions, Effects) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Expand each task into bite-sized TDD steps with exact code before executing it.

**Goal:** Fill the Media, Audio, Text, Captions and Effects tabs with working content:
- import and drag-and-drop,
- folders,
- smart visual search,
- matte creation,
- the Generate sub-view for image, video and audio,
- speech cleanup,
- text and title insertion,
- caption generation and transcript fixing,
- caption styles,
- effects and shader backgrounds.

This plan also adds the clip AI properties tab (variations, replace, upscale, video-to-audio), because it shares the generation service.

**Architecture:**
- **Generation service.** A store-bound service, `src/editor/services/generation-service.ts`, owns generation. It reuses the plan 01 lib modules (`generation/*`) and the existing backend wrappers, and follows the pre-cut request lifecycle: record the asset, add a placeholder, run in-process or start Temporal, poll, then place the output.
- **Media service.** `src/editor/services/media-service.ts` owns import, folders, search and matte.
- **Speech service.** `src/editor/services/speech-service.ts` owns transcription, silence, denoise and speakers.
- **Panels.** Panels are presentational and call these services.
- **Handoffs to the AI tab.** Actions that need the AI tab ("Organize with AI", "Ask AI about this clip") write `ui.pendingAgentRequest`. Plan 06 consumes it.

**Tech Stack:** React 19, Zustand store, Radix primitives from plans 02–04, `@tauri-apps/plugin-dialog` through the runtime adapter, lucide-react, Vitest, Playwright.

**Spec sections:** Left Tabs (Media, Audio, Text, Captions, Effects), Properties (AI tab), Behavior Details.
**Depends on:** plans 01–04.

## Global Constraints

- Prefix commands with `rtk`. Use Conventional Commits and stage only the named files.
- Files stay under 600 lines. Use tokens only.
- Tauri APIs, including dialogs and webview drag-drop, are imported only in `src/lib/runtime/adapters/*`. `scripts/runtime-boundary-policy.test.ts` enforces this. Add an adapter plus a `backendClient` capability when one is missing.
- Mock-completion controls render only when the fixture runtime is active: `selectRuntime` mode `fixture`, exposed as `useRuntimeMode()`. They never render in desktop mode.
- Consequential work stays explicit:
  - Generation starts only from the Generate button, with the cost and network notice visible.
  - Silence removal shows a range review before the ripple edit.
  - Caption regrouping asks for confirmation.
- Every tile has a `+` button that calls plan 03's `insertAssetAtPlayhead`. On touch devices (`(hover: none)`) the button is always visible. Tiles are draggable with the plan 03 drag-data MIME type.
- No internal IDs appear in UI text.
- Reference the pre-cut implementations of `media-bin.tsx`, `speech-workbench.tsx`, `captions-workbench.tsx`, `transcript-panel.tsx`, `text-library-panel.tsx`, `effect-catalog-panel.tsx`, `motion-template-library.tsx` and `matte-sheet.tsx` with `rtk git show <pre-cut-sha>:<path>`. Also reference the `editor-workspace.tsx` generation, transcription and silence handlers, found by searching for `runGenerateMediaInProcess`, `queueSelectedMediaTranscription` and `removeDetectedSilence`.

---

## File Map

### Services (`src/editor/services/`)

**`media-service.ts`**
- `importMediaFiles(paths?)`: opens the dialog adapter when `paths` is absent, then calls `importMediaToProject` and updates the project.
- `createFolder`, `renameFolder`, `deleteFolder`, `assignFolder`: project actions.
- `searchMedia(query, scope)`: `searchProjectMedia`, degrading to local `mediaMatchesSearch`.
- `rebuildIndex()`
- `createMatte(input)`: `createMatteInSplitProjectFolder`.

**`generation-service.ts`**
- `loadCatalog()`: `listGenerationModelCatalog` plus `generationModelCatalogFromPayload`.
- `startGeneration(request: MediaGenerationRequest)`, which runs:
  1. `recordGeneratedAsset`
  2. `generatedTimelinePlaceholderAction` when the request targets the timeline
  3. `runGenerateMediaInProcess`, or the Temporal start, depending on the `generationExecutionBackend` preference
  4. poll with `createGeneratedAssetPoller`
  5. `generatedOutputTimelineActions` on completion
- `cancelGeneration(assetId)`, `retryDownload(assetId)`
- `rerun(assetId)`, `createVariations(itemId, count)`, `replaceWithOutput(itemId, outputMediaId)`
- `upscale(mediaId, context)`, `videoToAudio(mediaId, kind, context)`

**`speech-service.ts`**
- `transcribe(mediaId, languageMode)`: `buildTemporalTranscribeMediaStartRequest` plus `startTemporalWorkflow`, with the fallback start request from `lib/jobs/temporal-fallback`.
- `buildCaptions(options)`: `buildCaptionItems` plus a dynamic caption track, in one batch.
- `silenceRanges()`: `timelineSilenceRippleRanges`.
- `removeSilences(ranges)`: `rippleDeleteRanges`.
- `denoise(itemId, amount)`
- `analyzeSpeakers(mediaId)`, `renameSpeaker(id, name)`
- `fixTranscriptWord(input)`: `editTranscriptWords` or `applyCaptionRepair`.

**`use-runtime-mode.ts`**
- Exposes the runtime mode (`desktop | browser | fixture`) from `lib/runtime/bootstrap`.

### Runtime adapters
- `src/lib/runtime/adapters/tauri-dialog.ts`: `openMediaFiles(): Promise<string[] | null>`. Add it only if no adapter covers this yet; check `lib/media-import.ts`.
- `src/lib/runtime/adapters/tauri-drag-drop.ts`: `listenForFileDrops(handler)` over the webview drag-drop event. It returns an unlisten function. A browser implementation handles HTML5 file drops (in fixture mode, paths come from `File.name`).

### Panels

**`src/editor/panels/media/`**
- `media-panel.tsx`
- `media-toolbar.tsx`: filter chips, search with a smart toggle, sort menu, folder breadcrumb, overflow menu.
- `media-grid.tsx`
- `media-tile.tsx`: thumbnail, duration badge, `+`, progress overlay, failed retry mark, context menu (Rename, Move to folder, Reveal on timeline, Delete…).
- `folder-tile.tsx`
- `folder-dialogs.tsx`
- `matte-dialog.tsx`
- `import-drop-zone.tsx`
- `replace-banner.tsx`: consumes `ui.replaceTargetItemId`. It reads "Choose media to replace <label>", with Cancel.

**`src/editor/panels/generate/`**
- `generate-view.tsx`: back arrow, mode chips, prompt, references, model select, option fields, footer with cost, network notice and Generate.
- `reference-slots.tsx`: `@` tag insertion via `trailingReferenceTagQuery`.
- `generation-options.tsx`: duration, aspect, resolution, quality, variations and voice, each shown only when its option list is non-empty.

**`src/editor/panels/audio/`**
- `audio-panel.tsx`
- `cleanup-cards.tsx`
- `silence-review-dialog.tsx`
- `speakers-dialog.tsx`
- `audio-list.tsx`: rows with play-preview through a shared `<audio>` using `backendMediaUrl`.

**`src/editor/panels/text/`**
- `text-panel.tsx`: Add text, text styles grid, Titles & lower thirds grid.
- `template-tile.tsx`: animated preview on hover (desktop) or tap (mobile).

**`src/editor/panels/captions/`**
- `captions-panel.tsx`: segmented "Transcript | Styles" control.
- `generate-captions-card.tsx`
- `transcript-view.tsx`: paragraphs by speaker, word spans, current word highlight, pause chips, low-confidence marks.
- `word-editor.tsx`
- `caption-styles-view.tsx`

**`src/editor/panels/effects/`**
- `effects-panel.tsx`: filter chips "Effects · Backgrounds". Plan 08 adds "Transitions".
- `effect-tile.tsx`
- `background-tile.tsx`

**`src/editor/properties/ai-tab.tsx`**
- Generated details (prompt, model, references, lineage) and a variation set switcher.
- Actions: Create variations, Replace with generated…, Upscale, Generate music/SFX from video.

### Pure modules
- `src/lib/media/media-filters.ts`: filter chip predicates (All/Video/Images/Generated, and All/Voice/Music/SFX/Generated for audio), plus sort comparators.
- `src/lib/captions/transcript-view-model.ts`
  - `transcriptParagraphs(project, mediaId)`: speaker-labelled paragraphs with word spans `{ index, text, startSeconds, endSeconds, confidence?, isPause? }`.
  - `currentWordIndex(paragraphs, playhead)`.
  - Pause chips for gaps ≥ 0.6 s.
  - Low confidence below 0.6, only when the transcript word type has a confidence field. Otherwise no marks. Verify the field in `src/lib/project.ts` `transcripts`.
- `src/lib/captions/caption-track.ts`: `captionBuildActions(project, options, newTrackId)`, which uses `planDropTarget` for a caption track plus `addItems`.
- `src/lib/audio/cleanup-summary.ts`: `silenceSummary(ranges)` returns `{ count, secondsSaved }`.

### Modified files
- `src/editor/shell/left-panel.tsx` and `mobile-layout.tsx`: route tabs to panels.
- `src/editor/store/ui-slice.ts`: `mediaFilter`, `mediaFolderId`, `mediaSearch`, `generateView: { open, mode } | null`, `replaceTargetItemId`, `revealMediaId`, `pendingAgentRequest`.
- `src/editor/properties/properties-panel.tsx`: add the AI tab for visual, audio and generated selections.
- `knip.jsonc`

---

### Task 1: Runtime mode, dialog and drag-drop adapters

- [ ] **Tests:** `useRuntimeMode` returns `fixture` when the marker is enabled in DEV; `tauri-dialog` and `tauri-drag-drop` adapters are covered with mocked `@tauri-apps/*` modules; the browser drop fallback converts HTML5 `DataTransfer` files.
- [ ] **Runtime boundary:** run `rtk node --test scripts/runtime-boundary-policy.test.ts`. It must pass.
- [ ] **Commit:** `feat(runtime): add media dialog and file drop adapters`

### Task 2: Pure media filters, transcript view model, caption track, cleanup summary

- [ ] **Media filter tests:** every chip against fixture media; sort by name, date and duration.
- [ ] **Transcript view model tests:**
  - Paragraphs split on speaker change.
  - Pause chips at a 0.6 s gap.
  - `currentWordIndex` boundaries.
  - Low-confidence marking only when confidence exists.
- [ ] **Caption track tests:** `captionBuildActions` creates a caption track when none exists and reuses an empty compatible one.
- [ ] **Cleanup summary tests:** `silenceSummary` rounds to 0.1 s.
- [ ] **Commit:** `feat(lib): add media filters, transcript view model, and caption track planning`

### Task 3: Media service and Media panel

- [ ] **Media service tests** (with mocked `backendRequest`):
  - Import updates the project and selects the new media.
  - The `BackendUnavailableError` import path shows "Import needs the desktop app" as `lastError`.
  - Folder actions emit the right project actions.
  - Search falls back to local matching when the index is unavailable.
- [ ] **Media panel:**
  - Import (primary) and Generate buttons.
  - Filter chips.
  - Search field with the "Smart search" toggle. Index status lines come from `indexedSearchStatusLabels`, and the rebuild action shows when `indexedSearchNeedsRebuild`.
  - Sort menu, folder breadcrumb, and an overflow menu with New folder, Create matte, Organize with AI and Rebuild search index.
  - Grid of folder and media tiles, and an empty state that is a drop zone.
  - "Organize with AI" sets `pendingAgentRequest = { prompt: organizeMediaPrompt, focusTab: "ai" }`.
  - A click on a tile calls `previewAsset(mediaId)`. Double-click, or `+`, inserts at the playhead.
  - Replace mode: with `replaceTargetItemId` set, a tile click commits `replaceTimelineItemWithGeneratedOutput` for generated media. For other media it commits a `trimItems` + `updateItemProperties`, or a source replacement if the model supports it. If no replace-media action exists (`rtk rg -n "replace" src/lib/project.ts`), implement it as `removeItems` + `addItems` at the same start and duration in one batch.
  - `revealMediaId` scrolls to the tile and flashes it.
- [ ] **Matte dialog:** color swatches, hex input via `normalizedHex`, aspect options, a preview from `mattePreviewSize`, and Create.
- [ ] **Panel tests:**
  - Chips filter the grid.
  - Search input filters.
  - Folder create, rename and delete dialogs (delete names the folder and item count).
  - `+` inserts.
  - Drag start sets the MIME payload.
  - Replace mode commits one batch.
  - The matte dialog validates hex.
  - Organize with AI sets the pending request.
- [ ] **Commit:** `feat(media): add the media panel with folders, search, matte, and replace mode`

### Task 4: Generation service and Generate sub-view

- [ ] **Generation service tests** (mock backend, fake timers):
  - Starting an in-process image generation records the asset, adds no placeholder for library-only requests, and calls `run_generate_media_in_process`.
  - A timeline-targeted video request adds a placeholder in the same batch as the record action.
  - A Temporal preference uses the start request flow.
  - The poller completes and places the output via `generatedOutputTimelineActions`.
  - Cancel calls `cancel_generate_media_in_process`.
  - A failure path records the failed status, and the grid tile shows the retry mark.
- [ ] **`generate-view.tsx`:**
  - Mode chips: Video/Image in Media, Music/SFX/Voice in Audio.
  - Prompt textarea with placeholder and readiness message from `provider-rules`.
  - Reference slots, shown only if `generationModelSupportsReferenceMedia`, with limit messages.
  - First and last frame slots when supported.
  - Model select from the catalog, and option fields from `settings-options`.
  - Footer: `formatGenerationCreditEstimate(selectedGenerationCost(...))`, the text "Uses <provider> · network", and a Generate button disabled with the readiness reason.
  - On submit: build the `MediaGenerationRequest` with `selectedGenerationSettings` and `typedGenerationReferenceMediaRefs`, call `startGeneration`, and close the view.
- [ ] **Generating tiles:** progress overlay while the asset is queued or running; failure mark with Retry (rerun) and a "Retry download" option when `generatedOutputHasProviderSourceUrl`.
- [ ] **Fixture-only mock controls:** when `useRuntimeMode() === "fixture"`, generating tiles show "Complete (fixture)" and "Fail (fixture)". Complete calls `completeMockGeneratedAssetInSplitProjectFolder`, or the fixture handler added in plan 09.
- [ ] **Tests:**
  - Options appear per model.
  - The cost updates when duration changes.
  - Generate is disabled on an empty prompt with the reason.
  - Submit closes the view and shows a progress tile.
  - Mock controls are hidden in desktop mode.
- [ ] **Commit:** `feat(generation): add the generation service and in-panel Generate view`

### Task 5: Audio panel and speech service

- [ ] **Speech service tests:**
  - Transcribe builds the Temporal start request and records a job.
  - `buildCaptions` commits one batch (track plus items).
  - `removeSilences` emits `rippleDeleteRanges` with the reviewed ranges only.
  - Denoise emits the plan 04 builder pair.
  - Rename speaker calls `rename_project_speaker`.
- [ ] **Audio panel:**
  - Import (audio filter) and Generate buttons. Generate opens `generate-view` in audio modes.
  - "Clean up speech" target select: the selected clip, or a source media for the whole project.
  - Cards:
    - **Remove silences:** count and time saved from `silenceSummary`. "Review" opens `silence-review-dialog`, which lists ranges with checkboxes, previews a range on click (seek), and has an "Apply N cuts" button.
    - **Reduce noise:** Apply. Local and reversible.
    - **Detect speakers:** analyze, then count, then "Rename" opens `speakers-dialog`.
  - When a card needs a transcription model and none is installed, it shows "Install a transcription model" and a button calling `onOpenModelSettings`, threaded from `EditorRoot` props into the store `ui.callbacks`.
  - Audio list: filter chips and rows with play/pause preview, name, "duration · kind", a mini waveform and `+`.
- [ ] **Tests:**
  - The silence review applies only checked ranges in one undo step.
  - The missing-model state shows the settings button.
  - Only one row plays at a time.
  - `+` inserts on an audio track, creating one if needed.
- [ ] **Commit:** `feat(audio): add the audio panel with speech cleanup and generation`

### Task 6: Text panel

- [ ] **"Add text":** inserts a default text overlay at the playhead on a text track. Reuse the pre-cut `TextLibraryPanel` add-text item shape. The label is "Text" and the text is "Your text".
- [ ] **Text styles grid:** text treatments from `motionTemplateCatalog`, rendered live with `motion-template-preview` logic (a component port under `panels/text/template-preview.tsx`).
- [ ] **"Titles & lower thirds" grid:** remaining templates grouped by `templateCategories`; hover-animates on desktop, tap-previews on mobile; insert via `createTemplateOverlayItem`.
- [ ] **Tests:**
  - Add text commits one batch and selects the new item (so Properties opens).
  - Template tiles insert the right `templateId`.
  - Drag payload kind is `template`.
- [ ] **Commit:** `feat(text): add the text panel with styles and animated titles`

### Task 7: Captions panel

- [ ] **Generate captions card (no transcript):**
  - Source media select, language (auto plus the list from the transcription model language modes), max words per line (1–8, default 4), and a censor profanity switch.
  - "Generate captions" runs transcription when needed, then builds the captions.
  - When the transcript already exists, it builds the captions directly.
- [ ] **Transcript view:**
  - Paragraphs from `transcriptParagraphs`, with the current word highlighted during playback.
  - Clicking a word seeks to it.
  - Double-clicking a word opens `word-editor`, an inline input. Enter commits `fixTranscriptWord`, which updates the words and the dependent caption text.
  - Low-confidence words get a wavy underline with the tooltip "Low confidence — double-click to fix".
  - Pause chips are shown inline.
  - Footer: caption count, "N words to check", and Regenerate (with confirmation when captions exist).
- [ ] **Styles view:** caption preset grid applied to all captions via `captionStyleActions(scope "all")`.
- [ ] **Layout contract for VC-006:** word spans carry `data-word-index`. The view keeps `selection.transcriptRange` state (unused now), so strike-and-delete can be added later without restructuring.
- [ ] **Tests:**
  - The empty state generates (mock job completion, then the build).
  - Clicking a word seeks.
  - Fixing a word commits one batch and updates the caption text.
  - Regenerate confirms.
  - The styles grid applies to all cues.
- [ ] **Commit:** `feat(captions): add caption generation, transcript fixing, and caption styles`

### Task 8: Effects panel (effects and backgrounds)

- [ ] **Effects:** `listVisualEffectCatalog()` tiles with name and category. Dropping onto a visual clip, or `+` with a visual clip selected, commits `updateItemEffects`, appending an instance via `nextCatalogEffectInstanceId`. Resource-backed effects open the Properties Video tab Effects section after applying. With no visual selection, the `+` tooltip says "Select a video or image clip".
- [ ] **Backgrounds:** `loadShaderBackgroundTemplates({ projectDir })` tiles; insert via `createShaderBackgroundTemplateItem` on a graphics track.
- [ ] **Tests:**
  - Applying an effect to the selected clip.
  - Blocked tooltip without a selection.
  - A background inserts a `hyperframe_scene` item on a created graphics track.
- [ ] **Commit:** `feat(effects): add the effects and backgrounds panel`

### Task 9: Clip AI properties tab

- [ ] **Generated details:** prompt, model label, references with thumbnails, lineage label, and a variation set switcher. "Use this" commits `replaceTimelineItemWithGeneratedOutput`.
- [ ] **Actions**, each routed through `generation-service`:
  - **Create variations:** 1, 2 or 4, via `generatedVariationSetId`.
  - **Replace with generated…:** opens the Generate view with the replacement placement intent.
  - **Upscale:** images and videos only, via `upscaleGenerationRequest`.
  - **Generate music/SFX from video:** video only, via `videoAudioGenerationRequest`.
  - Each action shows its cost estimate and the network notice before starting.
- [ ] **"Ask AI about this clip":** sets `pendingAgentRequest` with the clip context.
- [ ] **Tests:**
  - Details render for the fixture generated asset.
  - Upscale is hidden for audio.
  - The variation switcher commits a replace.
  - Cost is shown before start.
- [ ] **Commit:** `feat(properties): add the clip AI tab for variations, replace, and upscale`

### Task 10: Integration, knip cleanup, e2e

- [ ] **Routing:** route the left tabs (desktop) and sheets (mobile) to the panels, and remove `PanelPlaceholder` usage for these five tabs. AI keeps its placeholder until plan 06.
- [ ] **knip:** remove the consumed entries from the temporary `knip.jsonc` block. Delete `lib/generation-variations.ts` from the block, since it is now used.
- [ ] **`e2e/editor-panels.spec.ts`**, desktop 1440×900. Extend the fixture marker with the plan 09 handlers where required; until they exist, use the sample project, which applies actions locally.
  1. **Media:** filter "Images", then `+` on an image tile. A clip appears on a video track.
  2. **Text:** "Add text". The Properties Text tab opens.
  3. **Captions:** fix a transcript word. Its caption text updates. Undo reverts it.
  4. **Effects:** select a clip, then `+` on an effect. The Properties Effects section lists it.
  5. **Audio:** open the silence review, uncheck one range, apply. The duration shrinks by the checked ranges.
  6. **Generate:** open the Generate view. The cost is shown and Generate is disabled on an empty prompt.
- [ ] **Phone 402×874:** open the Media sheet, `+` a tile, close. The clip is on the timeline.
- [ ] **Visual check:** capture each tab at 1440×900 and 402×874 into `output/editor-panels/` and read the PNGs.
- [ ] **Gate:** `rtk pnpm verify:frontend`.
- [ ] **Commit:** `test(editor): cover media, text, captions, effects, and audio panel flows`

## Acceptance

- Every spec bullet for Media, Audio, Text, Captions and Effects (except Transitions) and the clip AI tab is implemented.
- Consequential and destructive actions follow the review and confirmation rules.
- Mock controls never render outside the fixture runtime.
- Import and drag-drop work in the Tauri app through adapters, and HTML5 drops work in the browser fixture.
