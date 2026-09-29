# Motion Template Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the MVP motion template library with one built-in kinetic lower-third template, a left sidebar `Media`/`Templates` tab flow, overlay timeline insertion, and Codex template reference validation.

**Architecture:** Start with a built-in TypeScript catalog whose records are shaped like a future provider/MCP response. The React UI inserts template overlays into the existing timeline model, while Rust extends Codex proposal validation so agent-selected templates can only reference known IDs after a valid EDL. Rendering real animated assets remains outside this plan; the inserted overlay stores a stable synthetic generated source and complete visual metadata.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Tailwind, lucide-react, Rust, serde, serde_json, cargo test.

---

## File Structure

- Create `src/lib/motion-templates.ts`: built-in motion template catalog, lookup helpers, and insertion helper.
- Create `src/lib/motion-templates.test.ts`: catalog and insertion helper tests.
- Create `src/components/workspace/motion-template-library.tsx`: compact template browser for the left sidebar `Templates` tab.
- Create `src/components/workspace/motion-template-library.test.tsx`: rendering and insert callback tests.
- Modify `src/components/workspace/editor-workspace.tsx`: replace the direct `MediaBin` left column with `Media`/`Templates` tabs and insert template overlays into local project state.
- Modify `src/components/workspace/editor-workspace.test.tsx`: verify tab switching and template insertion.
- Modify `src/components/workspace/timeline-editor.tsx`: style template overlay items distinctly and keep existing caption drag behavior.
- Modify `src/components/workspace/timeline-editor.test.tsx`: verify template overlay rendering.
- Modify `src-tauri/src/codex/proposal.rs`: add typed overlay proposal validation for template references.
- Modify `src-tauri/src/codex/app_server.rs`: include the built-in template summary in prompts and allow `templateId`/`fields` in overlay schema.
- Modify `src-tauri/tests/codex_app_server.rs`: add schema and proposal validation tests.

## Task 1: TypeScript Motion Template Catalog

**Files:**
- Create: `src/lib/motion-templates.ts`
- Create: `src/lib/motion-templates.test.ts`
- Modify: `src/lib/timeline.ts`

- [ ] **Step 1: Write the failing catalog tests**

Create `src/lib/motion-templates.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  createTemplateOverlayItem,
  getMotionTemplate,
  kineticLowerThirdTemplate,
  motionTemplateCatalog,
} from "./motion-templates";

describe("motion template catalog", () => {
  it("ships the built-in kinetic lower-third template with visual metadata", () => {
    expect(motionTemplateCatalog).toHaveLength(1);
    expect(kineticLowerThirdTemplate).toMatchObject({
      id: "kinetic-lower-third-v1",
      name: "Kinetic Lower Third",
      category: "lower_thirds",
      kind: "overlay",
      durationSeconds: 2.4,
      placement: { trackKind: "overlay", defaultStartSeconds: 0 },
      renderContract: { dimensions: "project", fps: "project", alpha: true },
    });
    expect(kineticLowerThirdTemplate.visualTreatment).toContain("lower-third");
    expect(kineticLowerThirdTemplate.motion).toContain("slide");
    expect(kineticLowerThirdTemplate.safeZone).toContain("10% margins");
    expect(kineticLowerThirdTemplate.avoid).toContain("opaque black slabs");
  });

  it("looks up templates by id", () => {
    expect(getMotionTemplate("kinetic-lower-third-v1")?.name).toBe("Kinetic Lower Third");
    expect(getMotionTemplate("missing-template")).toBeNull();
  });

  it("creates a canonical overlay item from a template and field overrides", () => {
    const item = createTemplateOverlayItem({
      templateId: "kinetic-lower-third-v1",
      itemId: "template-item-1",
      startSeconds: 1.25,
      fields: {
        headline: "Olha API",
        subline: "Founder",
      },
    });

    expect(item).toMatchObject({
      id: "template-item-1",
      kind: "overlay",
      startSeconds: 1.25,
      durationSeconds: 2.4,
      source: {
        type: "generated",
        artifactId: "template:kinetic-lower-third-v1:template-item-1",
      },
      label: "Kinetic Lower Third",
      properties: {
        templateId: "kinetic-lower-third-v1",
        templateFields: {
          headline: "Olha API",
          subline: "Founder",
        },
        visualTreatment: kineticLowerThirdTemplate.visualTreatment,
        motion: kineticLowerThirdTemplate.motion,
        safeZone: kineticLowerThirdTemplate.safeZone,
        avoid: kineticLowerThirdTemplate.avoid,
      },
    });
  });

  it("rejects unknown template ids and empty required fields", () => {
    expect(() =>
      createTemplateOverlayItem({
        templateId: "missing-template",
        itemId: "template-item-1",
        startSeconds: 0,
        fields: { headline: "Name", subline: "Role" },
      }),
    ).toThrow("Unknown motion template: missing-template");

    expect(() =>
      createTemplateOverlayItem({
        templateId: "kinetic-lower-third-v1",
        itemId: "template-item-1",
        startSeconds: 0,
        fields: { headline: "   ", subline: "Role" },
      }),
    ).toThrow("Template field headline cannot be empty");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk pnpm test -- src/lib/motion-templates.test.ts
```

Expected: FAIL because `src/lib/motion-templates.ts` does not exist.

- [ ] **Step 3: Add `template` metadata support to timeline types**

Modify `src/lib/timeline.ts` so `TimelineItemKind` stays unchanged but overlay items can be inspected through properties. Add this helper near `getTimelineItemText`:

```ts
export function isTemplateTimelineItem(item: TimelineItem): boolean {
  return typeof item.properties.templateId === "string";
}
```

- [ ] **Step 4: Implement the catalog and insertion helper**

Create `src/lib/motion-templates.ts`:

```ts
import type { TimelineItem, TrackKind } from "./timeline";

export type MotionTemplateCategory =
  | "titles"
  | "lower_thirds"
  | "captions"
  | "callouts"
  | "transitions";

export type MotionTemplateKind = "overlay" | "caption" | "hyperframe_scene" | "transition";

export interface MotionTemplateDefinition {
  id: string;
  name: string;
  category: MotionTemplateCategory;
  kind: MotionTemplateKind;
  durationSeconds: number;
  defaultTextFields: Record<string, string>;
  preview: {
    thumbnailKind: "css";
    description: string;
  };
  placement: {
    trackKind: TrackKind;
    defaultStartSeconds: number;
  };
  renderContract: {
    dimensions: "project";
    fps: "project";
    alpha: boolean;
  };
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}

export interface CreateTemplateOverlayItemInput {
  templateId: string;
  itemId: string;
  startSeconds: number;
  fields?: Record<string, string>;
}

export const kineticLowerThirdTemplate: MotionTemplateDefinition = {
  id: "kinetic-lower-third-v1",
  name: "Kinetic Lower Third",
  category: "lower_thirds",
  kind: "overlay",
  durationSeconds: 2.4,
  defaultTextFields: {
    headline: "Name / Role",
    subline: "Context label",
  },
  preview: {
    thumbnailKind: "css",
    description: "Translucent lower-third label with an accent rule and quick kinetic entry.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  visualTreatment:
    "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
  motion: "slide-and-fade in over 8 frames, hold, then soft fade out",
  safeZone: "keep essential text inside 10% margins and below face/action priority areas",
  avoid:
    "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
};

export const motionTemplateCatalog: MotionTemplateDefinition[] = [kineticLowerThirdTemplate];

export function getMotionTemplate(templateId: string): MotionTemplateDefinition | null {
  return motionTemplateCatalog.find((template) => template.id === templateId) ?? null;
}

export function createTemplateOverlayItem(input: CreateTemplateOverlayItemInput): TimelineItem {
  const template = getMotionTemplate(input.templateId);
  if (!template) {
    throw new Error(`Unknown motion template: ${input.templateId}`);
  }
  if (template.kind !== "overlay" || template.placement.trackKind !== "overlay") {
    throw new Error(`Motion template is not an overlay: ${input.templateId}`);
  }
  if (!Number.isFinite(input.startSeconds) || input.startSeconds < 0) {
    throw new Error("Template startSeconds must be a finite non-negative number");
  }

  const mergedFields = {
    ...template.defaultTextFields,
    ...(input.fields ?? {}),
  };

  for (const fieldName of Object.keys(template.defaultTextFields)) {
    if (!mergedFields[fieldName]?.trim()) {
      throw new Error(`Template field ${fieldName} cannot be empty`);
    }
    mergedFields[fieldName] = mergedFields[fieldName].trim();
  }

  return {
    id: input.itemId,
    kind: "overlay",
    startSeconds: input.startSeconds,
    durationSeconds: template.durationSeconds,
    source: {
      type: "generated",
      artifactId: `template:${template.id}:${input.itemId}`,
    },
    label: template.name,
    properties: {
      templateId: template.id,
      templateFields: mergedFields,
      templateCategory: template.category,
      renderContract: template.renderContract,
      visualTreatment: template.visualTreatment,
      motion: template.motion,
      safeZone: template.safeZone,
      avoid: template.avoid,
    },
  };
}
```

- [ ] **Step 5: Run the catalog tests**

Run:

```bash
rtk pnpm test -- src/lib/motion-templates.test.ts src/lib/timeline.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
rtk git add src/lib/motion-templates.ts src/lib/motion-templates.test.ts src/lib/timeline.ts src/lib/timeline.test.ts
rtk git commit -m "feat: add built-in motion template catalog"
```

## Task 2: Template Library Component

**Files:**
- Create: `src/components/workspace/motion-template-library.tsx`
- Create: `src/components/workspace/motion-template-library.test.tsx`

- [ ] **Step 1: Write the failing component tests**

Create `src/components/workspace/motion-template-library.test.tsx`:

```tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { kineticLowerThirdTemplate } from "@/lib/motion-templates";
import { MotionTemplateLibrary } from "./motion-template-library";

describe("MotionTemplateLibrary", () => {
  it("renders the built-in kinetic lower-third template", () => {
    render(<MotionTemplateLibrary templates={[kineticLowerThirdTemplate]} />);

    expect(screen.getByText("Kinetic Lower Third")).toBeInTheDocument();
    expect(screen.getByText("Lower thirds")).toBeInTheDocument();
    expect(screen.getByText("2.4s")).toBeInTheDocument();
    expect(screen.getByText("Overlays")).toBeInTheDocument();
  });

  it("calls onInsertTemplate when the insert button is clicked", () => {
    const onInsertTemplate = vi.fn();

    render(
      <MotionTemplateLibrary
        templates={[kineticLowerThirdTemplate]}
        onInsertTemplate={onInsertTemplate}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /insert kinetic lower third/i }));

    expect(onInsertTemplate).toHaveBeenCalledWith("kinetic-lower-third-v1");
  });

  it("renders an empty template state", () => {
    render(<MotionTemplateLibrary templates={[]} />);

    expect(screen.getByText("No motion templates available")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/workspace/motion-template-library.test.tsx
```

Expected: FAIL because `MotionTemplateLibrary` does not exist.

- [ ] **Step 3: Implement the component**

Create `src/components/workspace/motion-template-library.tsx`:

```tsx
import { Layers3, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { MotionTemplateDefinition } from "@/lib/motion-templates";

interface MotionTemplateLibraryProps {
  templates?: MotionTemplateDefinition[];
  onInsertTemplate?: (templateId: string) => void;
}

function formatCategory(category: MotionTemplateDefinition["category"]) {
  return category
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatTrack(trackKind: MotionTemplateDefinition["placement"]["trackKind"]) {
  if (trackKind === "overlay") {
    return "Overlays";
  }
  if (trackKind === "caption") {
    return "Captions";
  }
  if (trackKind === "hyperframe_scene") {
    return "HyperFrames";
  }
  return trackKind;
}

export function MotionTemplateLibrary({
  templates = [],
  onInsertTemplate,
}: MotionTemplateLibraryProps) {
  return (
    <Card className="h-full rounded-md">
      <CardHeader className="space-y-3">
        <CardTitle className="flex items-center gap-2">
          <Layers3 className="h-4 w-4" aria-hidden="true" />
          Templates
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        {templates.length === 0 ? (
          <div className="rounded-md border border-dashed bg-muted/30 p-3 text-sm text-muted-foreground">
            No motion templates available
          </div>
        ) : (
          templates.map((template) => (
            <article
              key={template.id}
              className="overflow-hidden rounded-md border bg-muted/30 text-xs"
            >
              <div className="relative h-20 bg-slate-950">
                <div className="absolute bottom-3 left-3 rounded-sm border-l-4 border-cyan-300 bg-slate-900/90 px-2 py-1 text-white shadow-lg">
                  <span className="block text-[11px] font-semibold leading-tight">
                    {template.defaultTextFields.headline}
                  </span>
                  <span className="block text-[10px] text-cyan-100">
                    {template.defaultTextFields.subline}
                  </span>
                </div>
                <div className="absolute right-3 top-3 h-1 w-8 rounded-full bg-orange-400" />
              </div>
              <div className="space-y-2 p-3">
                <div className="flex min-w-0 items-start justify-between gap-2">
                  <div className="min-w-0">
                    <h3 className="truncate font-medium">{template.name}</h3>
                    <p className="text-muted-foreground">
                      {formatCategory(template.category)} - {template.durationSeconds.toFixed(1)}s
                    </p>
                  </div>
                  <span className="shrink-0 rounded-sm border px-1.5 py-0.5 text-[10px] text-muted-foreground">
                    {formatTrack(template.placement.trackKind)}
                  </span>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 w-full justify-start"
                  aria-label={`Insert ${template.name}`}
                  onClick={() => onInsertTemplate?.(template.id)}
                >
                  <Plus className="h-3.5 w-3.5" aria-hidden="true" />
                  Insert
                </Button>
              </div>
            </article>
          ))
        )}
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Run the component tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/motion-template-library.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
rtk git add src/components/workspace/motion-template-library.tsx src/components/workspace/motion-template-library.test.tsx
rtk git commit -m "feat: add motion template library panel"
```

## Task 3: Left Sidebar Tabs And Manual Template Insertion

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Write failing workspace tests**

Append these tests to `src/components/workspace/editor-workspace.test.tsx`:

```tsx
  it("switches the left sidebar between media and templates", () => {
    render(<EditorWorkspace />);

    expect(screen.getByRole("button", { name: /input.mp4/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));

    expect(screen.getByText("Kinetic Lower Third")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /input.mp4/i })).not.toBeInTheDocument();
  });

  it("inserts the kinetic lower-third template onto the overlay track", () => {
    render(<EditorWorkspace />);

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));
    fireEvent.click(screen.getByRole("button", { name: /insert kinetic lower third/i }));

    expect(screen.getByRole("button", { name: "Kinetic Lower Third" })).toBeInTheDocument();
    expect(screen.getByText("Kinetic Lower Third")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the workspace has no `Templates` tab and no insert flow.

- [ ] **Step 3: Implement left sidebar state and insertion**

Modify imports in `src/components/workspace/editor-workspace.tsx`:

```tsx
import { Settings } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import type { EditJobRequest } from "@/lib/edit";
import { createTemplateOverlayItem, motionTemplateCatalog } from "@/lib/motion-templates";
import { importMediaToProject, type VideoProject } from "@/lib/project";
import { sampleTimeline, type Timeline, type TimelinePatch } from "@/lib/timeline";
import { AgentPanel } from "./agent-panel";
import { CaptionInspector } from "./caption-inspector";
import { MediaBin, type MediaImportStatus } from "./media-bin";
import { MotionTemplateLibrary } from "./motion-template-library";
import { PreviewPanel } from "./preview-panel";
import { TimelineEditor } from "./timeline-editor";
```

Add the tab type near `defaultProjectDir`:

```tsx
type SourcePanelTab = "media" | "templates";
```

Inside `EditorWorkspace`, add state:

```tsx
const [sourcePanelTab, setSourcePanelTab] = useState<SourcePanelTab>("media");
```

Add this helper inside `EditorWorkspace`:

```tsx
function insertTemplate(templateId: string) {
  setProject((current) => {
    const overlayTrack = current.timeline.tracks.find((track) => track.kind === "overlay");
    if (!overlayTrack) {
      return current;
    }

    const itemId = `template-${templateId}-${Date.now().toString(36)}`;
    const item = createTemplateOverlayItem({
      templateId,
      itemId,
      startSeconds: 0,
    });
    const tracks = current.timeline.tracks.map((track) =>
      track.id === overlayTrack.id
        ? {
            ...track,
            items: [...track.items, item].sort(
              (left, right) => left.startSeconds - right.startSeconds,
            ),
          }
        : track,
    );

    setSelectedItemId(item.id);

    return {
      ...current,
      timeline: {
        durationSeconds: recalculateDuration({ ...current.timeline, tracks }),
        tracks,
      },
    };
  });
}
```

Replace the left `MediaBin` usage in the main grid with:

```tsx
<aside className="flex min-h-[320px] flex-col rounded-md border bg-card">
  <div role="tablist" aria-label="Source panel" className="grid grid-cols-2 border-b text-sm">
    <button
      type="button"
      role="tab"
      aria-selected={sourcePanelTab === "media"}
      className={`px-3 py-2 font-medium ${
        sourcePanelTab === "media"
          ? "border-b-2 border-primary text-foreground"
          : "text-muted-foreground hover:text-foreground"
      }`}
      onClick={() => setSourcePanelTab("media")}
    >
      Media
    </button>
    <button
      type="button"
      role="tab"
      aria-selected={sourcePanelTab === "templates"}
      className={`px-3 py-2 font-medium ${
        sourcePanelTab === "templates"
          ? "border-b-2 border-primary text-foreground"
          : "text-muted-foreground hover:text-foreground"
      }`}
      onClick={() => setSourcePanelTab("templates")}
    >
      Templates
    </button>
  </div>
  <div className="min-h-0 flex-1">
    {sourcePanelTab === "media" ? (
      <MediaBin
        media={project.media}
        selectedMediaId={selectedMediaId}
        importStatus={importStatus}
        importError={importError}
        onImport={importMedia}
        onSelectMedia={setSelectedMediaId}
      />
    ) : (
      <MotionTemplateLibrary
        templates={motionTemplateCatalog}
        onInsertTemplate={insertTemplate}
      />
    )}
  </div>
</aside>
```

- [ ] **Step 4: Run the workspace tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS. If duplicate text causes the insertion assertion to be ambiguous, change the second assertion to check the timeline button only:

```tsx
expect(screen.getByRole("button", { name: "Kinetic Lower Third" })).toBeInTheDocument();
```

- [ ] **Step 5: Commit Task 3**

Run:

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat: insert templates from source sidebar"
```

## Task 4: Timeline Template Overlay Styling

**Files:**
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`

- [ ] **Step 1: Write the failing timeline rendering test**

Append this test to `src/components/workspace/timeline-editor.test.tsx`:

```tsx
  it("renders template overlay items with an overlay-specific style", () => {
    render(
      <TimelineEditor
        timeline={{
          durationSeconds: 4,
          tracks: [
            { id: "track-video", name: "Video", kind: "video", locked: false, items: [] },
            { id: "track-scenes", name: "HyperFrames", kind: "hyperframe_scene", locked: false, items: [] },
            {
              id: "track-overlays",
              name: "Overlays",
              kind: "overlay",
              locked: false,
              items: [
                {
                  id: "template-1",
                  kind: "overlay",
                  startSeconds: 0,
                  durationSeconds: 2.4,
                  source: { type: "generated", artifactId: "template:kinetic-lower-third-v1:template-1" },
                  label: "Kinetic Lower Third",
                  properties: { templateId: "kinetic-lower-third-v1" },
                },
              ],
            },
            { id: "track-captions", name: "Captions", kind: "caption", locked: false, items: [] },
            { id: "track-audio", name: "Audio", kind: "audio", locked: false, items: [] },
          ],
        }}
      />,
    );

    expect(screen.getByRole("button", { name: "Kinetic Lower Third" })).toHaveClass(
      "border-cyan-300",
    );
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx
```

Expected: FAIL because overlay items currently use the default blue clip style.

- [ ] **Step 3: Update `itemClassName` for overlays**

Modify `itemClassName` in `src/components/workspace/timeline-editor.tsx`:

```tsx
function itemClassName(kind: TimelineItemKind, selected: boolean) {
  const selectedClasses = selected ? "ring-2 ring-ring ring-offset-1" : "";
  const kindClasses =
    kind === "caption"
      ? "border-slate-700 bg-slate-950 text-white"
      : kind === "overlay"
        ? "border-cyan-300 bg-cyan-950 text-cyan-50"
        : kind === "audio_clip"
          ? "border-emerald-300 bg-emerald-50 text-emerald-950"
          : "border-blue-300 bg-blue-50 text-blue-950";

  return `h-9 w-full rounded-md border px-3 py-2 text-left text-xs ${kindClasses} ${selectedClasses}`;
}
```

- [ ] **Step 4: Run timeline tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
rtk git add src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat: style motion template overlays"
```

## Task 5: Codex Template Reference Validation

**Files:**
- Modify: `src-tauri/src/codex/proposal.rs`
- Modify: `src-tauri/src/codex/app_server.rs`
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing Rust proposal validation tests**

Append tests to `src-tauri/tests/codex_app_server.rs`:

```rust
#[test]
fn turn_request_lists_available_motion_templates() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");

    assert!(text.contains("Available motion templates"));
    assert!(text.contains("kinetic-lower-third-v1"));
    assert!(text.contains("required fields: headline, subline"));
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
            ["templateId"],
        serde_json::json!({ "type": "string" })
    );
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]["fields"]
            ["type"],
        serde_json::json!("object")
    );
}

#[test]
fn proposal_validation_rejects_unknown_template_ids() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "lower_third",
        "templateId": "missing-template",
        "startSeconds": 1.0,
        "durationSeconds": 2.4,
        "fields": { "headline": "Olha API", "subline": "Founder" },
        "brief": "Introduce the speaker.",
        "visualTreatment": "compact lower-third block with translucent backing",
        "motion": "slide-and-fade in over 8 frames",
        "safeZone": "keep essential text inside 10% margins",
        "avoid": "full-width opaque black slabs"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("unknown template must fail");

    assert!(error.to_string().contains("unknown motion template"));
}

#[test]
fn proposal_validation_rejects_template_overlays_without_required_fields() {
    let project = sample_project();
    let request = sample_edit_request();
    let mut proposal = valid_template_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "lower_third",
        "templateId": "kinetic-lower-third-v1",
        "startSeconds": 1.0,
        "durationSeconds": 2.4,
        "fields": { "headline": "   ", "subline": "Founder" },
        "brief": "Introduce the speaker.",
        "visualTreatment": "compact lower-third block with translucent backing",
        "motion": "slide-and-fade in over 8 frames",
        "safeZone": "keep essential text inside 10% margins",
        "avoid": "full-width opaque black slabs"
    })];

    let error = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect_err("empty template field must fail");

    assert!(error.to_string().contains("template field headline cannot be empty"));
}

#[test]
fn proposal_validation_accepts_known_template_overlay_after_valid_edl() {
    let project = sample_project();
    let request = sample_edit_request();
    let proposal = valid_template_proposal();

    let edl = validate_codex_edit_proposal(&project, &request, &proposal)
        .expect("known template overlay should pass");

    assert_eq!(edl.clips.len(), 1);
}

fn valid_template_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: "media-1".to_string(),
        clips: vec![CodexProposalClip {
            source_in: 1.0,
            source_out: 46.0,
            reason: "strong hook and complete thought".to_string(),
        }],
        captions: Vec::new(),
        overlays: vec![serde_json::json!({
            "kind": "lower_third",
            "templateId": "kinetic-lower-third-v1",
            "startSeconds": 1.0,
            "durationSeconds": 2.4,
            "fields": { "headline": "Olha API", "subline": "Founder" },
            "brief": "Introduce the speaker.",
            "visualTreatment": "compact lower-third block with translucent backing",
            "motion": "slide-and-fade in over 8 frames",
            "safeZone": "keep essential text inside 10% margins",
            "avoid": "full-width opaque black slabs"
        })],
        hyperframes: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 45.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}
```

- [ ] **Step 2: Run the Rust tests to verify they fail**

Run:

```bash
rtk pnpm rust:test -- codex_app_server
```

Expected: FAIL because the prompt does not list templates, overlay schema does not expose `templateId`, and proposal validation ignores template fields.

- [ ] **Step 3: Extend proposal validation**

Modify `src-tauri/src/codex/proposal.rs`.

Add constants below the structs:

```rust
const KINETIC_LOWER_THIRD_TEMPLATE_ID: &str = "kinetic-lower-third-v1";
const KINETIC_LOWER_THIRD_REQUIRED_FIELDS: &[&str] = &["headline", "subline"];
```

Add error variants:

```rust
    #[error("unknown motion template: {0}")]
    UnknownTemplateId(String),
    #[error("template field {0} cannot be empty")]
    EmptyTemplateField(String),
    #[error("template overlay metadata is missing: {0}")]
    MissingTemplateMetadata(&'static str),
```

In `validate_codex_edit_proposal`, call overlay validation after render review validation and before building the EDL:

```rust
    validate_template_overlays(&proposal.overlays)?;
```

Add helper functions:

```rust
fn validate_template_overlays(overlays: &[serde_json::Value]) -> Result<(), CodexProposalError> {
    for overlay in overlays {
        let Some(template_id) = overlay.get("templateId").and_then(serde_json::Value::as_str) else {
            continue;
        };

        if template_id != KINETIC_LOWER_THIRD_TEMPLATE_ID {
            return Err(CodexProposalError::UnknownTemplateId(template_id.to_string()));
        }

        for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
            if overlay
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err(CodexProposalError::MissingTemplateMetadata(key));
            }
        }

        let fields = overlay
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .ok_or(CodexProposalError::MissingTemplateMetadata("fields"))?;

        for field_name in KINETIC_LOWER_THIRD_REQUIRED_FIELDS {
            if fields
                .get(*field_name)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err(CodexProposalError::EmptyTemplateField((*field_name).to_string()));
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Extend app-server prompt and schema**

Modify `build_video_edit_prompt` in `src-tauri/src/codex/app_server.rs` by adding this block before `Rules:`:

```rust
Available motion templates:
- id: kinetic-lower-third-v1
  name: Kinetic Lower Third
  category: lower_thirds
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 2.4
  required fields: headline, subline
  visualTreatment: compact lower-third block with translucent backing, accent rule, and strong hierarchy
  motion: slide-and-fade in over 8 frames, hold, then soft fade out
  safeZone: keep essential text inside 10% margins and below face/action priority areas
  avoid: full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds

```

Modify the overlay schema properties in `codex_edit_proposal_output_schema()` to include:

```rust
                        "templateId": { "type": "string" },
                        "fields": {
                            "type": "object",
                            "additionalProperties": { "type": "string" }
                        },
```

Do not add `templateId` or `fields` to the required list, because generic overlays still remain valid.

- [ ] **Step 5: Run the Rust tests**

Run:

```bash
rtk pnpm rust:test -- codex_app_server
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

Run:

```bash
rtk git add src-tauri/src/codex/proposal.rs src-tauri/src/codex/app_server.rs src-tauri/tests/codex_app_server.rs
rtk git commit -m "feat: validate codex motion template references"
```

## Task 6: Full Verification And Visual QA

**Files:**
- Modify only if verification finds defects in files touched by Tasks 1-5.

- [ ] **Step 1: Run focused frontend tests**

Run:

```bash
rtk pnpm test -- src/lib/motion-templates.test.ts src/components/workspace/motion-template-library.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/timeline-editor.test.tsx
```

Expected: PASS.

- [ ] **Step 2: Run lint**

Run:

```bash
rtk pnpm lint
```

Expected: PASS.

- [ ] **Step 3: Run Rust tests**

Run:

```bash
rtk pnpm rust:test
```

Expected: PASS.

- [ ] **Step 4: Start the dev server for visual QA**

Run:

```bash
rtk pnpm dev
```

Expected: Vite prints a local URL, usually `http://127.0.0.1:5173/`. Keep the server running during visual QA.

- [ ] **Step 5: Inspect the app in Browser or Playwright**

Open the local URL and verify:

- the left sidebar shows `Media` and `Templates` tabs
- the `Media` tab still shows imported sample media
- the `Templates` tab shows `Kinetic Lower Third`
- clicking `Insert` creates a teal/cyan overlay item on the `Overlays` track
- text remains readable at desktop width and a narrow mobile-like width
- the right Codex panel remains visible and does not become the template browser

- [ ] **Step 6: Stop the dev server**

Stop the `rtk pnpm dev` process with `Ctrl-C`.

- [ ] **Step 7: Run full verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS.

- [ ] **Step 8: Commit verification fixes if any were required**

If Step 7 required code changes, run:

```bash
rtk git add src src-tauri
rtk git commit -m "fix: polish motion template library validation"
```

If Step 7 passed without changes, do not create an empty commit.
