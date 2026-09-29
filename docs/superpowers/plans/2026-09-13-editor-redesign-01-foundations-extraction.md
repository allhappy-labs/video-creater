# Editor Redesign 01 — Foundations and Domain Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every piece of pure domain logic out of `src/components/workspace/*` into tested `src/lib/` modules, delete dead code, and add the keymap registry. The old editor keeps working and every gate stays green, so plan 02 can delete the old UI without losing behavior.

**Architecture:** This is characterization-first extraction.

- For each helper group, first export the helpers in place.
- Record their current behavior with inline-snapshot tests.
- Move the code verbatim into a `src/lib/` module and point the old UI and the tests at the new module.
- Re-run the recorded tests unchanged.

Duplicate helpers are consolidated only when the copies are behaviorally identical. Variants that differ keep distinct names. No behavior changes in this plan.

**Tech Stack:** TypeScript 5.7, React 19 (unchanged call sites), Vitest 3 (`toMatchInlineSnapshot`), knip.

**Spec:** [2026-09-13 Editor UI/UX Redesign Design](../specs/2026-09-13-editor-ui-ux-redesign-design.md)

## Global Constraints

- Prefix every repository shell command with `rtk`.
- Use Conventional Commits and stage only the files named by each task.
- **No behavior changes.** Moved code is copied verbatim except for `export` keywords, import paths, and consolidation of proven-identical duplicates.
- **Characterization tests are never edited after recording.** A test that changes value after the move means the move changed behavior. Fix the move, not the test.
- **Do not create a directory that shadows an existing module.** `src/lib/timeline.ts` exists, so timeline helpers go to `src/lib/timeline-ops/`.
- **Keep `pnpm check:unused` green.** Every new module must be imported by production code or by its test in the same commit.
- **No global token, dependency, or visual change in this plan.** Visual baselines must stay byte-identical.
- The line numbers below were recorded at commit `3abf089d`. Re-locate each declaration by name before editing if the file has moved on.

## Characterization Protocol (used by every extraction task)

1. **Export in place.** Add `export` to each listed declaration in its current UI file. Do not change anything else.
2. **Write characterization tests.**
   - Create `src/lib/<dest>.test.ts`.
   - Import the helpers from their current UI file path, e.g. `@/components/workspace/media-bin`.
   - Call each helper with the listed inputs.
   - Assert with `expect(result).toMatchInlineSnapshot()`.
3. **Record the snapshots.** Run `rtk pnpm vitest run src/lib/<dest>.test.ts -u`, which writes the inline snapshots into the test file.
4. **Review the recorded values.** Read them against the helper source. If a value looks wrong, the helper has a latent bug. Record it as-is and note it under the task's "Observed quirks" in the commit body. Do not fix it here.
5. **Move the code.**
   - Create `src/lib/<dest>.ts` and move the declarations verbatim, including their private dependencies.
   - Export only the names used elsewhere.
   - Replace the originals in the UI file with an import from `@/lib/<dest>`.
6. **Point the test at the new module.** Change only the test's import path to `@/lib/<dest>`.
7. **Verify without updating snapshots.** Run `rtk pnpm vitest run src/lib/<dest>.test.ts` and confirm it passes without `-u`.
8. **Run the affected suites.** Run `rtk pnpm vitest run src/components/workspace/<affected>.test.tsx` for every UI file touched.
9. **Run static checks.** Run `rtk pnpm lint && rtk pnpm check:unused`.
10. **Commit.**

Shared fixture helpers for these tests live in `src/test-utils/editor-fixtures.ts`, which is created in Task 1.

---

## File Map

### New files

- `src/test-utils/editor-fixtures.ts` — deterministic project, media, generated-asset and job fixtures built from `createSampleProject()`.
- `src/lib/format.ts` — shared number, duration and timecode formatting.
- `src/lib/media/names.ts` — filename, display name, aspect ratio and quality labels.
- `src/lib/media/folder-tree.ts` — folder tree, path labels, slugs.
- `src/lib/media/search.ts` — local and indexed media search predicates and labels.
- `src/lib/media/matte.ts` — matte sizing and color normalization.
- `src/lib/media/preview-source.ts` — safe project paths, preview URLs, preview source selection, preview transport data.
- `src/lib/timeline-ops/item-properties.ts` — typed property readers and effect preservation.
- `src/lib/timeline-ops/ids.ts` — deterministic timeline item IDs.
- `src/lib/timeline-ops/reorder.ts` — source clip reorder context and updates.
- `src/lib/timeline-ops/silence.ts` — silence ripple ranges.
- `src/lib/timeline-ops/navigation.ts` — edit points, gaps, zoom, overview window, track labels, timecode.
- `src/lib/timeline-ops/keyframes.ts` — keyframe property configs, bounds, interpolation.
- `src/lib/timeline-ops/automation.ts` — fades, waveform peaks, automation points, source-range warnings.
- `src/lib/preview/canvas-geometry.ts` — motion style, canvas transform and crop math.
- `src/lib/preview/viewer-context.ts` — viewer context kind, toolbar placement, initial selection.
- `src/lib/captions/caption-items.ts` — caption cue building, style properties, word timings, repair action.
- `src/lib/captions/transcript-drafts.ts` — transcript word repair drafts.
- `src/lib/generation/types.ts` — generation request, model, option and limit types.
- `src/lib/generation/provider-rules.ts` — model IDs, catalog resolution, capability predicates, prompt rules.
- `src/lib/generation/settings-options.ts` — duration, aspect, resolution, quality, voice and image-count options and settings payload.
- `src/lib/generation/pricing.ts` — credit rate tables and cost estimates.
- `src/lib/generation/references.ts` — reference tags, typed refs and limit messages.
- `src/lib/generation/catalog.ts` — backend catalog payload normalization.
- `src/lib/generation/requests.ts` — upscale and video-to-audio requests, generated IDs.
- `src/lib/generation/assets.ts` — generated-asset lookups, titles, labels, sorting.
- `src/lib/generation/timeline-placement.ts` — placement intents, placeholders, output actions, source ranges.
- `src/lib/jobs/activity-records.ts` — activity records, job labels, job counts, start-request validation summaries.
- `src/lib/jobs/ids.ts` — job ID generators.
- `src/lib/jobs/temporal-fallback.ts` — Temporal workflow definitions and fallback start requests.
- `src/lib/export/profiles.ts` — export profile availability, disabled reasons, resolutions, dimensions, estimates.
- `src/lib/export/render-report.ts` — render report metric and check labels.
- `src/lib/agent/proposal-review-model.ts` — Codex proposal review model.
- `src/lib/agent/proposal-materialize.ts` — proposal → project actions.
- `src/lib/agent/project-merge.ts` — local action batch apply, metadata and analysis merges, history snapshot equality.
- `src/lib/agent/selection-context.ts` — selected media/clip/range contexts, mention targets, MCP setup snippet.
- `src/lib/agent/chat-transcript-storage.ts` — chat transcript persistence.
- `src/lib/templates/template-item.ts` — template fields, style, metadata, category labels.
- `src/lib/keymap.ts` — single keyboard shortcut registry.
- A `*.test.ts` beside every module above.

### Modified files

- Every UI file named in each task under `src/components/workspace/`. Declarations are replaced by imports.
- `src/components/workspace/caption-inspector.test.tsx`, `activity-panel.test.tsx`, `codex-proposal-review.test.tsx`, `export-job-start-request.test.ts`, `timeline-editor.test.tsx`, `motion-template-preview.test.tsx`, `editor-workspace.test.tsx` — the pure-helper tests listed in Task 22 move to the new lib test files.

---

### Task 1: Fixtures and dead code removal

**Files:**
- Create: `src/test-utils/editor-fixtures.ts`
- Create: `src/test-utils/editor-fixtures.test.ts`
- Modify: `src/components/workspace/editor-workspace.tsx` (delete L1076–1331 and the `sampleTimeline` import at L209)
- Modify: `src/components/workspace/media-bin.tsx` (delete `generationPlacementLabel` L657–667)
- Modify: `src/components/workspace/agent-panel.tsx` (delete `referenceThumbnailIcon` L477–487 and `selectedReferenceThumbnail` L489–529)

- [ ] **Step 1: Confirm the dead code has no references**

Run:
```bash
rtk rg -n "createDefaultSampleTimeline|generationPlacementLabel|selectedReferenceThumbnail|referenceThumbnailIcon|\bsampleTimeline\b" src
```
Expected:
- `createDefaultSampleTimeline` appears only at its definition and at the call inside the local `createSampleProject`.
- `sampleTimeline` appears only at its import and inside `createDefaultSampleTimeline`.
- `generationPlacementLabel` and `selectedReferenceThumbnail` appear only at their definitions.
- `referenceThumbnailIcon` appears only at its definition and inside `selectedReferenceThumbnail`.
- The local `createSampleProject` (L1134) is never called. The import on L216 is aliased `createBundledSampleProject`.

If any other reference exists, stop and report it.

- [ ] **Step 2: Delete the dead declarations**

Delete, in `editor-workspace.tsx`:
- `function createDefaultSampleTimeline` (L1076–1132),
- `function createSampleProject` (L1134–1331),
- `sampleTimeline` from the import list on L209. Remove the whole import statement if it becomes empty.

Delete the two `agent-panel.tsx` functions and the one `media-bin.tsx` function listed above. Remove any import that becomes unused. `tsc` does not flag unused imports here, so check each removed body's identifiers with `rtk rg`.

- [ ] **Step 3: Create the shared fixtures**

```ts
// src/test-utils/editor-fixtures.ts
import type {
  GeneratedAsset,
  MediaAsset,
  ProjectJobSummary,
  TimelineItem,
  TimelineTrack,
  VideoProject,
} from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { requiredValue } from "./required";

/** A fresh deep copy of the bundled sample project; safe to mutate per test. */
export function fixtureProject(): VideoProject {
  return structuredClone(createSampleProject());
}

export function fixtureMedia(project: VideoProject, kind: MediaAsset["kind"]): MediaAsset {
  return requiredValue(
    project.media.find((media) => media.kind === kind),
    `sample project media of kind ${kind}`,
  );
}

export function fixtureTrack(project: VideoProject, kind: TimelineTrack["kind"]): TimelineTrack {
  return requiredValue(
    project.timeline.tracks.find((track) => track.kind === kind),
    `sample project track of kind ${kind}`,
  );
}

export function fixtureItem(project: VideoProject, trackKind: TimelineTrack["kind"]): TimelineItem {
  return requiredValue(fixtureTrack(project, trackKind).items[0], `first item on ${trackKind} track`);
}

export function fixtureGeneratedAsset(project: VideoProject): GeneratedAsset {
  return requiredValue(project.generatedAssets?.[0], "sample project generated asset");
}

export function fixtureJobs(project: VideoProject): ProjectJobSummary[] {
  return project.jobs ?? [];
}
```

Adjust these field accesses to the real `VideoProject` shape in `src/lib/project.ts` before writing the file:
- `project.timeline.tracks`,
- `track.items`,
- `project.generatedAssets`,
- `project.jobs`.

Keep the exported function names.

```ts
// src/test-utils/editor-fixtures.test.ts
import { describe, expect, it } from "vitest";
import {
  fixtureGeneratedAsset,
  fixtureItem,
  fixtureMedia,
  fixtureProject,
} from "./editor-fixtures";

describe("editor fixtures", () => {
  it("returns independent sample project copies", () => {
    const first = fixtureProject();
    const second = fixtureProject();
    first.name = "changed";
    expect(second.name).not.toBe("changed");
  });

  it("finds representative media, items, and generated assets", () => {
    const project = fixtureProject();
    expect(fixtureMedia(project, "video").id).toBeTruthy();
    expect(fixtureItem(project, "video").id).toBeTruthy();
    expect(fixtureGeneratedAsset(project).id).toBeTruthy();
  });
});
```

- [ ] **Step 4: Verify**

Run:
```bash
rtk pnpm vitest run src/test-utils/editor-fixtures.test.ts src/components/workspace/editor-workspace.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/media-bin.test.tsx
rtk pnpm lint && rtk pnpm check:unused
```
Expected: all pass. If the sample project lacks a generated asset or a track kind, change the fixture helper to build the minimal value explicitly, then re-run.

- [ ] **Step 5: Commit**

```bash
rtk git add src/test-utils/editor-fixtures.ts src/test-utils/editor-fixtures.test.ts src/components/workspace/editor-workspace.tsx src/components/workspace/agent-panel.tsx src/components/workspace/media-bin.tsx
rtk git commit -m "refactor(workspace): remove dead sample and thumbnail helpers and add editor fixtures"
```

---

### Task 2: Shared formatting and media names

**Files:**
- Create: `src/lib/format.ts`, `src/lib/format.test.ts`, `src/lib/media/names.ts`, `src/lib/media/names.test.ts`
- Modify: `editor-workspace.tsx`, `media-bin.tsx`, `source-clip-inspector.tsx`, `project-timeline-inspector.tsx`, `preview-panel.tsx`, `timeline-editor.tsx`, `timeline-item.tsx`, `timeline-ruler.tsx`, `text-overlay-inspector.tsx`, `caption-inspector.tsx`

**Declarations**

`src/lib/format.ts`:

| Name | Source | Notes |
|---|---|---|
| `formatDurationLabel` | `project-timeline-inspector.tsx` `formatDuration` L38–50 | Canonical. `preview-panel.tsx` `formatPreviewDuration` L102–114 is the same helper. Consolidate only if the step 3 snapshots are equal. |
| `formatDurationBadge` | `media-bin.tsx` L317–331 | Keep separate if different. |
| `formatTimelinePlacementTime` | `media-bin.tsx` L333–342 | |
| `formatTimecode` | `timeline-editor.tsx` L517–536 | Duplicates: `timeline-item.tsx` L93–110 and `timeline-ruler.tsx` L31–48. |
| `formatSecondsShort` | `caption-inspector.tsx` `formatSeconds` L218–220 | Duplicate: `text-overlay-inspector.tsx` `formatSeconds` L25–27. |
| `pluralize` | `project-timeline-inspector.tsx` L60–62 | |
| `roundTimelineSeconds` | `editor-workspace.tsx` L1333–1335 | Duplicates: `media-bin.tsx` `roundGenerationTimelineSeconds` L344–346, `agent-panel.tsx` `roundTimelineDraftSeconds` L576–578, `timeline-overview.tsx` `roundedSeconds` L61–63. |
| `formatCredits` | `media-bin.tsx` L356–358 | |

`src/lib/media/names.ts`:

| Name | Source | Notes |
|---|---|---|
| `filenameFromPath`, `mediaDisplayName` | `editor-workspace.tsx` L2808–2814 | Identical copies: `media-bin.tsx` L309–315, `source-clip-inspector.tsx` L332–338. |
| `greatestCommonDivisor` | `editor-workspace.tsx` L2653–2663 | Identical copy: `source-clip-inspector.tsx` L287–297. `project-timeline-inspector.tsx` L64–75 truncates and can return 0. Keep it as `truncatedGreatestCommonDivisor` unless the snapshots match. |
| `aspectRatioLabel` | `editor-workspace.tsx` L2665–2668 | |
| `formatAspectRatioOrNull` | `source-clip-inspector.tsx` `formatAspectRatio` L299–306 | |
| `formatAspectRatioOrUnknown` | `project-timeline-inspector.tsx` `formatAspectRatio` L77–89 | |
| `mediaQualityLabel` | `editor-workspace.tsx` L2670–2692 | Copy: `source-clip-inspector.tsx` L308–330. |
| `mediaAssetMeta` | `media-bin.tsx` L376–381 | |
| `formatDimensions`, `formatFrameRate`, `formatOptionalSeconds` | `source-clip-inspector.tsx` L267–279 | |

- [ ] **Step 1: Export in place**

Export every declaration listed above, including the duplicate copies, in their current files.

- [ ] **Step 2: Write characterization tests comparing duplicates**

```ts
// src/lib/format.test.ts
import { describe, expect, it } from "vitest";
import { formatDuration } from "@/components/workspace/project-timeline-inspector";
import { formatPreviewDuration } from "@/components/workspace/preview-panel";
import { formatDurationBadge, formatTimelinePlacementTime, formatCredits, roundGenerationTimelineSeconds } from "@/components/workspace/media-bin";
import { formatTimecode } from "@/components/workspace/timeline-editor";
import { formatTimecode as itemTimecode } from "@/components/workspace/timeline-item";
import { formatTimecode as rulerTimecode } from "@/components/workspace/timeline-ruler";
import { formatSeconds as captionSeconds } from "@/components/workspace/caption-inspector";
import { formatSeconds as overlaySeconds } from "@/components/workspace/text-overlay-inspector";
import { pluralize } from "@/components/workspace/project-timeline-inspector";
import { roundTimelineSeconds } from "@/components/workspace/editor-workspace";
import { roundTimelineDraftSeconds } from "@/components/workspace/agent-panel";

const seconds = [0, 0.04, 0.5, 1, 59.999, 60, 61.25, 3599.5, 3600, 7322.125, -1, Number.NaN];

describe("format characterization", () => {
  it("formats durations", () => {
    expect(seconds.map((value) => [value, formatDuration(value)])).toMatchInlineSnapshot();
    expect(seconds.map((value) => formatPreviewDuration(value))).toEqual(seconds.map((value) => formatDuration(value)));
    expect(seconds.map((value) => [value, formatDurationBadge(value)])).toMatchInlineSnapshot();
  });

  it("formats timecodes identically across timeline copies", () => {
    const expected = seconds.map((value) => formatTimecode(value));
    expect(expected).toMatchInlineSnapshot();
    expect(seconds.map((value) => itemTimecode(value))).toEqual(expected);
    expect(seconds.map((value) => rulerTimecode(value))).toEqual(expected);
  });

  it("formats short seconds, placement time, credits, and plurals", () => {
    expect(seconds.map((value) => [captionSeconds(value), overlaySeconds(value)])).toMatchInlineSnapshot();
    expect(seconds.map((value) => formatTimelinePlacementTime(value))).toMatchInlineSnapshot();
    expect([0, 1, 12.5, 1000].map((value) => formatCredits(value))).toMatchInlineSnapshot();
    expect([[0, "clip"], [1, "clip"], [2, "clip"]].map(([count, noun]) => pluralize(count as number, noun as string))).toMatchInlineSnapshot();
  });

  it("rounds timeline seconds identically", () => {
    const expected = seconds.map((value) => roundTimelineSeconds(value));
    expect(expected).toMatchInlineSnapshot();
    expect(seconds.map((value) => roundGenerationTimelineSeconds(value))).toEqual(expected);
    expect(seconds.map((value) => roundTimelineDraftSeconds(value))).toEqual(expected);
  });
});
```

Use each helper's real signature. If a helper takes extra arguments (for example `pluralize(count, singular, plural)`), pass representative values and keep the same inputs across duplicates. When an `.toEqual(expected)` duplicate comparison fails, the copies are not identical. Replace that line with its own `toMatchInlineSnapshot()` and keep the variant under a distinct name in step 5.

```ts
// src/lib/media/names.test.ts
import { describe, expect, it } from "vitest";
import * as workspace from "@/components/workspace/editor-workspace";
import * as bin from "@/components/workspace/media-bin";
import * as clip from "@/components/workspace/source-clip-inspector";
import * as projectInspector from "@/components/workspace/project-timeline-inspector";
import { fixtureMedia, fixtureProject } from "@/test-utils/editor-fixtures";

const paths = ["media/input.mp4", "/abs/path/clip.MOV", "noext", "dir/", "", "a\\b\\c.wav"];
const dims: Array<[number, number]> = [[1920, 1080], [1080, 1920], [640, 360], [1000, 1000], [0, 0], [1366, 768]];

describe("media names characterization", () => {
  it("derives filenames and display names identically", () => {
    const project = fixtureProject();
    const media = fixtureMedia(project, "video");
    const expected = paths.map((path) => workspace.filenameFromPath(path));
    expect(expected).toMatchInlineSnapshot();
    expect(paths.map((path) => bin.filenameFromPath(path))).toEqual(expected);
    expect(paths.map((path) => clip.filenameFromPath(path))).toEqual(expected);
    expect(workspace.mediaDisplayName(media)).toMatchInlineSnapshot();
    expect(bin.mediaDisplayName(media)).toEqual(workspace.mediaDisplayName(media));
    expect(clip.mediaDisplayName(media)).toEqual(workspace.mediaDisplayName(media));
    expect(bin.mediaAssetMeta(media)).toMatchInlineSnapshot();
  });

  it("labels aspect ratios and quality", () => {
    expect(dims.map(([w, h]) => [workspace.greatestCommonDivisor(w, h), clip.greatestCommonDivisor(w, h), projectInspector.greatestCommonDivisor(w, h)])).toMatchInlineSnapshot();
    expect(dims.map(([w, h]) => [workspace.aspectRatioLabel(w, h), clip.formatAspectRatio(w, h), projectInspector.formatAspectRatio(w, h)])).toMatchInlineSnapshot();
    const project = fixtureProject();
    const media = fixtureMedia(project, "video");
    expect(workspace.mediaQualityLabel(media)).toMatchInlineSnapshot();
    expect(clip.mediaQualityLabel(media)).toEqual(workspace.mediaQualityLabel(media));
    expect([clip.formatDimensions(media), clip.formatFrameRate(media), clip.formatOptionalSeconds(4.25), clip.formatOptionalSeconds(undefined)]).toMatchInlineSnapshot();
  });
});
```

Match argument shapes to the real signatures. For example, `formatAspectRatio` may take a media asset instead of `(w, h)`. In that case build `{ ...media, width: w, height: h }` per row.

- [ ] **Step 3: Record and review**

Run: `rtk pnpm vitest run src/lib/format.test.ts src/lib/media/names.test.ts -u`
Expected: PASS, with inline snapshots written. Review every recorded value and every duplicate `toEqual`.

- [ ] **Step 4: Move and consolidate**

Create `src/lib/format.ts` and `src/lib/media/names.ts` with the canonical declarations copied verbatim.

- Consolidate duplicates proven identical in step 3: delete the copies and import the canonical name, aliasing where the local name differs, for example `import { roundTimelineSeconds as roundGenerationTimelineSeconds } from "@/lib/format"`.
- Keep variants that differ under the distinct names from the table.

Remove the temporary `export` keywords left on UI files for names that now come from lib.

- [ ] **Step 5: Retarget the tests**

- Replace the workspace imports in both tests with imports from `@/lib/format` and `@/lib/media/names`.
- Collapse duplicate comparisons into single calls on the canonical function and keep every recorded inline snapshot value unchanged.
- For variants, call the distinctly named lib function and keep its snapshot.

- [ ] **Step 6: Verify**

Run:
```bash
rtk pnpm vitest run src/lib/format.test.ts src/lib/media/names.test.ts src/components/workspace
rtk pnpm lint && rtk pnpm check:unused
```
Expected: all pass with no snapshot writes. `src/components/workspace` runs the whole folder, which is slow but required because many files changed.

- [ ] **Step 7: Commit**

```bash
rtk git add src/lib/format.ts src/lib/format.test.ts src/lib/media/names.ts src/lib/media/names.test.ts src/components/workspace/editor-workspace.tsx src/components/workspace/media-bin.tsx src/components/workspace/source-clip-inspector.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/preview-panel.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-item.tsx src/components/workspace/timeline-ruler.tsx src/components/workspace/text-overlay-inspector.tsx src/components/workspace/caption-inspector.tsx src/components/workspace/agent-panel.tsx src/components/workspace/timeline-overview.tsx
rtk git commit -m "refactor(lib): extract shared formatting and media name helpers"
```

---

### Task 3: Timeline item properties, IDs, reorder, silence

**Files:**
- Create: `src/lib/timeline-ops/item-properties.ts`, `ids.ts`, `reorder.ts`, `silence.ts` and a test for each
- Modify: `editor-workspace.tsx`, `source-clip-inspector.tsx`, `timeline-editor.tsx`, `text-overlay-inspector.tsx`, `agent-panel.tsx`

**Declarations**

`item-properties.ts`:

| Name | Source | Notes |
|---|---|---|
| `numberProperty` | `editor-workspace.tsx` L1392–1395 | Copies: `timelineItemNumberProperty` L2503–2506, `source-clip-inspector.tsx` L354–357, `timeline-editor.tsx` L882–885. |
| `stringProperty` | `editor-workspace.tsx` L1387–1390 | Copies: `source-clip-inspector.tsx` L501–504 and `timeline-editor.tsx` L997–1000. `text-overlay-inspector.tsx` L29–32 has a fallback and no trim; keep it as `stringPropertyOrFallback`. |
| `timelineItemSourceMediaId` | `editor-workspace.tsx` L2870–2880 | Copies: `source-clip-inspector.tsx` `sourceMediaId` L625–635 and `timeline-editor.tsx` `sourceMediaId` L1713–1722. |
| `projectActionEffectsForItem` | `editor-workspace.tsx` L982–1011 | Compare with `source-clip-inspector.tsx` `preservedEffects` L420–441. |
| `nextCatalogEffectInstanceId` | `editor-workspace.tsx` L1013–1025 | |
| `itemNeedsCanonicalViewerPreparation` | `editor-workspace.tsx` L3158–3187 | |
| `clampTimelinePlayhead` | `editor-workspace.tsx` L2830–2836 | Compare with `timeline-editor.tsx` `clampPlayheadSeconds` L538–544. |
| `transformNumberProperty`, `transformBooleanProperty`, `colorGradeNumberProperty`, `hasEffect`, `audioDenoiseAmount`, `audioDenoisePreparationStatus`, `stableEffectInstanceId`, `sourceEffectDrafts`, `isVisualOpacityItem`, `visualBlendModeProperty` | `source-clip-inspector.tsx` L359–509 | |
| `visualBlendModes`, `VisualBlendMode` | `source-clip-inspector.tsx` L75–94 | Move first. |
| `visualClipOpacity`, `itemMetadata`, `isGeneratedTimelineItem`, `generatedWorkflowStatusTitle` | `timeline-editor.tsx` L1002–1097 | Also move the types `GeneratedTimelineWorkflowStatus` L334–339 and `GeneratedTimelineProvenance` L341–345. |
| `isVisualOpacityClip` | `agent-panel.tsx` L531–539 | Compare with `isVisualOpacityItem`. |

`ids.ts`: `timelineMediaItemId` (L1967–1987) and `duplicateTimelineItemId` (L1989–2004) from `editor-workspace.tsx`.

`reorder.ts`:
- From `editor-workspace.tsx`: `sequenceGapSeconds` L1337–1351, `orderedTrackItems` L1353–1355, `getSourceClipReorderContext` L1357–1385.
- From `source-clip-inspector.tsx`: types `SourceClipReorderContext` and `SourceClipReorderUpdate` L42–55, and `reorderUpdate` L804–825.
- Compare `reorderUpdate` with `agent-panel.tsx` `reorderSelectedTimelineClip` L580–601 and its types `AgentTimelineClipReorderContext`/`Update` L159–172.

`silence.ts`: from `editor-workspace.tsx`, the constants L2408–2409, `timelineSilenceRippleRanges` L2411–2468, `currentTimelineSilenceRippleRange` L2470–2477, `rippleTrackIdsForItem` L2479–2489 and `validMediaSilenceRange` L2491–2501.

**Characterization inputs** (build from `fixtureProject()`):

- **Items to probe:**
  - every item in the project,
  - one item with `properties` removed,
  - one item with numeric properties given as strings `"0.5"`, empty strings, `NaN` and `Infinity`.
- **Effects:** one visual item with two effects, one of them with a `legacyEffectInstanceId`.
- **Reorder:** a video track with three sequential items. Probe the first, the middle, the last, and an item with a gap before it.
- **Silence:** attach `analysis.silenceRanges` to the video media, or the actual field name in `MediaAsset`, with ranges that are:
  - fully inside the item,
  - partially overlapping it,
  - outside it,
  - zero-length.

  Probe with the playhead inside and outside a range.
- **IDs:** a project that already contains `item-media-1`, `item-media-1-2`, and so on, so collision suffixes are exercised.

- [ ] **Step 1: Export in place** every declaration and duplicate listed above.
- [ ] **Step 2: Write characterization tests.** Create `src/lib/timeline-ops/{item-properties,ids,reorder,silence}.test.ts`.
  - Each test imports from the current UI paths.
  - Map the inputs above through each function into `toMatchInlineSnapshot()`.
  - Compare duplicates with `toEqual` against the canonical copy, as in Task 2.
  - For `Date.now`-based IDs, call `vi.useFakeTimers()` with `vi.setSystemTime(new Date("2026-09-13T00:00:00Z"))`.
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/timeline-ops -u`.
- [ ] **Step 4: Move and consolidate** as in Task 2, steps 4–5. Keep non-identical variants under distinct names:
  - `stringPropertyOrFallback`,
  - `preservedEffectsForInspector` (only if different from `projectActionEffectsForItem`),
  - `clampPlayheadSecondsForTimeline` (only if different).
- [ ] **Step 5: Verify**

```bash
rtk pnpm vitest run src/lib/timeline-ops src/components/workspace
rtk pnpm lint && rtk pnpm check:unused
```

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/timeline-ops src/components/workspace/editor-workspace.tsx src/components/workspace/source-clip-inspector.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/text-overlay-inspector.tsx src/components/workspace/agent-panel.tsx
rtk git commit -m "refactor(lib): extract timeline item property, id, reorder, and silence helpers"
```

---

### Task 4: Timeline navigation, automation, keyframes

**Files:**
- Create: `src/lib/timeline-ops/navigation.ts`, `automation.ts`, `keyframes.ts` and their tests
- Modify: `timeline-editor.tsx`, `timeline-overview.tsx`, `timeline-ruler.tsx`, `timeline-item.tsx`, `source-clip-inspector.tsx`, `keyframe-lane-editor.tsx`, `preview-panel.tsx`

**Declarations**

`navigation.ts`:
- From `timeline-editor.tsx`:
  - types `TimelineRangeSelection` L347–350, `TimelineGapSelection` L352–356, `TimelineRippleTrimRequest` L390–394,
  - `timelineGapAtSeconds` L358–376,
  - `formatTimestamp` L510–515,
  - `timelineEditPointSeconds`, `previousTimelineEditPoint`, `nextTimelineEditPoint` L546–583,
  - `clampZoomPercent` L585–594, `resolveTimelineOverviewViewState` L596–655, `snapTimelineSeconds` L657–665,
  - `trackEnabled`, `trackLanePrefix`, `trackLaneLabels`, `trackKindAccentLabel` L667–731,
  - `timelineInteractionErrorFeedback` L760–781,
  - `trackDisplayHeight`, `clampTrackDisplayHeight`, `buildTimelineTrackGeometry` L124–153 and constants L68–78.
- From `timeline-overview.tsx`: `finiteOr`, `clamp`, `normalizedDuration`, `normalizeWindow`, `interactionWindow`, `windowsMatch` L53–160.
- From `timeline-ruler.tsx`: `timelineBadgeLayout`, `renderedBadgeSeparation` L54–86.
- From `timeline-item.tsx`: `timelineItemDensity` L86–91, `formatInteractionTimecode` L112–115.
- From `preview-panel.tsx` and `timeline-editor.tsx`: `isEditableKeyboardTarget` (L205–216 and L797–808). It takes `EventTarget | null` and has no React dependency.

Left in the UI files because they are geometry for the old layout only and are deleted in plan 02: `resizeControlModeForWidth` L413–417, `compactResizeDockGeometry` L173–199, constants L154–171.

`automation.ts`: from `timeline-editor.tsx`:
- `suppliedWaveformPeaks` L1099–1112, `waveformPeaks` L1128–1130,
- `audioFadeOutSeconds`, `audioFadeInSeconds` L1132–1156,
- `timelineAutomationPoints` L1198–1223, `automationValueBounds`, `automationPointPosition` L1271–1291,
- `sourceBoundaryRange` L1614–1627, `sourceRangeDurationMismatch` L1655–1670.

Also `timeline-overview.tsx` `waveformPeaks` L162–168, renamed `overviewWaveformPeaks` if it differs.

`keyframes.ts`:
- From `source-clip-inspector.tsx`:
  - types `VisualMotionKeyframeProperty` L95–99,
  - `motionKeyframeDefault`, `motionKeyframeBounds` L511–520,
  - `visualKeyframePropertyConfigs`, `audioKeyframePropertyConfigs` L522–536,
  - `inspectorKeyframesByProperty` L540–581, `inspectorEffectParameterKeyframes` L583–611, `keyframePropertyConfigsForItem` L613–619.
- From `keyframe-lane-editor.tsx`: `KeyframePropertyConfig` L17–25, `sortedKeyframes` L57–59, `suggestedValue` L61–76.

**Characterization inputs:**
- **Timeline shape:** a timeline with two tracks.
  - Items at [0–2], [2–4], [5–8] seconds on the video track.
  - Items at [1–3] seconds on the audio track.
- **Playheads:** 0, 1.999, 2, 4.5, 8 and 20.
- **Gap and edit-point probes:** `timelineGapAtSeconds` at 4.5 (inside the gap) and at 1 (not a gap). Previous and next edit points from every playhead.
- **Zoom:** 0, 1, 99, 100, 1000 and `NaN`.
- **Overview window:** window inputs that are inside, overflowing, negative, and wider than the duration.
- **Snap:** candidates `[0, 2, 4, 5]` and thresholds 0.05 and 0.5.
- **Interaction errors:** the native overlap error string from `src/lib/project.ts` or `src-tauri`, found by searching for `overlap` in `timeline-editor.test.tsx`, plus an unrelated error.
- **Automation:** items with and without fades and volume keyframes. Keyframe lists that are unsorted, of length 1, and empty. `suggestedValue` at, before, after and between keyframes.
- **Keyboard targets:** `isEditableKeyboardTarget` on `document.createElement("input")`, `textarea`, a `div` with `contentEditable="true"`, a button, and `null`.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Write characterization tests** (`src/lib/timeline-ops/{navigation,automation,keyframes}.test.ts`) using the inputs above. Move the existing `timelineGapAtSeconds` test from `timeline-editor.test.tsx` (the `it("finds only bounded empty track gaps")` block starting L23) into `navigation.test.ts` verbatim, and delete it from the UI test.
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/timeline-ops -u`.
- [ ] **Step 4: Move and consolidate** as in Task 2.
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/timeline-ops src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/timeline-ops src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/timeline-overview.tsx src/components/workspace/timeline-ruler.tsx src/components/workspace/timeline-item.tsx src/components/workspace/source-clip-inspector.tsx src/components/workspace/keyframe-lane-editor.tsx src/components/workspace/preview-panel.tsx
rtk git commit -m "refactor(lib): extract timeline navigation, automation, and keyframe helpers"
```

---

### Task 5: Media folders, search, matte, preview sources, viewer context

**Files:**
- Create: `src/lib/media/folder-tree.ts`, `search.ts`, `matte.ts`, `preview-source.ts`, `src/lib/preview/viewer-context.ts` and their tests
- Modify: `media-bin.tsx`, `editor-workspace.tsx`, `project-timeline-inspector.tsx`, `matte-sheet.tsx`, `preview-panel.tsx`, `viewer-context-toolbar.tsx`

**Declarations**

`folder-tree.ts`:
- From `media-bin.tsx`: type `MediaFolderNode` L302–307, `mediaFolderTree` L822–862, `flattenMediaFolderNodes` L864–869.
- From `editor-workspace.tsx`: `mediaFolderPathLabel` L2040–2068 (copy: `project-timeline-inspector.tsx` L349–380), `mediaFolderSlug` L2628–2636, `mediaFolderIdForName` L2638–2651.

`search.ts`: from `media-bin.tsx` L616–786: `normalizeSearchText`, `isSearchableText`, `mediaMatchesSearch`, `generatedAssetMatchesSearch`, `stringField`, `mediaIdsFromIndexedSearch`, `indexedSearchStatusLabels`, `indexedSearchNeedsRebuild`, `searchScopeUsesLocalMedia`, `searchScopeUsesLocalGeneratedAssets`.

`matte.ts`: from `matte-sheet.tsx`: types `MatteAspectRatio` and `MatteCreateInput` L12–27, `matteAspectOptions` L12–20, `evenSize` L40–43, `mattePreviewSize` L45–66, `normalizedHex` L68–71.

`preview-source.ts`:
- From `preview-panel.tsx`: types `PreviewSource` L29–41 and `ViewerMode` L27, plus `parsePreviewDurationLabel` L116–139, `formatPreviewCurrentTime` L141–160, `previewTransportData` L162–203, `sourcePreviewStepSeconds` L218–233, `mediaElementDurationSeconds` L235–248, `sourceViewerTabId` L98–100.
- From `editor-workspace.tsx`: `safeProjectMediaPath` L2882–2908, `previewUrlForMedia` L2910–2923, `previewUrlsForMedia` L2925–2929, `previewUrlsForTimelineSources` L2931–2946, `selectedPreviewSource` L2948–2980, `formatViewerDuration` L2816–2828, `formatViewerSeconds` L2838–2840, `sourceRangeLabelForItem` L2842–2850.
- From `source-clip-inspector.tsx`: `sourceRangeLabel` L704–712.

`viewer-context.ts`:
- From `viewer-context-toolbar.tsx`: type `ViewerContextKind` L14–20.
- From `editor-workspace.tsx`: `viewerContextKindForItem` L3113–3143, `viewerContextToolbarPlacementForItem` L3145–3156, `initialSelectedTimelineItem` L3189–3203.
- From `preview-panel.tsx`: `resolveContextToolbarPlacement` L259–277, which needs `TimelinePreviewCanvasState`. Move that type from `timeline-preview-compositor.tsx` L32–38 into `src/lib/preview/canvas-geometry.ts` now, as a type-only file that Task 6 fills.

**Characterization inputs:**
- **Folders:**
  - a nested three-level tree,
  - an orphan whose parent is missing,
  - a cycle (A→B→A),
  - duplicate names for `mediaFolderIdForName`,
  - names with punctuation and Unicode for the slug.
- **Search:**
  - queries `""`, `"  "`, `"INPUT"`, `"voice over"`, `"émoji"`, and a folder-label match;
  - indexed search results with valid IDs, missing fields, and a `null` result;
  - index status payloads fresh, stale, building, and failed. Take their shape from `src/lib/project.ts` `search_project_media` / `rebuild_project_search_index` result types.
- **Matte:** every aspect option, hex strings `#fff`, `FFF`, `#abcdef`, `zzz`, `""`.
- **Safe paths:** `media/a.mp4`, `/etc/passwd`, `../x.mp4`, `file:///x`, `https://x`, `media/../x.mp4`, `C:\\x.mp4`.
- **Preview URLs:** use `sampleProjectBrowserDir` and `sampleProjectDir` as `projectDir`, and a non-sample directory with the transport stubbed. Stub it with `vi.mock("@/lib/runtime/backend-client", () => ({ backendMediaUrl: (path: string) => \`media://${path}\` }))`.
- **Preview transport data:** timeline mode and source mode, with and without a selected item.
- **Viewer context:** every item kind in the fixture project, plus `null`.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Write characterization tests** with the inputs above.
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/media src/lib/preview -u`.
- [ ] **Step 4: Move and consolidate.**
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/media src/lib/preview src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/media src/lib/preview src/components/workspace/media-bin.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/matte-sheet.tsx src/components/workspace/preview-panel.tsx src/components/workspace/viewer-context-toolbar.tsx src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/source-clip-inspector.tsx
rtk git commit -m "refactor(lib): extract media folder, search, matte, preview source, and viewer context helpers"
```

---

### Task 6: Preview canvas geometry and template items

**Files:**
- Modify: `src/lib/preview/canvas-geometry.ts` (created as types in Task 5)
- Create: `src/lib/preview/canvas-geometry.test.ts`, `src/lib/templates/template-item.ts`, `src/lib/templates/template-item.test.ts`
- Modify: `timeline-preview-compositor.tsx`, `motion-template-preview.tsx`, `motion-template-preview.test.tsx`, `preview-panel.tsx`, `editor-workspace.tsx`, `template-inspector.tsx`, `motion-template-library.tsx`, `text-library-panel.tsx`

**Declarations**

`canvas-geometry.ts`:
- From `timeline-preview-compositor.tsx`:
  - `findTimelineItem` L40–49, `previewMotionStyle` L235–289,
  - interaction types L291–337, `isCanvasCropInteraction` L339–348, minimum constants L350–351,
  - `clampCanvasValue`, `roundedCanvasValue`, `canonicalCanvasTransform`, `canonicalCanvasCrop`, `normalizeRotationDegrees`, `croppedCanvasEdges`, `resizedCanvasTransform` L353–483.
- From `motion-template-preview.tsx`: `motionTemplatePreviewLaneOccupancy` L40–44 and its table L25–38.

`previewMotionStyle` returns `CSSProperties`. Import the type only: `import type { CSSProperties } from "react"`.

`template-item.ts`:
- From `editor-workspace.tsx`: `defaultTemplateStyle` L3325–3329, `templateFieldsForItem` L3331–3348, `templateStyleForItem` L3350–3367, `templateMetadataForItem` L3369–3387.
- Compare with the `template-inspector.tsx` copies: `getTemplateFields` L21–28, `defaultTemplateStyle` L36–40, `getStyleValue` L44–54, `getMetadataValue` L56–67.
- Compare with `motion-template-preview.tsx` `getTemplateFields` L8–15.
- From `motion-template-library.tsx`:
  - `formatCategory` L17–20 (copy: `text-library-panel.tsx` L15–18), `formatSourceKind` L22–24, `formatTrack` L26–37, `formatPlacementTime` L39–44,
  - `categoryLabels`, `categoryOrder` L53–67, `templateCategories` L69–87, `backgroundCategories` L89–91,
  - `gradientLoopPanelStyles` L93–99, `splitPreviewLines` L101–107 (copies in `motion-template-preview.tsx` L17–23 and L46–52).

**Characterization inputs:**
- **Layers:** from `buildTimelinePreviewLayers` or the equivalent in `src/lib/timeline-preview.ts`, applied to the fixture project at times 0, 1 and 3. Probe `previewMotionStyle` for each layer, and for a layer with rotation, crop, opacity 0 and motion keyframes.
- **Crop and resize:**
  - `resizedCanvasTransform` for every handle, with pointer deltas that are positive, negative, and below the minimum size.
  - `croppedCanvasEdges` for each crop edge, at and past `minimumVisibleCropFraction`.
- **Rotation:** `normalizeRotationDegrees` for -720, -181, -180, 0, 180, 181 and 725.
- **Templates:**
  - template items with fields, without fields, and with style overrides;
  - every category from `src/lib/motion-templates.ts`;
  - preview text with `\n`, empty text, and very long text.
- **Existing test:** move the `motionTemplatePreviewLaneOccupancy` `it.each` from `motion-template-preview.test.tsx` (L53) verbatim.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Write characterization tests; move the existing lane occupancy test.**
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/preview src/lib/templates -u`.
- [ ] **Step 4: Move and consolidate.**
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/preview src/lib/templates src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/preview src/lib/templates src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/motion-template-preview.tsx src/components/workspace/motion-template-preview.test.tsx src/components/workspace/preview-panel.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/template-inspector.tsx src/components/workspace/motion-template-library.tsx src/components/workspace/text-library-panel.tsx
rtk git commit -m "refactor(lib): extract preview canvas geometry and template item helpers"
```

---

### Task 7: Captions and transcript drafts

**Files:**
- Create: `src/lib/captions/caption-items.ts`, `caption-items.test.ts`, `transcript-drafts.ts`, `transcript-drafts.test.ts`
- Modify: `caption-inspector.tsx`, `caption-inspector.test.tsx`, `editor-workspace.tsx`, `transcript-panel.tsx`

**Declarations**

`caption-items.ts`:
- From `caption-inspector.tsx`:
  - types L15–35 and L81–94 (`CaptionStylePreset`, `CaptionPlacement`, `CaptionMotionPreset`, `CaptionWordTiming`, `CaptionWordEasing`, `CaptionWordAnimationPreset`, `CaptionWordAnimation`, `CaptionBuildRange`, `CaptionBuildOptions`),
  - `captionWordTokens` L37–39, `captionWordTimings` L41–79, `captionStyleDetails` L96–121, `captionStyleProperties` L123–134, `buildCaptionItems` L136–200.
- From `editor-workspace.tsx`: `captionBuildRangeForSourceItem` L2852–2868, `captionRepairActionForItem` L1397–1418.

`transcript-drafts.ts`: from `transcript-panel.tsx`: type `TranscriptWordRepairInput` L5–11, `formatSecondsInput` L26–28, `canonicalDrafts` L30–41, `repairMatchesWord` L43–45.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Move the existing pure tests.** Move the two `buildCaptionItems` tests verbatim into `src/lib/captions/caption-items.test.ts`:
  - `it("builds timed caption cues with a shared group and visual contract")`, starting L273 of `caption-inspector.test.tsx`,
  - `it("maps transcript timing through a trimmed playback-rate range")`, starting L319.

  Keep their fixture setup and import them from `@/components/workspace/caption-inspector` for now.
- [ ] **Step 3: Add characterization tests**
  - `captionWordTokens` on `"  one  two\tthree\n"` and `""`.
  - `captionWordTimings` for an item with word timings, without them, and with a mismatched word count.
  - `captionStyleDetails` and `captionStyleProperties` for every `CaptionStylePreset`.
  - `captionBuildRangeForSourceItem` for a trimmed item at playback rate 1.5.
  - `captionRepairActionForItem` for an unchanged text, a changed text, and a non-caption item, under `vi.useFakeTimers()`.
  - `canonicalDrafts` and `repairMatchesWord` on a four-word transcript with one edited word.
- [ ] **Step 4: Record and review:** `rtk pnpm vitest run src/lib/captions -u`.
- [ ] **Step 5: Move the code; retarget imports** (tests and UI) to `@/lib/captions/*`. Delete the moved `it` blocks from `caption-inspector.test.tsx`.
- [ ] **Step 6: Verify:** `rtk pnpm vitest run src/lib/captions src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 7: Commit**

```bash
rtk git add src/lib/captions src/components/workspace/caption-inspector.tsx src/components/workspace/caption-inspector.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/transcript-panel.tsx
rtk git commit -m "refactor(lib): extract caption item and transcript draft helpers"
```

---

### Task 8: Generation types and provider rules

**Files:**
- Create: `src/lib/generation/types.ts`, `provider-rules.ts`, `provider-rules.test.ts`
- Modify: `media-bin.tsx`, `editor-workspace.tsx`, `source-clip-inspector.tsx`

**Declarations**

`types.ts`:
- From `media-bin.tsx`: `MediaGenerationMode` L52, `MediaGenerationTimelineTarget` L59–62, `MediaGenerationTimelineSourceRange` L63–67, `MediaGenerationTimelineTargets` L68–70, `MediaGenerationRequest` L118–138, `GenerationModel` L140, `GenerationModelOption` L141–181, `GenerationModelCatalog` L182–184, `GenerationReferencePromptTag` L186–190, `GenerationCreditRateTable` L1241, `AudioGenerationPricing` L1418–1421, `GenerationResolutionOption` L1509–1515, `GenerationDurationOption` L1629–1632, `GenerationDurationBounds` L1634–1638, `GenerationReferenceLimits` L2895–2903, `TypedGenerationReferenceMediaRefs` L2905–2907.
- From `source-clip-inspector.tsx`: `SourceClipUpscaleContext` L57–61, `SourceClipVideoAudioKind` L63, `SourceClipGenerationContext` and `SourceClipVideoAudioContext` L66–74.

Move the types first, in a type-only commit step. Update every `import type` in the UI files and in `*.test.tsx` files that import them. Find them with `rtk rg -n "MediaGenerationRequest|GenerationModelOption|MediaGenerationMode" src`.

`provider-rules.ts`: from `media-bin.tsx`:
- constants L73–109, `mockCompletableFalModelIds` L75–79, `generationProviderDisplayNames` L287–295, `defaultGenerationModels` L996–1000, `generationModelOptions` L1002–1041, `defaultGenerationModelValues` L1043–1047, `organizeMediaPrompt` L111–116,
- `generationProviderDisplayName` L297–300,
- `generationModelValue`, `generationRequestModel`, `generationModelOptionsFromCatalog`, `generationModelFromValue`, `selectedGenerationModel`, `isGenerationModelValueForMode`, `generatedAssetHasRerunnableModel`, `generationModelLabel`, `canCompleteWithMockWorker` L1049–1145,
- `generationModeLabel` L980–982, `generationModeFromAsset` L1475–1493,
- all capability predicates L2384–2589 and L2736–2838,
- `videoInputAudioCategory` L2484–2486, `validGenerationTimelineSourceRange` L2488–2510,
- prompt rules L2640–2734,
- `timelineTargetMode` L348–350, `timelineTargetKindLabel` L352–354.

Consolidate model ID constants duplicated in `editor-workspace.tsx` (`falWanVideoToVideoModelId` L680; upscale IDs near L2700) and in `source-clip-inspector.tsx` (L261) onto the `provider-rules.ts` exports.

**Characterization inputs:**
- `modes = ["image", "video", "audio"] as const`.
- `options = generationModelOptions` (the fallback lists) plus `generationModelOptionsFromCatalog(catalog)` for:
  - a catalog whose `uiCapabilities` override one video and one audio model. Take the shape from `GenerationModelOption` and `generationModelCatalogFromPayload` tests in `media-bin.test.tsx`; search for `uiCapabilities`.
  - `null`.
- The matrix test `for mode of modes, for option of options[mode]: record { value, label, displayProvider, [every predicate](mode, option), minPromptLength, placeholder, agentModePrompt, readiness("") , readiness("a short prompt") }` into one `toMatchInlineSnapshot()` per mode.
- `validGenerationTimelineSourceRange` with ranges: valid, zero-length, and with `null` bounds.
- `generationModeFromAsset` and `generatedAssetHasRerunnableModel` on the fixture generated asset plus variants with `kind` `image`, `video` and `audio`, and an unknown provider.

- [ ] **Step 1: Move types (type-only).** Create `types.ts` with the declarations copied verbatim and export them. Replace them in the source files with `import type { … } from "@/lib/generation/types"`, then run `rtk pnpm lint`. Then commit:

```bash
rtk git add src/lib/generation/types.ts src/components/workspace
rtk git commit -m "refactor(lib): move generation request and model types to lib"
```
- [ ] **Step 2: Export in place** the `provider-rules` declarations.
- [ ] **Step 3: Write the matrix characterization test** in `src/lib/generation/provider-rules.test.ts`.
- [ ] **Step 4: Record and review:** `rtk pnpm vitest run src/lib/generation/provider-rules.test.ts -u`. Check the recorded matrix against `src-tauri/src/generation/capabilities.rs` for three models, and note mismatches in the commit body.
- [ ] **Step 5: Move and consolidate.**
- [ ] **Step 6: Verify:** `rtk pnpm vitest run src/lib/generation src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/source-clip-inspector.test.tsx && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 7: Commit**

```bash
rtk git add src/lib/generation src/components/workspace/media-bin.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/source-clip-inspector.tsx
rtk git commit -m "refactor(lib): extract generation provider rules and capability predicates"
```

---

### Task 9: Generation settings options, pricing, references

**Files:**
- Create: `src/lib/generation/settings-options.ts`, `pricing.ts`, `references.ts` and their tests
- Modify: `media-bin.tsx`

**Declarations**

- `settings-options.ts`: from `media-bin.tsx` L1495–2638, every helper listed under "settings-options" in the extraction inventory:
  - duration: `generationDurationValue` through `selectedBoundedGenerationDurationValue`,
  - aspect, resolution and quality helpers,
  - `selectedGenerationSettings` L2139–2319,
  - image counts L2321–2382,
  - voices L2591–2638,
  - constants L1640–1734 and L1885–2011.
- `pricing.ts`: from `media-bin.tsx`:
  - `selectedGenerationCost` L1174–1230, `formatGenerationCreditEstimate` L1232–1239, `mockGenerationCreditBalance` L110,
  - `generationPricingKey`, `resolveGenerationRate`, `ceilGenerationCredits` L1243–1262,
  - rate tables and cost functions L1264–1473.
- `references.ts`: from `media-bin.tsx`:
  - `generationReferencePromptTags` L383–410, `trailingReferenceTagQuery` L412–415, `promptWithInsertedReferenceTag` L417–422,
  - `isVisualMediaAsset`, `isFrameReferenceMediaAsset`, `isSourceVideoMediaAsset` L604–614,
  - `generationReferenceMediaForModel` L2840–2854, `typedGenerationReferenceMediaRefs` L2856–2893,
  - `generationModelReferenceLimits` L2909–3052, `referenceLimitMessage` L3054–3068, `generationReferenceLimitMessage` L3070–3123, `referenceDurationLimitMessage` L3125–3141.

**Characterization inputs:** for every `(mode, option)` pair from `generationModelOptions`:
- **Settings:** for each option, record
  - its duration options, bounds, aspect options, resolution options, quality options, voice options and max images;
  - `selectedGenerationSettings`, called with the first valid value of every option (or `null`/`""`), `generateAudio` true and false, `instrumental` false, `voice` `""`, `lyrics` `"la la"`, style `"warm"` and the first quality value.
- **Pricing:** `selectedGenerationCost` with:
  - duration equal to the first and the last duration option,
  - resolution equal to the first and the last resolution,
  - image counts 1 and max,
  - prompts `""` and `"x".repeat(1200)`.

  Also `formatGenerationCreditEstimate` for `0`, `12`, `12.5` and `"varies"`.
- **References:**
  - media lists of 0, 1 and 12 images, one video of 30 s, one audio of 400 s, and mixed media;
  - `generationReferenceLimitMessage` for each option at those lists;
  - `generationReferencePromptTags` for mixed media;
  - `trailingReferenceTagQuery` for `"make @in"`, `"@"`, `"email a@b.c"`, `""`;
  - `promptWithInsertedReferenceTag` for each of those queries.

Record the settings, pricing and reference matrices as one inline snapshot per mode. Split them further if a snapshot exceeds 400 lines.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Write the characterization matrices.**
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/generation -u`.
- [ ] **Step 4: Move the code.** Import chains:
  - `settings-options` imports `provider-rules` and `types`,
  - `pricing` imports `settings-options`, `provider-rules` and `types`,
  - `references` imports `provider-rules` and `types`.
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/generation src/components/workspace/media-bin.test.tsx && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/generation src/components/workspace/media-bin.tsx
rtk git commit -m "refactor(lib): extract generation settings, pricing, and reference rules"
```

---

### Task 10: Generation catalog, requests, assets, timeline placement

**Files:**
- Create: `src/lib/generation/catalog.ts`, `requests.ts`, `assets.ts`, `timeline-placement.ts` and their tests
- Modify: `editor-workspace.tsx`, `media-bin.tsx`, `source-clip-inspector.tsx`, `project-timeline-inspector.tsx`, `agent-panel.tsx`

**Declarations**

`catalog.ts`: from `editor-workspace.tsx`: `isMediaGenerationMode` L3474–3476, `isGenerationCatalogKind` L3478–3482, `generationModelCatalogFromPayload` L3484–3538.

`requests.ts`: from `editor-workspace.tsx`: prompt constants L676–679, `upscaleGenerationRequest` L2694–2746, `videoAudioGenerationRequest` L2748–2806, `generatedVariationId` L1420–1422, `generatedVariationSetId` L1424–1426, `generatedMediaAssetId` L1428–1430.

`assets.ts`:
- From `editor-workspace.tsx`: `generatedAssetForTimelineItem` L3089–3111, `uniqueStringValues` L2990–2992, `generatedReferenceMediaIds` L2994–3001.
- From `source-clip-inspector.tsx`: `findGeneratedAssetForItem` L637–657 (compare with `generatedAssetForTimelineItem`), `generatedAssetHasMedia`, `findGeneratedAssetForMedia`, `findGeneratedOutputForItem`, `findGeneratedOutputForMedia`, `findMediaAsset` L659–716, and `generatedAssetTitle` L340–352 as `generatedAssetTitleWithPrompt`.
- From `media-bin.tsx`:
  - `findGeneratedAssetForOutput` L935–944,
  - `generatedAssetTitle` L818–820 as `generatedAssetTitleOrPrompt`,
  - `generatedReferenceLabel`, `generatedLineageLabel`, `generatedPendingOutputLabel`, `isActiveGeneratedAsset`, `generatedOutputHasProviderSourceUrl`, `generatedAssetNeedsHistoryCard`, `humanizeWorkflowToken`, `generatedWorkflowLabel` L871–933,
  - `generatedAssetCreatedAtMs`, `sortGeneratedAssetsByCreatedAtDesc` L788–816,
  - `formatGeneratedResolution`, `formatGeneratedDuration`, `generatedOutputFileLabel`, `generatedOutputMeta` L946–978.
- From `project-timeline-inspector.tsx`: `generatedAssetTitle` L321–323 as `generatedAssetTitleOrId`, `generatedReferenceCount`, `generatedModelLabel` L325–335, `promptExcerpt` L382–385, `recentGeneratedAssets` L387–391.
- From `agent-panel.tsx`: `selectedGeneratedClipSettingsLabels` L541–574.

`timeline-placement.ts`: from `editor-workspace.tsx`:
- `generatedAssetPlacementIntent` L2016–2020, `generatedAssetPlacementContextLabel` L2022–2038 (compare with `media-bin.tsx` `generatedAssetPlacementSearchLabel` L641–655 and `project-timeline-inspector.tsx` `generatedPlacementLabel` L337–347),
- `generatedReplacementPlacementIntent` L2070–2074 (compare with `source-clip-inspector.tsx` `generatedComposerPlacementForItem` L281–285),
- `timelineGenerationTrackForKind`, `timelineGenerationTargetForKind` L2076–2107, `timelineGenerationTargets` L2398–2406,
- `mediaGenerationTargetKind` L2109–2111, `generatedTimelineStartSecondsFromSettings` L2113–2126,
- `timelineItemAcceptsGeneratedOutputMedia` L2128–2138, `generatedOutputMediaType` L2142–2166 and type `GeneratedOutputMediaType` L2140,
- `generatedTimelinePlaceholderItemId` L2006–2014, `generatedTimelinePlaceholderAction` L2168–2223, `generatedTimelinePlaceholderForAsset` L2225–2242,
- `generatedOutputTimelineItemId` L1956–1965, `generatedTimelineOutputItem` L2244–2272, `generatedTimelineOutputLabel` L2274–2279, `generatedOutputTimelineActions` L2281–2396,
- `timelineGenerationSourceRange` L2508–2537, `timelineVisualSourceEndSeconds` L2539–2548, `timelineHasVisualSourceInRange` L2550–2566, `isVisualTimelineSourceItem` L2568–2575,
- `mediaTimelineAction` L2577–2626.

**Characterization inputs** (fake timers fixed at `2026-09-13T00:00:00Z`):
- **Catalog payload:** enabled models, a disabled model (via preferences), an unknown kind, and `null`.
- **Requests:**
  - `upscaleGenerationRequest` for image and video media, with and without context;
  - `videoAudioGenerationRequest` for both kinds.
- **Asset lookups:**
  - generated assets in states pending, completed, failed and replaced-output, plus one with references;
  - `sortGeneratedAssetsByCreatedAtDesc` with equal timestamps and invalid dates.
- **Placement and actions:**
  - `generatedTimelinePlaceholderAction` for a video request, an audio request, and a request with a `replace` placement intent;
  - `generatedOutputTimelineActions` for a project with a placeholder, one without, and output video media that has audio;
  - `timelineGenerationSourceRange` for a null range, a range over visual items, and a range over a gap;
  - `mediaTimelineAction` for video, audio, image and a missing media ID.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Write characterization tests.**
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/generation -u`.
- [ ] **Step 4: Move and consolidate.** Keep the three title variants under their distinct names.
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/generation src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/generation src/components/workspace/editor-workspace.tsx src/components/workspace/media-bin.tsx src/components/workspace/source-clip-inspector.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/agent-panel.tsx
rtk git commit -m "refactor(lib): extract generation catalog, requests, asset lookups, and timeline placement"
```

---

### Task 11: Jobs, Temporal fallback, export profiles, render report

**Files:**
- Create: `src/lib/jobs/activity-records.ts`, `ids.ts`, `temporal-fallback.ts`, `src/lib/export/profiles.ts`, `render-report.ts` and their tests
- Modify: `activity-panel.tsx`, `activity-panel.test.tsx`, `project-timeline-inspector.tsx`, `editor-workspace.tsx`, `editor-workspace.test.tsx`, `export-job-start-request.test.ts` (delete after moving), `export-sheet.tsx`, `render-report-panel.tsx`

**Declarations**

`activity-records.ts`:
- From `activity-panel.tsx`: type `ActivityJobRecord` L12–20, `nonEmpty` L38–41, `embeddedCodexProposal` L43–76, `buildActivityJobRecords` L78–120, `formatJobKind` L122–128 as `formatJobKindTitleCase`, `formatJobStatus` L130–132.
- From `project-timeline-inspector.tsx`:
  - `formatJobKind` L134–143 as `formatJobKindSentenceCase`,
  - `activeJobCount`, `activeGenerationJobCount` L120–132,
  - `latestRenderReport`, `latestExportArtifact`, `recentExportArtifacts` L106–118,
  - `startRequestStatus` L215–238, `canShowStartWorkflowAction` L240–242, `startRequestStringInput`, `startRequestValidationInput`, `validationString` L244–259, `startRequestVideoValidationSummary`, `startRequestAudioValidationSummary` L261–284,
  - `projectResolutionLabel`, `projectFrameRateLabel`, `projectAspectRatioLabel` L91–104.
- From `editor-workspace.tsx`: `mergeProjectJobs` L1539–1551, `latestFailedRenderJob` L1453–1460.

`ids.ts`: from `editor-workspace.tsx` L1436–1477: `generatedCodexEditJobId`, `generatedNleExportJobId`, `generatedRenderJobId`, `generatedRenderAttemptId`, `generatedSaveRangeJobId`, `generatedMediaExportJobId`, `generatedTranscribeMediaJobId`.

`temporal-fallback.ts`: from `editor-workspace.tsx`:
- `TemporalWorkflowKind` L704–709, `temporalWorkflowDefinitions` L713–759, `getFallbackTemporalWorkflowDefinition` L763–765,
- `temporalIdSegment` L692–700, `mockTemporalRunId` L682–690,
- `isTemporalWorkerEnvironmentReport` L579–598, `exportJobWithStartRequest` L426–436, `buildTemporalJobSummary` L1841–1877,
- `buildFallbackGenerateMediaStartRequest` L1879–1914, `buildFallbackTranscribeMediaStartRequest` L1916–1940, `buildFallbackTemporalStartResultAction` L1942–1954.

`profiles.ts`:
- From `editor-workspace.tsx`: `InProcessExportProfile` L406, `inProcessExportLabel` L408–420, `fallbackExportProfileAvailability` L600–673, `mediaExportOutputPath` L1479–1486, `exportProfileById` L1488–1498, `exportProfileDisabledReason` L1500–1516, `exportPolicyStatusLabel` L1518–1529, `exportProfileRuntimeDetail` L1531–1537.
- From `export-sheet.tsx`: types L19–54, `resolutionOptions` L96–106, `defaultResolutionForQuality` L108–117, `dimensionsForResolution` L119–131, `evenDimension` L133–136, `draftExportDimensions` L138–144, `profileQualityAvailability` L146–153, `isCapabilityCheckedVideoProfile` L155–157, `firstProfile` L159–165, `firstDraftVideoProfile` L167–178, `formatDurationTimecode` L180–194, `formatEstimatedSize` L196–204.

`render-report.ts`:
- From `render-report-panel.tsx`: `deliveryQualityLabel` L28–32, `qaMetricEntries` L38–40, `normalizeArtifactPath` L42–44, `artifactBackedFrameCount` L46–54, `formatTimelineSeconds` L56–58, `formatMismatchRatio` L60–62.
- From `project-timeline-inspector.tsx`: the copies at L52–58, `renderCheckLabel` L177–183, `renderCheckEntries` L185–187.

**Existing tests to move verbatim:**
- `activity-panel.test.tsx` `it("joins only persisted project evidence and orders every project job by update time")` (L132) → `src/lib/jobs/activity-records.test.ts`.
- All of `export-job-start-request.test.ts` → `src/lib/jobs/temporal-fallback.test.ts`. Delete the old file.
- `editor-workspace.test.tsx` `it("uses canonical Temporal transcribe media fallback metadata")` (L1673) → `temporal-fallback.test.ts`.

**Characterization inputs:**
- **Jobs:** jobs across every status and kind, including jobs with and without `startRequest`, a mismatched start request, an embedded proposal, and an unresolved proposal ID.
- **`buildTemporalJobSummary`:** mock `@/lib/project` `requestTemporalJobSummary` to resolve, and to reject with `BackendUnavailableError`.
- **Export profiles:** every profile in `fallbackExportProfileAvailability`, crossed with `canExportMediaProfiles` true and false and `temporalExecution` true and false.
- **Dimensions:** `draftExportDimensions` for 1920×1080, 1080×1920, 1001×777 and 0×0.
- **Formatting:** `formatEstimatedSize` for 0, 1023, 1 MiB, 1.5 GiB and NaN. `formatDurationTimecode` for the Task 2 seconds list.
- **Render report:** payloads with no report, a passing report, and a report with frame mismatches.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Move the existing pure tests; write characterization tests.**
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/jobs src/lib/export -u`.
- [ ] **Step 4: Move and consolidate.**
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/jobs src/lib/export src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/jobs src/lib/export src/components/workspace/activity-panel.tsx src/components/workspace/activity-panel.test.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/export-job-start-request.test.ts src/components/workspace/export-sheet.tsx src/components/workspace/render-report-panel.tsx
rtk git commit -m "refactor(lib): extract job records, temporal fallbacks, export profiles, and render report helpers"
```

---

### Task 12: Agent proposal model, materialization, merge, selection context, transcript storage

**Files:**
- Create: `src/lib/agent/proposal-review-model.ts`, `proposal-materialize.ts`, `project-merge.ts`, `selection-context.ts`, `chat-transcript-storage.ts` and their tests
- Modify: `codex-proposal-review.tsx`, `codex-proposal-review.test.tsx`, `editor-workspace.tsx`, `agent-panel.tsx`

**Declarations**

`proposal-review-model.ts`:
- From `codex-proposal-review.tsx`: model types L9–103, `visualRoles` L105–113, `visualRoleByKind` L115–127, `requiredVisualFields` L129–138, helpers L140–377 (`unknownRecord`, `nonEmptyString`, `finiteNumber`, `firstFiniteNumber`, `normalizedDimensions`, `normalizedAlpha`, `normalizedRole`, `humanRole`, `joinHumanList`, `pathIssue`, `normalizeVisualCollection`, `validateEdl`, `normalizeProjectActions`), `buildCodexProposalReviewModel` L379–419, `formatTimestamp` L421–431 as `formatProposalTimestamp`, `renderCriteria` L459–469.
- From `editor-workspace.tsx`: `codexProposalTranscriptSummary` L437–443, `codexProposalMomentSummaries` L445–452.

`proposal-materialize.ts`: from `editor-workspace.tsx`: `materializeCodexProposalActions` L1604–1673, `timelineTrackIdForKind` L1675–1681, `pushAddItemsAction` L1683–1692, `materializeCodexCaptionItem`, `materializeCodexOverlayItem`, `materializeCodexHyperframeItem` L1694–1782, `stringValue`, `numberValue`, `copyStringProperty`, `copyNumberProperty`, `visibleProposalText` L1790–1839. `unknownRecord` at L1784–1788 duplicates the proposal-review-model copy and is consolidated.

`project-merge.ts`: from `editor-workspace.tsx`: `applyProjectActionsLocally` L1594–1602, `mergeCodexProjectMetadata` L1553–1570, `mergeProjectMediaAnalysis` L1572–1592, `projectHistorySnapshot` L2982–2984, `projectSnapshotsEqual` L2986–2988.

`selection-context.ts`:
- From `agent-panel.tsx`: types L56–118 and L174–181 (`AgentSelectedReferenceContext`, `AgentSelectedMediaContext`, `AgentSelectedTimelineClipContext`, `AgentSelectedTimelineRangeContext`, `AgentMentionTarget`), `codexToolDisplayName` L458–475.
- From `editor-workspace.tsx`: `agentSelectedMediaContext` L3003–3087, `agentSelectedGeneratedReferences` L3205–3240, `agentSelectedTimelineClipContext` L3242–3323, `agentMentionTargets` L3389–3439, `trackForTimelineItem` L3441–3447, `AgentSetupClient` L711, `projectFolderLabel` L966–969, `agentSetupSnippet` L1035–1074, `projectProfileInitial` L1432–1434.

`chat-transcript-storage.ts`: from `agent-panel.tsx` L293–367: `codexChatTranscriptStorageKey`, `maxStoredTranscriptEntries`, `scopedCodexChatTranscriptStorageKey`, `browserStorage`, `isTranscriptEntry`, `loadStoredTranscriptEntries`, `saveStoredTranscriptEntries`, `clearStoredTranscriptEntries`. The transcript entry type moves with them.

**Existing tests to move verbatim** from `codex-proposal-review.test.tsx` into `proposal-review-model.test.ts`:
- `it("preserves each clip's resolved media identity in a multi-source EDL")` (L114),
- `it.each("requires an explicit role or kind for %s")` (L213),
- `it("maps supported Rust visual kinds into approved review roles")` (L245).

`it("awaits Rust validation before enabling proposal application")` (L90) stays in the UI test, because it renders.

**Characterization inputs:**
- **Proposal fixtures:** reuse those in `codex-proposal-review.test.tsx` and `editor-workspace.test.tsx`. Search for `CodexEditProposal` literals. Probe `materializeCodexProposalActions` with:
  - a proposal with captions, overlays and hyperframes,
  - a proposal targeting a missing track kind (expect a throw; record via `expect(() => …).toThrowErrorMatchingInlineSnapshot()`),
  - an empty proposal.
- **Merge:** `mergeCodexProjectMetadata` and `mergeProjectMediaAnalysis` with disjoint and overlapping metadata.
- **Selection context:** `agentSelectedMediaContext` and `agentSelectedTimelineClipContext` for every media and item in the fixture project, plus a generated clip.
- **Mentions:** `agentMentionTargets` for the fixture project.
- **Setup snippet:** `agentSetupSnippet` for every `AgentSetupClient`.
- **Transcript storage:** use jsdom `localStorage`:
  - save 0, 1 and `maxStoredTranscriptEntries + 5` entries, then load them;
  - load corrupt JSON, and an entry missing fields;
  - clear.

- [ ] **Step 1: Export in place.**
- [ ] **Step 2: Move the pure tests; write characterization tests.**
- [ ] **Step 3: Record and review:** `rtk pnpm vitest run src/lib/agent -u`.
- [ ] **Step 4: Move and consolidate.**
- [ ] **Step 5: Verify:** `rtk pnpm vitest run src/lib/agent src/components/workspace && rtk pnpm lint && rtk pnpm check:unused`.
- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/agent src/components/workspace/codex-proposal-review.tsx src/components/workspace/codex-proposal-review.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/agent-panel.tsx
rtk git commit -m "refactor(lib): extract agent proposal model, materialization, merges, and selection context"
```

---

### Task 13: Keymap registry

**Files:**
- Create: `src/lib/keymap.ts`, `src/lib/keymap.test.ts`

The registry is the single source for handlers, tooltips, menus and the shortcuts sheet in the new editor. In this plan it is data plus pure matching, with no UI wiring. Its test keeps it knip-reachable.

- [ ] **Step 1: Write the failing test**

```ts
// src/lib/keymap.test.ts
import { describe, expect, it } from "vitest";
import {
  editorShortcuts,
  formatShortcut,
  matchShortcut,
  shortcutById,
  type ShortcutPlatform,
} from "./keymap";

function keyEvent(init: Partial<KeyboardEventInit> & { key: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", init);
}

describe("keymap", () => {
  it("has unique ids and unique bindings per scope", () => {
    const ids = editorShortcuts.map((shortcut) => shortcut.id);
    expect(new Set(ids).size).toBe(ids.length);
    const bindings = editorShortcuts.flatMap((shortcut) =>
      shortcut.bindings.map((binding) => `${shortcut.scope}:${binding}`),
    );
    expect(new Set(bindings).size).toBe(bindings.length);
  });

  it("preserves the existing timeline and preview shortcuts", () => {
    expect(shortcutById("timeline.selectTool").bindings).toEqual(["V"]);
    expect(shortcutById("timeline.bladeTool").bindings).toEqual(["C"]);
    expect(shortcutById("timeline.split").bindings).toEqual(["S", "Mod+K"]);
    expect(shortcutById("timeline.markIn").bindings).toEqual(["I"]);
    expect(shortcutById("timeline.markOut").bindings).toEqual(["O"]);
    expect(shortcutById("timeline.delete").bindings).toEqual(["Delete", "Backspace"]);
    expect(shortcutById("timeline.rippleDelete").bindings).toEqual(["Shift+Delete", "Shift+Backspace"]);
    expect(shortcutById("timeline.trimStart").bindings).toEqual(["["]);
    expect(shortcutById("timeline.trimEnd").bindings).toEqual(["]"]);
    expect(shortcutById("playback.toggle").bindings).toEqual(["Space"]);
  });

  it("adds global undo, redo, export, shortcuts, and tab bindings", () => {
    expect(shortcutById("editor.undo").bindings).toEqual(["Mod+Z"]);
    expect(shortcutById("editor.redo").bindings).toEqual(["Shift+Mod+Z"]);
    expect(shortcutById("editor.export").bindings).toEqual(["Mod+E"]);
    expect(shortcutById("editor.shortcuts").bindings).toEqual(["Mod+/"]);
    expect(shortcutById("editor.tab.ai").bindings).toEqual(["Mod+1"]);
    expect(shortcutById("editor.tab.effects").bindings).toEqual(["Mod+6"]);
  });

  it("matches events using the platform modifier", () => {
    const mac: ShortcutPlatform = "macos";
    const linux: ShortcutPlatform = "linux";
    expect(matchShortcut(keyEvent({ key: "z", metaKey: true }), "global", mac)?.id).toBe("editor.undo");
    expect(matchShortcut(keyEvent({ key: "z", ctrlKey: true }), "global", linux)?.id).toBe("editor.undo");
    expect(matchShortcut(keyEvent({ key: "z", ctrlKey: true }), "global", mac)).toBeNull();
    expect(matchShortcut(keyEvent({ key: "Z", metaKey: true, shiftKey: true }), "global", mac)?.id).toBe("editor.redo");
    expect(matchShortcut(keyEvent({ key: "s" }), "timeline", linux)?.id).toBe("timeline.split");
    expect(matchShortcut(keyEvent({ key: " " }), "timeline", linux)?.id).toBe("playback.toggle");
  });

  it("formats bindings per platform", () => {
    expect(formatShortcut("Shift+Mod+Z", "macos")).toBe("⇧⌘Z");
    expect(formatShortcut("Shift+Mod+Z", "linux")).toBe("Ctrl+Shift+Z");
    expect(formatShortcut("Space", "linux")).toBe("Space");
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `rtk pnpm vitest run src/lib/keymap.test.ts`
Expected: FAIL with "Cannot find module './keymap'".

- [ ] **Step 3: Before implementing, confirm the existing bindings**

Read `timeline-editor.tsx` near L2500–2563 and the keyboard handler, and the `preview-panel.tsx` keyboard handler. Confirm the existing bindings above: V, C, S, Mod+K, I, O, Delete/Backspace, Shift+Delete, Mod+D/C/X/V, Shift+Mod+V, arrows, Home/End, PageUp/PageDown, `[`, `]`, Esc, Space. If a binding differs, the code is the source of truth. Update the test expectation to match the code and note it in the commit body.

- [ ] **Step 4: Implement**

```ts
// src/lib/keymap.ts
export type ShortcutPlatform = "macos" | "linux" | "windows";
export type ShortcutScope = "global" | "timeline" | "preview";

export interface EditorShortcut {
  readonly id: string;
  readonly label: string;
  readonly group: "Editing" | "Timeline" | "Playback" | "Navigation" | "Panels";
  readonly scope: ShortcutScope;
  /** Bindings like "Mod+K", "Shift+Delete", "Space". "Mod" is ⌘ on macOS, Ctrl elsewhere. */
  readonly bindings: readonly string[];
}

export const editorShortcuts: readonly EditorShortcut[] = [
  { id: "editor.undo", label: "Undo", group: "Editing", scope: "global", bindings: ["Mod+Z"] },
  { id: "editor.redo", label: "Redo", group: "Editing", scope: "global", bindings: ["Shift+Mod+Z"] },
  { id: "editor.export", label: "Export", group: "Editing", scope: "global", bindings: ["Mod+E"] },
  { id: "editor.import", label: "Import media", group: "Editing", scope: "global", bindings: ["Mod+I"] },
  { id: "editor.shortcuts", label: "Keyboard shortcuts", group: "Panels", scope: "global", bindings: ["Mod+/"] },
  { id: "editor.tab.ai", label: "AI", group: "Panels", scope: "global", bindings: ["Mod+1"] },
  { id: "editor.tab.media", label: "Media", group: "Panels", scope: "global", bindings: ["Mod+2"] },
  { id: "editor.tab.audio", label: "Audio", group: "Panels", scope: "global", bindings: ["Mod+3"] },
  { id: "editor.tab.text", label: "Text", group: "Panels", scope: "global", bindings: ["Mod+4"] },
  { id: "editor.tab.captions", label: "Captions", group: "Panels", scope: "global", bindings: ["Mod+5"] },
  { id: "editor.tab.effects", label: "Effects", group: "Panels", scope: "global", bindings: ["Mod+6"] },
  { id: "editor.clearSelection", label: "Clear selection", group: "Editing", scope: "global", bindings: ["Escape"] },
  { id: "timeline.selectTool", label: "Select tool", group: "Timeline", scope: "timeline", bindings: ["V"] },
  { id: "timeline.bladeTool", label: "Blade tool", group: "Timeline", scope: "timeline", bindings: ["C"] },
  { id: "timeline.split", label: "Split at playhead", group: "Timeline", scope: "timeline", bindings: ["S", "Mod+K"] },
  { id: "timeline.markIn", label: "Mark in", group: "Timeline", scope: "timeline", bindings: ["I"] },
  { id: "timeline.markOut", label: "Mark out", group: "Timeline", scope: "timeline", bindings: ["O"] },
  { id: "timeline.delete", label: "Delete", group: "Timeline", scope: "timeline", bindings: ["Delete", "Backspace"] },
  { id: "timeline.rippleDelete", label: "Ripple delete", group: "Timeline", scope: "timeline", bindings: ["Shift+Delete", "Shift+Backspace"] },
  { id: "timeline.duplicate", label: "Duplicate", group: "Timeline", scope: "timeline", bindings: ["Mod+D"] },
  { id: "timeline.copy", label: "Copy", group: "Timeline", scope: "timeline", bindings: ["Mod+C"] },
  { id: "timeline.cut", label: "Cut", group: "Timeline", scope: "timeline", bindings: ["Mod+X"] },
  { id: "timeline.paste", label: "Paste", group: "Timeline", scope: "timeline", bindings: ["Mod+V"] },
  { id: "timeline.pasteInsert", label: "Paste insert", group: "Timeline", scope: "timeline", bindings: ["Shift+Mod+V"] },
  { id: "timeline.trimStart", label: "Trim start to playhead", group: "Timeline", scope: "timeline", bindings: ["["] },
  { id: "timeline.trimEnd", label: "Trim end to playhead", group: "Timeline", scope: "timeline", bindings: ["]"] },
  { id: "timeline.nudgeLeft", label: "Nudge left", group: "Timeline", scope: "timeline", bindings: ["ArrowLeft"] },
  { id: "timeline.nudgeRight", label: "Nudge right", group: "Timeline", scope: "timeline", bindings: ["ArrowRight"] },
  { id: "navigation.previousEdit", label: "Previous edit point", group: "Navigation", scope: "timeline", bindings: ["PageUp"] },
  { id: "navigation.nextEdit", label: "Next edit point", group: "Navigation", scope: "timeline", bindings: ["PageDown"] },
  { id: "navigation.start", label: "Go to start", group: "Navigation", scope: "timeline", bindings: ["Home"] },
  { id: "navigation.end", label: "Go to end", group: "Navigation", scope: "timeline", bindings: ["End"] },
  { id: "playback.toggle", label: "Play / pause", group: "Playback", scope: "timeline", bindings: ["Space"] },
  { id: "preview.toggle", label: "Play / pause", group: "Playback", scope: "preview", bindings: ["Space"] },
  { id: "preview.stepBack", label: "Previous frame", group: "Playback", scope: "preview", bindings: ["ArrowLeft"] },
  { id: "preview.stepForward", label: "Next frame", group: "Playback", scope: "preview", bindings: ["ArrowRight"] },
  { id: "preview.start", label: "Go to start", group: "Playback", scope: "preview", bindings: ["Home"] },
  { id: "preview.end", label: "Go to end", group: "Playback", scope: "preview", bindings: ["End"] },
];

const byId = new Map(editorShortcuts.map((shortcut) => [shortcut.id, shortcut]));

export function shortcutById(id: string): EditorShortcut {
  const shortcut = byId.get(id);
  if (!shortcut) throw new Error(`Unknown shortcut: ${id}`);
  return shortcut;
}

interface ParsedBinding {
  readonly key: string;
  readonly mod: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

function parseBinding(binding: string): ParsedBinding {
  const parts = binding.split("+");
  const key = parts[parts.length - 1] ?? "";
  return {
    key: key === "Space" ? " " : key.length === 1 ? key.toLowerCase() : key,
    mod: parts.includes("Mod"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
  };
}

function eventKey(event: KeyboardEvent): string {
  return event.key.length === 1 ? event.key.toLowerCase() : event.key;
}

export function matchShortcut(
  event: KeyboardEvent,
  scope: ShortcutScope,
  platform: ShortcutPlatform,
): EditorShortcut | null {
  const modPressed = platform === "macos" ? event.metaKey : event.ctrlKey;
  const otherModPressed = platform === "macos" ? event.ctrlKey : event.metaKey;
  if (otherModPressed) return null;
  for (const shortcut of editorShortcuts) {
    if (shortcut.scope !== scope) continue;
    for (const binding of shortcut.bindings) {
      const parsed = parseBinding(binding);
      if (
        parsed.key === eventKey(event) &&
        parsed.mod === modPressed &&
        parsed.shift === event.shiftKey &&
        parsed.alt === event.altKey
      ) {
        return shortcut;
      }
    }
  }
  return null;
}

const macSymbols: Record<string, string> = { Mod: "⌘", Shift: "⇧", Alt: "⌥" };

export function formatShortcut(binding: string, platform: ShortcutPlatform): string {
  const parts = binding.split("+");
  if (platform === "macos") {
    return parts.map((part) => macSymbols[part] ?? part).join("");
  }
  const order = ["Mod", "Shift", "Alt"];
  const modifiers = order.filter((modifier) => parts.includes(modifier));
  const key = parts.filter((part) => !order.includes(part));
  return [...modifiers.map((modifier) => (modifier === "Mod" ? "Ctrl" : modifier)), ...key].join("+");
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `rtk pnpm vitest run src/lib/keymap.test.ts && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS.

In the "matches events" test, `Shift+Mod+Z` is sent with `key: "Z"` and `eventKey` lowercases single characters, so it matches. The `ArrowLeft`/`ArrowRight` bindings are scoped separately for timeline and preview, so they don't collide. The uniqueness test is per scope.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/keymap.ts src/lib/keymap.test.ts
rtk git commit -m "feat(lib): add the editor keyboard shortcut registry"
```

---

### Task 14: Leftover audit and full gate

**Files:**
- Modify: any workspace UI file where Step 1 finds a leftover pure helper

- [ ] **Step 1: Audit for remaining top-level pure helpers**

Run:
```bash
rtk rg -n "^(export )?(function|const) [a-z][A-Za-z0-9]*" src/components/workspace --glob '!*.test.*'
```
For each remaining top-level declaration, decide whether it is presentational: JSX, class names, icons, React hooks, DOM event plumbing, tour or skills copy, storage for old layout panes, or the old-layout geometry listed in Task 4. If it is not presentational, move it with the characterization protocol into the closest module above. Record the final list of intentionally left UI helpers in the commit body.

- [ ] **Step 2: Run the full frontend gate**

Run: `rtk pnpm verify:frontend`
Expected: PASS, including `visual:qa:browser-release` with unchanged baselines, because this plan changes no visuals.

- [ ] **Step 3: Confirm no helper moved twice and no import cycles**

Run:
```bash
rtk rg -n "from \"@/components/workspace" src/lib
```
Expected: no output. Lib must not import UI.

- [ ] **Step 4: Commit (only if Step 1 moved anything)**

```bash
rtk git add src/lib src/components/workspace
rtk git commit -m "refactor(lib): move remaining pure workspace helpers to lib"
```

## Self-Review Checklist (for the plan executor)

- Every module in the File Map exists, has a test, and is imported by the old UI.
- No `src/lib/**` file imports from `src/components/**`.
- Characterization snapshots were recorded before the move and never edited afterwards.
- Duplicates were consolidated only where `toEqual` proved identity.
- `pnpm verify:frontend` passes with unchanged visual baselines.
