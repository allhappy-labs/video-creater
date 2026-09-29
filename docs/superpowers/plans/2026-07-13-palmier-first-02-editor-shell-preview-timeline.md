# Palmier-First Editor Shell, Preview, and Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the editor shell, preview transport, timeline geometry, and empty Codex rail visually match the approved Palmier-first desktop target without removing Video Creater behavior.

**Architecture:** Keep `EditorWorkspace` as the state owner, but move secondary header controls into a focused menu and calculate responsive pane visibility through a pure layout-budget function. Reconcile the dirty timeline against the committed variable-height implementation before applying Palmier's 50px default row, 100px track header, 24px ruler, flat toolbar, filmstrip, waveform, and selection treatments. Preserve the existing canonical preview compositor, timeline actions, agent proposal flow, compact one-pane switcher, and saved pane preferences.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, shadcn/ui buttons, lucide-react, Vitest, Testing Library, Vite, Playwright CLI, ffmpeg.

---

## Execution guardrails

- The worktree is intentionally dirty. Before editing any implementation file, run `rtk git diff -- <path>` and retain the output under `output/parity-audit-2026-07-13/dirty-baselines/`. Never use `git checkout`, `git restore`, `git reset`, or a whole-file replacement from `HEAD`.
- `src/components/workspace/timeline-editor.tsx` currently removes committed variable track geometry, resize handles, advanced navigation, and native-menu routing while adding newer sync-lock and ripple behavior. Reconcile the two states manually: the committed implementation is a source for retained behavior, not a file to copy over the dirty tree.
- Before every commit, run `rtk git diff --cached --name-only`. Stage only the paths and hunks owned by that task. If a file was already dirty before this plan, use `rtk git add -p <path>` and accept only the new coherent parity/reconciliation hunks.
- Do not remove Default, Media, or Vertical presets; source tabs; canonical preview preparation; crop/transform interaction; generation and render status; timeline snapping, range, ripple, selection, automation, fade, track lock/sync-lock, resize, reorder, filmstrip, or waveform behavior; Codex sessions, mentions, proposals, jobs, or diagnostics.
- Use `reference/Sources/PalmierPro/Utilities/Constants.swift`, `reference/Sources/PalmierPro/Preview/PreviewContainerView.swift`, and `reference/Sources/PalmierPro/Agent/Panel/AgentPanelView.swift` as the numeric and interaction references. Use `output/parity-audit-2026-07-13/reference/02-editor.png` and `08-agent-editor.png` as the visual references.

### Task 1: Encode Palmier pane geometry and the responsive pane budget

**Files:**
- Modify: `src/lib/workspace-layout-state.ts:1-151`
- Test: `src/lib/workspace-layout-state.test.ts:1-119`

- [ ] **Step 1: Save the dirty baseline for the layout files**

Run:

```bash
rtk mkdir -p output/parity-audit-2026-07-13/dirty-baselines
rtk git diff -- src/lib/workspace-layout-state.ts src/lib/workspace-layout-state.test.ts > output/parity-audit-2026-07-13/dirty-baselines/task-01-layout.patch
```

Expected: the command exits `0`; the patch is empty if these files were clean and otherwise records their pre-task diff without changing the worktree.

- [ ] **Step 2: Write failing tests for Palmier defaults, row allocation, and automatic collapse**

Add `workspaceResponsiveVisibility` to the import list and add these tests to `src/lib/workspace-layout-state.test.ts`:

```ts
it("uses Palmier desktop pane defaults and minimums", () => {
  expect(defaultWorkspaceLayoutState.widths).toEqual({
    codex: 240,
    media: 500,
    inspector: 260,
  });

  const clamped = workspaceLayoutReducer(defaultWorkspaceLayoutState, {
    type: "resizePane",
    pane: "inspector",
    width: 1,
  });
  expect(clamped.widths.inspector).toBe(150);
});

it("gives the default timeline twice the flexible height of the preview deck", () => {
  expect(workspacePresetGridTemplate(defaultWorkspaceLayoutState)).toMatchObject({
    columns: "500px minmax(25rem, 1fr) 260px",
    rows: "minmax(20rem, 1fr) minmax(16rem, 2fr)",
    areas: '"media preview inspector" "timeline timeline timeline"',
  });
});

it("collapses secondary panes without mutating stored visibility", () => {
  const requested = defaultWorkspaceLayoutState;

  expect(workspaceResponsiveVisibility(requested, true, 1440)).toEqual({
    codex: true,
    media: true,
    inspector: true,
  });
  expect(workspaceResponsiveVisibility(requested, true, 1280)).toEqual({
    codex: false,
    media: true,
    inspector: true,
  });
  expect(workspaceResponsiveVisibility(requested, true, 1024)).toEqual({
    codex: false,
    media: true,
    inspector: false,
  });
  expect(requested.visible).toEqual({ codex: true, media: true, inspector: true });
});
```

- [ ] **Step 3: Run the focused test and verify the new contract fails**

Run: `rtk pnpm test -- src/lib/workspace-layout-state.test.ts`

Expected: FAIL because `workspaceResponsiveVisibility` is not exported and the defaults are still `{ codex: 240, media: 320, inspector: 300 }`.

- [ ] **Step 4: Implement the exact desktop metrics and responsive visibility resolver**

Replace the default widths and bounds, add the layout metrics and resolver, and update the default preset rows in `src/lib/workspace-layout-state.ts`:

```ts
export const workspaceLayoutMetrics = {
  compactBreakpoint: 1024,
  fullDesktopBreakpoint: 1440,
  panelGap: 5,
  previewMinimumWidth: 400,
  previewMinimumHeight: 320,
} as const;

export const defaultWorkspaceLayoutState: WorkspaceLayoutState = {
  preset: "default",
  visible: { codex: true, media: true, inspector: true },
  widths: { codex: 240, media: 500, inspector: 260 },
  focusedPane: "timeline",
  maximizedPane: null,
};

const paneWidthBounds = {
  codex: { min: 240, max: 640 },
  media: { min: 280, max: 640 },
  inspector: { min: 150, max: 560 },
} as const;

export function workspaceResponsiveVisibility(
  state: WorkspaceLayoutState,
  codexRailOpen: boolean,
  viewportWidth: number,
) {
  const visible = {
    codex: codexRailOpen && state.visible.codex,
    media: state.visible.media,
    inspector: state.visible.inspector,
  };
  if (!Number.isFinite(viewportWidth) || viewportWidth < workspaceLayoutMetrics.compactBreakpoint) {
    return visible;
  }
  if (viewportWidth >= workspaceLayoutMetrics.fullDesktopBreakpoint) {
    return visible;
  }

  const requiredWidth = () =>
    workspaceLayoutMetrics.previewMinimumWidth +
    (visible.codex ? state.widths.codex + workspaceLayoutMetrics.panelGap : 0) +
    (visible.media ? state.widths.media + workspaceLayoutMetrics.panelGap : 0) +
    (visible.inspector ? state.widths.inspector + workspaceLayoutMetrics.panelGap : 0);

  for (const pane of ["codex", "inspector", "media"] as const) {
    if (requiredWidth() <= viewportWidth) break;
    visible[pane] = false;
  }
  return visible;
}
```

Use this exact default branch in `workspacePresetGridTemplate` while leaving Media and Vertical preset area maps intact:

```ts
return {
  columns: `${media} minmax(25rem, 1fr) ${inspector}`,
  rows: state.preset === "media"
    ? "minmax(20rem, 1fr) minmax(16rem, 2fr)"
    : "minmax(20rem, 1fr) minmax(16rem, 2fr)",
  areas: state.preset === "media"
    ? '"media preview inspector" "media timeline timeline"'
    : '"media preview inspector" "timeline timeline timeline"',
};
```

- [ ] **Step 5: Update stale exact-value assertions and run the layout tests**

Change the existing clamp expectations from `240/640` to `280/640` for Media, and change persisted corrupt Inspector fallback expectations from `300` to `260`.

Run: `rtk pnpm test -- src/lib/workspace-layout-state.test.ts`

Expected: PASS; all workspace-layout tests exit `0` and the three new Palmier contract tests pass.

- [ ] **Step 6: Commit the pure layout contract**

```bash
rtk git add -p src/lib/workspace-layout-state.ts src/lib/workspace-layout-state.test.ts
rtk git diff --cached --check
rtk git commit -m "feat(editor): add Palmier responsive pane budget"
```

Expected: one Conventional Commit containing only the layout-state and test hunks.

### Task 2: Compact the editor title bar and apply the pane budget without losing controls

**Files:**
- Create: `src/components/workspace/workspace-layout-menu.tsx`
- Create: `src/components/workspace/workspace-layout-menu.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx:516-570,3112-3121,3286-3292,7337-7355,7392-7841,8085-9018`
- Test: `src/components/workspace/editor-workspace.test.tsx:1874-1903`

- [ ] **Step 1: Save the dirty editor-workspace baseline**

Run:

```bash
rtk git diff -- src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-02-editor-workspace.patch
```

Expected: exit `0`; the existing dirty editor changes are preserved in the ignored baseline patch.

- [ ] **Step 2: Write the failing compact-menu interaction test**

Create `src/components/workspace/workspace-layout-menu.test.tsx`:

```tsx
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultWorkspaceLayoutState } from "@/lib/workspace-layout-state";
import { WorkspaceLayoutMenu } from "./workspace-layout-menu";

describe("WorkspaceLayoutMenu", () => {
  it("keeps layout and render controls behind one compact trigger", () => {
    const onSetPreset = vi.fn();
    const onTogglePane = vi.fn();
    const onRender = vi.fn();
    render(
      <WorkspaceLayoutMenu
        layout={defaultWorkspaceLayoutState}
        renderQuality="draftWebm"
        renderRunning={false}
        onRenderQualityChange={vi.fn()}
        onSetPreset={onSetPreset}
        onToggleMaximize={vi.fn()}
        onTogglePane={onTogglePane}
        onRender={onRender}
      />,
    );

    expect(screen.queryByRole("menu", { name: "Workspace controls" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Workspace controls" }));
    const menu = screen.getByRole("menu", { name: "Workspace controls" });
    fireEvent.change(within(menu).getByLabelText("Workspace layout preset"), {
      target: { value: "media" },
    });
    fireEvent.click(within(menu).getByRole("menuitemcheckbox", { name: "Media panel" }));
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Render draft" }));

    expect(onSetPreset).toHaveBeenCalledWith("media");
    expect(onTogglePane).toHaveBeenCalledWith("media");
    expect(onRender).toHaveBeenCalledWith("draftWebm");
  });
});
```

- [ ] **Step 3: Add failing editor-shell assertions**

Append this test to `src/components/workspace/editor-workspace.test.tsx`:

```tsx
it("uses one compact Palmier title bar and five-pixel pane gaps", () => {
  render(<EditorWorkspace />);

  const chrome = screen.getByRole("banner", { name: "Project editor chrome" });
  const panes = screen.getByRole("region", { name: "Editor panes" });
  const preset = screen.getByRole("region", { name: "Workspace preset panes" });

  expect(chrome).toHaveClass("h-9", "px-2", "py-0");
  expect(chrome).not.toHaveClass("min-h-12");
  expect(within(chrome).getByText("Sample Project")).toBeInTheDocument();
  expect(within(chrome).getByRole("status")).toHaveClass("sr-only");
  expect(panes).toHaveClass("gap-[5px]", "p-[5px]");
  expect(preset).toHaveClass("gap-[5px]");
});
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
rtk pnpm test -- src/components/workspace/workspace-layout-menu.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because `WorkspaceLayoutMenu` does not exist and the current header still uses `min-h-12`, multi-row padding, and 12px pane gaps.

- [ ] **Step 5: Create the compact workspace menu**

Create `src/components/workspace/workspace-layout-menu.tsx` with this complete implementation:

```tsx
import { useEffect, useRef, useState } from "react";
import { Ellipsis, Images, Maximize2, PanelRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { RenderQualityControl } from "./render-quality-control";
import type { RenderQualityProfile } from "@/lib/render";
import type {
  WorkspaceLayoutPreset,
  WorkspaceLayoutState,
} from "@/lib/workspace-layout-state";

interface WorkspaceLayoutMenuProps {
  layout: WorkspaceLayoutState;
  renderQuality: RenderQualityProfile;
  renderRunning: boolean;
  onSetPreset: (preset: WorkspaceLayoutPreset) => void;
  onTogglePane: (pane: "media" | "inspector") => void;
  onToggleMaximize: () => void;
  onRenderQualityChange: (quality: RenderQualityProfile) => void;
  onRender: (quality: RenderQualityProfile) => void;
}

export function WorkspaceLayoutMenu({
  layout,
  renderQuality,
  renderRunning,
  onSetPreset,
  onTogglePane,
  onToggleMaximize,
  onRenderQualityChange,
  onRender,
}: WorkspaceLayoutMenuProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setOpen(false);
      triggerRef.current?.focus();
    };
    document.addEventListener("keydown", close);
    return () => document.removeEventListener("keydown", close);
  }, [open]);

  return (
    <div className="relative">
      <Button
        ref={triggerRef}
        type="button"
        variant="ghost"
        size="icon"
        className="h-7 w-7 rounded-[5px]"
        aria-label="Workspace controls"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <Ellipsis className="h-4 w-4" aria-hidden="true" />
      </Button>
      {open ? (
        <div
          role="menu"
          aria-label="Workspace controls"
          className="absolute right-0 top-8 z-50 grid w-64 gap-1 rounded-xl border border-white/15 bg-[#1e1e1e] p-2 text-xs shadow-2xl"
        >
          <label className="grid gap-1 px-2 py-1 text-muted-foreground">
            <span>Layout</span>
            <select
              aria-label="Workspace layout preset"
              value={layout.preset}
              className="h-8 rounded-md border border-white/15 bg-[#161616] px-2 text-foreground"
              onChange={(event) => onSetPreset(event.target.value as WorkspaceLayoutPreset)}
            >
              <option value="default">Default layout</option>
              <option value="media">Media layout</option>
              <option value="vertical">Vertical layout</option>
            </select>
          </label>
          <Button
            role="menuitemcheckbox"
            aria-checked={layout.visible.media}
            variant="ghost"
            size="sm"
            className="justify-start"
            onClick={() => onTogglePane("media")}
          >
            <Images className="h-4 w-4" aria-hidden="true" />
            Media panel
          </Button>
          <Button
            role="menuitemcheckbox"
            aria-checked={layout.visible.inspector}
            variant="ghost"
            size="sm"
            className="justify-start"
            onClick={() => onTogglePane("inspector")}
          >
            <PanelRight className="h-4 w-4" aria-hidden="true" />
            Inspector panel
          </Button>
          <Button
            role="menuitem"
            variant="ghost"
            size="sm"
            className="justify-start"
            onClick={onToggleMaximize}
          >
            <Maximize2 className="h-4 w-4" aria-hidden="true" />
            {layout.maximizedPane ? "Restore layout" : "Maximize focused panel"}
          </Button>
          <div className="my-1 h-px bg-white/10" />
          <RenderQualityControl value={renderQuality} onChange={onRenderQualityChange} />
          <Button
            role="menuitem"
            variant="ghost"
            size="sm"
            className="justify-start"
            disabled={renderRunning}
            onClick={() => {
              onRender(renderQuality);
              setOpen(false);
            }}
          >
            {renderQuality === "draftWebm" ? "Render draft" : "Render final"}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 6: Apply responsive visibility without mutating persisted preferences**

Import `workspaceResponsiveVisibility`, then add this viewport state beside the existing layout reducer in `editor-workspace.tsx`:

```tsx
const [workspaceViewportWidth, setWorkspaceViewportWidth] = useState(() =>
  typeof window === "undefined" ? 1440 : window.innerWidth,
);

useEffect(() => {
  const measure = () => setWorkspaceViewportWidth(window.innerWidth);
  window.addEventListener("resize", measure);
  return () => window.removeEventListener("resize", measure);
}, []);
```

Replace the current `effectiveWorkspaceLayout` block with:

```tsx
const responsiveVisibility = workspaceResponsiveVisibility(
  workspaceLayout,
  codexRailOpen,
  workspaceViewportWidth,
);
const effectiveWorkspaceLayout = {
  ...workspaceLayout,
  visible: responsiveVisibility,
};
const workspaceColumns = workspaceGridTemplate(effectiveWorkspaceLayout);
const workspacePresetGrid = workspacePresetGridTemplate(effectiveWorkspaceLayout);
const desktopPaneHiddenClass = (pane: WorkspacePane) => {
  if (workspaceLayout.maximizedPane) {
    return workspaceLayout.maximizedPane === pane ? "" : "lg:hidden";
  }
  if (pane === "timeline") return "";
  return responsiveVisibility[pane] ? "" : "lg:hidden";
};
```

Do not dispatch a layout action from the resize listener; the saved `workspaceLayout.visible` values must remain unchanged when the window crosses a breakpoint.

- [ ] **Step 7: Replace the multi-row chrome with the compact title bar**

Import `WorkspaceLayoutMenu`. Make these exact edits to the existing header without replacing its working callbacks:

1. Replace the current `<header>` opening tag with the opening tag below.
2. Change both existing Home and Codex icon buttons from `h-8 w-8 rounded-md` to `h-7 w-7 rounded-[5px]`; leave their children, click handlers, titles, and ARIA labels unchanged.
3. Replace the current centered project-title/status wrapper with the exact `<div className="min-w-0 text-center">` block below.
4. Delete the current always-visible layout preset, pane visibility, maximize, render-quality, and Render controls from the header and mount `WorkspaceLayoutMenu` once at the start of the existing right-hand actions wrapper.
5. Change the existing Export trigger from `h-8` to `h-7`; leave its menu, disabled state, callbacks, and result messages unchanged.

```tsx
<header
  role="banner"
  aria-label="Project editor chrome"
  className="grid h-9 shrink-0 grid-cols-[1fr_minmax(0,2fr)_1fr] items-center border-b border-white/12 bg-[#0a0a0a] px-2 py-0"
>
```

Use this exact centered project-title/status block:

```tsx
<div className="min-w-0 text-center">
  <h1 className="truncate text-xs font-semibold" title={`${project.name} - Edited`}>
    {project.name} <span className="font-normal text-muted-foreground">- Edited</span>
  </h1>
  <span role={projectSaveStatus === "failed" ? "alert" : "status"} className="sr-only">
    {projectSaveStatus === "saved"
      ? "Saved"
      : projectSaveStatus === "saving"
        ? "Saving"
        : projectSaveStatus === "failed"
          ? "Save failed"
          : "Unsaved changes"}
  </span>
</div>
```

Mount this exact menu invocation immediately before the current Export wrapper:

```tsx
<WorkspaceLayoutMenu
  layout={workspaceLayout}
  renderQuality={selectedRenderQuality}
  renderRunning={exportStatus === "exporting"}
  onSetPreset={(preset) => dispatchWorkspaceLayout({ type: "setPreset", preset })}
  onTogglePane={(pane) => dispatchWorkspaceLayout({ type: "togglePane", pane })}
  onToggleMaximize={() => dispatchWorkspaceLayout({ type: "toggleMaximize" })}
  onRenderQualityChange={setSelectedRenderQuality}
  onRender={(quality) => void exportWebm(quality)}
/>
```

- [ ] **Step 8: Apply Palmier's five-pixel shell spacing**

Replace the two layout class constants/strings with:

```tsx
const editorPaneBaseClassName =
  "grid min-h-0 min-w-0 w-full flex-1 auto-rows-[minmax(20rem,max-content)] grid-cols-1 gap-[5px] overflow-auto p-[5px] lg:grid-rows-[minmax(0,1fr)] lg:overflow-hidden";
```

Use this class on the preset wrapper:

```tsx
className={`${workspaceLayout.maximizedPane === "codex" ? "lg:hidden" : ""} flex min-h-0 min-w-0 flex-col gap-[5px] lg:grid`}
```

Change the preview/timeline wrapper's `gap-3` to `gap-[5px]`; keep local overflow and the compact one-pane classes unchanged.

- [ ] **Step 9: Run the menu and workspace tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/workspace-layout-menu.test.tsx src/components/workspace/editor-workspace.test.tsx src/lib/workspace-layout-state.test.ts
```

Expected: PASS; the compact menu callbacks, title-bar contract, pane-budget tests, and existing editor behavior tests all exit `0`.

- [ ] **Step 10: Commit the shell slice**

```bash
rtk git add src/components/workspace/workspace-layout-menu.tsx src/components/workspace/workspace-layout-menu.test.tsx
rtk git add -p src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git diff --cached --check
rtk git commit -m "feat(editor): compact the Palmier-first workspace shell"
```

Expected: one Conventional Commit containing the compact menu, responsive shell integration, five-pixel gaps, and focused tests; unrelated dirty editor hunks remain unstaged.

### Task 3: Let the preview fill its pane and build Palmier's five-control transport

**Files:**
- Modify: `src/components/workspace/preview-panel.tsx:1-365,942-1214`
- Test: `src/components/workspace/preview-panel.test.tsx:509-560,896-929,1156-1188,1268-1315,1548-1604`

- [ ] **Step 1: Save the dirty preview baseline**

Run:

```bash
rtk git diff -- src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-03-preview.patch
```

Expected: exit `0`; no implementation file changes.

- [ ] **Step 2: Replace the stale bounded-height test with a failing fill contract**

Replace the test named `bounds the desktop preview viewport so timeline tools stay in the first viewport` with:

```tsx
it("fills the allocated preview pane without a viewport-height cap", () => {
  render(
    <PreviewPanelWithViewerTabs
      viewerMode="timeline"
      timeline={timelinePreviewTimeline}
      selectedItem={timelinePreviewTimeline.tracks[0].items[0]}
      onSelectViewerMode={vi.fn()}
    />,
  );

  const panel = screen.getByRole("region", { name: "Preview panel" });
  const viewport = screen.getByRole("region", { name: "Preview viewport" });
  expect(panel).toHaveClass("flex", "h-full", "min-h-0", "flex-col");
  expect(viewport).toHaveClass("min-h-[320px]", "flex-1");
  expect(viewport).not.toHaveClass("lg:max-h-[min(28vh,24rem)]");
  expect(viewport).not.toHaveClass("aspect-video");
  expect(screen.queryByRole("button", { name: "Play preview" })).not.toBeInTheDocument();
});
```

Add this transport test:

```tsx
it("exposes Palmier start, frame, play, frame, and end transport controls", () => {
  const onSeekTimelinePlayback = vi.fn();
  render(
    <PreviewPanelWithViewerTabs
      viewerMode="timeline"
      timeline={timelinePreviewTimeline}
      selectedItem={timelinePreviewTimeline.tracks[0].items[0]}
      playheadSeconds={3}
      onSeekTimelinePlayback={onSeekTimelinePlayback}
      onToggleTimelinePlayback={vi.fn()}
      onSelectViewerMode={vi.fn()}
    />,
  );

  const transport = screen.getByRole("region", { name: "Preview transport" });
  for (const name of [
    "Preview jump to start",
    "Preview step backward",
    "Preview transport play",
    "Preview step forward",
    "Preview jump to end",
  ]) {
    expect(within(transport).getByRole("button", { name })).toBeInTheDocument();
  }
  fireEvent.click(within(transport).getByRole("button", { name: "Preview jump to start" }));
  fireEvent.click(within(transport).getByRole("button", { name: "Preview jump to end" }));
  expect(onSeekTimelinePlayback).toHaveBeenNthCalledWith(1, 0);
  expect(onSeekTimelinePlayback).toHaveBeenNthCalledWith(2, 12);
});
```

- [ ] **Step 3: Run the preview test and verify it fails**

Run: `rtk pnpm test -- src/components/workspace/preview-panel.test.tsx`

Expected: FAIL because the preview still has `28vh`, the large central play button is mounted, and start/end controls do not exist.

- [ ] **Step 4: Replace `PreviewTransport` with the separated scrubber and five-button row**

Keep `previewTransportData`, time formatting, stepping, source/timeline callbacks, and badges. Replace only the JSX returned by `PreviewTransport` with:

```tsx
return (
  <section aria-label="Preview transport" className="shrink-0 border-t border-white/12 text-xs">
    <input
      type="range"
      aria-label="Preview scrubber"
      aria-valuetext={formatPreviewCurrentTime(scrubberValue)}
      min={0}
      max={scrubberMax}
      step={0.001}
      value={scrubberValue}
      disabled={scrubberDisabled}
      className="block h-1 w-full cursor-pointer appearance-none bg-transparent accent-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
      style={{
        "--preview-scrubber-progress": `${roundedProgress}%`,
        background: `linear-gradient(to right, hsl(var(--foreground)) 0%, hsl(var(--foreground)) ${roundedProgress}%, hsl(var(--muted)) ${roundedProgress}%, hsl(var(--muted)) 100%)`,
      } as CSSProperties}
      onChange={(event) => {
        const nextSeconds = Number(event.currentTarget.value);
        if (Number.isFinite(nextSeconds)) onSeekPlayback?.(nextSeconds);
      }}
    />
    <div className="grid h-9 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center px-3">
      <div className="truncate font-mono text-[11px] text-foreground">
        {currentTimeLabel} / {transport.durationLabel}
      </div>
      <div className="flex items-center gap-2">
        <button type="button" aria-label="Preview jump to start" title="Jump to start" disabled={!canSeekPlayback} className={previewTransportButtonClassName} onClick={() => onSeekPlayback?.(0)}>
          <SkipBack className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
        <button type="button" aria-label="Preview step backward" title={timelineTransport ? "Step timeline backward" : "Step source backward (Left Arrow)"} disabled={!canStepPlayback} className={previewTransportButtonClassName} onClick={canStepPlayback ? onStepBackward : undefined}>
          <ChevronLeft className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
        <button type="button" aria-label={isPlaying ? "Preview transport pause" : "Preview transport play"} title={isPlaying ? "Pause preview (Space)" : "Play preview (Space)"} disabled={!canTogglePlayback} className={previewTransportButtonClassName} onClick={canTogglePlayback ? onTogglePlayback : undefined}>
          {isPlaying ? <Pause className="h-3.5 w-3.5" aria-hidden="true" /> : <Play className="h-3.5 w-3.5" aria-hidden="true" />}
        </button>
        <button type="button" aria-label="Preview step forward" title={timelineTransport ? "Step timeline forward" : "Step source forward (Right Arrow)"} disabled={!canStepPlayback} className={previewTransportButtonClassName} onClick={canStepPlayback ? onStepForward : undefined}>
          <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
        <button type="button" aria-label="Preview jump to end" title="Jump to end" disabled={!canSeekPlayback} className={previewTransportButtonClassName} onClick={() => onSeekPlayback?.(scrubberMax)}>
          <SkipForward className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>
      <div className="flex min-w-0 items-center justify-end gap-1">
        {transport.primaryLabel ? <span className="truncate text-[10px] text-muted-foreground">{transport.primaryLabel}</span> : null}
        {transport.badges.map((badge) => <span key={badge} className="px-1 text-[10px] font-medium text-muted-foreground">{badge}</span>)}
      </div>
    </div>
  </section>
);
```

Add this constant above `PreviewTransport`:

```ts
const previewTransportButtonClassName =
  "flex h-7 w-7 items-center justify-center rounded-[5px] text-muted-foreground transition hover:bg-white/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40";
```

- [ ] **Step 5: Make the preview pane height-owned and remove the central overlay**

Make four surgical class/element edits so the source renderer and compositor JSX remain untouched:

1. Change the outer `PreviewPanel` section class to `flex h-full min-h-0 min-w-0 flex-col`.
2. Change the tabs wrapper class to `shrink-0 px-2 pt-1`.
3. Change the wrapper around the viewport, `PreviewTransport`, and conditional `RenderReportPanel` to `flex min-h-0 flex-1 flex-col px-2 pb-2`.
4. Replace the viewport class with the exact value below; this removes the `aspect-video` and `lg:max-h-[min(28vh,24rem)]` caps while keeping the viewport at least 400x320 on desktop.

```tsx
className="relative flex min-h-[320px] min-w-[400px] flex-1 items-center justify-center overflow-hidden bg-neutral-950 text-white max-[1023px]:min-w-0"
```

Delete only the `Button` with `aria-label={previewIsPlaying ? "Pause preview" : "Play preview"}` that sits inside `Preview viewport`. Do not edit the source `<video>` / `<img>` branches, `TimelinePreviewCompositor`, canonical-frame rendering, recovery alerts, transport prop values, conditional report panel, or the shared Space-key handler.

- [ ] **Step 6: Update large-control tests to use the transport control**

In the source playback tests, replace queries for `Play preview` / `Pause preview` with `Preview transport play` / `Preview transport pause`. Keep the same `HTMLMediaElement.play` and `pause` assertions. Keep the editable-focus and image-source disabled tests, but target the transport button.

- [ ] **Step 7: Run preview and compositor regressions**

Run:

```bash
rtk pnpm test -- src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx
```

Expected: PASS; the fill contract, five-control transport, source playback, timeline playback, frame stepping, canonical layers, crop/transform, failure recovery, and keyboard tests all exit `0`.

- [ ] **Step 8: Commit the preview slice**

```bash
rtk git add -p src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx
rtk git diff --cached --check
rtk git commit -m "feat(preview): fill the Palmier viewer pane"
```

Expected: one Conventional Commit containing only preview layout/transport and test changes.

### Task 4: Reconcile variable track geometry and apply Palmier timeline density

**Files:**
- Modify: `src/components/workspace/timeline-editor.tsx:1-205,450-753,1473-1779,2694-2745,3280-4260`
- Test: `src/components/workspace/timeline-editor.test.tsx:1-3065`
- Modify: `src/components/workspace/editor-workspace.tsx:3615-3625,8575-8984`
- Test: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Save current and committed timeline references without changing either**

Run:

```bash
rtk git diff -- src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-04-timeline-current.patch
rtk git show HEAD:src/components/workspace/timeline-editor.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-04-timeline-head.tsx
rtk git show HEAD:src/components/workspace/timeline-editor.test.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-04-timeline-head.test.tsx
```

Expected: exit `0`; the current dirty sync-lock/ripple implementation and the committed variable-geometry implementation are both available for comparison under ignored `output/`.

- [ ] **Step 2: Add failing geometry and styling tests**

Restore/adapt the committed variable-height test and add the Palmier constants test in `timeline-editor.test.tsx`:

```tsx
it("shares 50px default and persisted variable geometry across headers, canvas, and clips", () => {
  const timeline = {
    ...sampleTimeline,
    tracks: sampleTimeline.tracks.map((track) => ({
      ...track,
      displayHeight: track.id === "track-video" ? 32 : undefined,
    })),
  };
  render(<TimelineEditor timeline={timeline} selectedItemId="caption-1" />);

  expect(screen.getByTestId("timeline-ruler-header")).toHaveStyle({ height: "24px" });
  expect(screen.getByTestId("timeline-track-grid")).toHaveStyle({
    gridTemplateColumns: "100px minmax(0, 1fr)",
  });
  expect(screen.getByTestId("timeline-track-label-track-video")).toHaveStyle({ height: "32px" });
  expect(screen.getByTestId("timeline-track-label-track-scenes")).toHaveStyle({ height: "50px" });
  expect(screen.getByRole("button", { name: "Opening clip" }).parentElement).toHaveStyle({
    top: "4px",
    height: "24px",
  });
});

it("uses a 38px flat toolbar, subtle selection, and full-height audio waveform", () => {
  render(<TimelineEditor timeline={sampleTimeline} selectedItemId="music-bed" />);

  expect(screen.getByRole("toolbar", { name: "Timeline tools" })).toHaveClass("h-[38px]");
  const selected = screen.getByRole("button", { name: "Music bed" });
  expect(selected).toHaveClass("ring-1");
  expect(selected).not.toHaveClass("ring-2", "ring-offset-1");
  expect(within(selected).getByRole("img", { name: "Waveform for Music bed" })).toHaveClass(
    "inset-y-1",
  );
});
```

Restore/adapt this accessibility test from the committed file, retaining current sync-lock controls in the fixture:

```tsx
it("previews, commits, cancels, and keyboard-resizes track height accessibly", () => {
  const onSetTrackDisplayHeight = vi.fn();
  render(
    <TimelineEditor
      timeline={sampleTimeline}
      selectedItemId="caption-1"
      onSetTrackDisplayHeight={onSetTrackDisplayHeight}
    />,
  );
  const handle = screen.getByRole("separator", { name: "Resize Video track height" });
  fireEvent(handle, new MouseEvent("pointerdown", { bubbles: true, clientY: 100 }));
  fireEvent(handle, new MouseEvent("pointermove", { bubbles: true, clientY: 130 }));
  expect(screen.getByTestId("timeline-track-label-track-video")).toHaveStyle({ height: "80px" });
  fireEvent(handle, new MouseEvent("pointerup", { bubbles: true, clientY: 130 }));
  expect(onSetTrackDisplayHeight).toHaveBeenCalledWith("track-video", 80);
  fireEvent.keyDown(handle, { key: "ArrowUp" });
  expect(onSetTrackDisplayHeight).toHaveBeenLastCalledWith("track-video", 48);
});
```

- [ ] **Step 3: Run timeline tests and verify the dirty-tree regression is visible**

Run: `rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx`

Expected: FAIL because the dirty tree currently fixes `rowHeight` at `44`, uses 112px headers and a 32px ruler, and removes the `onSetTrackDisplayHeight` path.

- [ ] **Step 4: Restore the variable-height geometry core while retaining current sync-lock/ripple code**

Replace `const rowHeight = 44` with the committed geometry core:

```ts
const defaultTrackDisplayHeight = 50;
const minimumTrackDisplayHeight = 32;
const maximumTrackDisplayHeight = 200;
const trackVerticalInset = 4;
const timelineRulerHeight = 24;
const timelineTrackHeaderWidth = 100;

function trackDisplayHeight(track: TimelineTrack) {
  const stored = track.displayHeight;
  return typeof stored === "number" && Number.isFinite(stored)
    ? clampTrackDisplayHeight(stored)
    : defaultTrackDisplayHeight;
}

function clampTrackDisplayHeight(value: number) {
  return Math.min(maximumTrackDisplayHeight, Math.max(minimumTrackDisplayHeight, value));
}

function buildTimelineTrackGeometry(tracks: readonly TimelineTrack[]) {
  let top = 0;
  const entries = tracks.map((track, index) => {
    const height = trackDisplayHeight(track);
    const entry = { trackId: track.id, index, top, height, bottom: top + height };
    top += height;
    return entry;
  });
  const byId = new Map(entries.map((entry) => [entry.trackId, entry]));
  const trackIndexAtY = (y: number) => {
    if (entries.length === 0) return -1;
    const clampedY = Math.min(Math.max(y, 0), Math.max(0, top - 0.001));
    return entries.find((entry) => clampedY >= entry.top && clampedY < entry.bottom)?.index ?? entries.length - 1;
  };
  return { entries, byId, totalHeight: top, trackIndexAtY };
}
```

Add this prop to `TimelineEditorProps`, destructure it, and keep the current `onToggleTrackSyncLocked` and `onReorderTrack` props:

```ts
onSetTrackDisplayHeight?: (trackId: string, displayHeight: number) => void;
```

Add this state and derived geometry after `trackById`:

```ts
const [trackHeightPreviews, setTrackHeightPreviews] = useState<Record<string, number>>({});
const trackGeometry = useMemo(
  () => buildTimelineTrackGeometry(
    timeline.tracks.map((track) => ({
      ...track,
      displayHeight: trackHeightPreviews[track.id] ?? track.displayHeight,
    })),
  ),
  [timeline.tracks, trackHeightPreviews],
);
const canvasHeight = trackGeometry.totalHeight;
```

Restore this complete keyboard- and pointer-accessible `TrackHeightHandle` before `stringProperty`; do not replace adjacent `TrackControls` or `TrackReorderControls`:

```tsx
function TrackHeightHandle({
  track,
  displayHeight,
  onPreview,
  onCommit,
  onCancel,
}: {
  track: TimelineTrack;
  displayHeight: number;
  onPreview: (displayHeight: number) => void;
  onCommit: (displayHeight: number) => void;
  onCancel: () => void;
}) {
  const dragRef = useRef<{
    pointerId: number;
    startY: number;
    startHeight: number;
    previewHeight: number;
  } | null>(null);
  return (
    <span
      role="separator"
      tabIndex={0}
      aria-label={`Resize ${track.name} track height`}
      aria-orientation="horizontal"
      aria-valuemin={minimumTrackDisplayHeight}
      aria-valuemax={maximumTrackDisplayHeight}
      aria-valuenow={displayHeight}
      title="Resize track height"
      className="absolute inset-x-0 -bottom-1 z-30 h-2 cursor-row-resize bg-transparent outline-none transition-colors hover:bg-cyan-400/30 focus-visible:bg-cyan-400/40 focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-cyan-300"
      onPointerDown={(event) => {
        event.preventDefault();
        event.stopPropagation();
        event.currentTarget.setPointerCapture?.(event.pointerId);
        dragRef.current = {
          pointerId: event.pointerId,
          startY: event.clientY,
          startHeight: displayHeight,
          previewHeight: displayHeight,
        };
      }}
      onPointerMove={(event) => {
        const drag = dragRef.current;
        if (!drag || drag.pointerId !== event.pointerId) return;
        const previewHeight = clampTrackDisplayHeight(
          drag.startHeight + event.clientY - drag.startY,
        );
        drag.previewHeight = previewHeight;
        onPreview(previewHeight);
      }}
      onPointerUp={(event) => {
        const drag = dragRef.current;
        if (!drag || drag.pointerId !== event.pointerId) return;
        event.preventDefault();
        event.stopPropagation();
        dragRef.current = null;
        onCommit(drag.previewHeight);
      }}
      onPointerCancel={() => {
        dragRef.current = null;
        onCancel();
      }}
      onKeyDown={(event) => {
        if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
        event.preventDefault();
        event.stopPropagation();
        const step = event.shiftKey ? 10 : 2;
        const nextHeight = clampTrackDisplayHeight(
          displayHeight + (event.key === "ArrowDown" ? step : -step),
        );
        onCommit(nextHeight);
      }}
    />
  );
}
```

Mount it at the bottom of every track-header row only when the track and persistence callback exist:

```tsx
<TrackHeightHandle
  track={track}
  displayHeight={trackGeometry.byId.get(track.id)?.height ?? defaultTrackDisplayHeight}
  onPreview={(displayHeight) =>
    setTrackHeightPreviews((current) => ({ ...current, [track.id]: displayHeight }))
  }
  onCommit={(displayHeight) => {
    setTrackHeightPreviews((current) => {
      const next = { ...current };
      delete next[track.id];
      return next;
    });
    onSetTrackDisplayHeight?.(track.id, displayHeight);
  }}
  onCancel={() =>
    setTrackHeightPreviews((current) => {
      const next = { ...current };
      delete next[track.id];
      return next;
    })
  }
/>
```

- [ ] **Step 5: Replace fixed row arithmetic with cumulative geometry**

Use these exact expressions everywhere the current dirty file uses `rowIndex * rowHeight`, `rows.length * rowHeight`, or `rowHeight - 8`:

```ts
const geometry = trackGeometry.entries[rowIndex];
const rowTop = geometry?.top ?? 0;
const rowHeight = geometry?.height ?? defaultTrackDisplayHeight;
const clipTop = rowTop + trackVerticalInset;
const clipHeight = Math.max(1, rowHeight - trackVerticalInset * 2);
```

Apply them to gap selection, marquee hit testing, move target resolution, duplicate vertical translation, ripple previews, clip wrappers, drop targets, canvas accent rails, and filmstrip `trackHeight`. The exact rendered wrapper style is:

```tsx
style={{
  left: `${previewStartSeconds * pixelsPerSecond}px`,
  top: `${clipTop}px`,
  height: `${clipHeight}px`,
  width: `${previewDurationSeconds * pixelsPerSecond}px`,
}}
```

The exact header/drop-row style is:

```tsx
style={{ height: `${trackGeometry.byId.get(row.id)?.height ?? defaultTrackDisplayHeight}px` }}
```

Use `trackGeometry.trackIndexAtY(pointerY)` for vertical pointer-to-track hit testing and use the difference between destination/source `geometry.top` values for duplicate/move preview translation. Preserve the current ripple-resize/shift preview calculations by substituting only their vertical values.

- [ ] **Step 6: Apply the 100px header, 24px ruler, 38px toolbar, and flat clip styling**

Use inline grid geometry so Tailwind cannot purge dynamic classes:

```tsx
<div
  data-testid="timeline-ruler-header"
  className="grid bg-[#161616] text-xs font-medium text-muted-foreground"
  style={{
    gridTemplateColumns: `${timelineTrackHeaderWidth}px minmax(0, 1fr)`,
    height: `${timelineRulerHeight}px`,
  }}
>
```

```tsx
<div
  data-testid="timeline-track-grid"
  className="grid"
  style={{ gridTemplateColumns: `${timelineTrackHeaderWidth}px minmax(0, 1fr)` }}
>
```

Change the toolbar and control treatment to:

```tsx
className="flex h-[38px] min-w-0 items-center justify-between gap-2 border-b border-white/12 bg-[#161616] px-2"
```

```ts
const timelineToolButtonClassName =
  "flex h-7 w-7 items-center justify-center rounded-[5px] border border-transparent text-muted-foreground transition hover:bg-white/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40";
```

Use `timelineToolButtonClassName` for unselected Undo, Redo, Select, Razor, Split, marks, text, template, Zoom Out, and Zoom In controls; add `bg-white/10 text-foreground` for the active Select/Razor mode.

Replace the selection and clip base in `itemClassName` with:

```ts
const selectedClasses = selected
  ? "ring-1 ring-inset ring-white/70"
  : "hover:brightness-110";
return `relative h-full w-full overflow-hidden rounded-[3px] border px-1.5 py-1 text-left text-[11px] ${kindClasses} ${selectedClasses} ${disabledClasses} ${cursorClass}`;
```

Change the waveform class to:

```tsx
className="pointer-events-none absolute inset-x-1 inset-y-1 z-0 flex items-center gap-px opacity-80"
```

Keep speech/dead-air masks, peaks, automation, fades, source boundaries, labels, and AI/job badges mounted above or below the waveform exactly as they are now.

- [ ] **Step 7: Restore canonical track-height persistence through EditorWorkspace**

Add this function beside other track mutations in `editor-workspace.tsx`:

```ts
function setTimelineTrackDisplayHeight(trackId: string, displayHeight: number) {
  void applyProjectAction({ type: "setTrackDisplayHeight", trackId, displayHeight });
}
```

Pass it to `TimelineEditor`:

```tsx
onSetTrackDisplayHeight={setTimelineTrackDisplayHeight}
```

Keep the dirty tree's current `onToggleTrackSyncLocked`, `onReorderTrack`, ripple planning, filmstrip request, selection, and project-action callbacks alongside it.

- [ ] **Step 8: Run timeline and workspace regressions**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS; variable-height persistence/resize, 50/100/24 geometry, 38px toolbar, filmstrips, full waveforms, subtle selection, sync locks, ripple, snapping, multi-selection, automation, fades, and workspace action routing all exit `0`.

- [ ] **Step 9: Commit the reconciled timeline slice**

```bash
rtk git add -p src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git diff --cached --check
rtk git commit -m "feat(timeline): restore Palmier track geometry"
```

Expected: one Conventional Commit containing the geometry reconciliation, Palmier styling, persistence callback, and tests; unrelated dirty timeline/editor hunks remain unstaged.

### Task 5: Flatten the new-chat Codex rail and add safe starter actions

**Files:**
- Modify: `src/components/workspace/agent-panel.tsx:1-42,2360-3070`
- Test: `src/components/workspace/agent-panel.test.tsx:2086-2140`

- [ ] **Step 1: Save the dirty AgentPanel baseline**

Run:

```bash
rtk git diff -- src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx > output/parity-audit-2026-07-13/dirty-baselines/task-05-agent-panel.patch
```

Expected: exit `0`; existing session/mention/proposal changes are recorded and untouched.

- [ ] **Step 2: Write the failing starter-action test**

Add this test to `agent-panel.test.tsx`:

```tsx
it("populates the composer from Palmier starter actions without running silently", () => {
  const onGenerateEdit = vi.fn();
  render(
    <AgentPanel
      mediaId="media-hero"
      transcriptionModelReady
      runtimeReady
      onGenerateEdit={onGenerateEdit}
    />,
  );

  const starters = screen.getByRole("group", { name: "Codex starter actions" });
  expect(within(starters).getAllByRole("button")).toHaveLength(7);
  fireEvent.click(within(starters).getByRole("button", { name: "Generate B-roll" }));
  expect(screen.getByLabelText("Prompt")).toHaveValue(
    "Generate B-roll for my timeline. Inspect the current edit, identify sections that would benefit from cutaways, generate suitable B-roll, and place it where it supports the story.",
  );
  expect(onGenerateEdit).not.toHaveBeenCalled();
});
```

Update the existing rail-chrome test to expect `min-h-0`, no `min-h-[520px]`, no `max-h-[calc(100vh-7rem)]`, and a composer with `border-t` but without `rounded-md` or `shadow-sm`.

- [ ] **Step 3: Run the AgentPanel test and verify it fails**

Run: `rtk pnpm test -- src/components/workspace/agent-panel.test.tsx`

Expected: FAIL because the starter group does not exist and the rail/composer still use capped, card-heavy classes.

- [ ] **Step 4: Add the exact Palmier starter catalog**

Add `Aperture`, `Captions`, `Folder`, `Music`, `Sparkles`, `AudioWaveform`, and `type LucideIcon` to the lucide import. Add this catalog above `AgentPanel`:

```ts
interface AgentStarterAction {
  title: string;
  prompt: string;
  icon: LucideIcon;
}

const agentStarterActions: readonly AgentStarterAction[] = [
  { title: "Generate an AI video", icon: Sparkles, prompt: "Generate an AI video of " },
  { title: "Generate B-roll", icon: Film, prompt: "Generate B-roll for my timeline. Inspect the current edit, identify sections that would benefit from cutaways, generate suitable B-roll, and place it where it supports the story." },
  { title: "Create a letterbox opening", icon: Aperture, prompt: "Create a cinematic opening for my timeline. Use the first visual clip, animate a subtle letterbox matte with top and bottom crop keyframes, starting from crop to uncrop, and keep the motion restrained and polished." },
  { title: "Add captions to my timeline", icon: Captions, prompt: "Add captions to my timeline. Transcribe spoken audio in timeline clips, build readable caption phrases on word boundaries, and place them as text clips aligned to the edit." },
  { title: "Create a voiceover", icon: AudioWaveform, prompt: "Create a voiceover for my timeline. Draft concise narration for the current edit, generate the voiceover, and add it to an audio track aligned with the timeline." },
  { title: "Generate music and sync to my timeline", icon: Music, prompt: "Score my timeline with music. Inspect the edit's mood and pacing, generate music for the full timeline, and place it on an audio track aligned to the edit." },
  { title: "Organize my media into structured folders", icon: Folder, prompt: "Organize my media into structured folders. Review all assets, create clearly named folders by role, scene, or type, move assets into them, and rename generic files when useful. Don't delete anything or change the timeline." },
];
```

- [ ] **Step 5: Render starters only for a genuinely empty chat and keep diagnostics disclosed**

Define:

```ts
const showStarterActions =
  appServerConversationEntries.length === 0 &&
  transcriptEntries.length === 0 &&
  !showLatestRequest;
```

At the top of the chat scroll area, render:

```tsx
{showStarterActions ? (
  <section className="grid place-items-center px-3 py-8 text-xs">
    <div className="w-full max-w-sm">
      <p className="mb-2 text-center font-medium text-muted-foreground">
        Ask anything, or start with:
      </p>
      <div role="group" aria-label="Codex starter actions" className="grid gap-1">
        {agentStarterActions.map((starter) => {
          const Icon = starter.icon;
          return (
            <button
              key={starter.title}
              type="button"
              className="flex min-h-8 items-center gap-2 rounded-[5px] border border-white/12 bg-white/[0.03] px-2.5 py-1.5 text-left font-medium text-foreground transition hover:bg-white/[0.08] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              onClick={() => {
                setPrompt(starter.prompt);
                setDismissedMentionKey(null);
              }}
            >
              <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              <span>{starter.title}</span>
            </button>
          );
        })}
      </div>
      <details className="mt-3 border-t border-white/10 pt-2 text-[11px] text-muted-foreground">
        <summary className="cursor-pointer">Project context diagnostics</summary>
        <div className="mt-2 grid gap-1">
          {defaultContextTranscriptEntries.map((entry) => (
            <TranscriptToolRow key={entry.id} entry={entry} />
          ))}
        </div>
      </details>
    </div>
  </section>
) : null}
```

Immediately after the starter section, insert `{!showStarterActions ? (<>` before the current `appServerConversationEntries.slice(-3)` expression and insert `</>) : null}` after the final proposal/template card. This gates the already-implemented chat transcript without rewriting it. Keep the existing app-server turns, transcript entries, statuses, jobs, proposals, errors, review/apply controls, and undo actions byte-for-byte inside that fragment.

Change the rail and composer classes to:

```tsx
className="flex h-full min-h-0 flex-col overflow-hidden"
```

```tsx
className="shrink-0 space-y-2 border-t border-white/12 bg-[#161616] p-2"
```

- [ ] **Step 6: Run all AgentPanel tests**

Run: `rtk pnpm test -- src/components/workspace/agent-panel.test.tsx`

Expected: PASS; starter clicks populate only the prompt, session controls remain available, mention resolution and direct canonical actions retain their behavior, proposals still require review/apply, and the rail no longer caps itself against the workspace pane.

- [ ] **Step 7: Commit the Codex starter slice**

```bash
rtk git add -p src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx
rtk git diff --cached --check
rtk git commit -m "feat(agent): add Palmier-style starter actions"
```

Expected: one Conventional Commit containing only new-chat hierarchy/starter and focused test changes.

### Task 6: Verify same-state visuals, update the parity tracker, and run the full regression gate

**Files:**
- Modify: `docs/parity.md:406-503`
- Evidence only, ignored: `output/parity-audit-2026-07-13/editor-shell/`

- [ ] **Step 1: Run focused and full frontend verification**

Run:

```bash
rtk pnpm test -- src/lib/workspace-layout-state.test.ts src/components/workspace/workspace-layout-menu.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/agent-panel.test.tsx
rtk pnpm lint
rtk pnpm build
rtk pnpm test
rtk git diff --check
```

Expected: every command exits `0`; Vitest reports no failed tests, TypeScript reports no errors, Vite produces `dist/`, and diff checking prints no whitespace errors.

- [ ] **Step 2: Start the browser build for visual verification**

Run in a dedicated terminal:

```bash
rtk pnpm dev
```

Expected: Vite reports `http://127.0.0.1:1420` (or the configured local port) and remains running. If Vite selects another port, pass that exact URL to every command below.

- [ ] **Step 3: Run the existing durable visual-QA matrix**

Run:

```bash
rtk pnpm visual:qa:browser -- --url http://127.0.0.1:1420 --out output/parity-audit-2026-07-13/editor-shell/full-matrix
```

Expected: exit `0`; all declared screenshots are non-empty and every scenario reports zero document/root horizontal overflow.

- [ ] **Step 4: Capture the additional 1280x720 and 1024x768 shell states**

Run:

```bash
rtk mkdir -p output/parity-audit-2026-07-13/editor-shell
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell open http://127.0.0.1:1420
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell run-code 'async (page) => { const home = page.getByRole("region", { name: "Project home" }); if (await home.count()) await page.getByRole("button", { name: "Open sample project" }).click(); await page.getByRole("main", { name: "Video editor workspace" }).waitFor(); }'
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell resize 1280 720
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell run-code 'async (page) => { const result = await page.evaluate(() => ({ width: innerWidth, scrollWidth: document.documentElement.scrollWidth, codex: getComputedStyle(document.querySelector("[aria-label=\"Codex agent rail\"]")).display, inspector: getComputedStyle(document.querySelector("[aria-label=\"Inspector rail\"]")).display })); if (result.width !== 1280 || result.scrollWidth > result.width || result.codex !== "none" || result.inspector === "none") throw new Error(JSON.stringify(result)); }'
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell screenshot --filename output/parity-audit-2026-07-13/editor-shell/video-creater-editor-1280x720.png
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell resize 1024 768
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell run-code 'async (page) => { const result = await page.evaluate(() => ({ width: innerWidth, scrollWidth: document.documentElement.scrollWidth, codex: getComputedStyle(document.querySelector("[aria-label=\"Codex agent rail\"]")).display, inspector: getComputedStyle(document.querySelector("[aria-label=\"Inspector rail\"]")).display })); if (result.width !== 1024 || result.scrollWidth > result.width || result.codex !== "none" || result.inspector !== "none") throw new Error(JSON.stringify(result)); }'
rtk /Users/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh --session palmier-editor-shell screenshot --filename output/parity-audit-2026-07-13/editor-shell/video-creater-editor-1024x768.png
```

Expected: both assertion calls exit `0`; the 1280 state auto-collapses Codex but keeps Media/Viewer/Inspector, the 1024 state auto-collapses Codex and Inspector, both screenshots are non-empty, and neither viewport overflows horizontally.

- [ ] **Step 5: Build and inspect the same-state side-by-side artifact**

Run:

```bash
rtk ffmpeg -y -i output/parity-audit-2026-07-13/reference/02-editor.png -i output/parity-audit-2026-07-13/editor-shell/video-creater-editor-1280x720.png -filter_complex '[0:v]scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=black[left];[1:v]scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=black[right];[left][right]hstack=inputs=2' -frames:v 1 output/parity-audit-2026-07-13/editor-shell/palmier-video-creater-editor-side-by-side.png
```

Expected: ffmpeg exits `0` and writes a non-empty 2560x720 PNG. Inspect it with the image viewer and reject the slice if any of these remain visibly wrong: title bar wraps; pane gaps exceed 5px; viewer is not dominant in the upper deck; timeline receives less height than the preview deck; ruler/header/rows do not align; filmstrips or waveforms leave most clip height unused; the selected clip has a thick offset ring; the empty Codex rail begins with diagnostic cards instead of starter actions.

- [ ] **Step 6: Record accurate evidence and remaining limits in `docs/parity.md`**

Change the audit verdict so it no longer claims all-screen similarity, then add this subsection after the existing editor evidence:

```md
### Palmier-first shell evidence — 2026-07-13

The approved Palmier-first shell slice is implemented and visually verified for the default editor, preview transport, representative video/audio timeline clips, and empty Codex starter state. The desktop title bar is one 36px band; panel gaps are 5px; the default pane widths are 500px Media, 260px Inspector, and 240px Codex; the responsive budget preserves stored preferences while auto-collapsing Codex at 1280x720 and Codex plus Inspector at 1024x768. The preview owns its pane without the former 28vh cap, and its separated scrubber/transport exposes start, frame-back, play/pause, frame-forward, and end. Timeline defaults are 50px rows, a 100px header, a 24px ruler, and a 38px flat toolbar while persisted 32-200px track resizing remains canonical.

Evidence is retained under `output/parity-audit-2026-07-13/editor-shell/`: the complete browser matrix, `video-creater-editor-1280x720.png`, `video-creater-editor-1024x768.png`, and `palmier-video-creater-editor-side-by-side.png`. Focused tests, the full frontend suite, lint, production build, zero-overflow assertions, and manual same-state inspection passed.

This closes only the shell/preview/timeline/starter slice. Project home, media generation, captions/speech, contextual inspector states, and Export require their own same-state captures before the tracker can claim all-screen visual parity. Palmier's preview snapshot action is not represented by a real Video Creater action in this slice and remains tracked rather than shipped as a decorative control.
```

Replace the sentence `Current verdict: closed for the audited editor UI scope` with:

```md
**Current verdict: reopened for the 2026-07-13 Palmier-first visual contract.** Earlier functional audit items remain valid, but fresh installed-app screenshots supersede the prior broad visual-closure wording. Each newer slice is closed only after focused tests and inspected same-state evidence; the shell/preview/timeline/starter slice below is verified, while the other named screens remain open.
```

Keep every Video Creater-only capability row. Do not delete earlier evidence; label it as functional or historical where it conflicts with the newer visual contract.

- [ ] **Step 7: Commit the verified tracker update**

```bash
rtk git add docs/parity.md
rtk git diff --cached --check
rtk git commit -m "docs(parity): verify Palmier editor shell slice"
```

Expected: one documentation-only Conventional Commit. Ignored screenshots remain under `output/` and are not staged.

- [ ] **Step 8: Verify final commit boundaries and leave unrelated dirty work untouched**

Run:

```bash
rtk git log -5 --oneline
rtk git status --short
rtk git diff --check
```

Expected: the six meaningful commits for layout, shell, preview, timeline, agent, and tracker all appear in recent history; `git diff --check` exits `0`; any pre-existing unrelated dirty files remain present and unstaged rather than reset or silently included.
