# Palmier-Parity Agentic Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring Video Creater from a strong agentic edit pipeline prototype to a polished Palmier-class agentic video editor with real timeline preview, direct agent tools, complete generation loops, production render/export lifecycle, transcription readiness, HyperFrame handling, and editor-grade UI states.

**Architecture:** Keep Rust/Tauri as the canonical owner of project state, validation, render plans, logs, and artifacts. React remains the dense editor shell and proposal review surface; Codex and MCP tools return structured actions that Rust validates before mutation.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vite, Tailwind, shadcn-style UI primitives, GStreamer/GES render backend, FluidAudio/Parakeet transcription, Codex app server/MCP tools, Vitest, Cargo tests.

---

## Scope

This plan turns the Palmier comparison gaps into executable work. It supersedes none of the earlier plans; it integrates the relevant unfinished slices:

- `docs/superpowers/plans/2026-06-19-render-controls-report-review-ui.md`
- `docs/superpowers/plans/2026-06-23-palmier-generation-composer.md`
- `docs/superpowers/plans/2026-06-24-mp4-export-policy-report.md`
- `docs/superpowers/plans/2026-06-27-codex-local-tool-control-plane.md`
- `docs/superpowers/plans/2026-06-27-real-render-export-path.md`
- `docs/superpowers/plans/2026-06-28-codex-mcp-tool-server-parity.md`
- `docs/superpowers/plans/2026-06-28-coreml-temporal-transcription.md`
- `docs/superpowers/plans/2026-06-29-native-model-runtime-e2e-transcription.md`

## File Structure

Expected file responsibilities after implementation:

- `src/lib/timeline-preview.ts`: deterministic, browser-side timeline preview model for ordinary video/image/generated/text/template items.
- `src/components/workspace/timeline-preview-compositor.tsx`: React compositor for the timeline preview viewport.
- `src/components/workspace/preview-panel.tsx`: route source mode to media playback and timeline mode to the compositor with loading/error states.
- `src/components/workspace/preview-panel.test.tsx`: coverage for ordinary timeline preview, missing media, and template overlay rendering.
- `src-tauri/src/codex/tools.rs`: Palmier-parity local tools for timeline inspection, project mutation validation, media search summaries, generation defaults, export starts, and render starts.
- `src-tauri/tests/codex_mcp_server.rs` and `src-tauri/tests/codex_app_server.rs`: tool-list, schema, validation, and mutation tests.
- `src/components/workspace/agent-panel.tsx`: visible tool activity, proposal/history enablement, action-specific status, and retry affordances.
- `src/components/workspace/editor-workspace.tsx`: generation loop state, render/export workflow state, dirty/save status, and top chrome action wiring.
- `src/components/workspace/media-bin.tsx`: generated asset rerun/replace/upscale/retry/download states.
- `src-tauri/src/generation/fal.rs` and `src-tauri/src/generation/mock.rs`: generation completion actions that preserve timeline placement and source references.
- `src-tauri/src/render_pipeline/proposal.rs`: final-quality render profile selection, HyperFrame support or early rejection, and semantic validation hooks.
- `src-tauri/src/project/export_profiles.rs`: production availability for approved local export profiles.
- `src-tauri/src/transcription/runtime.rs`: native model runtime path that no longer returns the model-IO stub error.
- `src/components/workspace/render-report-panel.tsx`: empty/loading/failed/retry states and links to artifacts/logs.
- `src/components/workspace/render-quality-control.tsx`: wired into editor render/export controls.

---

## Task 1: Real Timeline Preview For Ordinary Clips

**Files:**
- Create: `src/lib/timeline-preview.ts`
- Create: `src/components/workspace/timeline-preview-compositor.tsx`
- Modify: `src/components/workspace/preview-panel.tsx`
- Test: `src/components/workspace/preview-panel.test.tsx`
- Test: `src/lib/timeline-preview.test.ts`

- [ ] **Step 1: Write failing model tests**

Add `src/lib/timeline-preview.test.ts` with these cases:

```ts
import { describe, expect, it } from "vitest";
import { buildTimelinePreviewFrame } from "./timeline-preview";
import type { MediaAsset } from "./project";
import type { Timeline } from "./timeline";

const media: MediaAsset[] = [
  {
    id: "clip-1-media",
    relativePath: "media/clip-1.mp4",
    kind: "video",
    durationSeconds: 10,
    width: 1920,
    height: 1080,
    fps: 30,
    folderId: null,
  },
  {
    id: "image-1-media",
    relativePath: "media/still.png",
    kind: "image",
    durationSeconds: 4,
    width: 1280,
    height: 720,
    fps: null,
    folderId: null,
  },
];

const timeline: Timeline = {
  durationSeconds: 8,
  tracks: [
    {
      id: "track-video",
      kind: "video",
      label: "Video",
      locked: false,
      enabled: true,
      items: [
        {
          id: "clip-1",
          kind: "clip",
          trackId: "track-video",
          startSeconds: 1,
          durationSeconds: 3,
          label: "Opening",
          source: { type: "media", mediaId: "clip-1-media" },
          properties: { sourceIn: 2, sourceOut: 5, opacity: 0.8 },
        },
        {
          id: "image-1",
          kind: "clip",
          trackId: "track-video",
          startSeconds: 5,
          durationSeconds: 2,
          label: "Still",
          source: { type: "media", mediaId: "image-1-media" },
          properties: {},
        },
      ],
    },
  ],
};

describe("timeline preview frame", () => {
  it("returns active media layers at the playhead", () => {
    const frame = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 });

    expect(frame.layers).toHaveLength(1);
    expect(frame.layers[0]).toMatchObject({
      itemId: "clip-1",
      mediaId: "clip-1-media",
      mediaKind: "video",
      sourceTimeSeconds: 3,
      opacity: 0.8,
    });
  });

  it("reports empty preview when no item is active", () => {
    const frame = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 4.5 });

    expect(frame.layers).toEqual([]);
    expect(frame.status).toBe("empty");
  });

  it("reports missing media for broken timeline sources", () => {
    const broken: Timeline = {
      ...timeline,
      tracks: [
        {
          ...timeline.tracks[0],
          items: [
            {
              ...timeline.tracks[0].items[0],
              source: { type: "media", mediaId: "missing-media" },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({ timeline: broken, media, playheadSeconds: 2 });

    expect(frame.status).toBe("missing-media");
    expect(frame.issues[0]).toContain("missing-media");
  });
});
```

- [ ] **Step 2: Run the failing model test**

Run: `rtk pnpm test src/lib/timeline-preview.test.ts`

Expected: FAIL because `src/lib/timeline-preview.ts` does not exist.

- [ ] **Step 3: Implement the preview model**

Create `src/lib/timeline-preview.ts`:

```ts
import type { MediaAsset } from "./project";
import type { Timeline, TimelineItem } from "./timeline";

export type TimelinePreviewStatus = "ready" | "empty" | "missing-media";

export interface TimelinePreviewLayer {
  itemId: string;
  label: string;
  mediaId: string;
  mediaKind: MediaAsset["kind"];
  relativePath: string;
  timelineStartSeconds: number;
  timelineEndSeconds: number;
  sourceTimeSeconds: number;
  opacity: number;
}

export interface TimelinePreviewFrame {
  status: TimelinePreviewStatus;
  playheadSeconds: number;
  layers: TimelinePreviewLayer[];
  issues: string[];
}

export function buildTimelinePreviewFrame(input: {
  timeline: Timeline;
  media: MediaAsset[];
  playheadSeconds: number;
}): TimelinePreviewFrame {
  const mediaById = new Map(input.media.map((asset) => [asset.id, asset]));
  const layers: TimelinePreviewLayer[] = [];
  const issues: string[] = [];

  for (const track of input.timeline.tracks) {
    if (track.enabled === false) {
      continue;
    }

    for (const item of track.items) {
      if (!isActiveAt(item, input.playheadSeconds) || item.source.type !== "media") {
        continue;
      }

      const asset = mediaById.get(item.source.mediaId);
      if (!asset) {
        issues.push(`Timeline item ${item.id} references missing media ${item.source.mediaId}.`);
        continue;
      }

      const offset = input.playheadSeconds - item.startSeconds;
      const sourceIn = numberProperty(item, "sourceIn") ?? 0;
      layers.push({
        itemId: item.id,
        label: item.label,
        mediaId: asset.id,
        mediaKind: asset.kind,
        relativePath: asset.relativePath,
        timelineStartSeconds: item.startSeconds,
        timelineEndSeconds: item.startSeconds + item.durationSeconds,
        sourceTimeSeconds: roundSeconds(sourceIn + offset),
        opacity: numberProperty(item, "opacity") ?? 1,
      });
    }
  }

  return {
    status: issues.length > 0 ? "missing-media" : layers.length > 0 ? "ready" : "empty",
    playheadSeconds: input.playheadSeconds,
    layers,
    issues,
  };
}

function isActiveAt(item: TimelineItem, playheadSeconds: number) {
  return (
    playheadSeconds >= item.startSeconds &&
    playheadSeconds < item.startSeconds + item.durationSeconds
  );
}

function numberProperty(item: TimelineItem, key: string) {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function roundSeconds(value: number) {
  return Math.round(value * 1000) / 1000;
}
```

- [ ] **Step 4: Verify model tests pass**

Run: `rtk pnpm test src/lib/timeline-preview.test.ts`

Expected: PASS.

- [ ] **Step 5: Write failing preview UI tests**

Extend `src/components/workspace/preview-panel.test.tsx` with tests that render `PreviewPanel` in timeline mode using one video media asset and one selected ordinary timeline item. Assert:

```ts
expect(screen.getByLabelText("Timeline preview viewport")).toBeInTheDocument();
expect(screen.getByText("Opening")).toBeInTheDocument();
expect(screen.queryByText("Select a motion template")).not.toBeInTheDocument();
```

Also add a missing-media test:

```ts
expect(screen.getByText(/references missing media/i)).toBeInTheDocument();
```

- [ ] **Step 6: Run preview UI tests to verify they fail**

Run: `rtk pnpm test src/components/workspace/preview-panel.test.tsx`

Expected: FAIL because timeline mode still renders only `MotionTemplatePreview`.

- [ ] **Step 7: Implement the compositor component**

Create `src/components/workspace/timeline-preview-compositor.tsx`:

```tsx
import { AlertCircle, Film } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import type { MediaAsset } from "@/lib/project";
import type { Timeline } from "@/lib/timeline";
import { MotionTemplatePreview } from "./motion-template-preview";

export function TimelinePreviewCompositor({
  timeline,
  media,
  playheadSeconds,
}: {
  timeline: Timeline;
  media: MediaAsset[];
  playheadSeconds: number;
}) {
  const frame = buildTimelinePreviewFrame({ timeline, media, playheadSeconds });
  const topLayer = frame.layers.at(-1) ?? null;

  if (frame.status === "missing-media") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 px-5 text-center text-sm text-amber-100">
        <AlertCircle className="h-5 w-5" aria-hidden="true" />
        <span>{frame.issues[0]}</span>
      </div>
    );
  }

  if (!topLayer) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-sm text-white/70">
        <Film className="h-5 w-5" aria-hidden="true" />
        <span>No timeline media at playhead</span>
      </div>
    );
  }

  return (
    <div aria-label="Timeline preview viewport" className="absolute inset-0 bg-black">
      {topLayer.mediaKind === "image" ? (
        <img
          alt={topLayer.label}
          src={convertFileSrc(topLayer.relativePath)}
          className="h-full w-full object-contain"
          style={{ opacity: topLayer.opacity }}
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center bg-neutral-900 text-white">
          <span className="rounded-sm border border-white/15 bg-white/10 px-3 py-2 text-sm">
            {topLayer.label}
          </span>
        </div>
      )}
      {timeline.tracks.flatMap((track) => track.items).map((item) => (
        <MotionTemplatePreview key={item.id} item={item} />
      ))}
    </div>
  );
}
```

- [ ] **Step 8: Wire `PreviewPanel` timeline mode**

Modify `src/components/workspace/preview-panel.tsx` so timeline mode renders `TimelinePreviewCompositor` with the project timeline, media list, and current playhead. If `PreviewPanel` lacks `timeline` or `media` props, add them and update call sites in `editor-workspace.tsx`.

- [ ] **Step 9: Verify preview UI tests pass**

Run: `rtk pnpm test src/components/workspace/preview-panel.test.tsx src/lib/timeline-preview.test.ts`

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src/lib/timeline-preview.ts src/lib/timeline-preview.test.ts src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.tsx
git commit -m "feat: add timeline preview compositor"
```

---

## Task 2: Palmier-Parity Agent Tool Surface

**Files:**
- Modify: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/tests/codex_mcp_server.rs`
- Modify: `src-tauri/tests/codex_app_server.rs`
- Modify: `src/components/workspace/agent-panel.tsx`
- Test: `src/components/workspace/agent-panel.test.tsx`

- [ ] **Step 1: Write failing Rust tool-list tests**

In `src-tauri/tests/codex_mcp_server.rs`, assert that `list_codex_local_tools()` includes these tool names:

```rust
let names = list_codex_local_tools()
    .into_iter()
    .map(|tool| tool.name)
    .collect::<Vec<_>>();
assert!(names.contains(&"video_creater.inspect_timeline".to_string()));
assert!(names.contains(&"video_creater.add_clips".to_string()));
assert!(names.contains(&"video_creater.split_clips".to_string()));
assert!(names.contains(&"video_creater.set_clip_properties".to_string()));
assert!(names.contains(&"video_creater.export_project".to_string()));
assert!(names.contains(&"video_creater.search_media".to_string()));
```

- [ ] **Step 2: Run the failing Rust tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml codex_mcp -- --test-threads=1`

Expected: FAIL because the new tool descriptors are absent.

- [ ] **Step 3: Add tool descriptors**

In `src-tauri/src/codex/tools.rs`, add descriptors with these exact names and categories:

```rust
tool_descriptor(
    "video_creater.inspect_timeline",
    "Inspect Timeline",
    "query",
    "Return bounded composited timeline context around a frame, including active clips, overlays, captions, source ranges, and known validation issues.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "startSeconds": { "type": "number", "minimum": 0.0 },
            "endSeconds": { "type": "number", "minimum": 0.0 },
            "maxItems": { "type": "integer", "minimum": 1, "maximum": 50 }
        }
    }),
),
tool_descriptor(
    "video_creater.add_clips",
    "Add Clips",
    "mutation",
    "Validate and apply source-backed timeline insertions as project actions.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "actions": { "type": "array", "items": { "type": "object" } }
        },
        "required": ["actions"]
    }),
),
tool_descriptor(
    "video_creater.split_clips",
    "Split Clips",
    "mutation",
    "Validate and apply split source-clip project actions.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "actions": { "type": "array", "items": { "type": "object" } }
        },
        "required": ["actions"]
    }),
),
tool_descriptor(
    "video_creater.set_clip_properties",
    "Set Clip Properties",
    "mutation",
    "Validate and apply source clip, caption, overlay, or template property updates.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "actions": { "type": "array", "items": { "type": "object" } }
        },
        "required": ["actions"]
    }),
),
tool_descriptor(
    "video_creater.export_project",
    "Export Project",
    "export",
    "Build a validated export workflow start request for the current project.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "projectDir": { "type": "string", "minLength": 1 },
            "jobId": { "type": "string", "minLength": 1 },
            "profile": { "type": "string", "enum": ["mp4H264", "mp4H265", "proResMov"] },
            "outputPath": { "type": "string", "minLength": 1 }
        },
        "required": ["projectDir", "jobId", "profile", "outputPath"]
    }),
),
tool_descriptor(
    "video_creater.search_media",
    "Search Media",
    "query",
    "Search media metadata, generated prompts, transcript words, and timeline labels with bounded results.",
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "query": { "type": "string", "minLength": 1 },
            "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
        },
        "required": ["query"]
    }),
),
```

- [ ] **Step 4: Implement dispatch handlers**

Route the new mutation tools through existing `ProjectAction` validation/application. For `inspect_timeline`, return a bounded JSON payload:

```json
{
  "timelineDurationSeconds": 42.0,
  "window": { "startSeconds": 0.0, "endSeconds": 8.0 },
  "activeItems": [
    {
      "trackId": "track-video",
      "itemId": "clip-1",
      "kind": "clip",
      "label": "Opening",
      "startSeconds": 0.0,
      "durationSeconds": 4.0,
      "source": { "type": "media", "mediaId": "media-1" },
      "sourceIn": 12.0,
      "sourceOut": 16.0
    }
  ],
  "issues": []
}
```

- [ ] **Step 5: Add mutation validation tests**

In `src-tauri/tests/codex_mcp_server.rs`, call `video_creater.add_clips` with an invalid empty action array and assert the result is an error. Then call it with a valid `insertTimelineItem` or existing equivalent action shape and assert `mutatesProject` is true.

- [ ] **Step 6: Add agent-panel activity UI test**

In `src/components/workspace/agent-panel.test.tsx`, render a tool transcript containing `video_creater.inspect_timeline` and assert the row label is visible:

```ts
expect(screen.getByText("Inspect Timeline")).toBeInTheDocument();
```

- [ ] **Step 7: Verify tool tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_mcp codex_app_server -- --test-threads=1
rtk pnpm test src/components/workspace/agent-panel.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/codex/tools.rs src-tauri/tests/codex_mcp_server.rs src-tauri/tests/codex_app_server.rs src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx
git commit -m "feat: expand agent timeline tool surface"
```

---

## Task 3: Timeline-Native Generation Loop

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/source-clip-inspector.tsx`
- Modify: `src-tauri/src/generation/fal.rs`
- Modify: `src-tauri/src/generation/mock.rs`
- Test: `src/components/workspace/media-bin.test.tsx`
- Test: `src/components/workspace/editor-workspace.test.tsx`
- Test: `src-tauri/tests/generation_provider.rs`

- [ ] **Step 1: Write failing generation-loop tests**

Add UI tests that assert a generated timeline clip exposes these actions:

```ts
expect(screen.getByRole("button", { name: /rerun generation/i })).toBeInTheDocument();
expect(screen.getByRole("button", { name: /replace selected clip/i })).toBeInTheDocument();
expect(screen.getByRole("button", { name: /retry download/i })).toBeInTheDocument();
```

Add an editor-workspace test that completes a generated asset and verifies the queued timeline item is replaced by the generated media item while preserving `startSeconds`, `durationSeconds`, and `trackId`.

- [ ] **Step 2: Run failing UI tests**

Run:

```bash
rtk pnpm test src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because rerun/replace/download retry states are incomplete or not exposed consistently.

- [ ] **Step 3: Add generated asset action model**

In `editor-workspace.tsx`, centralize generated asset actions into functions with these behaviors:

- `rerunGeneratedAsset(assetId)`: creates a new queued generated asset using the prior prompt/model/references/settings.
- `replaceSelectedClipWithGeneratedOutput(assetId, outputMediaId)`: keeps the selected timeline item timing/track and changes its source to the generated media.
- `retryGeneratedAssetDownload(assetId)`: reuses the stored pending output URL or returns a visible failure state when no retry URL exists.

- [ ] **Step 4: Harden Rust generation completion actions**

In `src-tauri/src/generation/fal.rs` and `src-tauri/src/generation/mock.rs`, ensure completion actions include:

```json
{
  "generatedAssetId": "asset-id",
  "generatedOutputMediaId": "media-id",
  "placementIntent": "timeline_append | replace_clip | library_only",
  "references": {
    "mediaIds": [],
    "firstFrameMediaId": null,
    "lastFrameMediaId": null
  }
}
```

Use the existing project action types when possible; add a new typed action only if no existing action can express the behavior.

- [ ] **Step 5: Add Rust provider tests**

In `src-tauri/tests/generation_provider.rs`, add assertions that FAL and mock completion preserve `placementIntent`, output dimensions, duration, fps, and generated output media id.

- [ ] **Step 6: Verify generation loop tests**

Run:

```bash
rtk pnpm test src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx
rtk cargo test --manifest-path src-tauri/Cargo.toml generation_provider -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/components/workspace/media-bin.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/source-clip-inspector.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx src-tauri/src/generation/fal.rs src-tauri/src/generation/mock.rs src-tauri/tests/generation_provider.rs
git commit -m "feat: complete timeline-native generation loop"
```

---

## Task 4: Render And Export Lifecycle

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/render-quality-control.tsx`
- Modify: `src/components/workspace/render-report-panel.tsx`
- Modify: `src-tauri/src/project/export_profiles.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Test: `src/components/workspace/render-quality-control.test.tsx`
- Test: `src/components/workspace/render-report-panel.test.tsx`
- Test: `src/components/workspace/editor-workspace.test.tsx`
- Test: `src-tauri/tests/export_profiles.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing UI tests for render controls**

In `editor-workspace.test.tsx`, assert the top chrome exposes:

```ts
expect(screen.getByRole("combobox", { name: /render quality/i })).toBeInTheDocument();
expect(screen.getByRole("button", { name: /render draft/i })).toBeInTheDocument();
expect(screen.getByRole("button", { name: /export/i })).toBeInTheDocument();
```

In `render-report-panel.test.tsx`, assert empty, running, failed, and retry states:

```ts
expect(screen.getByText("No render report yet")).toBeInTheDocument();
expect(screen.getByText(/render running/i)).toBeInTheDocument();
expect(screen.getByRole("button", { name: /retry render/i })).toBeInTheDocument();
```

- [ ] **Step 2: Run failing UI tests**

Run:

```bash
rtk pnpm test src/components/workspace/render-quality-control.test.tsx src/components/workspace/render-report-panel.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because render quality and report lifecycle states are not fully wired.

- [ ] **Step 3: Wire `RenderQualityControl` into editor chrome**

Add editor state:

```ts
const [renderQualityProfile, setRenderQualityProfile] =
  useState<RenderQualityProfile>(defaultRenderQualityProfile);
```

Render:

```tsx
<RenderQualityControl
  value={renderQualityProfile}
  onChange={setRenderQualityProfile}
/>
```

Use the selected profile when building render workflow requests.

- [ ] **Step 4: Add render report lifecycle states**

Update `RenderReportPanel` props to include:

```ts
status: "empty" | "queued" | "running" | "completed" | "failed";
onRetryRender?: () => void;
onOpenArtifact?: (path: string) => void;
onOpenLog?: (path: string) => void;
```

Render visible states for each status with plain action labels: `Render draft`, `Retry render`, `Open artifact`, `Open log`.

- [ ] **Step 5: Write failing export profile tests**

In `src-tauri/tests/export_profiles.rs`, add a test that can run under an explicit approval env:

```rust
std::env::set_var("VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES", "1");
let report = mp4_export_profile_availability_report();
assert!(report.iter().any(|profile| profile.profile == ExportProfile::Mp4H264 && profile.available));
std::env::remove_var("VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES");
```

- [ ] **Step 6: Implement opt-in export availability**

In `src-tauri/src/project/export_profiles.rs`, when `VIDEO_CREATER_ENABLE_LOCAL_EXPORT_PROFILES=1`, return approved availability for H.264, H.265, and ProRes with required runtime names preserved.

- [ ] **Step 7: Add final render profile support**

In `src-tauri/src/render_pipeline/proposal.rs`, replace hardcoded `RenderQualityProfile::DraftWebm` selection with a parsed input profile. Keep default `DraftWebm` when no profile is supplied.

- [ ] **Step 8: Verify render/export tests**

Run:

```bash
rtk pnpm test src/components/workspace/render-quality-control.test.tsx src/components/workspace/render-report-panel.test.tsx src/components/workspace/editor-workspace.test.tsx
rtk cargo test --manifest-path src-tauri/Cargo.toml export_profiles render_pipeline -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/components/workspace/editor-workspace.tsx src/components/workspace/render-quality-control.tsx src/components/workspace/render-report-panel.tsx src/components/workspace/render-quality-control.test.tsx src/components/workspace/render-report-panel.test.tsx src/components/workspace/editor-workspace.test.tsx src-tauri/src/project/export_profiles.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/export_profiles.rs src-tauri/tests/render_pipeline.rs
git commit -m "feat: wire render and export lifecycle"
```

---

## Task 5: Native Transcription Readiness

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/src/transcription/fluidaudio.rs`
- Modify: `src-tauri/src/transcription/job.rs`
- Modify: `src/components/settings/model-settings.tsx`
- Modify: `src/App.tsx`
- Test: `src-tauri/tests/transcription_e2e.rs`
- Test: `src/components/settings/model-settings.test.tsx`

- [ ] **Step 1: Write failing runtime test**

In `src-tauri/tests/transcription_e2e.rs`, add a test that asserts native runtime selection never returns an `"awaiting model IO wiring"` error when a verified model path and helper are configured.

- [ ] **Step 2: Run failing transcription test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml transcription_e2e -- --test-threads=1
```

Expected: FAIL on the current stub native path unless the environment skips real-model execution.

- [ ] **Step 3: Complete native runtime IO wiring**

In `runtime.rs`, route verified model path, media path, language, and output artifact path into the FluidAudio helper. Return a typed transcript result containing:

```rust
pub struct NativeTranscriptionOutput {
    pub engine: String,
    pub raw_artifact_path: PathBuf,
    pub words: Vec<TranscriptWord>,
    pub segments: Vec<TranscriptSegment>,
}
```

- [ ] **Step 4: Surface readiness in settings**

In `model-settings.tsx`, show these states:

- `Native runtime ready`
- `Model verified`
- `Model missing files`
- `Runtime unsupported on this platform`
- `Transcription helper failed`

Use existing model status actions: activate, verify, remove, cancel download.

- [ ] **Step 5: Verify transcription UI tests**

Run:

```bash
rtk pnpm test src/components/settings/model-settings.test.tsx
rtk cargo test --manifest-path src-tauri/Cargo.toml transcription_e2e -- --test-threads=1
```

Expected: PASS, with real model tests skipped only when the required env/config is absent.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/transcription/runtime.rs src-tauri/src/transcription/fluidaudio.rs src-tauri/src/transcription/job.rs src-tauri/tests/transcription_e2e.rs src/components/settings/model-settings.tsx src/components/settings/model-settings.test.tsx src/App.tsx
git commit -m "feat: complete native transcription readiness"
```

---

## Task 6: HyperFrame Contract And Render Support

**Files:**
- Modify: `src-tauri/src/codex/proposal.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src/components/workspace/agent-panel.tsx`
- Modify: `src/components/workspace/template-inspector.tsx`
- Test: `src-tauri/tests/render_pipeline.rs`
- Test: `src-tauri/tests/codex_app_server.rs`
- Test: `src/components/workspace/agent-panel.test.tsx`

- [ ] **Step 1: Write failing contract tests**

In `src-tauri/tests/codex_app_server.rs`, add a proposal validation test with a HyperFrame layer missing `visualTreatment`, `motion`, `safeZone`, and `avoid`. Assert validation rejects it before render.

In `src-tauri/tests/render_pipeline.rs`, add a supported minimal HyperFrame fixture that becomes a transparent overlay artifact.

- [ ] **Step 2: Run failing HyperFrame tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_app_server render_pipeline -- --test-threads=1
```

Expected: FAIL because HyperFrames are accepted by schema but rejected by renderer.

- [ ] **Step 3: Validate HyperFrame visual metadata early**

In `src-tauri/src/codex/proposal.rs`, apply the same metadata requirement used for overlays to every HyperFrame object:

```json
{
  "visualTreatment": "kinetic cutout scene with transparent edges",
  "motion": "scale in 120ms, hold, slide out 160ms",
  "safeZone": "text and focal objects inside 10% margins",
  "avoid": "full-width black slabs, static text-only cards, centered plain boxes"
}
```

- [ ] **Step 4: Implement the first renderable HyperFrame path**

In `render_pipeline/proposal.rs`, support one minimal HyperFrame kind: transparent overlay generated from an existing graphics/template layer. Reject only unknown HyperFrame kinds with a precise error that names the unsupported `kind`.

- [ ] **Step 5: Add UI feedback for unsupported HyperFrames**

In `agent-panel.tsx`, when proposal validation returns unsupported HyperFrame errors, show:

```text
HyperFrame layer is not renderable yet. Ask Codex to express it as a caption, overlay, or template layer.
```

- [ ] **Step 6: Verify HyperFrame tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml codex_app_server render_pipeline -- --test-threads=1
rtk pnpm test src/components/workspace/agent-panel.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/codex/proposal.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs src-tauri/tests/codex_app_server.rs src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/template-inspector.tsx
git commit -m "feat: harden hyperframe proposal rendering"
```

---

## Task 7: Editor Polish States And Navigation

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/preview-panel.tsx`
- Modify: `src/components/workspace/render-report-panel.tsx`
- Modify: `src/components/workspace/project-timeline-inspector.tsx`
- Test: `src/components/workspace/editor-workspace.test.tsx`
- Test: `src/components/workspace/media-bin.test.tsx`
- Test: `src/components/workspace/preview-panel.test.tsx`
- Test: `src/components/workspace/render-report-panel.test.tsx`

- [ ] **Step 1: Write failing polish tests**

Add tests for:

```ts
expect(screen.getByText(/saved/i)).toBeInTheDocument();
expect(screen.getByText(/saving/i)).toBeInTheDocument();
expect(screen.getByText(/preview failed/i)).toBeInTheDocument();
expect(screen.getByRole("button", { name: /retry preview/i })).toBeInTheDocument();
expect(screen.getByRole("button", { name: /home/i })).not.toHaveAttribute("aria-disabled", "false");
```

- [ ] **Step 2: Run failing polish tests**

Run:

```bash
rtk pnpm test src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/render-report-panel.test.tsx
```

Expected: FAIL because save state, preview retry, and inert navigation are not normalized.

- [ ] **Step 3: Add explicit save/dirty status**

Add a compact status in editor chrome:

- `Saved`
- `Saving`
- `Unsaved changes`
- `Save failed`

Set status from project action persistence success/failure paths.

- [ ] **Step 4: Make inert navigation honest**

For the current non-home implementation, make the Home button disabled with `aria-disabled="true"` and tooltip text `Project home is not available in this build`. Do not leave it looking active.

- [ ] **Step 5: Add preview failure recovery**

When media preview fails, render a visible recovery state:

```tsx
<div role="alert">
  <p>Preview failed</p>
  <Button onClick={onRetryPreview}>Retry preview</Button>
</div>
```

- [ ] **Step 6: Add guided empty states**

Replace plain empty copy in media bin, render report panel, and inspector with one action per empty state:

- Empty media bin: `Import media`
- No transcript: `Transcribe selected media`
- No render report: `Render draft`
- No selected timeline item: `Select a clip`

- [ ] **Step 7: Verify polish tests**

Run:

```bash
rtk pnpm test src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/render-report-panel.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/components/workspace/editor-workspace.tsx src/components/workspace/media-bin.tsx src/components/workspace/preview-panel.tsx src/components/workspace/render-report-panel.tsx src/components/workspace/project-timeline-inspector.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/render-report-panel.test.tsx
git commit -m "feat: polish editor state feedback"
```

---

## Task 8: End-To-End Palmier-Parity Acceptance

**Files:**
- Modify: `src-tauri/src/bin/video-creater-e2e-combined.rs`
- Modify: `src-tauri/src/bin/video-creater-codex-e2e.rs`
- Modify: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Add combined acceptance scenario**

Extend the combined E2E fixture to prove this workflow:

1. Import media.
2. Transcribe media or use stored fixture transcript.
3. Generate a real EDL with multiple `sourceIn/sourceOut` clips.
4. Add captions and one visual overlay with `visualTreatment`, `motion`, `safeZone`, and `avoid`.
5. Render draft.
6. Validate duration, stream presence, artifact paths, log path, and selected source ranges.
7. Attach the render report to the project.

- [ ] **Step 2: Add Codex tool acceptance scenario**

Extend `video-creater-codex-e2e.rs` so Codex local tools can:

1. Inspect project context.
2. Inspect timeline.
3. Validate a proposal.
4. Apply project actions.
5. Build render/export workflow requests.

- [ ] **Step 3: Run acceptance checks**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
rtk pnpm e2e:combined
rtk pnpm e2e:codex
```

Expected: PASS. If GStreamer or real transcription prerequisites are absent, the E2E binary must print a clear skip reason and exit successfully only for explicitly optional real-runtime cases.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/bin/video-creater-e2e-combined.rs src-tauri/src/bin/video-creater-codex-e2e.rs src-tauri/tests/one_click_edit.rs src-tauri/tests/render_pipeline.rs src/components/workspace/editor-workspace.test.tsx
git commit -m "test: add palmier parity acceptance coverage"
```

---

## Execution Order

Recommended order:

1. Task 1: Real timeline preview.
2. Task 7: Polish states, because preview and chrome issues are user-visible.
3. Task 2: Agent tools.
4. Task 3: Generation loop.
5. Task 4: Render/export lifecycle.
6. Task 5: Native transcription readiness.
7. Task 6: HyperFrame contract/render support.
8. Task 8: End-to-end acceptance.

## Acceptance Criteria

The work is complete when:

- Timeline mode previews ordinary media clips, images, generated clips, text, captions, and templates without a blank viewport.
- Agent/MCP tools can inspect, mutate, generate, render, and export through typed validation.
- Generated assets can be rerun, retried, placed, and used to replace existing timeline clips while preserving timing.
- Render/export controls expose draft/final quality and visible queued/running/failed/completed states.
- MP4/ProRes export availability is explicit and can be enabled through approved runtime configuration.
- Native transcription readiness is visible and no longer blocked by model IO stub errors when configured.
- HyperFrame proposals either render through the first supported path or fail early with actionable validation feedback.
- Editor chrome communicates save state, preview failures, empty states, and disabled navigation honestly.
- `rtk pnpm verify` and the relevant E2E commands pass or skip only optional real-runtime checks with explicit reasons.

## Self-Review

- Spec coverage: All comparison gaps are represented by a task: preview, agent tool parity, generation loop, render/export, transcription, HyperFrames, and polish states.
- Red-flag scan: This plan avoids unresolved-marker wording and vague "handle edge cases" steps. Every task has file paths, concrete tests, commands, and acceptance behavior.
- Type consistency: New TypeScript names use `TimelinePreviewFrame`, `TimelinePreviewLayer`, and `TimelinePreviewCompositor`; new tool names use the `video_creater.*` namespace consistently; Rust export profile names match existing `ExportProfile` variants.
