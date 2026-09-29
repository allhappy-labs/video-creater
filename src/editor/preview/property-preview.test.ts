import { describe, expect, it } from "vitest";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { timelineWithPropertyPreview } from "./property-preview";

describe("timelineWithPropertyPreview", () => {
  it("returns the same timeline without a preview or for a missing item", () => {
    const { timeline } = fixtureProject();
    expect(timelineWithPropertyPreview(timeline, null)).toBe(timeline);
    expect(timelineWithPropertyPreview(timeline, { itemId: "missing", patch: { opacity: 0.3 } })).toBe(timeline);
  });

  it("overrides the item's properties on a copy", () => {
    const project = fixtureProject();
    const item = fixtureItem(project, "video");
    item.properties.rotationDegrees = 10;
    const previewed = timelineWithPropertyPreview(project.timeline, { itemId: item.id, patch: { opacity: 0.3, rotationDegrees: 45 } });
    const layer = buildTimelinePreviewFrame({ timeline: previewed, media: project.media, playheadSeconds: 1 }).layers[0];
    expect(layer).toMatchObject({ opacity: 0.3, rotationDegrees: 45 });
    expect(item.properties).toMatchObject({ rotationDegrees: 10 });
    expect(item.properties.opacity).toBeUndefined();
    expect(previewed.tracks[1]).toBe(project.timeline.tracks[1]);
  });

  it("shows the previewed value over the keyframes of that property only", () => {
    const project = fixtureProject();
    const item = fixtureItem(project, "video");
    item.properties.keyframes = {
      rotationDegrees: [
        { atSeconds: 0, value: -90, easing: "linear" },
        { atSeconds: 2, value: 90, easing: "linear" },
      ],
      positionX: [{ atSeconds: 0, value: 40, easing: "linear" }],
    };
    const previewed = timelineWithPropertyPreview(project.timeline, { itemId: item.id, patch: { rotationDegrees: 12 } });
    const layer = buildTimelinePreviewFrame({ timeline: previewed, media: project.media, playheadSeconds: 1 }).layers[0];
    expect(layer).toMatchObject({ rotationDegrees: 12, positionX: 40 });
    expect(item.properties.keyframes).toHaveProperty("rotationDegrees");
  });
});
