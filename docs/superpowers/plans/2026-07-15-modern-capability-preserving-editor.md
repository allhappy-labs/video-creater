# Modern Capability-Preserving Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize Video Creater into the approved dual-contextual-rail editor while keeping every current media, generation, timeline, inspector, Codex, workflow, render, and export capability reachable and validated.

**Architecture:** Keep `EditorWorkspace` as the canonical React state/callback owner and Rust as the canonical project mutation, proposal validation, render-plan, job, log, and artifact owner. Add pure information-architecture and layout-budget modules, compose existing workflow components through focused rail/drawer shells, and extract only presentation boundaries that currently force unrelated workflows into `EditorWorkspace` or `MediaBin`.

**Tech Stack:** React 19, TypeScript 5.7, Tailwind CSS 3.4, shadcn-style local UI primitives, `lucide-react`, Vitest, Testing Library, Playwright browser visual QA, Tauri 2.11, Rust.

## Global Constraints

- Always prefix shell commands with `rtk`.
- Use Conventional Commits.
- Preserve every capability in `docs/superpowers/specs/2026-07-14-modern-editor-information-architecture-design.md`.
- Keep the top bar approximately 52 pixels, the global rail approximately 64 pixels, the default context panel approximately 304 pixels, and the collapsed inspector rail approximately 52 pixels.
- Use responsive bands at 1600+, 1280-1599, 1024-1279, and below 1024 pixels without mutating saved user preferences.
- Keep the viewer and timeline protected; opening a drawer must not remove or overlap the timeline.
- Use `lucide-react` icons and accessible names/tooltips for icon-only controls.
- Rust remains the canonical project mutation, proposal validation, render-plan, cancellation, log, and artifact owner.
- Codex returns structured project actions; it must not write canonical project files directly.
- Generated edits must present a real `sourceIn`/`sourceOut` EDL before captions, titles, overlays, effects, or HyperFrames.
- Every generated visual layer must expose `visualTreatment`, `motion`, `safeZone`, and `avoid`.
- Errors must render at the owning surface; toasts may supplement but never replace actionable error UI.
- Preserve existing keyboard shortcuts, focus visibility, reduced-motion behavior, and status text independent of color.
- Do not add a rail destination merely because it exists in the CapCut reference; route only current Video Creater capabilities.
- Pin CI actions to immutable commit SHAs if any workflow file changes; this plan does not require workflow changes.

---

## Program Slices

The approved specification spans several independently reviewable surfaces. This plan keeps them in one ordered program while requiring every task to produce a working, testable state:

1. information architecture and preservation manifest;
2. responsive layout budget;
3. global rail and editor shell;
4. left destination ownership;
5. viewer and contextual canvas toolbar;
6. right inspector dock;
7. timeline dock and consolidated actions;
8. Codex and structured proposal review;
9. Activity, Render Review, and Export routing;
10. responsive/accessibility convergence;
11. visual QA, regression gates, and tracker evidence.

## File Structure

### New files

- `src/lib/editor-information-architecture.ts`: destination types, selection routing, and the capability-preservation manifest.
- `src/lib/editor-information-architecture.test.ts`: exhaustive route and selection tests.
- `src/lib/editor-layout-budget.ts`: pure responsive presentation resolver that never mutates saved preferences.
- `src/lib/editor-layout-budget.test.ts`: exact breakpoint and Codex/inspector budget tests.
- `src/components/workspace/editor-navigation-rail.tsx`: accessible global rail with source destinations and global actions.
- `src/components/workspace/editor-navigation-rail.test.tsx`: labels, selection, badges, keyboard navigation, and focus.
- `src/components/workspace/editor-shell.tsx`: top/context/viewer/inspector/timeline/Codex composition and resize boundaries.
- `src/components/workspace/editor-shell.test.tsx`: geometry landmarks and responsive presentation.
- `src/components/workspace/editor-source-panel.tsx`: one left context destination at a time.
- `src/components/workspace/editor-source-panel.test.tsx`: destination ownership and no-duplicate rendering.
- `src/components/workspace/text-library-panel.tsx`: Add Text and reusable text-treatment entry point.
- `src/components/workspace/text-library-panel.test.tsx`: creation and selected-text routing.
- `src/components/workspace/effect-catalog-panel.tsx`: executable effect catalog, selection target, applied list, and preparation status.
- `src/components/workspace/effect-catalog-panel.test.tsx`: catalog, target, apply, clear, and disabled reasons.
- `src/components/workspace/viewer-context-toolbar.tsx`: compact selection-aware canvas actions.
- `src/components/workspace/viewer-context-toolbar.test.tsx`: per-selection action visibility and accessible labels.
- `src/components/workspace/inspector-dock.tsx`: selection routing, collapsed rail, resizable drawer, and remembered destination.
- `src/components/workspace/inspector-dock.test.tsx`: destination matrices, multi-selection, locked state, and focus.
- `src/components/workspace/timeline-selection-toolbar.tsx`: existing selected-clip actions moved into the timeline toolbar.
- `src/components/workspace/timeline-selection-toolbar.test.tsx`: wide and overflow action reachability.
- `src/components/workspace/codex-drawer.tsx`: global replace/pin presentation around `AgentPanel`.
- `src/components/workspace/codex-drawer.test.tsx`: replacement, pinning, focus, and width-budget behavior.
- `src/components/workspace/codex-proposal-review.tsx`: approved Intent-to-Review sequence, EDL-first layers, Rust validation, and decisions.
- `src/components/workspace/codex-proposal-review.test.tsx`: EDL-first ordering, visual contract, Rust errors, and decisions.
- `src/components/workspace/activity-panel.tsx`: project-wide workflow/job source of truth.
- `src/components/workspace/activity-panel.test.tsx`: ordering, status, applicable actions, and output/report links.

### Modified files

- `src/lib/workspace-layout-state.ts`: preserve saved preferences while adopting context-panel width bounds and shell terminology.
- `src/lib/workspace-layout-state.test.ts`: migration, resize, presets, maximize, and persistence.
- `src/lib/responsive-rail-state.ts`: retain the existing below-1024 one-pane switcher and add explicit drawer-close transitions.
- `src/lib/responsive-rail-state.test.ts`: compact-view and drawer transitions.
- `src/components/workspace/editor-workspace.tsx`: compose new shell components and supply existing state/callbacks.
- `src/components/workspace/editor-workspace.test.tsx`: preservation, routing, render status, and integration coverage.
- `src/components/workspace/media-bin.tsx`: external navigation mode and destination-panel composition; media workflow logic remains here.
- `src/components/workspace/media-bin.test.tsx`: external rail mode, generation ownership, and legacy workflow preservation.
- `src/components/workspace/preview-panel.tsx`: contextual toolbar slot and render-review removal from permanent viewer space.
- `src/components/workspace/preview-panel.test.tsx`: toolbar slot, transport, preview states, and render-review routing.
- `src/components/workspace/timeline-editor.tsx`: selection-toolbar slot and removal of duplicated selected-action strip.
- `src/components/workspace/timeline-editor.test.tsx`: editing controls, selection actions, tracks, drag/resize, and automation.
- `src/components/workspace/agent-panel.tsx`: use extracted proposal review and compact workflow links.
- `src/components/workspace/agent-panel.test.tsx`: sessions, context, composer, workflow links, and proposal application.
- `src/components/workspace/project-timeline-inspector.tsx`: use `ActivityPanel` and retain Project/Activity separation.
- `src/components/workspace/project-timeline-inspector.test.tsx`: project facts and activity routing.
- `src/components/workspace/export-sheet.tsx`: accept an explicit entry intent of draft review or destination selection.
- `src/components/workspace/export-sheet.test.tsx`: entry intent, capabilities, progress, cancellation, errors, and focus.
- `scripts/browser-visual-qa.mjs`: add deterministic approved IA states and viewport captures.
- `src/browser-visual-qa-palmier-scenarios.test.ts`: require new scenario names and geometry hooks.
- `docs/visual-qa/browser-visual-baseline-manifest.json`: record intentional baseline state set after visual approval.
- `docs/parity.md`: add truthful evidence only after fresh tests and captures.

---

### Task 1: Encode the Information Architecture and Preservation Manifest

**Files:**
- Create: `src/lib/editor-information-architecture.ts`
- Create: `src/lib/editor-information-architecture.test.ts`
- Reference: `docs/superpowers/specs/2026-07-14-modern-editor-information-architecture-design.md`

**Interfaces:**
- Produces: `EditorSourceDestination`, `EditorGlobalDestination`, `EditorSelectionKind`, `EditorInspectorDestination`, `editorSourceDestinations`, `inspectorDestinationsForSelection()`, `defaultInspectorDestinationForSelection()`, and `editorCapabilityRoutes`.
- Consumed by: Tasks 3-9.

- [ ] **Step 1: Write the failing destination and preservation tests**

```ts
import { describe, expect, it } from "vitest";
import {
  defaultInspectorDestinationForSelection,
  editorCapabilityRoutes,
  editorSourceDestinations,
  inspectorDestinationsForSelection,
} from "./editor-information-architecture";

describe("modern editor information architecture", () => {
  it("keeps the approved source destinations in stable order", () => {
    expect(editorSourceDestinations.map((destination) => destination.id)).toEqual([
      "media",
      "generate",
      "templates",
      "text",
      "captions",
      "transcript",
      "audio",
      "effects",
    ]);
  });

  it("routes every preserved capability group to one unique primary owner", () => {
    expect(Object.keys(editorCapabilityRoutes).sort()).toEqual([
      "activity",
      "audio",
      "captions",
      "codex",
      "effects",
      "export",
      "generation",
      "inspector",
      "media",
      "preview",
      "render-review",
      "templates",
      "text",
      "timeline",
      "transcript",
      "workspace",
    ]);
    const primaryOwners = Object.values(editorCapabilityRoutes).map((route) => route.primary);
    expect(primaryOwners.every((primary) => primary.length > 0)).toBe(true);
    expect(new Set(primaryOwners).size).toBe(primaryOwners.length);
  });

  it("uses selection-aware inspector destinations without mixing details and AI Edit", () => {
    expect(inspectorDestinationsForSelection("visual")).toEqual([
      "basic", "transform", "motion", "effects", "ai-edit",
    ]);
    expect(inspectorDestinationsForSelection("audio")).toEqual([
      "basic", "volume", "fades", "denoise", "speakers",
    ]);
    expect(inspectorDestinationsForSelection("none")).toEqual([
      "project", "activity", "render-review",
    ]);
    expect(defaultInspectorDestinationForSelection("generated")).toBe("details");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `rtk pnpm exec vitest run src/lib/editor-information-architecture.test.ts`

Expected: FAIL because `editor-information-architecture.ts` does not exist.

- [ ] **Step 3: Implement the exact IA types, routes, and defaults**

```ts
export type EditorSourceDestination =
  | "media" | "generate" | "templates" | "text"
  | "captions" | "transcript" | "audio" | "effects";

export type EditorGlobalDestination = "home" | "codex" | "activity" | "settings";
export type EditorSelectionKind =
  | "none" | "visual" | "audio" | "caption" | "template" | "generated" | "multiple";
export type EditorInspectorDestination =
  | "basic" | "transform" | "motion" | "effects" | "ai-edit"
  | "volume" | "fades" | "denoise" | "speakers"
  | "content" | "style" | "placement" | "timing"
  | "fields" | "inputs" | "details" | "references" | "prompt"
  | "selection" | "common-properties" | "project" | "activity" | "render-review";

export const editorSourceDestinations = [
  { id: "media", label: "Media" },
  { id: "generate", label: "Generate" },
  { id: "templates", label: "Templates" },
  { id: "text", label: "Text" },
  { id: "captions", label: "Captions" },
  { id: "transcript", label: "Transcript" },
  { id: "audio", label: "Audio" },
  { id: "effects", label: "Effects" },
] as const satisfies readonly { id: EditorSourceDestination; label: string }[];

const inspectorRoutes: Record<EditorSelectionKind, readonly EditorInspectorDestination[]> = {
  none: ["project", "activity", "render-review"],
  visual: ["basic", "transform", "motion", "effects", "ai-edit"],
  audio: ["basic", "volume", "fades", "denoise", "speakers"],
  caption: ["content", "style", "motion", "placement", "timing"],
  template: ["basic", "fields", "motion", "effects", "inputs"],
  generated: ["details", "references", "prompt", "ai-edit"],
  multiple: ["selection", "common-properties", "timing", "effects"],
};

export function inspectorDestinationsForSelection(kind: EditorSelectionKind) {
  return inspectorRoutes[kind];
}

export function defaultInspectorDestinationForSelection(kind: EditorSelectionKind) {
  return inspectorRoutes[kind][0];
}

export const editorCapabilityRoutes = {
  workspace: { primary: "top-bar", shortcuts: ["global-rail"] },
  codex: { primary: "global:codex", shortcuts: [] },
  media: { primary: "source:media", shortcuts: [] },
  generation: { primary: "source:generate", shortcuts: ["media:generated-output"] },
  templates: { primary: "source:templates", shortcuts: ["timeline:add-template"] },
  text: { primary: "source:text", shortcuts: ["timeline:add-text"] },
  captions: { primary: "source:captions", shortcuts: ["inspector:caption"] },
  transcript: { primary: "source:transcript", shortcuts: ["inspector:transcript-status"] },
  audio: { primary: "source:audio", shortcuts: ["inspector:audio"] },
  effects: { primary: "source:effects", shortcuts: ["inspector:effects"] },
  preview: { primary: "viewer", shortcuts: [] },
  inspector: { primary: "inspector:selection", shortcuts: [] },
  timeline: { primary: "timeline", shortcuts: [] },
  activity: { primary: "global:activity", shortcuts: ["inspector:activity"] },
  "render-review": { primary: "inspector:render-review", shortcuts: ["top-bar:render-status"] },
  export: { primary: "export-sheet", shortcuts: ["top-bar:export"] },
} as const;
```

- [ ] **Step 4: Run the test and typecheck**

Run: `rtk pnpm exec vitest run src/lib/editor-information-architecture.test.ts && rtk pnpm lint`

Expected: the IA test passes and TypeScript reports no errors.

- [ ] **Step 5: Commit the IA contract**

```bash
rtk git add src/lib/editor-information-architecture.ts src/lib/editor-information-architecture.test.ts
rtk git commit -m "feat(editor): define capability-preserving IA"
```

---

### Task 2: Add the Pure Responsive Layout Budget

**Files:**
- Create: `src/lib/editor-layout-budget.ts`
- Create: `src/lib/editor-layout-budget.test.ts`
- Modify: `src/lib/workspace-layout-state.ts`
- Modify: `src/lib/workspace-layout-state.test.ts`

**Interfaces:**
- Consumes: saved `WorkspaceLayoutState` and the Codex/inspector open state.
- Produces: `EditorLayoutBudget` and `resolveEditorLayoutBudget(input)`.
- Rule: the resolver returns effective presentation only; it does not change or persist `WorkspaceLayoutState`. Timeline is always reachable, but below 1024 pixels only the active compact view is visible.

- [ ] **Step 1: Write failing breakpoint and restoration tests**

```ts
import { describe, expect, it } from "vitest";
import { resolveEditorLayoutBudget } from "./editor-layout-budget";

describe("editor layout budget", () => {
  it.each([
    [1728, "wide"],
    [1440, "desktop"],
    [1100, "constrained"],
    [900, "single"],
  ] as const)("resolves %ipx as %s", (width, mode) => {
    expect(resolveEditorLayoutBudget({
      width,
      contextPanelOpen: true,
      inspectorOpen: true,
      codexOpen: true,
      codexPinned: true,
    }).mode).toBe(mode);
  });

  it("pins Codex only when the wide budget preserves the viewer", () => {
    expect(resolveEditorLayoutBudget({
      width: 1728,
      contextPanelOpen: true,
      inspectorOpen: true,
      codexOpen: true,
      codexPinned: true,
    }).codexPresentation).toBe("pinned");
    expect(resolveEditorLayoutBudget({
      width: 1440,
      contextPanelOpen: true,
      inspectorOpen: true,
      codexOpen: true,
      codexPinned: true,
    }).codexPresentation).toBe("replace-context");
  });

  it("keeps the timeline reachable in every mode", () => {
    for (const width of [1728, 1440, 1100, 900]) {
      expect(resolveEditorLayoutBudget({
        width,
        contextPanelOpen: true,
        inspectorOpen: true,
        codexOpen: true,
        codexPinned: true,
      }).timelineReachable).toBe(true);
    }
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `rtk pnpm exec vitest run src/lib/editor-layout-budget.test.ts`

Expected: FAIL because the resolver does not exist.

- [ ] **Step 3: Implement the pure resolver**

```ts
export type EditorLayoutMode = "wide" | "desktop" | "constrained" | "single";
export type InspectorPresentation = "docked" | "drawer" | "single-pane" | "closed";
export type CodexPresentation = "pinned" | "replace-context" | "single-pane" | "closed";

export interface EditorLayoutBudgetInput {
  width: number;
  contextPanelOpen: boolean;
  inspectorOpen: boolean;
  codexOpen: boolean;
  codexPinned: boolean;
}

export interface EditorLayoutBudget {
  mode: EditorLayoutMode;
  contextPanelVisible: boolean;
  inspectorPresentation: InspectorPresentation;
  codexPresentation: CodexPresentation;
  timelineReachable: true;
}

export function resolveEditorLayoutBudget(input: EditorLayoutBudgetInput): EditorLayoutBudget {
  const mode: EditorLayoutMode = input.width >= 1600
    ? "wide"
    : input.width >= 1280
      ? "desktop"
      : input.width >= 1024
        ? "constrained"
        : "single";
  if (mode === "single") {
    return {
      mode,
      contextPanelVisible: false,
      inspectorPresentation: input.inspectorOpen ? "single-pane" : "closed",
      codexPresentation: input.codexOpen ? "single-pane" : "closed",
      timelineReachable: true,
    };
  }
  const codexPresentation: CodexPresentation = !input.codexOpen
    ? "closed"
    : mode === "wide" && input.codexPinned
      ? "pinned"
      : "replace-context";
  return {
    mode,
    contextPanelVisible: input.contextPanelOpen && codexPresentation !== "replace-context",
    inspectorPresentation: !input.inspectorOpen
      ? "closed"
      : mode === "wide"
        ? "docked"
        : "drawer",
    codexPresentation,
    timelineReachable: true,
  };
}
```

- [ ] **Step 4: Align saved width defaults without changing the storage key**

Change `defaultWorkspaceLayoutState.widths` to `{ codex: 360, media: 304, inspector: 320 }` and bounds to Codex `280-440`, Media `260-400`, Inspector `280-420`. Keep `video-creater.workspace-layout.v1`; `parseWorkspaceLayoutState()` already clamps previously saved values safely.

Add this exact migration assertion:

```ts
expect(parseWorkspaceLayoutState({
  widths: { codex: 240, media: 640, inspector: 560 },
})).toMatchObject({ widths: { codex: 280, media: 400, inspector: 420 } });
```

- [ ] **Step 5: Run focused layout tests**

Run: `rtk pnpm exec vitest run src/lib/editor-layout-budget.test.ts src/lib/workspace-layout-state.test.ts src/lib/responsive-rail-state.test.ts`

Expected: all focused layout tests pass; no saved-state mutation occurs in the budget resolver.

- [ ] **Step 6: Commit the layout contract**

```bash
rtk git add src/lib/editor-layout-budget.ts src/lib/editor-layout-budget.test.ts src/lib/workspace-layout-state.ts src/lib/workspace-layout-state.test.ts
rtk git commit -m "feat(editor): add responsive layout budget"
```

---

### Task 3: Build the Accessible Global Rail and Shell Primitives

**Files:**
- Create: `src/components/workspace/editor-navigation-rail.tsx`
- Create: `src/components/workspace/editor-navigation-rail.test.tsx`
- Create: `src/components/workspace/editor-shell.tsx`
- Create: `src/components/workspace/editor-shell.test.tsx`

**Interfaces:**
- Consumes: Task 1 destinations and Task 2 `EditorLayoutBudget`.
- Produces: `EditorNavigationRail` and `EditorShell`.

- [ ] **Step 1: Write failing rail interaction tests**

```tsx
it("renders every approved destination and global action", () => {
  renderRail();
  for (const label of [
    "Home", "Codex", "Media", "Generate", "Templates", "Text",
    "Captions", "Transcript", "Audio", "Effects", "Activity", "Settings",
  ]) {
    expect(screen.getByRole("button", { name: label })).toBeVisible();
  }
});

it("activates one source destination and exposes Activity status in text", () => {
  const onSelectSource = vi.fn();
  renderRail({ activeSource: "media", activityStatus: "2 failed", onSelectSource });
  expect(screen.getByRole("button", { name: "Media" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Activity, 2 failed" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Generate" }));
  expect(onSelectSource).toHaveBeenCalledWith("generate");
});
```

- [ ] **Step 2: Run the rail test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-navigation-rail.test.tsx`

Expected: FAIL because the rail is missing.

- [ ] **Step 3: Implement the rail using one typed definition list**

```tsx
export interface EditorNavigationRailProps {
  activeSource: EditorSourceDestination;
  sourcePanelOpen: boolean;
  codexOpen: boolean;
  activityStatus?: string;
  onHome: () => void;
  onToggleCodex: () => void;
  onSelectSource: (destination: EditorSourceDestination) => void;
  onOpenActivity: () => void;
  onOpenSettings: () => void;
}
```

Map Task 1 destinations to `Images`, `Sparkles`, `PanelsTopLeft`, `Type`, `Captions`, `TextSearch`, `AudioLines`, and `WandSparkles`. Render each as a 64-pixel-wide button containing a 20-pixel icon and an 10-pixel visible label. Use `aria-pressed` for source/Codex state and `aria-label={`Activity, ${activityStatus}`}` when a badge exists.

- [ ] **Step 4: Write failing shell geometry tests**

```tsx
it("keeps viewer and timeline as protected editor regions", () => {
  renderShell({ budget: wideBudget });
  expect(screen.getByRole("region", { name: "Editor viewer" })).toHaveClass("min-w-0");
  expect(screen.getByRole("region", { name: "Editor timeline" })).toBeVisible();
  expect(screen.getByRole("separator", { name: "Resize source panel" })).toBeVisible();
  expect(screen.getByRole("separator", { name: "Resize timeline" })).toBeVisible();
});
```

- [ ] **Step 5: Implement the shell slots and CSS-grid areas**

```tsx
export interface EditorShellProps {
  budget: EditorLayoutBudget;
  topBar: ReactNode;
  navigationRail: ReactNode;
  contextPanel: ReactNode;
  codexPanel: ReactNode;
  viewer: ReactNode;
  inspectorRail: ReactNode;
  inspectorDrawer: ReactNode;
  timeline: ReactNode;
  onResizeContext: (width: number) => void;
  onResizeInspector: (width: number) => void;
  onResizeTimeline: (height: number) => void;
}
```

Use named landmarks and local overflow owners. The timeline slot is always mounted. In `desktop` and `constrained`, inspector/Codex drawers receive absolute upper-deck positioning and must stop above the timeline separator. In `single`, render only the active compact view supplied by `EditorWorkspace`.

- [ ] **Step 6: Run component tests**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-shell.test.tsx`

Expected: rail and shell tests pass, including keyboard focus and Activity status text.

- [ ] **Step 7: Commit the shell primitives**

```bash
rtk git add src/components/workspace/editor-navigation-rail.tsx src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-shell.tsx src/components/workspace/editor-shell.test.tsx
rtk git commit -m "feat(editor): add dual-rail shell primitives"
```

---

### Task 4: Route All Left Destinations Without Duplicating Workflows

**Files:**
- Create: `src/components/workspace/editor-source-panel.tsx`
- Create: `src/components/workspace/editor-source-panel.test.tsx`
- Create: `src/components/workspace/text-library-panel.tsx`
- Create: `src/components/workspace/text-library-panel.test.tsx`
- Create: `src/components/workspace/effect-catalog-panel.tsx`
- Create: `src/components/workspace/effect-catalog-panel.test.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/motion-template-library.tsx`
- Modify: `src/components/workspace/motion-template-library.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: `EditorSourceDestination` and the existing MediaBin/caption/speech/transcript/template/effect callbacks.
- Produces: `EditorSourcePanel` with exactly one mounted destination.

- [ ] **Step 1: Write the source-panel ownership test**

```tsx
it.each([
  ["media", "Project media"],
  ["generate", "Generate media"],
  ["templates", "Template assets"],
  ["text", "Text library"],
  ["captions", "Captions workbench"],
  ["transcript", "Transcript"],
  ["audio", "Audio workspace"],
  ["effects", "Effect catalog"],
] as const)("renders only the %s destination", (destination, label) => {
  renderSourcePanel(destination);
  expect(screen.getByRole("region", { name: label })).toBeVisible();
  expect(screen.getAllByTestId("active-editor-source-destination")).toHaveLength(1);
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-source-panel.test.tsx`

Expected: FAIL because the source-panel router is missing.

- [ ] **Step 3: Implement a render-prop source router**

```tsx
export type EditorSourcePanels = Record<EditorSourceDestination, ReactNode>;

export function EditorSourcePanel({
  destination,
  panels,
}: {
  destination: EditorSourceDestination;
  panels: EditorSourcePanels;
}) {
  return (
    <section
      aria-label={`${editorSourceDestinations.find((item) => item.id === destination)?.label} panel`}
      data-testid="active-editor-source-destination"
      className="h-full min-h-0 min-w-0 overflow-hidden"
    >
      {panels[destination]}
    </section>
  );
}
```

- [ ] **Step 4: Add external-navigation mode to MediaBin**

Extend `MediaBinProps` with:

```ts
externalNavigation?: boolean;
activeDestination?: Extract<EditorSourceDestination, "media" | "generate" | "audio">;
```

When `externalNavigation` is true, do not render MediaBin's internal Media/Captions/Audio rail. Render the existing media library for `media`, the existing attached generation composer as a full-height panel for `generate`, or the existing audio workspace for `audio`. MediaBin does not accept or route the other five destinations; that remains the single responsibility of `EditorSourcePanel`. Do not move provider, folder, search, generation, reference, history, retry, or lifecycle logic out of MediaBin in this task.

Change the existing Audio sub-tabs from Speech/Music/Effects to Library/Speech/Music. Library reuses the existing media-card renderer with a `kind === "audio"` filter, including waveforms, selected state, preview, generated lifecycle, drag/drop, and provenance. Speech continues to render the supplied `SpeechWorkbench`. Music opens the same generation composer in audio/music mode. Effects leaves Audio because the new Effects destination is its single primary owner. Add tests for all three Audio tabs and for audio-card drag/selection preservation.

- [ ] **Step 5: Implement Text and Effects entry panels around existing callbacks**

```tsx
export interface TextLibraryPanelProps {
  onAddText: () => void;
  selectedTextLabel?: string;
  onInspectSelectedText?: () => void;
}

export interface EffectCatalogPanelProps {
  targetLabel: string | null;
  effects: readonly VisualEffectDescriptor[];
  appliedEffectIds: readonly string[];
  preparationStatus?: "idle" | "preparing" | "ready" | "failed";
  disabledReason?: string;
  onApply: (effectId: string) => void;
  onClear: () => void;
}
```

Text renders Add Text, recent treatments supplied by the existing template catalog, and Inspect Selected Text. Effects renders search, category/preset filters derived from `VisualEffectDescriptor.category`, executable catalog entries, explicit target, applied state, preparation state, Apply, and Clear. Color/LUT-backed entries remain visible through the catalog's `colorEffect`/`resourceKey` metadata and route detailed parameters to the inspector. It never invents unavailable effects or labels an entry as a LUT when the catalog has no resource.

Extend `MotionTemplateLibrary` with a labeled search field and category chips derived from the existing template and shader-background catalogs. Filtering changes only visible cards; preview thumbnails, direct insertion, and the existing drag payloads remain unchanged. Categories must cover the catalog values that currently represent overlays/title treatments, captions, HyperFrames, transitions, and shader backgrounds; do not create empty reference-only categories.

- [ ] **Step 6: Compose all eight destinations in EditorWorkspace**

Replace `mediaPanelTab` with:

```ts
const [sourceDestination, setSourceDestination] = useState<EditorSourceDestination>("media");
const [sourcePanelOpen, setSourcePanelOpen] = useState(true);
```

Route existing callbacks so Generate opens with the existing `generationComposerOpen` state; Captions renders `CaptionsWorkbench`; Transcript renders `TranscriptPanel`; Templates renders `MotionTemplateLibrary`; Text and Effects use the new entry panels. Selecting the active rail destination toggles `sourcePanelOpen`; selecting a different destination opens the panel.

Create one `mediaWorkflowPanel` node with `externalNavigation` enabled and `activeDestination` derived as `"media"`, `"generate"`, or `"audio"`. Assign that same node to the Media, Generate, and Audio entries in `EditorSourcePanels`; assign the five focused panel nodes to their own entries. `EditorSourcePanel` mounts only the selected entry, so MediaBin retains one stateful media/generation/audio workflow owner without receiving unrelated destination content.

- [ ] **Step 7: Run preservation-focused source tests**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-source-panel.test.tsx src/components/workspace/text-library-panel.test.tsx src/components/workspace/effect-catalog-panel.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/editor-workspace.test.tsx -t "Media|Generate|Templates|Text|Captions|Transcript|Audio|Effects|folder|reference|generation|Library|Speech|Music|category|search"`

Expected: new routing tests and existing media/generation/folder/reference tests pass.

- [ ] **Step 8: Commit the left destination slice**

```bash
rtk git add src/components/workspace/editor-source-panel.tsx src/components/workspace/editor-source-panel.test.tsx src/components/workspace/text-library-panel.tsx src/components/workspace/text-library-panel.test.tsx src/components/workspace/effect-catalog-panel.tsx src/components/workspace/effect-catalog-panel.test.tsx src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/motion-template-library.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(editor): route source workflows through global rail"
```

---

### Task 5: Add the Viewer Deck and Contextual Canvas Toolbar

**Files:**
- Create: `src/components/workspace/viewer-context-toolbar.tsx`
- Create: `src/components/workspace/viewer-context-toolbar.test.tsx`
- Modify: `src/components/workspace/preview-panel.tsx`
- Modify: `src/components/workspace/preview-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Produces: `ViewerContextToolbar` and `PreviewPanel.contextToolbar`.
- Consumes: existing selected item/source, direct transform callbacks, reveal/replace callbacks, and prepared-preview status.

- [ ] **Step 1: Write the failing selection-specific toolbar tests**

```tsx
it("keeps the visual toolbar compact and source-aware", () => {
  renderToolbar({ kind: "visual", canReplace: true });
  for (const label of ["Reveal source", "Fit or fill", "Crop", "Transform", "Replace", "More canvas actions"]) {
    expect(screen.getByRole("button", { name: label })).toBeVisible();
  }
  expect(screen.queryByRole("button", { name: "Ripple delete" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Edit effect parameters" })).not.toBeInTheDocument();
});

it("shows caption safe-zone and text actions without provider settings", () => {
  renderToolbar({ kind: "caption", safeZoneVisible: true });
  expect(screen.getByRole("button", { name: "Edit caption text" })).toBeVisible();
  expect(screen.getByRole("status", { name: "Caption safe zone active" })).toBeVisible();
  expect(screen.queryByText("Provider")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Implement the typed compact toolbar**

```ts
export type ViewerContextKind = "visual" | "caption" | "template" | "lottie" | "generated";

export interface ViewerContextToolbarProps {
  kind: ViewerContextKind;
  canReplace: boolean;
  safeZoneVisible: boolean;
  disabledReason?: string;
  onRevealSource: () => void;
  onFitFill: () => void;
  onCrop: () => void;
  onTransform: () => void;
  onReplace?: () => void;
  onEditCaptionText?: () => void;
  onOpenMore: () => void;
}
```

Render only canvas-frequency actions. Use tooltip text for disabled reasons. Do not add delete, link, ripple, detailed effects, automation, or provider settings.

- [ ] **Step 3: Add a toolbar slot and remove permanent render review from PreviewPanel**

Add `contextToolbar?: ReactNode` to `PreviewPanelProps` and render it inside the canvas positioning context above the selected visual. Keep all timeline/source tabs, transport, prepared frames, direct canvas controls, preview failures, and Retry Preview unchanged. Remove only the permanently allocated center render-review region; route its trigger in Task 9.

- [ ] **Step 4: Run viewer and compositor tests**

Run: `rtk pnpm exec vitest run src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/editor-workspace.test.tsx -t "viewer|preview|canvas|source tab|Retry preview|render review"`

Expected: toolbar tests pass; preview transport, source tabs, canonical frames, direct controls, and failure recovery remain green.

- [ ] **Step 5: Commit the viewer slice**

```bash
rtk git add src/components/workspace/viewer-context-toolbar.tsx src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(editor): add contextual viewer toolbar"
```

---

### Task 6: Build the Selection-Aware Inspector Dock

**Files:**
- Create: `src/components/workspace/inspector-dock.tsx`
- Create: `src/components/workspace/inspector-dock.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Reuse: `src/components/workspace/contextual-inspector-tabs.tsx`

**Interfaces:**
- Consumes: Task 1 selection/destination routes and existing inspector React nodes.
- Produces: collapsed rail plus one active drawer with remembered destination by `EditorSelectionKind`.

- [ ] **Step 1: Write the failing routing and focus tests**

```tsx
it("routes generated details and AI Edit without rendering both", () => {
  renderDock({ selectionKind: "generated", activeDestination: "details" });
  expect(screen.getByRole("button", { name: "Details" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("region", { name: "Generated details" })).toBeVisible();
  expect(screen.queryByRole("region", { name: "Generated AI edit" })).not.toBeInTheDocument();
});

it("shows only compatible multi-selection properties", () => {
  renderDock({ selectionKind: "multiple", selectionCount: 3 });
  expect(screen.getByText("3 items selected")).toBeVisible();
  expect(screen.getByRole("button", { name: "Common properties" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "Lottie inputs" })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Implement the inspector dock API**

```tsx
export interface InspectorDockProps {
  selectionKind: EditorSelectionKind;
  selectionLabel: string;
  selectionCount: number;
  lockedReason?: string;
  activeDestination: EditorInspectorDestination;
  open: boolean;
  panels: Partial<Record<EditorInspectorDestination, ReactNode>>;
  onSelectDestination: (destination: EditorInspectorDestination) => void;
  onToggleDestination: (destination: EditorInspectorDestination) => void;
  onRevealSource?: () => void;
  onResize: (width: number) => void;
}
```

Use Task 1 routes to render stable icon order. Render facts in Basic/Details; use existing source, audio, caption, template, project, activity, and render components as panel nodes. Clicking the active destination closes the drawer. Keep locked content readable and render `lockedReason` as text.

- [ ] **Step 3: Replace the full-height inspector rail in EditorWorkspace**

Derive `EditorSelectionKind` from selected items/media. Supply the existing `SourceClipInspector`, `CaptionInspector`, `TextOverlayInspector`, `TemplateInspector`, `ProjectTimelineInspector`, `ActivityPanel` (Task 9), and `RenderReportPanel` nodes. Preserve all canonical callbacks; do not copy mutation logic into `InspectorDock`.

- [ ] **Step 4: Run all focused inspector tests**

Run: `rtk pnpm exec vitest run src/components/workspace/inspector-dock.test.tsx src/components/workspace/source-clip-inspector.test.tsx src/components/workspace/caption-inspector.test.tsx src/components/workspace/text-overlay-inspector.test.tsx src/components/workspace/template-inspector.test.tsx src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/editor-workspace.test.tsx -t "inspector|Details|AI Edit|transform|audio|caption|template|project|multi-selection|locked"`

Expected: all inspector editing/provenance tests pass with the new shell routing.

- [ ] **Step 5: Commit the inspector slice**

```bash
rtk git add src/components/workspace/inspector-dock.tsx src/components/workspace/inspector-dock.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(editor): add selection-aware inspector dock"
```

---

### Task 7: Consolidate Timeline and Selected-Clip Actions

**Files:**
- Create: `src/components/workspace/timeline-selection-toolbar.tsx`
- Create: `src/components/workspace/timeline-selection-toolbar.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Delete after replacement: `src/components/workspace/selected-clip-actions.tsx`
- Delete after replacement: `src/components/workspace/selected-clip-actions.test.tsx`

**Interfaces:**
- Produces: `TimelineSelectionToolbar` and a `selectionToolbar` slot in `TimelineEditor`.
- Preserves: every current selected-clip callback and shortcut.

- [ ] **Step 1: Write the failing wide/overflow reachability tests**

```tsx
it("keeps frequent selected actions visible on wide timelines", () => {
  renderToolbar({ compact: false, selectionCount: 1 });
  for (const label of ["Link selected", "Remove selected", "Nudge -0.25 seconds", "Nudge +0.25 seconds"]) {
    expect(screen.getByRole("button", { name: label })).toBeVisible();
  }
});

it("keeps every selected action reachable from the compact menu", () => {
  renderToolbar({ compact: true, selectionCount: 1, canDecompose: true });
  fireEvent.click(screen.getByRole("button", { name: "More clip actions" }));
  for (const label of [
    "Link selected", "Unlink selected", "Remove selected", "Ripple delete selected",
    "Duplicate selected", "Apply grain", "Apply vignette", "Clear effects", "Decompose sequence",
  ]) {
    expect(screen.getByRole("menuitem", { name: label })).toBeVisible();
  }
});
```

- [ ] **Step 2: Implement the typed toolbar using existing callbacks**

```ts
export interface TimelineSelectionToolbarProps {
  selectionCount: number;
  compact: boolean;
  canLink: boolean;
  canUnlink: boolean;
  canRemove: boolean;
  canRippleDelete: boolean;
  canDuplicate: boolean;
  canNudge: boolean;
  canApplyEffects: boolean;
  canDecompose: boolean;
  disabledReasons?: Partial<Record<
    "link" | "unlink" | "remove" | "ripple-delete" | "duplicate" | "nudge" | "effects" | "decompose",
    string
  >>;
  onLink: () => void;
  onUnlink: () => void;
  onRemove: () => void;
  onRippleDelete: () => void;
  onDuplicate: () => void;
  onNudge: (deltaSeconds: -0.25 | 0.25) => void;
  onApplyEffect: (effect: "grain" | "vignette") => void;
  onClearEffects: () => void;
  onDecompose: () => void;
}
```

Use icon buttons on wide layouts and a labeled menu on compact layouts. Preserve disabled reasons through tooltips and menu descriptions.

- [ ] **Step 3: Mount selection actions in TimelineEditor's existing toolbar**

Add `selectionToolbar?: ReactNode` beside existing undo/redo, select, razor, split, source marks, text, templates, snapping, and zoom. Remove the separate selected-action strip in `EditorWorkspace` only after its complete callback set is passed to `TimelineSelectionToolbar`, then delete the superseded `selected-clip-actions` component and test. Existing keyboard shortcuts remain owned by `TimelineEditor`/`EditorWorkspace`; the toolbar is a presentation route, not a second command implementation.

- [ ] **Step 4: Run the full timeline regression file and workspace action tests**

Run: `rtk pnpm exec vitest run src/components/workspace/timeline-selection-toolbar.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx -t "timeline|selected|split|trim|ripple|duplicate|copy|paste|drag|resize|keyframe|fade|track|filmstrip|waveform"`

Expected: all timeline editing behavior remains green; no duplicate action strip remains.

- [ ] **Step 5: Commit the timeline slice**

```bash
rtk git add -A src/components/workspace/timeline-selection-toolbar.tsx src/components/workspace/timeline-selection-toolbar.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/selected-clip-actions.tsx src/components/workspace/selected-clip-actions.test.tsx
rtk git commit -m "feat(timeline): consolidate selected clip actions"
```

---

### Task 8: Add the Global Codex Drawer and EDL-First Proposal Review

**Files:**
- Create: `src/components/workspace/codex-drawer.tsx`
- Create: `src/components/workspace/codex-drawer.test.tsx`
- Create: `src/components/workspace/codex-proposal-review.tsx`
- Create: `src/components/workspace/codex-proposal-review.test.tsx`
- Modify: `src/components/workspace/agent-panel.tsx`
- Modify: `src/components/workspace/agent-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: Task 2 `CodexPresentation`, existing `AgentPanel`, and `CodexEditProposal`/project-action validation results.
- Produces: replace/pin drawer and the approved eight-stage review sequence.

- [ ] **Step 1: Write failing drawer presentation tests**

```tsx
it("replaces the source panel on desktop and pins only on wide layouts", () => {
  const { rerender } = renderDrawer({ presentation: "replace-context" });
  expect(screen.getByRole("complementary", { name: "Codex" })).toHaveAttribute("data-presentation", "replace-context");
  rerender(<TestDrawer presentation="pinned" />);
  expect(screen.getByRole("complementary", { name: "Codex" })).toHaveAttribute("data-presentation", "pinned");
});
```

- [ ] **Step 2: Write failing proposal-stage, EDL-order, and visual-contract tests**

```tsx
it("shows all approved stages and a real EDL before visual layers", () => {
  renderProposal(validProposal);
  const headings = screen.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent);
  expect(headings).toEqual([
    "Intent", "Transcript", "Moments", "Primary EDL",
    "Timeline", "Layers", "Render", "Review",
  ]);
  expect(screen.getByText("00:01.200-00:03.800")).toBeVisible();
  expect(screen.getByText("Reason: hook")).toBeVisible();
});

it("requires the complete visual-layer contract", () => {
  renderProposal(proposalWithInvalidOverlay);
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Overlay is missing source beat, dimensions, frame rate, alpha, motion, and safe-zone metadata",
  );
  expect(screen.getByRole("button", { name: "Apply proposal" })).toBeDisabled();
});
```

- [ ] **Step 3: Implement staged proposal review types**

```ts
export interface ProposalEdlRow {
  mediaId: string;
  sourceIn: number;
  sourceOut: number;
  timelineStart: number;
  reason: string;
}

export interface ProposalVisualRow {
  role: "title_card" | "overlay" | "lower_third" | "caption" | "diagram" | "transition" | "hyperframe";
  sourceBeat: string;
  timelineStart: number;
  duration: number;
  text: string | null;
  dimensions: { width: number; height: number } | null;
  frameRate: number | null;
  alphaBehavior: string;
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}
```

Add a pure `buildCodexProposalReviewModel()` beside the component. It consumes the existing `CodexEditProposal`, prompt/target context already owned by `EditorWorkspace`, transcript readiness/summary, detected moment summaries when present, active timeline identity, and Rust `ProjectValidationIssue[]`. It safely normalizes the proposal's currently `unknown[]` caption/overlay/HyperFrame arrays into `ProposalVisualRow[]`; missing or invalid fields remain visible validation issues rather than being filled with invented defaults.

`CodexProposalReview` renders the exact sequence `Intent -> Transcript -> Moments -> EDL -> Timeline -> Layers -> Render -> Review`. Empty Transcript or Moments stages render an explicit Not supplied state. Primary EDL maps `proposal.mediaId` plus every `CodexProposalClip` into canonical media identity, `sourceIn`, `sourceOut`, timeline position/duration, and reason. Timeline shows target timeline/track/lock consequences. Layers follows EDL and exposes every visual field above. Render shows the existing `CodexRenderReview` criteria. Review contains path-specific Rust validation issues and a fixed Revise Prompt/Reject/Apply Proposal footer. The component does not create project mutations itself.

- [ ] **Step 4: Replace AgentPanel's inline proposal block**

Keep sessions, starter tasks, context chips, conversation/tool transcript, mention suggestions, readiness, composer, and workflow links in `AgentPanel`. Delegate proposal rendering to `CodexProposalReview`; `onApplyProposal` continues to call the existing Rust-backed batch path in `EditorWorkspace`.

- [ ] **Step 5: Run agent, proposal, and canonical action tests**

Run: `rtk pnpm exec vitest run src/components/workspace/codex-drawer.test.tsx src/components/workspace/codex-proposal-review.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/editor-workspace.test.tsx -t "Codex|session|mention|proposal|project actions|EDL|workflow|undo"`

Expected: proposal application remains Rust-validated and undoable; invalid EDL/visual metadata blocks Apply.

- [ ] **Step 6: Run the Rust proposal contract tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml codex::proposal -- --test-threads=1`

Expected: Rust proposal parsing/validation tests pass unchanged.

- [ ] **Step 7: Commit the Codex slice**

```bash
rtk git add src/components/workspace/codex-drawer.tsx src/components/workspace/codex-drawer.test.tsx src/components/workspace/codex-proposal-review.tsx src/components/workspace/codex-proposal-review.test.tsx src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(agent): add EDL-first proposal review drawer"
```

---

### Task 9: Make Activity, Render Review, and Export One Evidence-Backed Flow

**Files:**
- Create: `src/components/workspace/activity-panel.tsx`
- Create: `src/components/workspace/activity-panel.test.tsx`
- Modify: `src/components/workspace/project-timeline-inspector.tsx`
- Modify: `src/components/workspace/project-timeline-inspector.test.tsx`
- Modify: `src/components/workspace/export-sheet.tsx`
- Modify: `src/components/workspace/export-sheet.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Reuse: `src/components/workspace/render-report-panel.tsx`

**Interfaces:**
- Produces: `ActivityJobRecord`, `buildActivityJobRecords()`, `ActivityPanel`, and `ExportSheetEntryIntent = "draft-review" | "destination"`.
- Consumes: existing project jobs, Temporal readiness, generated assets, render reports, export artifacts, and callbacks.

- [ ] **Step 1: Write failing activity ordering/action tests**

```tsx
it("is the complete project-wide job source ordered by update time", () => {
  renderActivityPanel({
    records: [
      { job: completedJob, targetLabel: null, proposalAvailable: false, outputPath: "exports/final.mp4", reportId: "completed", logPath: "renders/completed/render.log" },
      { job: failedJob, targetLabel: null, proposalAvailable: false, outputPath: null, reportId: null, logPath: "renders/failed/render.log" },
      { job: runningJob, targetLabel: null, proposalAvailable: false, outputPath: null, reportId: null, logPath: null },
    ],
  });
  expect(screen.getAllByRole("article").map((row) => row.getAttribute("data-job-id"))).toEqual([
    runningJob.id, failedJob.id, completedJob.id,
  ]);
  expect(screen.getByRole("button", { name: `Cancel ${runningJob.id}` })).toBeVisible();
  expect(screen.getByRole("button", { name: `Retry ${failedJob.id}` })).toBeVisible();
  expect(screen.getByRole("button", { name: `Open log ${failedJob.id}` })).toBeVisible();
  expect(screen.getByRole("button", { name: `Open output ${completedJob.id}` })).toBeVisible();
});
```

- [ ] **Step 2: Implement the typed ActivityPanel**

```ts
export interface ActivityPanelProps {
  records: readonly ActivityJobRecord[];
  onOpenTarget: (jobId: string) => void;
  onOpenProposal: (jobId: string) => void;
  onCancel: (jobId: string) => void;
  onRetry: (jobId: string) => void;
  onOpenOutput: (jobId: string) => void;
  onOpenReport: (jobId: string) => void;
  onOpenLog: (jobId: string) => void;
}

export interface ActivityJobRecord {
  job: ProjectJobSummary;
  targetLabel: string | null;
  proposalAvailable: boolean;
  outputPath: string | null;
  reportId: string | null;
  logPath: string | null;
}
```

Import the existing project types and `orderRecentProjectJobs` from `@/lib/project`. `buildActivityJobRecords(project)` joins `project.jobs` to generated assets by shared ID, render reports by report/job ID, and export artifacts by `jobId`; it never invents an output/report/log link. Order the complete record list using `orderRecentProjectJobs(records.map(({ job }) => job), records.length)`, then map the IDs back to records. Render applicable actions only. Preserve queued, running, progress, blocked, failed, completed, and cancelled status text. `ProjectTimelineInspector` uses this component for its Activity destination rather than duplicating rows.

- [ ] **Step 3: Write failing Render Status/Export entry tests**

```tsx
it("opens draft review and final delivery through one sheet with distinct intent", async () => {
  renderWorkspace();
  fireEvent.click(screen.getByRole("button", { name: "Render status" }));
  expect(await screen.findByRole("dialog", { name: "Export project" })).toHaveAttribute("data-entry-intent", "draft-review");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  expect(await screen.findByRole("dialog", { name: "Export project" })).toHaveAttribute("data-entry-intent", "destination");
});
```

- [ ] **Step 4: Add explicit entry intent to ExportSheet**

```ts
export type ExportSheetEntryIntent = "draft-review" | "destination";

interface ExportSheetProps {
  entryIntent: ExportSheetEntryIntent;
  // retain every existing capability, selection, progress, error, and callback prop
}
```

`draft-review` selects the available Draft video profile without bypassing capability checks. `destination` opens the destination groups. Both retain Video, Premiere XML, DaVinci/FCPXML, and Palmier Package routes, quality, resolution, progress, cancellation, failure, and focus trapping.

- [ ] **Step 5: Route render lifecycle outcomes to the exact inspector destination**

The top-bar Render Status control always opens `ExportSheet` with `entryIntent="draft-review"`, including while a job is active, completed, or failed, so progress/cancellation/output/failure remain in the one underlying render flow. After a render result is recorded, successful validation opens `InspectorDock` at `render-review` with `RenderReportPanel`; failure opens the same destination focused on its actionable reason, Retry Render, artifacts, and log. Activity's Open report/log actions route to that exact record. The global Activity button opens `activity`. Do not reintroduce permanent render-review height below Preview.

- [ ] **Step 6: Run activity/export/render tests**

Run: `rtk pnpm exec vitest run src/components/workspace/activity-panel.test.tsx src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/export-sheet.test.tsx src/components/workspace/render-report-panel.test.tsx src/components/workspace/editor-workspace.test.tsx -t "Activity|workflow|Render|Export|Premiere|DaVinci|Palmier|capability|cancel|retry|artifact|stream"`

Expected: all destinations and evidence states pass; failures remain actionable.

- [ ] **Step 7: Commit the activity/render/export slice**

```bash
rtk git add src/components/workspace/activity-panel.tsx src/components/workspace/activity-panel.test.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/export-sheet.tsx src/components/workspace/export-sheet.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(editor): unify activity render and export review"
```

---

### Task 10: Converge Responsive, Focus, and State Behavior

**Files:**
- Modify: `src/lib/responsive-rail-state.ts`
- Modify: `src/lib/responsive-rail-state.test.ts`
- Modify: `src/components/workspace/editor-shell.tsx`
- Modify: `src/components/workspace/editor-shell.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/index.css`

**Interfaces:**
- Consumes: Task 2 layout budget and existing compact views.
- Produces: exact wide/desktop/constrained/single behavior with focus restoration.

- [ ] **Step 1: Add failing responsive-state tests**

```ts
it("closes a transient drawer without changing the saved compact destination", () => {
  expect(nextResponsiveRailState(
    { compactView: "timeline", codexRailOpen: true },
    { type: "closeCodexRail" },
  )).toEqual({ compactView: "timeline", codexRailOpen: false });
});
```

Add `closeCodexRail` and `selectCompactView` behavior without changing `responsiveRailStorageKey`.

- [ ] **Step 2: Add failing width and focus integration tests**

```tsx
it.each([
  [1728, "wide"], [1440, "desktop"], [1100, "constrained"], [900, "single"],
] as const)("uses the %s layout at %ipx", async (width, mode) => {
  setViewportWidth(width);
  renderWorkspace();
  expect(screen.getByRole("main", { name: "Video editor workspace" })).toHaveAttribute("data-layout-mode", mode);
  expect(screen.getByRole("region", { name: "Editor timeline" })).toBeVisible();
});

it("returns focus to the rail trigger after a drawer closes", () => {
  renderWorkspace();
  const generate = screen.getByRole("button", { name: "Generate" });
  fireEvent.click(generate);
  fireEvent.click(screen.getByRole("button", { name: "Close Generate" }));
  expect(generate).toHaveFocus();
});
```

- [ ] **Step 3: Implement automatic effective collapse without persistence writes**

Use `ResizeObserver` on the editor shell and Task 2 `resolveEditorLayoutBudget()`. Auto-collapse is rendered state only. Continue saving only direct user layout actions through `saveWorkspaceLayoutState()` and direct compact choices through `saveResponsiveRailState()`.

- [ ] **Step 4: Add dark focus, invalid-target, and reduced-motion tokens**

Add named utility classes in `src/index.css` for rail focus, drawer shadow, valid/invalid drop outlines, and reduced-motion-safe drawer transitions. Reuse existing CSS variables; do not introduce a second theme.

- [ ] **Step 5: Run responsive and accessibility-focused tests**

Run: `rtk pnpm exec vitest run src/lib/editor-layout-budget.test.ts src/lib/workspace-layout-state.test.ts src/lib/responsive-rail-state.test.ts src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-shell.test.tsx src/components/workspace/inspector-dock.test.tsx src/components/workspace/codex-drawer.test.tsx src/components/workspace/editor-workspace.test.tsx -t "responsive|compact|focus|keyboard|tooltip|drawer|layout|viewport|disabled"`

Expected: all viewport bands retain a visible Timeline destination; the default compact view shows Timeline, and focus/accessibility behavior passes.

- [ ] **Step 6: Commit responsive convergence**

```bash
rtk git add src/lib/responsive-rail-state.ts src/lib/responsive-rail-state.test.ts src/components/workspace/editor-shell.tsx src/components/workspace/editor-shell.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/index.css
rtk git commit -m "feat(editor): converge responsive drawer behavior"
```

---

### Task 11: Add Deterministic Visual QA and Close Only Verified Claims

**Files:**
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-palmier-scenarios.test.ts`
- Modify: `docs/visual-qa/browser-visual-baseline-manifest.json`
- Modify: `docs/parity.md`
- Capture: `docs/visual-qa/browser-visual-baseline/*.png`

**Interfaces:**
- Consumes: all preceding UI states.
- Produces: deterministic desktop/narrow evidence and truthful parity wording.

- [ ] **Step 1: Add failing scenario-name coverage**

Require these exact deterministic scenario IDs:

```ts
const requiredModernEditorScenarios = [
  "modern-editor-default-wide",
  "modern-editor-default-desktop",
  "modern-editor-default-constrained",
  "modern-editor-media-folder",
  "modern-editor-generate-image",
  "modern-editor-generate-video",
  "modern-editor-generate-audio",
  "modern-editor-templates",
  "modern-editor-text",
  "modern-editor-captions",
  "modern-editor-transcript",
  "modern-editor-audio-speech",
  "modern-editor-effects",
  "modern-editor-inspector-visual",
  "modern-editor-inspector-audio",
  "modern-editor-inspector-caption",
  "modern-editor-inspector-template",
  "modern-editor-generated-inspector",
  "modern-editor-codex-empty",
  "modern-editor-codex-active",
  "modern-editor-codex-proposal",
  "modern-editor-activity-queued",
  "modern-editor-activity-running",
  "modern-editor-activity-failed",
  "modern-editor-activity-completed",
  "modern-editor-export",
  "modern-editor-preview-failed",
  "modern-editor-render-failed",
  "modern-editor-timeline-drag",
  "modern-editor-timeline-invalid-drop",
  "modern-editor-timeline-resize",
  "modern-editor-timeline-multiselect",
  "modern-editor-timeline-automation",
  "modern-editor-narrow",
];
```

Test that `scripts/browser-visual-qa.mjs` exports every ID with deterministic fixture state and viewport.

- [ ] **Step 2: Run the scenario test to verify failure**

Run: `rtk pnpm exec vitest run src/browser-visual-qa-palmier-scenarios.test.ts`

Expected: FAIL listing the new missing scenario IDs.

- [ ] **Step 3: Implement the scenarios and geometry assertions**

Use 1728x1080 for wide, 1440x900 for desktop, 1100x800 for constrained, and 900x900 for single-pane captures. The three default scenarios prove the first three bands; `modern-editor-narrow` proves the below-1024 one-pane switcher. For every capture assert:

- the global rail is visible;
- exactly one source destination is mounted when the context panel is open;
- viewer and timeline landmarks are visible together except single-pane source/inspector captures;
- no horizontal document overflow exists;
- the inspector drawer stops above the timeline;
- icon-only buttons have non-empty accessible names;
- active failure states include Retry/Open Logs/Open Artifact text as applicable.

- [ ] **Step 4: Run focused and full frontend verification**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk pnpm build
rtk git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 5: Run Rust boundary tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml codex::proposal -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_action -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_export -- --test-threads=1
```

Expected: canonical proposal, mutation, render, and export tests pass unchanged.

- [ ] **Step 6: Capture the complete browser matrix**

Run: `rtk pnpm visual:qa:browser`

Expected: every required modern-editor scenario produces a PNG and a successful report under `output/playwright/browser-visual-qa`.

- [ ] **Step 7: Inspect captures before changing baselines**

Open every required image and reject the capture set if any of these are true:

- a major capability region is absent without its one-click rail/drawer route visible;
- viewer or timeline is obscured;
- source and inspector drawers overlap;
- text or controls clip;
- selected, running, blocked, failed, or completed state is unclear;
- the timeline loses track identity, clip labels, drag/resize feedback, filmstrips, waveforms, or status.

- [ ] **Step 8: Refresh the approved baseline and manifest**

Copy only inspected intentional captures to `docs/visual-qa/browser-visual-baseline`, update hashes/dimensions in the manifest through the existing baseline workflow, then run:

Run: `rtk pnpm visual:qa:browser-release`

Expected: comparison and baseline policy pass at threshold `0.01` and channel threshold `4`.

- [ ] **Step 9: Update parity evidence truthfully**

Add dated evidence to `docs/parity.md` that names the exact tests, viewport captures, preserved capability routes, and any remaining unimplemented or release-unproven behavior. Do not claim packaged/native proof from browser captures.

- [ ] **Step 10: Commit the verified visual slice**

```bash
rtk git add scripts/browser-visual-qa.mjs src/browser-visual-qa-palmier-scenarios.test.ts docs/visual-qa/browser-visual-baseline docs/visual-qa/browser-visual-baseline-manifest.json docs/parity.md
rtk git commit -m "test(editor): verify modern workspace preservation"
```

---

## Final Acceptance Gate

Before declaring the program complete, verify all of the following from current output:

- Every Task 1 capability route has a visible primary destination.
- Media, generation, captions, transcript, audio, effects, viewer, inspector, timeline, Codex, activity, render, and export focused test suites pass.
- The full TypeScript test suite, lint, and build pass.
- The Rust proposal/action/render/export boundaries pass.
- All required browser visual states exist and were inspected.
- No responsive mode removes Timeline from the one-pane switcher or mutates saved preferences automatically.
- Codex Apply Proposal remains Rust-validated and undoable.
- Proposal review shows a real EDL before layers.
- Visual layers show `visualTreatment`, `motion`, `safeZone`, and `avoid`.
- Render success remains backed by duration, stream, timing, comparison, artifact, and log evidence.
- `rtk git status --short` contains no unintended files.
