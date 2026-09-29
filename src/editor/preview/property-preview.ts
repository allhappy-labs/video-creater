import type { Timeline } from "@/lib/timeline";
import type { UiSlice } from "../store/ui-slice";

type PropertyPreview = NonNullable<UiSlice["propertyPreview"]>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * The timeline with a live Properties value applied over one item's properties, for building the
 * preview frame while a control is dragged. The project is never mutated: only the patched item and
 * its track are copied. Keyframe lanes of patched properties are dropped from the copy so the
 * dragged value shows at the playhead instead of the interpolated one.
 */
export function timelineWithPropertyPreview(timeline: Timeline, preview: PropertyPreview | null): Timeline {
  if (!preview) return timeline;
  const trackIndex = timeline.tracks.findIndex((track) => track.items.some((item) => item.id === preview.itemId));
  const track = timeline.tracks[trackIndex];
  if (!track) return timeline;
  const patchedKeys = new Set(Object.keys(preview.patch));
  const items = track.items.map((item) => {
    if (item.id !== preview.itemId) return item;
    const properties: Record<string, unknown> = { ...item.properties, ...preview.patch };
    const { keyframes } = item.properties;
    if (isRecord(keyframes)) {
      properties.keyframes = Object.fromEntries(Object.entries(keyframes).filter(([property]) => !patchedKeys.has(property)));
    }
    return { ...item, properties };
  });
  const tracks = timeline.tracks.map((candidate, index) => (index === trackIndex ? { ...track, items } : candidate));
  return { ...timeline, tracks };
}
