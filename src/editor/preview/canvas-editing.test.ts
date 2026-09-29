import { describe, expect, it } from "vitest";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { canvasEditableItemIds, cropModePlayheadSeconds, frameWithCanvasOverride, inlineTextEditableItemIds, interactiveCanvasLayers } from "./canvas-editing";

function textOverlayProject() {
  const project = fixtureProject();
  fixtureTrack(project, "overlay").items.push(
    { id: "title", kind: "overlay", startSeconds: 0, durationSeconds: 2, source: { type: "text", text: "Title" }, label: "Title", properties: {} },
    {
      id: "template",
      kind: "overlay",
      startSeconds: 0,
      durationSeconds: 2,
      source: { type: "text", text: "Lower third" },
      label: "Lower third",
      properties: { templateId: "kinetic-lower-third-v1" },
    },
  );
  return project;
}

describe("canvas editing model", () => {
  it("edits visual clips on unlocked tracks only", () => {
    const project = fixtureProject();
    expect([...canvasEditableItemIds(project.timeline)]).toEqual(["item-1", "sample-generated-clip"]);
    fixtureTrack(project, "video").locked = true;
    expect(canvasEditableItemIds(project.timeline).size).toBe(0);
  });

  it("edits plain text overlays and captions inline, but not templates or locked tracks", () => {
    const project = textOverlayProject();
    expect([...inlineTextEditableItemIds(project.timeline)]).toEqual(["title", "caption-1", "caption-2"]);
    fixtureTrack(project, "caption").locked = true;
    expect([...inlineTextEditableItemIds(project.timeline)]).toEqual(["title"]);
  });

  it("hit-tests visible media layers, or prepared and uncovered-safe layers once canonical frames are ready", () => {
    const project = fixtureProject();
    fixtureItem(project, "video").properties.blendMode = "add";
    const frame = buildTimelinePreviewFrame({ timeline: project.timeline, media: project.media, playheadSeconds: 1 });
    const urls = { "media-1": "/p/media/input.mp4" };
    const ids = (canonical: Parameters<typeof interactiveCanvasLayers>[0]["canonical"], coverage: string[], mediaPreviewUrls: Record<string, string> = urls) =>
      interactiveCanvasLayers({ frame, canonical, coverageItemIds: new Set(coverage), mediaPreviewUrls }).map((layer) => layer.itemId);
    expect(ids(null, [])).toEqual(["item-1"]);
    expect(ids(null, [], {})).toEqual([]);
    expect(ids({ status: "pending" }, [])).toEqual([]);
    expect(ids({ status: "ready" }, [])).toEqual([]);
    expect(ids({ status: "ready" }, ["item-1"], {})).toEqual(["item-1"]);
  });

  it("applies a transient canvas override to one layer", () => {
    const project = fixtureProject();
    const frame = buildTimelinePreviewFrame({ timeline: project.timeline, media: project.media, playheadSeconds: 1 });
    expect(frameWithCanvasOverride(frame, null)).toBe(frame);
    const overridden = frameWithCanvasOverride(frame, { itemId: "item-1", patch: { rotationDegrees: 30, centerX: 0.25 } });
    expect(overridden.layers[0]).toMatchObject({ itemId: "item-1", rotationDegrees: 30, centerX: 0.25 });
    expect(frame.layers[0]).toMatchObject({ rotationDegrees: 0 });
    expect(overridden.overlayLayers).toBe(frame.overlayLayers);
  });

  it("moves a playhead outside the clip half a frame into it for crop mode", () => {
    const clip = { startSeconds: 4, durationSeconds: 4 };
    expect(cropModePlayheadSeconds(clip, 1, 24)).toBeCloseTo(4 + 1 / 48);
    expect(cropModePlayheadSeconds(clip, 8, 24)).toBeCloseTo(4 + 1 / 48);
    expect(cropModePlayheadSeconds(clip, 4, 24)).toBeNull();
    expect(cropModePlayheadSeconds(clip, 7.9, 24)).toBeNull();
    // Clips shorter than a frame use their midpoint; an unset frame rate falls back to 30 fps.
    expect(cropModePlayheadSeconds({ startSeconds: 2, durationSeconds: 0.01 }, 0, 24)).toBeCloseTo(2.005);
    expect(cropModePlayheadSeconds(clip, 0, 0)).toBeCloseTo(4 + 1 / 60);
  });
});
