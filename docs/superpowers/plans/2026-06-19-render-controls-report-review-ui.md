# Render Controls And Report Review UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add frontend render-quality controls and a compact render report review surface that uses exact `draftWebm` and `finalWebm` wire values.

**Architecture:** Add a small frontend render model module, two focused workspace UI components, and local demo-state integration in `EditorWorkspace`. This first slice does not run real Tauri render jobs; it creates the UI and typed contract that later backend render-job wiring will use.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Tailwind, existing shadcn-style UI primitives, lucide-react icons.

---

## File Structure

- Create `src/lib/render.ts`: frontend render quality/report types, labels, command quality parsing, and a sample report factory.
- Create `src/lib/render.test.ts`: unit tests locking exact `draftWebm` and `finalWebm` values and helper behavior.
- Create `src/components/workspace/render-quality-control.tsx`: compact segmented control for draft/final WebM.
- Create `src/components/workspace/render-report-panel.tsx`: compact report review panel with empty/report states and artifact path display.
- Modify `src/components/workspace/preview-panel.tsx`: accept and render `RenderReportPanel` props below the preview surface.
- Modify `src/components/workspace/editor-workspace.tsx`: own selected quality/sample report state, wire header render actions, pass report state to preview.
- Modify `src/components/workspace/editor-workspace.test.tsx`: integration tests for default draft quality, final selection, finished-render sample report, and report review display.

## Task 1: Frontend Render Types And Helpers

**Files:**
- Create: `src/lib/render.ts`
- Create: `src/lib/render.test.ts`

- [ ] **Step 1: Write failing helper tests**

Create `src/lib/render.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  buildSampleRenderReport,
  commandQualityProfile,
  renderQualityOptions,
  renderQualityProfileLabel,
  type RenderQualityProfile,
} from "./render";

describe("render model", () => {
  it("uses exact Rust render quality wire values", () => {
    const values = renderQualityOptions.map((option) => option.value);

    expect(values).toEqual<RenderQualityProfile[]>(["draftWebm", "finalWebm"]);
  });

  it("labels render quality profiles for editor controls", () => {
    expect(renderQualityProfileLabel("draftWebm")).toBe("Draft WebM");
    expect(renderQualityProfileLabel("finalWebm")).toBe("Final WebM");
  });

  it("extracts command quality metadata from render args", () => {
    expect(
      commandQualityProfile({
        program: "gstreamer-ges",
        args: ["--input=source.mp4", "--quality=finalWebm"],
      }),
    ).toBe("finalWebm");
  });

  it("builds a sample report with the requested final quality command", () => {
    const report = buildSampleRenderReport("finalWebm");

    expect(report.command.args).toContain("--quality=finalWebm");
    expect(report.summary.outputPath).toBe("renders/final.webm");
    expect(report.graphics[0]?.renderer).toBe("gpu");
    expect(report.graphics[0]?.visualQaStatus).toBe("passed");
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk pnpm test -- --run src/lib/render.test.ts
```

Expected: FAIL because `src/lib/render.ts` does not exist.

- [ ] **Step 3: Add render model helpers**

Create `src/lib/render.ts`:

```ts
export type RenderQualityProfile = "draftWebm" | "finalWebm";

export interface RenderQualityOption {
  value: RenderQualityProfile;
  label: string;
  description: string;
}

export interface RenderGraphicsReport {
  layerId: string;
  renderer: "gpu" | "software" | string;
  qualityProfile: string | null;
  visualQaStatus: string | null;
}

export interface RenderReportSummary {
  status: string;
  durationSeconds: number | null;
  outputPath: string | null;
}

export interface RenderCommandSpec {
  program: string;
  args: string[];
}

export interface RenderReport {
  jobId: string;
  summary: RenderReportSummary;
  command: RenderCommandSpec;
  artifacts: string[];
  graphics: RenderGraphicsReport[];
}

export const defaultRenderQualityProfile: RenderQualityProfile = "draftWebm";

export const renderQualityOptions: RenderQualityOption[] = [
  {
    value: "draftWebm",
    label: "Draft WebM",
    description: "Fast iteration render for reviewing timing, captions, and graphics.",
  },
  {
    value: "finalWebm",
    label: "Final WebM",
    description: "Finished WebM render intent for accepted edits.",
  },
];

export function renderQualityProfileLabel(profile: RenderQualityProfile): string {
  return renderQualityOptions.find((option) => option.value === profile)?.label ?? profile;
}

export function commandQualityProfile(command: RenderCommandSpec): RenderQualityProfile | null {
  for (const arg of command.args) {
    if (arg === "--quality=draftWebm") {
      return "draftWebm";
    }
    if (arg === "--quality=finalWebm") {
      return "finalWebm";
    }
  }
  return null;
}

export function buildSampleRenderReport(profile: RenderQualityProfile): RenderReport {
  const final = profile === "finalWebm";
  const outputPath = final ? "renders/final.webm" : "renders/draft.webm";

  return {
    jobId: final ? "sample-final-render" : "sample-draft-render",
    summary: {
      status: "ready",
      durationSeconds: 4,
      outputPath,
    },
    command: {
      program: "gstreamer-ges",
      args: [
        "--input=source.mp4",
        `--output=${outputPath}`,
        "--size=1920x1080",
        "--fps=24.000",
        `--quality=${profile}`,
      ],
    },
    artifacts: [
      outputPath,
      "renders/graphics/shader-hook-bg/preview.png",
      "renders/graphics/shader-hook-bg/manifest.json",
      "renders/graphics/shader-hook-bg/frames/frame-000000.png",
    ],
    graphics: [
      {
        layerId: "shader-hook-bg",
        renderer: "gpu",
        qualityProfile: "hq-neon-wireframe-shader-v1",
        visualQaStatus: "passed",
      },
    ],
  };
}
```

- [ ] **Step 4: Run helper tests to verify pass**

Run:

```bash
rtk pnpm test -- --run src/lib/render.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit render model**

Run:

```bash
rtk git add src/lib/render.ts src/lib/render.test.ts
rtk git commit -m "feat: add frontend render report model"
```

## Task 2: Render Quality And Report Components

**Files:**
- Create: `src/components/workspace/render-quality-control.tsx`
- Create: `src/components/workspace/render-report-panel.tsx`
- Modify: `src/components/workspace/preview-panel.tsx`

- [ ] **Step 1: Write failing component integration tests**

Add this import block to `src/components/workspace/editor-workspace.test.tsx` if needed:

```ts
import { within } from "@testing-library/react";
```

Append tests to `describe("EditorWorkspace", () => { ... })`:

```ts
  it("defaults render quality to draft WebM and shows no report initially", () => {
    render(<EditorWorkspace />);

    expect(screen.getByRole("button", { name: "Draft WebM" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByText("No render report yet.")).toBeInTheDocument();
  });

  it("selects final WebM for finished render intent", () => {
    render(<EditorWorkspace />);

    fireEvent.click(screen.getByRole("button", { name: "Final WebM" }));
    fireEvent.click(screen.getByRole("button", { name: "Render final" }));

    expect(screen.getByText("sample-final-render")).toBeInTheDocument();
    expect(screen.getByText("Final WebM")).toBeInTheDocument();
    expect(screen.getByTitle("--quality=finalWebm")).toBeInTheDocument();
  });

  it("shows GPU renderer mode, quality profile, QA status, and artifacts in render review", () => {
    render(<EditorWorkspace />);

    fireEvent.click(screen.getByRole("button", { name: "Render draft" }));

    const review = screen.getByRole("region", { name: "Render review" });
    expect(within(review).getByText("shader-hook-bg")).toBeInTheDocument();
    expect(within(review).getByText("gpu")).toBeInTheDocument();
    expect(within(review).getByText("hq-neon-wireframe-shader-v1")).toBeInTheDocument();
    expect(within(review).getByText("passed")).toBeInTheDocument();
    expect(
      within(review).getByTitle("renders/graphics/shader-hook-bg/preview.png"),
    ).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk pnpm test -- --run src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the render quality control and report panel do not exist.

- [ ] **Step 3: Create render quality control**

Create `src/components/workspace/render-quality-control.tsx`:

```tsx
import { Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  renderQualityOptions,
  type RenderQualityProfile,
} from "@/lib/render";

interface RenderQualityControlProps {
  value: RenderQualityProfile;
  onChange: (value: RenderQualityProfile) => void;
}

export function RenderQualityControl({ value, onChange }: RenderQualityControlProps) {
  return (
    <div className="flex min-w-0 items-center gap-1 rounded-md border bg-background p-1">
      {renderQualityOptions.map((option) => {
        const selected = value === option.value;
        return (
          <Button
            key={option.value}
            type="button"
            variant={selected ? "secondary" : "ghost"}
            size="sm"
            className="h-8 px-2 text-xs"
            aria-pressed={selected}
            title={option.description}
            onClick={() => onChange(option.value)}
          >
            {selected ? <Check className="h-3.5 w-3.5" aria-hidden="true" /> : null}
            {option.label}
          </Button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 4: Create render report panel**

Create `src/components/workspace/render-report-panel.tsx`:

```tsx
import { FileText, Gauge, MonitorCheck } from "lucide-react";
import {
  commandQualityProfile,
  renderQualityProfileLabel,
  type RenderQualityProfile,
  type RenderReport,
} from "@/lib/render";

interface RenderReportPanelProps {
  report: RenderReport | null;
  selectedQuality: RenderQualityProfile;
}

function compactPath(path: string): string {
  if (path.length <= 44) {
    return path;
  }
  return `...${path.slice(-41)}`;
}

export function RenderReportPanel({ report, selectedQuality }: RenderReportPanelProps) {
  if (!report) {
    return (
      <section
        role="region"
        aria-label="Render review"
        className="mt-3 rounded-md border bg-muted/30 p-3 text-xs text-muted-foreground"
      >
        No render report yet.
      </section>
    );
  }

  const commandQuality = commandQualityProfile(report.command);
  const outputPath = report.summary.outputPath;

  return (
    <section
      role="region"
      aria-label="Render review"
      className="mt-3 space-y-3 rounded-md border bg-background p-3 text-xs"
    >
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 font-medium">
            <MonitorCheck className="h-3.5 w-3.5" aria-hidden="true" />
            <span>{report.jobId}</span>
          </div>
          <div className="mt-1 min-w-0 text-muted-foreground">
            {outputPath ? (
              <span title={outputPath}>{compactPath(outputPath)}</span>
            ) : (
              <span>No output path recorded</span>
            )}
          </div>
        </div>
        <div className="shrink-0 rounded-sm border px-2 py-1 text-muted-foreground">
          {report.summary.status}
        </div>
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        <div className="rounded-md border bg-muted/30 p-2">
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <Gauge className="h-3.5 w-3.5" aria-hidden="true" />
            Selected quality
          </div>
          <div className="mt-1 font-medium">{renderQualityProfileLabel(selectedQuality)}</div>
        </div>
        <div className="rounded-md border bg-muted/30 p-2">
          <div className="text-muted-foreground">Command quality</div>
          <div
            className="mt-1 font-medium"
            title={commandQuality ? `--quality=${commandQuality}` : "No quality metadata"}
          >
            {commandQuality ? renderQualityProfileLabel(commandQuality) : "Not recorded"}
          </div>
        </div>
      </div>

      <div className="space-y-1">
        <div className="font-medium">Graphics</div>
        {report.graphics.length > 0 ? (
          <div className="grid gap-1">
            {report.graphics.map((graphics) => (
              <div
                key={graphics.layerId}
                className="grid gap-1 rounded-md border px-2 py-1.5 sm:grid-cols-[1.2fr_0.7fr_1.4fr_0.6fr]"
              >
                <span className="min-w-0 truncate" title={graphics.layerId}>
                  {graphics.layerId}
                </span>
                <span>{graphics.renderer}</span>
                <span className="min-w-0 truncate" title={graphics.qualityProfile ?? "none"}>
                  {graphics.qualityProfile ?? "none"}
                </span>
                <span>{graphics.visualQaStatus ?? "not-run"}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-md border bg-muted/30 px-2 py-1.5 text-muted-foreground">
            No graphics artifacts recorded.
          </div>
        )}
      </div>

      <div className="space-y-1">
        <div className="flex items-center gap-1.5 font-medium">
          <FileText className="h-3.5 w-3.5" aria-hidden="true" />
          Artifacts
        </div>
        <div className="grid gap-1">
          {report.artifacts.slice(0, 4).map((artifact) => (
            <div
              key={artifact}
              className="min-w-0 truncate rounded-md border bg-muted/30 px-2 py-1 text-muted-foreground"
              title={artifact}
            >
              {compactPath(artifact)}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Update preview panel**

Modify `src/components/workspace/preview-panel.tsx` to accept report props:

```tsx
import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { RenderQualityProfile, RenderReport } from "@/lib/render";
import type { TimelineItem } from "@/lib/timeline";
import { MotionTemplatePreview } from "./motion-template-preview";
import { RenderReportPanel } from "./render-report-panel";

interface PreviewPanelProps {
  selectedItem?: TimelineItem | null;
  renderReport?: RenderReport | null;
  selectedRenderQuality?: RenderQualityProfile;
}

export function PreviewPanel({
  selectedItem,
  renderReport = null,
  selectedRenderQuality = "draftWebm",
}: PreviewPanelProps) {
  return (
    <Card className="h-full rounded-md">
      <CardHeader>
        <CardTitle>Preview</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="relative flex aspect-video items-center justify-center overflow-hidden rounded-md border bg-neutral-950 text-white">
          <MotionTemplatePreview item={selectedItem ?? null} />
          <Button variant="secondary" size="icon" aria-label="Play preview">
            <Play className="h-4 w-4" />
          </Button>
        </div>
        <RenderReportPanel report={renderReport} selectedQuality={selectedRenderQuality} />
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 6: Run component tests to verify they still fail at workspace integration only**

Run:

```bash
rtk pnpm test -- --run src/components/workspace/editor-workspace.test.tsx
```

Expected: still FAIL because `EditorWorkspace` has not yet wired state and controls.

- [ ] **Step 7: Commit components**

Run:

```bash
rtk git add src/components/workspace/render-quality-control.tsx src/components/workspace/render-report-panel.tsx src/components/workspace/preview-panel.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat: add render review workspace components"
```

## Task 3: Workspace Integration And Verification

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Wire render state and controls in workspace**

Modify imports in `src/components/workspace/editor-workspace.tsx`:

```tsx
import {
  buildSampleRenderReport,
  defaultRenderQualityProfile,
  type RenderQualityProfile,
  type RenderReport,
} from "@/lib/render";
import { RenderQualityControl } from "./render-quality-control";
```

Add state inside `EditorWorkspace`:

```tsx
  const [selectedRenderQuality, setSelectedRenderQuality] = useState<RenderQualityProfile>(
    defaultRenderQualityProfile,
  );
  const [renderReport, setRenderReport] = useState<RenderReport | null>(null);
```

Add helper functions:

```tsx
  function renderDraft() {
    setSelectedRenderQuality("draftWebm");
    setRenderReport(buildSampleRenderReport("draftWebm"));
  }

  function renderFinal() {
    setSelectedRenderQuality("finalWebm");
    setRenderReport(buildSampleRenderReport("finalWebm"));
  }
```

Replace the header render button area:

```tsx
          <RenderQualityControl
            value={selectedRenderQuality}
            onChange={setSelectedRenderQuality}
          />
          <Button type="button" size="sm" onClick={renderDraft}>
            Render draft
          </Button>
          <Button type="button" size="sm" onClick={renderFinal}>
            Render final
          </Button>
```

Update preview panel usage:

```tsx
          <PreviewPanel
            selectedItem={selectedTimelineItem}
            renderReport={renderReport}
            selectedRenderQuality={selectedRenderQuality}
          />
```

- [ ] **Step 2: Run workspace tests to verify pass**

Run:

```bash
rtk pnpm test -- --run src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS.

- [ ] **Step 3: Run render model tests**

Run:

```bash
rtk pnpm test -- --run src/lib/render.test.ts
```

Expected: PASS.

- [ ] **Step 4: Run lint**

Run:

```bash
rtk pnpm lint
```

Expected: PASS.

- [ ] **Step 5: Run full frontend tests**

Run:

```bash
rtk pnpm test
```

Expected: PASS.

- [ ] **Step 6: Commit workspace integration**

Run:

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat: wire render quality review into workspace"
```

## Final Verification

- [ ] **Step 1: Run full project verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS.

- [ ] **Step 2: Inspect working tree**

Run:

```bash
rtk git status --short --branch
```

Expected: only pre-existing untracked `renders/` remains.

## Self-Review

Spec coverage:

- Render quality selector: Task 2 and Task 3.
- Exact `draftWebm` and `finalWebm` wire values: Task 1 tests and helpers.
- Finished render uses `finalWebm`: Task 2 integration test and Task 3 `renderFinal`.
- Report review surface: Task 2 components and Task 3 workspace integration.
- No real render execution: all state uses `buildSampleRenderReport`.

Placeholder scan:

- No unresolved placeholder words are present.
- Every task has exact files, commands, and expected outcomes.

Type consistency:

- `RenderQualityProfile` is the frontend type.
- Wire values are `draftWebm` and `finalWebm`.
- Report fields use frontend camelCase matching Rust JSON: `jobId`, `qualityProfile`, `visualQaStatus`.
