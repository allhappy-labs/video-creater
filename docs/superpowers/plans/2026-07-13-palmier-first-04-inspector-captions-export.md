# Palmier-First Inspector, Captions, and Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore Palmier's contextual inspector, captions/speech workbench, and native export sheet without removing Video Creater's richer editing, generation, transcript, or export workflows.

**Architecture:** Recover the small reusable inspector-tab primitive, then compose existing inspector content into explicit contextual destinations instead of duplicating workflow logic. Add focused `CaptionsWorkbench` and `ExportSheet` components whose props are typed callbacks supplied by `EditorWorkspace`, leaving canonical project mutation and Tauri commands in their current owner.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, Lucide icons, Vitest, Testing Library, Tauri command wrappers, existing project/timeline models.

---

## File Structure

- Restore `src/components/workspace/contextual-inspector-tabs.tsx`: accessible roving tab strip shared by project, source, clip, and text contexts.
- Restore `src/components/workspace/contextual-inspector-tabs.test.tsx`: tab/panel semantics and keyboard navigation.
- Create `src/components/workspace/captions-workbench.tsx`: compact source/settings/style/animation/placement shell around existing caption callbacks.
- Create `src/components/workspace/captions-workbench.test.tsx`: workbench disclosure, preview, and agent-prompt tests.
- Create `src/components/workspace/export-sheet.tsx`: destination/profile selection and accessible sheet shell.
- Create `src/components/workspace/export-sheet.test.tsx`: focus, destination, availability, and submit contract tests.
- Modify `src/components/workspace/source-clip-inspector.tsx`: split details and AI actions into contextual panels.
- Modify `src/components/workspace/source-clip-inspector.test.tsx`: replace flattened-inspector assertions with Palmier tab assertions while retaining every workflow test.
- Modify `src/components/workspace/project-timeline-inspector.tsx`: split Project and Activity.
- Modify `src/components/workspace/project-timeline-inspector.test.tsx`: assert compact project rows and activity routing.
- Modify `src/components/workspace/media-bin.tsx`: mount the captions workbench in the restored rail and expose Speech/Music grouping.
- Modify `src/components/workspace/media-bin.test.tsx`: caption and speech rail integration.
- Modify `src/components/workspace/editor-workspace.tsx`: own active contextual state, wire callbacks, and replace the export menu with `ExportSheet`.
- Modify `src/components/workspace/editor-workspace.test.tsx`: exact tab and export workflow integration.
- Modify `docs/parity.md`: reopen stale claims before implementation and close only states backed by fresh evidence.

### Task 1: Reopen the stale inspector tracker entry

**Files:**
- Modify: `docs/parity.md:517-518`

- [ ] **Step 1: Change the inspector row from complete to visual-regression**

Use explicit wording that the functionality remains but the current dirty tree removed the verified tabs/rail:

```markdown
| P1 | visual-regression | Contextual inspector | Functional inspector workflows remain, but the current tree flattens Details and AI Edit and removes the reusable contextual tab strip. Fresh installed-Palmier screenshots `04`, `08`, and `09` require the tabbed hierarchy to be restored and reverified. | Reopened 2026-07-13. |
```

- [ ] **Step 2: Verify the tracker diff is isolated**

Run: `rtk git diff --check -- docs/parity.md && rtk git diff -- docs/parity.md`

Expected: no whitespace errors; only the stale contextual-inspector row changes. The media row was already handled by Plan 03.

- [ ] **Step 3: Commit the truthful tracker state**

```bash
rtk git add docs/parity.md
rtk git commit -m "docs(parity): reopen contextual editor regressions"
```

### Task 2: Restore the accessible contextual tab primitive

**Files:**
- Restore: `src/components/workspace/contextual-inspector-tabs.tsx`
- Restore: `src/components/workspace/contextual-inspector-tabs.test.tsx`

- [ ] **Step 1: Restore the test and add compact Palmier geometry assertions**

Restore the committed tests and add:

```tsx
it("uses Palmier's compact header and active underline", () => {
  render(
    <ContextualInspectorTabs
      ariaLabel="Inspector"
      tabs={tabs}
      activeTab="project"
      onTabChange={vi.fn()}
    >
      Project content
    </ContextualInspectorTabs>,
  );

  expect(screen.getByRole("tablist", { name: "Inspector views" })).toHaveClass(
    "h-7",
    "border-b",
  );
  expect(screen.getByRole("tab", { name: "Project" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});
```

- [ ] **Step 2: Run the test to verify the deleted primitive fails**

Run: `rtk pnpm exec vitest run src/components/workspace/contextual-inspector-tabs.test.tsx`

Expected: FAIL because `./contextual-inspector-tabs` does not exist.

- [ ] **Step 3: Restore the component and apply compact classes**

Read the committed implementation with `rtk git show 7fc5d6f4:src/components/workspace/contextual-inspector-tabs.tsx` and its test with `rtk git show 7fc5d6f4:src/components/workspace/contextual-inspector-tabs.test.tsx`; create the working files through the patch tool. The tab strip shell must be:

```tsx
<div
  role="tablist"
  aria-label={`${ariaLabel} views`}
  className="flex h-7 min-w-0 snap-x snap-mandatory items-end overflow-x-auto border-b border-white/15 px-3 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
>
  {tabs.map((tab) => (
    <button
      key={tab.id}
      role="tab"
      aria-selected={tab.id === active.id}
      tabIndex={tab.id === active.id ? 0 : -1}
      className="relative flex h-7 min-w-0 flex-1 snap-start items-center justify-center px-2 text-[11px] font-medium text-white/80 aria-selected:text-white aria-selected:after:absolute aria-selected:after:inset-x-3 aria-selected:after:bottom-0 aria-selected:after:h-px aria-selected:after:bg-white"
      onClick={() => onTabChange(tab.id)}
      onKeyDown={(event) => activateFromKeyboard(event, tab.id)}
    >
      {tab.label}
    </button>
  ))}
</div>
```

Keep the committed ArrowLeft/ArrowRight/Home/End logic, disabled-tab skipping, `aria-controls`, single-tab suppression, and horizontally scrollable narrow behavior.

- [ ] **Step 4: Run the focused tests**

Run: `rtk pnpm exec vitest run src/components/workspace/contextual-inspector-tabs.test.tsx`

Expected: all contextual tab tests PASS.

- [ ] **Step 5: Commit the primitive**

```bash
rtk git add src/components/workspace/contextual-inspector-tabs.tsx src/components/workspace/contextual-inspector-tabs.test.tsx
rtk git commit -m "feat(editor): restore contextual inspector tabs"
```

### Task 3: Reintroduce Details and AI Edit for selected sources

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.tsx`
- Modify: `src/components/workspace/source-clip-inspector.test.tsx`

- [ ] **Step 1: Replace the contradictory flattened-inspector test**

Replace `renders generated details and AI edit actions directly without inspector tabs` with:

```tsx
it("separates generated details from AI edit workflows", () => {
  render(
    <SourceClipInspector
      item={generatedClipItem}
      media={generatedMedia}
      generatedAssets={[generatedAsset]}
      onQueueVariation={vi.fn()}
    />,
  );

  const tabs = screen.getByRole("tablist", {
    name: "Generated source inspector views",
  });
  expect(within(tabs).getByRole("tab", { name: "Details" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(screen.getByRole("group", { name: "Generated details" })).toBeVisible();
  expect(screen.queryByRole("region", { name: "Generated AI edit" })).not.toBeInTheDocument();

  fireEvent.click(within(tabs).getByRole("tab", { name: "AI Edit" }));
  expect(screen.getByRole("region", { name: "Generated AI edit" })).toBeVisible();
  expect(screen.queryByRole("group", { name: "Generated details" })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the source inspector test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/source-clip-inspector.test.tsx -t "separates generated details"`

Expected: FAIL because the tablist does not exist and both surfaces render together.

- [ ] **Step 3: Add explicit contextual state and content**

Add the import and local state:

```tsx
import { useEffect, useState } from "react";
import { ContextualInspectorTabs } from "./contextual-inspector-tabs";

type SourceInspectorView = "details" | "ai-edit";

const [sourceInspectorView, setSourceInspectorView] =
  useState<SourceInspectorView>("details");

useEffect(() => {
  setSourceInspectorView("details");
}, [item.id]);
```

Wrap generated/imported source content:

```tsx
<ContextualInspectorTabs
  ariaLabel={isGenerated ? "Generated source inspector" : "Source inspector"}
  tabs={[
    { id: "details", label: "Details" },
    { id: "ai-edit", label: "AI Edit", disabled: !hasAiActions },
  ]}
  activeTab={sourceInspectorView}
  onTabChange={(id) => setSourceInspectorView(id as SourceInspectorView)}
>
  {sourceInspectorView === "details" ? detailsContent : aiEditContent}
</ContextualInspectorTabs>
```

Keep every existing callback and eligibility check in its current content block. Do not copy provider or mutation logic into the tab component.

- [ ] **Step 4: Group AI actions under Palmier headings**

The AI panel structure must be:

```tsx
<div role="region" aria-label="Generated AI edit" className="grid gap-5 py-3">
  {scopeContent ? <InspectorSection title="Scope">{scopeContent}</InspectorSection> : null}
  <InspectorSection title="AI Enhance">{enhanceActions}</InspectorSection>
  <InspectorSection title="AI Audio">{audioActions}</InspectorSection>
  {advancedActions ? (
    <details>
      <summary className="cursor-pointer text-[10px] font-semibold uppercase tracking-[0.15em] text-white/35">
        Advanced
      </summary>
      <div className="pt-3">{advancedActions}</div>
    </details>
  ) : null}
</div>
```

- [ ] **Step 5: Run the complete source inspector test file**

Run: `rtk pnpm exec vitest run src/components/workspace/source-clip-inspector.test.tsx`

Expected: all existing source, generated, rerun, variation, upscale, audio, reference, and eligibility tests PASS with the new tab helper opening AI Edit where needed.

- [ ] **Step 6: Commit selected-source tabs**

```bash
rtk git add src/components/workspace/source-clip-inspector.tsx src/components/workspace/source-clip-inspector.test.tsx
rtk git commit -m "feat(editor): restore source details and AI edit tabs"
```

### Task 4: Restore Project and Activity inspector destinations

**Files:**
- Modify: `src/components/workspace/project-timeline-inspector.tsx`
- Modify: `src/components/workspace/project-timeline-inspector.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Add a failing workspace test**

```tsx
it("keeps project settings separate from activity and diagnostics", async () => {
  render(<EditorWorkspace initialProject={createSplitProject()} projectDir="/tmp/project" />);

  const tabs = screen.getByRole("tablist", { name: "Project inspector views" });
  expect(within(tabs).getByRole("tab", { name: "Project" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(screen.getByText("Resolution")).toBeVisible();
  expect(screen.queryByText("Latest render")).not.toBeInTheDocument();

  fireEvent.click(within(tabs).getByRole("tab", { name: /Activity/ }));
  expect(await screen.findByText("Latest render")).toBeVisible();
  expect(screen.queryByText("Resolution")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the failing test**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-workspace.test.tsx -t "keeps project settings separate"`

Expected: FAIL because the project inspector has no tablist and activity is inline.

- [ ] **Step 3: Add a typed project inspector view**

In `ProjectTimelineInspector`, accept content explicitly:

```tsx
interface ProjectTimelineInspectorProps {
  projectContent: ReactNode;
  activityContent: ReactNode;
  activityStatusLabel?: string;
}

type ProjectInspectorView = "project" | "activity";
```

Render `ContextualInspectorTabs` with `Project` and `Activity`, keeping the current project name/path/duration/resolution/frame-rate/aspect rows in Project and moving jobs, AI media, workflow/Temporal, latest render, diagnostics, provenance, and export notices to Activity.

- [ ] **Step 4: Preserve active status and keyboard navigation**

Pass a compact label such as `${activeJobCount} active` only when non-zero. Add a test that ArrowRight moves focus and calls `onTabChange("activity")` through the shared primitive.

- [ ] **Step 5: Run focused inspector/workspace tests**

Run: `rtk pnpm exec vitest run src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/editor-workspace.test.tsx -t "Project|Activity|project settings separate"`

Expected: focused tests PASS.

- [ ] **Step 6: Commit project/activity separation**

```bash
rtk git add src/components/workspace/project-timeline-inspector.tsx src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(editor): separate project activity inspector"
```

### Task 5: Create the compact captions workbench

**Files:**
- Create: `src/components/workspace/captions-workbench.tsx`
- Create: `src/components/workspace/captions-workbench.test.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Write the workbench contract test**

```tsx
it("exposes Palmier caption setup and drafts reviewed agent actions", () => {
  const onDraftAgentPrompt = vi.fn();
  render(
    <CaptionsWorkbench
      sources={[{ id: "media-1", label: "Interview" }]}
      sourceId="media-1"
      mode="local"
      language="auto"
      maxWords={null}
      censorProfanity={false}
      preview={<span>Captions will look like this</span>}
      onSourceChange={vi.fn()}
      onModeChange={vi.fn()}
      onLanguageChange={vi.fn()}
      onMaxWordsChange={vi.fn()}
      onCensorProfanityChange={vi.fn()}
      onBuildCaptions={vi.fn()}
      onDraftAgentPrompt={onDraftAgentPrompt}
    />,
  );

  expect(screen.getByRole("combobox", { name: "Caption source" })).toHaveValue("media-1");
  expect(screen.getByRole("radio", { name: "Local" })).toBeChecked();
  expect(screen.getByRole("button", { name: "Style" })).toHaveAttribute("aria-expanded", "false");
  expect(screen.getByText("Captions will look like this")).toBeVisible();

  fireEvent.click(screen.getByRole("button", { name: "Agent Mode" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Remove filler words" }));
  expect(onDraftAgentPrompt).toHaveBeenCalledWith(
    expect.stringContaining("Do not change caption timing without a reviewed project action"),
  );
});
```

- [ ] **Step 2: Run the test to verify the component is missing**

Run: `rtk pnpm exec vitest run src/components/workspace/captions-workbench.test.tsx`

Expected: FAIL because `CaptionsWorkbench` does not exist.

- [ ] **Step 3: Implement the typed workbench shell**

Create these exported types and props:

```tsx
export type CaptionTranscriptionMode = "local" | "cloud";

export interface CaptionSourceOption {
  id: string;
  label: string;
}

export interface CaptionsWorkbenchProps {
  sources: readonly CaptionSourceOption[];
  sourceId: string | null;
  mode: CaptionTranscriptionMode;
  language: string;
  maxWords: number | null;
  censorProfanity: boolean;
  preview: ReactNode;
  stylePanel?: ReactNode;
  animationPanel?: ReactNode;
  placementPanel?: ReactNode;
  advancedEditor?: ReactNode;
  onSourceChange: (id: string) => void;
  onModeChange: (mode: CaptionTranscriptionMode) => void;
  onLanguageChange: (language: string) => void;
  onMaxWordsChange: (count: number | null) => void;
  onCensorProfanityChange: (value: boolean) => void;
  onBuildCaptions: () => void;
  onDraftAgentPrompt: (prompt: string) => void;
}
```

Render compact Source and Settings sections followed by reusable disclosure buttons:

```tsx
function WorkbenchDisclosure({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <section className="border-t border-white/10">
      <button
        type="button"
        aria-expanded={open}
        className="flex h-11 w-full items-center gap-2 text-left text-[11px] font-semibold uppercase tracking-[0.16em] text-white/35"
        onClick={() => setOpen((value) => !value)}
      >
        <ChevronRight className={cn("h-3.5 w-3.5", open && "rotate-90")} />
        {label}
      </button>
      {open ? <div className="pb-4">{children}</div> : null}
    </section>
  );
}
```

- [ ] **Step 4: Add the exact safe agent menu prompts**

Map the four actions to deterministic prompt bodies:

```tsx
const captionAgentActions = [
  ["Remove filler words", "Remove filler words from the selected caption source. Preserve meaning. Do not change caption timing without a reviewed project action."],
  ["Fix names & jargon", "Correct names and domain jargon in the selected caption source. Preserve word timing unless a reviewed project action explicitly changes it."],
  ["Add emoji", "Suggest restrained emoji additions for the selected captions. Return a reviewed proposal and preserve timing."],
  ["Translate", "Translate the selected captions to the chosen language. Preserve source timing and return reviewed project actions."],
] as const;
```

- [ ] **Step 5: Mount the workbench under the restored Captions rail destination**

In `MediaBin`, pass the existing transcript-backed caption builder/editor, selected source, production preview, local transcription defaults, and `onDraftGenerationAgentPrompt`. Keep detailed word repair under `advancedEditor` rather than as the default rail body.

- [ ] **Step 6: Run captions and media tests**

Run: `rtk pnpm exec vitest run src/components/workspace/captions-workbench.test.tsx src/components/workspace/caption-inspector.test.tsx src/components/workspace/transcript-panel.test.tsx src/components/workspace/media-bin.test.tsx -t "caption|Caption|rail"`

Expected: workbench and existing caption/transcript tests PASS.

- [ ] **Step 7: Commit the captions workbench**

```bash
rtk git add src/components/workspace/captions-workbench.tsx src/components/workspace/captions-workbench.test.tsx src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx
rtk git commit -m "feat(captions): add Palmier-style caption workbench"
```

### Task 6: Consolidate speech and silence controls

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Add a failing Speech/Music test**

```tsx
it("routes speaker and silence actions through the Audio rail", () => {
  const onIdentifySpeakers = vi.fn();
  const onRemoveDetectedSilence = vi.fn();
  renderMediaBin({ onIdentifySpeakers, onRemoveDetectedSilence });

  fireEvent.click(screen.getByRole("tab", { name: "Audio" }));
  expect(screen.getByRole("tab", { name: "Speech" })).toHaveAttribute("aria-selected", "true");
  fireEvent.click(screen.getByRole("button", { name: "Identify Speakers" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove Silence" }));

  expect(onIdentifySpeakers).toHaveBeenCalledTimes(1);
  expect(onRemoveDetectedSilence).toHaveBeenCalledTimes(1);
});
```

- [ ] **Step 2: Run the failing test**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx -t "routes speaker and silence"`

Expected: FAIL because the Audio rail and Speech/Music tabs are absent.

- [ ] **Step 3: Add Audio rail props without moving workflow ownership**

Add optional callbacks to `MediaBinProps`:

```tsx
onMarkSpeakers?: (enabled: boolean) => void;
onIdentifySpeakers?: () => void;
onMarkSilence?: (enabled: boolean) => void;
onRemoveDetectedSilence?: () => void;
speechMarkingEnabled?: boolean;
silenceMarkingEnabled?: boolean;
```

Render compact Speech/Music tabs. Speech owns the four controls; Music renders the existing audio-generation composer. Wire callbacks from `EditorWorkspace` to the current analysis and ripple-delete paths.

- [ ] **Step 4: Verify production actions remain exact**

Add workspace expectations that Identify Speakers invokes the existing speaker-analysis command and Remove Silence emits the same canonical ripple-delete action currently used by the media toolbar.

- [ ] **Step 5: Run focused tests**

Run: `rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx -t "speaker|silence|Speech|Music"`

Expected: all focused speaker, silence, and audio generation tests PASS.

- [ ] **Step 6: Commit speech consolidation**

```bash
rtk git add src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(audio): consolidate speech and silence controls"
```

### Task 7: Build the accessible Palmier export sheet

**Files:**
- Create: `src/components/workspace/export-sheet.tsx`
- Create: `src/components/workspace/export-sheet.test.tsx`

- [ ] **Step 1: Write destination and focus tests**

```tsx
it("groups export profiles by destination and restores trigger focus", () => {
  const onOpenChange = vi.fn();
  const onExport = vi.fn();
  const trigger = document.createElement("button");
  trigger.textContent = "Export";
  document.body.append(trigger);
  trigger.focus();
  const returnFocusRef = { current: trigger };
  const props = {
    timelineName: "Timeline 1",
    durationSeconds: 12,
    width: 736,
    height: 400,
    fps: 24,
    profiles,
    onOpenChange,
    onExport,
    returnFocusRef,
  };

  const { rerender } = render(<ExportSheet {...props} open />);

  expect(screen.getByRole("dialog", { name: "Export" })).toHaveClass("w-[560px]", "h-[520px]");
  expect(screen.getByRole("radio", { name: "Video" })).toBeChecked();
  expect(screen.getByRole("combobox", { name: "Codec" })).toHaveValue("h264");
  expect(screen.getByText("736×400")).toBeVisible();

  fireEvent.click(screen.getByRole("radio", { name: "Timeline" }));
  expect(screen.getByRole("combobox", { name: "Timeline format" })).toBeVisible();

  fireEvent.keyDown(screen.getByRole("dialog", { name: "Export" }), { key: "Escape" });
  rerender(<ExportSheet {...props} open={false} />);
  expect(trigger).toHaveFocus();
});
```

- [ ] **Step 2: Run the test to verify the sheet is missing**

Run: `rtk pnpm exec vitest run src/components/workspace/export-sheet.test.tsx`

Expected: FAIL because `ExportSheet` does not exist.

- [ ] **Step 3: Define export sheet types**

```tsx
import type { ExportProfile } from "@/lib/project";

export type ExportDestination = "video" | "timeline" | "palmier-project";
export type ExportSheetProfileId =
  | "draftWebm"
  | "finalWebm"
  | "premiereXmeml"
  | "davinciFcpxml"
  | ExportProfile;

export interface ExportSheetProfile {
  id: ExportSheetProfileId;
  destination: ExportDestination;
  label: string;
  codec?: "h264" | "h265" | "prores" | "vp9";
  fileType: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ExportSheetSelection {
  destination: ExportDestination;
  profileId: ExportSheetProfileId;
  resolution: "match-timeline";
}
```

`ExportSheetProps` supplies timeline metadata, profiles, busy/error/progress state, `onOpenChange`, `onExport`, and the return-focus ref.

- [ ] **Step 4: Implement modal semantics and Palmier geometry**

The shell must be:

```tsx
<div className="fixed inset-0 z-50 grid place-items-center bg-black/55 p-4" onMouseDown={onBackdropMouseDown}>
  <section
    ref={dialogRef}
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    className="grid h-[min(520px,calc(100vh-32px))] w-[min(560px,calc(100vw-32px))] grid-rows-[auto_1fr_auto] overflow-hidden rounded-[28px] border border-white/20 bg-[#161616]/95 shadow-2xl backdrop-blur-xl"
    onKeyDown={onDialogKeyDown}
  >
    <h2 id={titleId} className="px-5 py-3 text-[28px] font-light tracking-[-0.5px]">Export</h2>
    {settingsContent}
    {footerContent}
  </section>
</div>
```

Implement Tab/Shift+Tab cycling inside the dialog, Escape close, backdrop close only when the backdrop itself is clicked, and focus restoration.

- [ ] **Step 5: Implement destination-specific rows and footer**

Video shows Codec, File Type, Resolution, and Frame Rate. Timeline shows Timeline Format and compatibility summary; FCPXML additionally shows Version and Target, while XMEML does not. Palmier Project shows package collection and missing-media summary. The footer renders duration, honest estimated size or `—`, resolution, file type, Cancel, and Export.

Unavailable profiles remain visible and disabled with `unavailableReason` adjacent to the selector; do not hide policy/runtime evidence.

- [ ] **Step 6: Run the sheet tests**

Run: `rtk pnpm exec vitest run src/components/workspace/export-sheet.test.tsx`

Expected: destination, disabled-reason, focus trap, Escape, Cancel, submit, and footer tests PASS.

- [ ] **Step 7: Commit the standalone sheet**

```bash
rtk git add src/components/workspace/export-sheet.tsx src/components/workspace/export-sheet.test.tsx
rtk git commit -m "feat(export): add Palmier-style export sheet"
```

### Task 8: Wire every existing export workflow into the sheet

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Replace the export-menu integration test**

Update the existing `Export options` test to open the dialog and assert destination/profile mapping:

```tsx
fireEvent.click(screen.getByRole("button", { name: "Export" }));
const dialog = screen.getByRole("dialog", { name: "Export" });
expect(within(dialog).getByRole("radio", { name: "Video" })).toBeChecked();
expect(within(dialog).getByRole("option", { name: "H.264" })).toBeDisabled();
expect(dialog).toHaveTextContent("Missing GStreamer factory: vtenc_h264");

fireEvent.click(within(dialog).getByRole("radio", { name: "Timeline" }));
expect(within(dialog).getByRole("option", { name: "Premiere XML" })).toBeEnabled();

fireEvent.click(within(dialog).getByRole("radio", { name: "Palmier Project" }));
expect(within(dialog).getByText("Collect project media into a portable package")).toBeVisible();
```

- [ ] **Step 2: Run the integration test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-workspace.test.tsx -t "offers local and workflow export paths"`

Expected: FAIL because Export still opens a menu.

- [ ] **Step 3: Map current profiles without changing their executors**

Build `ExportSheetProfile[]` from `selectedRenderQuality`, `exportProfileAvailability`, and split-project eligibility. Route selections:

```tsx
async function executeExportSelection(selection: ExportSheetSelection) {
  switch (selection.profileId) {
    case "draftWebm": return exportWebm("draftWebm");
    case "finalWebm": return exportWebm("finalWebm");
    case "premiereXmeml": return exportNleXml("premiereXmeml");
    case "davinciFcpxml": return exportNleXml("davinciFcpxml");
    case "palmierProject": return exportPalmierProjectPackage();
    default: return exportMediaProfile(
      exportProfileById(exportProfileAvailability, selection.profileId),
    );
  }
}
```

Do not introduce a second Tauri or Temporal execution path.

- [ ] **Step 4: Preserve every current export assertion**

Update tests only at the UI entry point. Keep the exact assertions for:

- `renderWebmToSplitProjectFolder` draft/final calls.
- Premiere and DaVinci XML command payloads.
- Palmier package collection and missing-media summary.
- MP4/H.264, H.265, and ProRes Temporal start requests.
- Runtime/policy disabled reasons.
- Export completion/error notices and artifact paths.

- [ ] **Step 5: Run focused export tests**

Run: `rtk pnpm exec vitest run src/components/workspace/editor-workspace.test.tsx -t "Export|export|MP4|Premiere|DaVinci|Palmier Project|WebM"`

Expected: all focused export tests PASS through the dialog.

- [ ] **Step 6: Run the complete inspector/captions/export slice**

Run:

```bash
rtk pnpm exec vitest run \
  src/components/workspace/contextual-inspector-tabs.test.tsx \
  src/components/workspace/source-clip-inspector.test.tsx \
  src/components/workspace/project-timeline-inspector.test.tsx \
  src/components/workspace/captions-workbench.test.tsx \
  src/components/workspace/caption-inspector.test.tsx \
  src/components/workspace/transcript-panel.test.tsx \
  src/components/workspace/export-sheet.test.tsx \
  src/components/workspace/media-bin.test.tsx \
  src/components/workspace/editor-workspace.test.tsx
```

Expected: all selected files PASS.

- [ ] **Step 7: Run lint and diff checks**

Run: `rtk pnpm lint && rtk git diff --check`

Expected: both commands exit 0.

- [ ] **Step 8: Commit the export integration**

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(export): route workflows through destination sheet"
```

### Task 9: Close only freshly verified contextual states

**Files:**
- Modify: `docs/parity.md`

- [ ] **Step 1: Add exact implementation evidence**

Update the reopened rows with component/test names and retained screenshot directories. Use `Implemented; visual QA pending` until Plan 05 has produced and inspected same-state reference/implementation comparisons.

- [ ] **Step 2: Record Video Creater-only preserved capabilities**

In the relevant sections, explicitly retain:

```markdown
- Video Creater-only: detailed transcript repair history, per-word caption animation, provider capability diagnostics, WebM profiles, H.265/ProRes, Temporal export workflows, and richer generated-output replacement remain available beneath Palmier-compatible contextual surfaces.
```

- [ ] **Step 3: Verify tracker truthfulness**

Run: `rtk rg -n "Contextual inspector|Media library chrome|Video Creater-only" docs/parity.md && rtk git diff --check -- docs/parity.md`

Expected: rows cite current tests and do not claim visual closure before comparison evidence exists.

- [ ] **Step 4: Commit the slice tracker update**

```bash
rtk git add docs/parity.md
rtk git commit -m "docs(parity): record contextual surface implementation"
```
