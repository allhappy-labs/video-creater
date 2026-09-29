# Editor Source Tabs Remediation Design

## Status

Draft for user review on 2026-07-21.

This specification turns the fresh Media, Generate, Templates, Text, Captions, Transcript, Audio, and Effects audit into one implementation contract. It is a focused correction to `2026-07-14-modern-editor-information-architecture-design.md`; that document remains authoritative where this specification does not explicitly change it.

## Evidence Base

The audit ran against `main` at `19b59f34` with the live Vite application and real Playwright pointer, keyboard, and click interaction. Captures are in `output/playwright/media-tabs-audit-2026-07-21/`, including desktop, constrained, compact-desktop, and narrow editor states.

The current focused component baseline is healthy: 266 tests pass across the navigation rail, source panel, Media, Templates, Text, Captions, Transcript, Speech, Effects, responsive rail state, and editor information architecture. Those tests prove existing behavior but do not cover the geometry, overflow, duplication, or recovery problems found visually.

### Confirmed findings

1. Selecting any non-Media source destination changes the workspace rows through `sourceToolExpanded`, shrinking the timeline to roughly one visible track.
2. Generate uses viewport breakpoints inside a 260–400 pixel context panel. At the audited 300 pixel pane, a 258 pixel footer region had a 341 pixel scroll width.
3. Captions wraps an already `aspect-video` preview in another `aspect-video` container, producing an oversized blank-looking preview and pushing primary actions away.
4. Templates forces two columns in a narrow context panel, inserts button-triggered templates at template default time zero rather than the playhead, and presents category and placement labels as if they were the same taxonomy.
5. Text lists recent treatments as passive rows, so it duplicates Templates without supporting the text-creation job.
6. Transcript renders every word as a large repair card with its own Apply button, obscuring transcript reading and making multi-word correction slow.
7. Audio > Music embeds the complete Generate Audio composer, duplicating the canonical Generate destination.
8. Effects explains that a visual clip must be selected but offers no control to return focus to an eligible timeline target.
9. The left rail mixes global navigation and editor sources without visual groups, repeats desktop Home/Codex/Media affordances found in the header, uses low-contrast 9–10 pixel status copy, and relies on browser-native titles rather than consistent hover/focus help.
10. The desktop header wraps at constrained desktop widths and consumes editing height already needed by the timeline.
11. Speech information icons are decorative and reveal no explanation.
12. Browser visual QA emits Settings-acceptance console errors from an incomplete Tauri bridge, weakening screenshot evidence even when the editor itself works.

Media folder navigation, source selection, source-viewer opening, inspector routing, source-panel keyboard roving, and narrow single-pane switching worked correctly and are regression-protected rather than redesigned.

## Decision

Use incremental editor-shell hardening with one stable geometry model and panel-local responsive layout.

- The active source destination must never choose workspace row geometry.
- A persisted timeline-deck ratio controls upper/lower workspace allocation on multi-pane desktop layouts.
- Source panels adapt to their own inline size rather than the browser viewport.
- Generate remains the only complete generation composer.
- Repeated source-panel UI becomes direct and task-oriented without changing canonical project schemas or Rust mutation ownership.
- Existing working Media, inspector, viewer, timeline, and compact navigation contracts remain intact.

This approach fixes the structural causes while keeping changes localized to the existing React workspace and project-action boundaries.

## Alternatives Considered

### 1. Patch each source panel independently

This would remove obvious overflow and density problems quickly, but tab selection would still mutate workspace rows and future panels could reintroduce viewport-driven overflow. Rejected because it leaves the root geometry defect intact.

### 2. Replace the rail and panels with a new drawer system

A new shell could rationalize every destination at once, but it would reopen working selection, inspector, Codex, responsive, and timeline behavior. Rejected as disproportionate to the audited failures.

### 3. Stable shell plus incremental panel remediation — selected

This preserves the current information architecture, fixes the layout dependency once, and lets every visible problem receive a focused behavioral test and screenshot. It offers the best correctness-to-risk ratio.

## Goals

- Keep the timeline usable and geometrically stable while switching among all eight source destinations.
- Eliminate source-panel horizontal overflow at supported and stress-test pane widths.
- Make every source destination perform one clear primary job without duplicating another destination.
- Make insertion actions honor the current playhead unless the user explicitly drops at another time.
- Keep primary actions visible or sticky in long source-panel workflows.
- Preserve canonical project actions, undo/redo, Rust validation, saved layout state, and compact single-pane navigation.
- Produce repeatable, console-clean screenshot and interaction evidence at desktop, constrained desktop, compact desktop, and narrow sizes.

## Non-Goals

- No project, timeline, media, transcript, template, generated-asset, or Rust schema migration.
- No rewrite of Media search, folder management, generation providers, speech analysis, caption building, effect execution, timeline collision behavior, or inspector controls.
- No new generation provider or template asset.
- No mobile-first editor redesign.
- No removal of a currently working capability.

## Workspace Geometry

### Stable row ownership

Remove the `sourceToolExpanded`/`workspacePresetRows` branch that derives grid rows from `sourceDestination` or `generationComposerOpen`. `workspacePresetGridTemplate` is the sole owner of multi-pane rows.

Add `timelineRatio` to `WorkspaceLayoutState` and a `resizeTimeline` reducer action. The ratio represents the lower timeline deck's share of available multi-pane height and is:

- `0.45` by default;
- clamped to `0.30–0.70` after accounting for the existing preview and timeline minimum heights;
- persisted in `video-creater.workspace-layout.v1` with backward-compatible parsing;
- preserved when changing presets and source destinations;
- ignored in single-pane mode, where the existing compact-view routing remains authoritative.

`workspacePresetGridTemplate` converts the ratio to proportional `fr` weights while retaining the existing grid areas and minimum heights. Default, Media, and Vertical presets continue to change areas, not source-tab semantics.

### Timeline resize control

The multi-pane timeline top edge gains a real horizontal separator named `Resize timeline deck`.

- Pointer dragging updates the ratio live and commits the persisted state on release.
- Arrow Up/Down changes the lower deck by 16 pixels; Shift+Arrow changes it by 64 pixels.
- Home and End move to the allowed maximum and minimum timeline sizes.
- The separator exposes `aria-orientation="horizontal"`, `aria-valuemin`, `aria-valuemax`, and `aria-valuenow`.
- Maximize and single-pane modes hide the separator.

Switching tabs must not move the separator or change the timeline height.

## Panel-Local Responsive Contract

The source library aside becomes an inline-size query container. Source-panel layout responds to the panel, not `sm`, `md`, `lg`, or `xl` viewport breakpoints.

The contract has three useful ranges:

| Panel inline size | Behavior |
| --- | --- |
| Under 280 px | One-column cards and controls; short labels; wrapping metadata |
| 280–359 px | One-column task forms; two-column compact option pairs where safe |
| 360 px and wider | Two-column template cards and denser generation tuning controls |

Every direct child must use `min-width: 0`; chips and metadata wrap or truncate intentionally. No panel may create page-level horizontal scrolling.

Generate's header and submit footer use sticky positioning relative to the source scroll region at every desktop viewport size. The footer control grid follows the source-container ranges above; it does not use `sm:grid-cols-4` or `xl:sticky`. The queue button remains fully visible, and readiness copy wraps within the pane.

Templates uses one column below 360 pixels and two columns at or above 360 pixels. Card buttons span the available width and thumbnails retain useful proportions.

## Destination Designs

### Media

Keep the current working browser, folder, selected-source, viewer, and inspector flows. Add only regression assertions that Media does not alter the timeline ratio and does not overflow at the tested source widths.

### Generate

Generate remains the sole complete image/video/audio composer and history surface.

- Image, Video, and Audio modes remain in place.
- Header credit/history/close controls wrap without hiding Close.
- Model-specific fields remain available and keyboard reachable.
- The tuning/footer layout is source-container responsive.
- Queue readiness and the queue action remain visible at the bottom of the source panel.
- Closing Generate returns to the project library without mutating workspace geometry.

### Templates

All button-based insertions use `timelinePlayheadSeconds` for motion templates and shader backgrounds. Timeline drag/drop keeps its explicit drop time and therefore overrides the playhead.

The same playhead rule applies to the source panel, timeline template drawer, and template suggestions. A template-defined `defaultStartSeconds` is used only by non-editor import/migration code, not by an explicit editor Insert action.

Template metadata uses two labeled concepts:

- `Style: Captions`, `Style: Transition`, or the normalized template category;
- `Places on: Overlay`, `Places on: Captions`, or `Places on: HyperFrames`.

This makes unusual but valid combinations explicit instead of contradictory. Insert buttons include the time in their accessible description, for example `Insert Kinetic Lower Third at 00:01.325`.

### Text

Text owns ordinary editable copy and reusable text treatments.

- `Add Text` inserts at the playhead as it does today.
- Every recent treatment is a button with a small treatment preview, category metadata, and `Add at playhead` action.
- Activating a treatment uses the same canonical template insertion path and selects the inserted item.
- `Inspect Selected Text` remains selection-dependent and routes to the existing inspector.
- Text does not repeat search/filter controls from Templates.

### Captions

`CaptionsWorkbench` owns the single preview aspect-ratio frame. Callers pass preview content only and must not add another aspect-ratio wrapper.

The Placement disclosure keeps a compact preview with a visible sample or explicit empty-state copy. The bottom Build Captions/Agent Mode action row remains sticky within the source panel, visible after scrolling, and does not cover disclosure content.

### Transcript

Replace per-word repair cards with a dense transcript editor:

- one compact row per word;
- a time button that seeks the shared playhead to the word start;
- inline text, start, and end fields;
- repaired and modified indicators that do not increase row height;
- a sticky `Apply N changes` action for all valid changed rows;
- a `Discard changes` action that restores canonical transcript values.

The panel keeps local drafts keyed by word index. Applying creates one existing `editTranscriptWords` project action whose `edits` array contains every changed word. Invalid time ranges are identified inline and block the batch without discarding valid drafts. Successful application clears modified state and preserves the existing repair history. No direct project mutation is introduced.

### Audio

Audio retains Library, Speech, and Music tabs.

- Library shows project audio assets.
- Speech keeps speakers and silence analysis.
- Music shows only project audio assets categorized as music, plus an empty state when none exist.
- A primary `Generate music` shortcut changes the active source destination to Generate, opens Audio mode, and focuses the generation prompt.
- Music never embeds the full generation composer.

Speech section info icons become focusable help buttons with hover/focus tooltips explaining what Mark Speakers and Mark Silence do, what project data they affect, and that no timeline edit occurs until an explicit action is chosen.

### Effects

When there is no eligible visual target, Effects shows a `Choose a visual clip` action. Activating it:

- switches compact mode to Timeline when necessary;
- focuses the timeline editor on desktop and compact layouts;
- announces `Select a video, overlay, or HyperFrame clip to apply an effect` through the existing live-region pattern.

It does not silently select or mutate an arbitrary clip. Once an eligible clip is selected, the existing target label, Apply, Configure in inspector, preparation, retry, and Clear behavior remains unchanged.

## Navigation Rail and Header

### Ownership and grouping

The desktop rail is the primary owner of Home, Codex, source destinations, Activity, and Settings. The desktop header removes duplicate Home, Codex, and Media controls. Those controls remain available in the compact header only when the full rail is not presented.

The rail has three visually separated groups while preserving one roving keyboard sequence:

1. Global: Home and Codex.
2. Sources: Media through Effects.
3. Project/system: Activity and Settings.

The source group remains in the established order. Arrow keys, Home, End, `aria-pressed`, and focus retention continue to work across separators.

### Legibility and help

- Rail labels use at least 11 pixel type and stronger inactive contrast.
- Running/failed Activity state uses a concise badge/dot and an accessible full label rather than a second 9 pixel text line.
- Every rail control has a consistent hover/focus tooltip; `title` may remain as a fallback but is not the primary help surface.
- Active state keeps the cyan indicator and must pass contrast checks against the dark rail.

### Constrained desktop header

At 1024 pixels and wider, the project header stays one row. Project identity and save state truncate first. Secondary layout controls move into the existing overflow/menu pattern before Export or render status is hidden. At narrower single-pane widths, the compact header may use its existing alternate navigation, but it must not show both compact and full-rail copies of the same destination.

## Visual-QA Runtime Integrity

Browser visual QA must explicitly identify itself before React bootstrap. The Settings acceptance runner must no-op for that browser-only visual-QA session without weakening packaged acceptance behavior.

The visual-QA Tauri bridge must either implement every listener cleanup function it advertises or omit that API so application code follows the browser fallback. Test-only suppression of arbitrary console errors is forbidden.

The browser harness fails a scenario on uncaught page errors or `console.error`, with a narrow allowlist only for scenarios whose purpose is to display a known failure. The source-tab scenarios have no allowlisted console errors.

## State and Data Flow

```text
rail selection ──> sourceDestination ──> one context panel
                                      (no row-geometry branch)

workspace separator ──> resizeTimeline ──> timelineRatio ──> persisted grid rows

Insert/Apply action ──> existing project action ──> Rust validation/persistence
                    └─> local browser fallback used by current tests

Audio > Generate music ──> Generate destination + Audio mode + prompt focus
```

Source-panel drafts such as transcript edits remain component-local until an explicit Apply. Timeline, template, caption, effect, and text changes continue through the existing canonical action APIs so undo/redo and native validation remain authoritative.

## Error Handling

- Invalid persisted `timelineRatio` values fall back to `0.45`; finite out-of-range values clamp.
- A template insertion with no compatible track keeps the current no-mutation behavior and adds an actionable inline error in the source panel.
- Transcript batch validation identifies every invalid row and does not partially apply the batch.
- Generation configuration and provider failures remain at the Generate surface with the existing Settings recovery action.
- Effect preview-preparation failures retain Retry; missing-target recovery is separate from preparation recovery.
- Visual-QA bootstrap errors fail the harness instead of being logged and ignored.

## Accessibility and Keyboard Contract

- All new buttons have visible focus states and accessible names that describe their destination or mutation.
- Tooltips appear on both hover and keyboard focus and are referenced with `aria-describedby` while visible.
- Sticky action bars remain in DOM order after their editable content.
- The timeline separator supports pointer and keyboard control and exposes current value semantics.
- Transcript time buttons seek without stealing subsequent editing focus; batch status is announced politely.
- Effects target recovery moves focus to the timeline and announces the required selection type.
- Rail grouping does not break its roving tab stop or wraparound arrow behavior.

## Implementation Boundaries

Expected primary files:

- `src/lib/workspace-layout-state.ts` and its tests;
- `src/components/workspace/editor-workspace.tsx` and focused workspace tests;
- `src/components/workspace/media-bin.tsx` and Media tests;
- `src/components/workspace/motion-template-library.tsx`;
- `src/components/workspace/text-library-panel.tsx`;
- `src/components/workspace/captions-workbench.tsx`;
- `src/components/workspace/transcript-panel.tsx`;
- `src/components/workspace/effect-catalog-panel.tsx`;
- `src/components/workspace/editor-navigation-rail.tsx`;
- `src/components/workspace/speech-workbench.tsx`;
- source-panel styles in `src/index.css` or a colocated stylesheet;
- `src/main.tsx`, the visual-QA fixture bridge, and `scripts/browser-visual-qa.mjs`;
- related focused tests and visual-QA scenario definitions.

Component extraction is allowed when it removes duplicated generation or tooltip logic, but broad shell or state-management rewrites are outside scope.

## Acceptance Criteria

### Structural layout

1. On a 1440×900 editor, switch Media → Generate → Templates → Text → Captions → Transcript → Audio → Effects. The workspace grid row value, timeline separator position, and timeline height remain within one CSS pixel throughout.
2. A saved timeline ratio restores after remount and remains unchanged by preset and source-destination changes.
3. The timeline separator passes pointer and keyboard tests, including clamping and unrelated-pointer rejection.
4. At 1440×900 and 1100×800, the default timeline deck occupies at least 35% of usable workspace height and shows at least three complete track rows without changing source tabs.
5. At 1024×720, the header remains one row and the timeline remains directly usable. At 900×900, existing single-pane source/timeline switching remains functional.

### Panel sizing and actions

6. For every source destination rendered at 240, 260, 304, and 400 pixel container widths, `scrollWidth <= clientWidth + 1` for the panel root and action footer.
7. Generate Close and Queue remain visible and keyboard reachable in every tested mode and width.
8. Captions renders exactly one aspect-ratio preview frame and keeps its primary action row visible while the body scrolls.
9. Template and shader-background Insert actions place the new item at the current playhead; timeline drops still use the drop time.
10. Template metadata visibly distinguishes style/category from destination track.
11. Every Text treatment can be inserted at the playhead and selects the created item.
12. Transcript supports seek, multi-row draft editing, one canonical batch apply, discard, invalid-row blocking, and repair indicators.
13. Audio > Music contains a filtered library and `Generate music`; it does not contain the generation composer. The shortcut opens Generate in Audio mode and focuses Prompt.
14. Effects missing-target recovery focuses Timeline and announces which clips are eligible without changing project data.
15. Speech help appears on hover and focus with meaningful explanatory copy.

### Navigation and runtime evidence

16. Desktop shows one primary Home, one primary Codex, and one primary Media navigation control; compact mode shows one compact equivalent and not the full duplicate.
17. Rail grouping, arrow navigation, Home/End, active state, Activity status, tooltips, and contrast have focused tests.
18. The source-tab browser run reports zero uncaught errors and zero unexpected `console.error` messages.
19. Fresh screenshots are captured for all eight destinations at 1440×900, every Generate mode, every Audio subtab, Generate at 1024×720, default at 1100×800, and single-pane navigation at 900×900.
20. Visual review confirms no clipped controls, nested blank preview, header wrap, opaque caption slab, unintended timeline collapse, or source-panel horizontal scroll.

## Verification Plan

Implementation is complete only after all of the following are fresh and green:

1. Focused unit/component tests for every changed component and state reducer.
2. Editor workspace interaction tests for tab-stable geometry, playhead insertion, canonical routing, focus recovery, and compact behavior.
3. Browser interaction assertions for all eight live destinations, not fixture-only screenshots.
4. Automated panel-local overflow measurement at the four specified widths.
5. `pnpm test` for the complete frontend suite.
6. `pnpm build` for TypeScript and Vite production output.
7. Existing source-quality policy checks relevant to changed files.
8. Fresh browser visual-QA captures and manual image inspection at all acceptance viewports.
9. A final `git diff --check` and clean review of changes against this specification.

Browser evidence proves the web editor behavior. It does not by itself prove packaged Tauri or physical macOS behavior; any packaged verification performed later must be reported separately.

## Delivery Sequence

1. Lock the current failures in tests: row instability, overflow, nested preview, wrong insert time, duplicate Music composer, passive recovery, and visual-QA console errors.
2. Implement persisted stable workspace rows and the timeline separator.
3. Add source-container responsive rules and repair Generate/Templates/Captions geometry.
4. Implement Templates/Text/Transcript/Audio/Effects behavior changes.
5. Clean up rail/header ownership, grouping, help, and constrained-width behavior.
6. Repair visual-QA bootstrap integrity and enforce console-clean runs.
7. Run the full acceptance and screenshot matrix; correct regressions before completion.

## Verified Result

The 2026-07-21 implementation satisfies this specification in browser-backed testing:

| Risk reviewed | Result | Evidence |
| --- | --- | --- |
| Source-panel clipping or horizontal scroll | Pass | All eight destinations were measured at 240, 260, 304, and 400 pixel container widths. |
| Blank or nested preview surfaces | Pass | Captions exposes one preview frame; every live source destination completed the screenshot run. |
| Header wrapping and duplicate navigation | Pass | Desktop, 1100x800, 1024x720, and 900x900 scenarios passed layout and ownership assertions. |
| Timeline collapse or tab-dependent movement | Pass | The separator position and timeline row remain stable while switching all eight destinations; pointer and keyboard resize tests pass. |
| Hidden or ambiguous primary actions | Pass | Generate, Templates, Text, Transcript, Audio, and Effects action flows passed focused interaction tests. |
| Text contrast and help affordances | Pass | Rail labels, status, grouped tooltips, and Speech help are covered by focused tests and reviewed screenshots. |
| Browser runtime errors | Pass | The visual-QA harness failed on `pageerror` and unexpected `console.error`; the release run completed cleanly. |

Fresh verification totals:

- frontend suite: 107 files and 2,097 tests passed;
- source-quality suite: 31 tests passed;
- lint and production build passed;
- browser release matrix: 82 expected screenshots, zero missing, zero unexpected, zero comparison mismatches;
- baseline manifest integrity and the `darwin-arm64` platform threshold policy passed.

The durable reviewed screenshots and integrity metadata are stored in `docs/visual-qa/browser-visual-baseline/` and `docs/visual-qa/browser-visual-baseline-manifest.json`.
