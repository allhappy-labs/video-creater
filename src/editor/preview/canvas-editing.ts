import { isTemplateTimelineItem, type Timeline, type TimelineItem } from "@/lib/timeline";
import type { TimelinePreviewFrame, TimelinePreviewLayer } from "@/lib/timeline-preview";
import type { CompositorCanonicalState } from "./canonical-frames";

const fallbackFps = 30;

const canvasEditableItemKinds: ReadonlySet<TimelineItem["kind"]> = new Set(["video_clip", "image_clip", "lottie_clip", "generated_clip"]);

/** A transient canvas edit (a drag in progress, or a crop mode draft) drawn over one layer. */
export interface CanvasLayerOverride {
  readonly itemId: string;
  readonly patch: Readonly<
    Partial<Pick<TimelinePreviewLayer, "centerX" | "centerY" | "width" | "height" | "rotationDegrees" | "cropTop" | "cropRight" | "cropBottom" | "cropLeft">>
  >;
}

function unlockedItems(timeline: Timeline, include: (item: TimelineItem) => boolean): ReadonlySet<string> {
  return new Set(timeline.tracks.flatMap((track) => (track.locked ? [] : track.items.filter(include).map((item) => item.id))));
}

/** Legacy editable canvas layers: visual clips on unlocked tracks. */
export function canvasEditableItemIds(timeline: Timeline): ReadonlySet<string> {
  return unlockedItems(timeline, (item) => canvasEditableItemKinds.has(item.kind));
}

/** Items whose text edits inline on the canvas: plain text overlays and captions on unlocked tracks. */
export function inlineTextEditableItemIds(timeline: Timeline): ReadonlySet<string> {
  return unlockedItems(
    timeline,
    (item) => item.kind === "caption" || (item.kind === "overlay" && item.source.type === "text" && !isTemplateTimelineItem(item)),
  );
}

/**
 * Layers a canvas click can hit. Once canonical frames are ready: every layer that needs no
 * preparation or is covered by a prepared frame. Otherwise: the DOM media layers the compositor draws.
 */
export function interactiveCanvasLayers(input: {
  readonly frame: TimelinePreviewFrame;
  readonly canonical: CompositorCanonicalState;
  readonly coverageItemIds: ReadonlySet<string>;
  readonly mediaPreviewUrls: Readonly<Record<string, string | null | undefined>>;
}): readonly TimelinePreviewLayer[] {
  const { frame, canonical, coverageItemIds, mediaPreviewUrls } = input;
  if (canonical?.status === "ready") {
    return frame.layers.filter((layer) => !layer.canonicalPreparationRequired || coverageItemIds.has(layer.itemId));
  }
  return frame.layers.filter((layer) => !(canonical && layer.canonicalPreparationRequired) && Boolean(mediaPreviewUrls[layer.mediaId]));
}

export function frameWithCanvasOverride(frame: TimelinePreviewFrame, override: CanvasLayerOverride | null): TimelinePreviewFrame {
  if (!override) return frame;
  return { ...frame, layers: frame.layers.map((layer) => (layer.itemId === override.itemId ? { ...layer, ...override.patch } : layer)) };
}

/**
 * The playhead crop mode needs, or null to keep it. Crop mode edits the clip's canvas layer, which
 * exists only while the clip is active, so a playhead outside the clip moves half a frame past its
 * start (the midpoint for clips shorter than a frame).
 */
export function cropModePlayheadSeconds(
  item: Pick<TimelineItem, "startSeconds" | "durationSeconds">,
  playheadSeconds: number,
  fps: number,
): number | null {
  if (playheadSeconds >= item.startSeconds && playheadSeconds < item.startSeconds + item.durationSeconds) return null;
  const halfFrame = 0.5 / (fps > 0 ? fps : fallbackFps);
  return item.startSeconds + Math.min(halfFrame, item.durationSeconds / 2);
}
