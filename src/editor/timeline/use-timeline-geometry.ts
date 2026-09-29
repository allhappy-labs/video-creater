import { useLayoutEffect, useMemo, useState, type RefObject } from "react";
import type { Timeline, TimelineItem, TimelineTrack } from "@/lib/timeline";
import type { ClipTrackTarget } from "@/lib/timeline-ops/clip-drag";
import type { MarqueeGeometry } from "@/lib/timeline-ops/marquee";
import { basePixelsPerSecond } from "@/lib/timeline-ops/navigation";
import { orderedTracksByBand, trackDisplayNames } from "@/lib/timeline-ops/track-bands";
import {
  timelineItemIntersectsWindow,
  timelineViewportWindow,
  type TimelineViewportWindow,
} from "@/lib/timeline-viewport";
import { useEditorStore } from "../store/editor-store-context";
import { clamp, zoomBounds } from "../store/persisted-layout";

export const timelineHeaderWidth = { desktop: 118, mobile: 0 } as const;
export const timelineRowHeight = { mainVideo: 58, other: 34, keyframeLane: 40 } as const;
/**
 * Heights rows compact down to when the lanes viewport is too short for every row. Touch rows keep
 * a 24 px clip lane for transition edge handles.
 */
export const timelineMinimumRowHeight = {
  desktop: { mainVideo: 46, other: 28 },
  touch: { mainVideo: 48, other: 30 },
} as const;
export const timelineRulerHeight = 24;
/** Vertical gap between a clip and its row edges. */
export const timelineClipInset = 3;
/** Distance from a row edge within which a dragged clip targets a new track between rows. */
const insertZonePixels = 8;

/** Empty space after the last clip so the end of the timeline is never flush with the edge. */
const trailingPixels = 160;
/** jsdom and first paint report no width; the legacy timeline used the same fallback. */
const fallbackViewportWidth = 720;
const overscanViewports = 1;

export interface TimelineRow {
  readonly track: TimelineTrack;
  /** Visible name, for example "Video 1". */
  readonly name: string;
  readonly top: number;
  readonly height: number;
  /** The first video track in band order: the tall main row. */
  readonly main: boolean;
}

export interface TimelineGeometry {
  readonly pixelsPerSecond: number;
  readonly headerWidth: number;
  /**
   * Scrollable space before and after the lanes. Mobile pads by half the viewport so the fixed
   * centre playhead can reach time 0 and the end; desktop has none.
   */
  readonly centerPadding: number;
  /** Width of the lanes viewport, excluding the header column. */
  readonly viewportWidth: number;
  /** Scrollable lane content width. */
  readonly contentWidth: number;
  /** `contentWidth` expressed in seconds. */
  readonly renderedDurationSeconds: number;
  /** Horizontal scroll clamped to the scrollable range (padding included). */
  readonly scrollLeft: number;
  readonly window: TimelineViewportWindow;
  readonly rows: readonly TimelineRow[];
  /** The keyframe lane under the selected clip's track, when shown. */
  readonly keyframeLane: { readonly trackId: string; readonly top: number; readonly height: number } | null;
  readonly totalHeight: number;
  secondsToX(seconds: number): number;
  xToSeconds(x: number): number;
}

interface TimelineGeometryInput {
  readonly timeline: Timeline;
  readonly zoomPercent: number;
  readonly scrollLeft: number;
  readonly viewportWidth: number;
  /**
   * Height of the scroll container (ruler included). Rows compact toward their minimum heights so
   * every row fits above the overview bar; 0 (unmeasured) keeps nominal heights.
   */
  readonly viewportHeight?: number;
  readonly mobile: boolean;
  /** Track that gets the keyframe lane row right below it. */
  readonly keyframeLaneTrackId?: string | null;
}

export function computeTimelineGeometry(input: TimelineGeometryInput): TimelineGeometry {
  const pixelsPerSecond = (basePixelsPerSecond * clamp(input.zoomPercent, zoomBounds.min, zoomBounds.max)) / 100;
  const viewportWidth = Math.max(1, input.viewportWidth);
  const durationSeconds = Math.max(0, input.timeline.durationSeconds);
  const centerPadding = input.mobile ? viewportWidth / 2 : 0;
  const contentWidth = input.mobile
    ? Math.max(1, Math.ceil(durationSeconds * pixelsPerSecond))
    : Math.max(viewportWidth, Math.ceil(durationSeconds * pixelsPerSecond + trailingPixels));
  const renderedDurationSeconds = contentWidth / pixelsPerSecond;
  const scrollLeft = clamp(input.scrollLeft, 0, Math.max(0, contentWidth + centerPadding * 2 - viewportWidth));
  const window = timelineViewportWindow({
    durationSeconds: renderedDurationSeconds,
    pixelsPerSecond,
    scrollLeft: scrollLeft - centerPadding,
    viewportWidth,
    overscanViewports,
  });

  const names = trackDisplayNames(input.timeline);
  const tracks = orderedTracksByBand(input.timeline);
  const mainTrackId = tracks.find((track) => track.kind === "video")?.id ?? null;
  const keyframeLaneHeight = tracks.some((track) => track.id === input.keyframeLaneTrackId) ? timelineRowHeight.keyframeLane : 0;
  const heights = fittedRowHeights(
    tracks.map((track) => track.id === mainTrackId),
    (input.viewportHeight ?? 0) - timelineRulerHeight - keyframeLaneHeight,
    input.mobile ? timelineMinimumRowHeight.touch : timelineMinimumRowHeight.desktop,
  );
  let top = 0;
  let keyframeLane: TimelineGeometry["keyframeLane"] = null;
  const rows = tracks.map((track, index): TimelineRow => {
    const main = track.id === mainTrackId;
    const height = heights[index] ?? timelineRowHeight.other;
    const row = { track, name: names.get(track.id) ?? track.name, top, height, main };
    top += height;
    if (track.id === input.keyframeLaneTrackId) {
      keyframeLane = { trackId: track.id, top, height: timelineRowHeight.keyframeLane };
      top += timelineRowHeight.keyframeLane;
    }
    return row;
  });

  return {
    pixelsPerSecond,
    headerWidth: input.mobile ? timelineHeaderWidth.mobile : timelineHeaderWidth.desktop,
    centerPadding,
    viewportWidth,
    contentWidth,
    renderedDurationSeconds,
    scrollLeft,
    window,
    rows,
    keyframeLane,
    totalHeight: top,
    secondsToX: (seconds) => seconds * pixelsPerSecond,
    xToSeconds: (x) => x / pixelsPerSecond,
  };
}

/**
 * Row heights for rows flagged main or not. When the nominal heights overflow `available` (lane
 * pixels under the ruler), each row gives up the same share of its room above its minimum, rounded
 * so the sum fits; rows never go below the minimum, and what still overflows scrolls.
 */
function fittedRowHeights(mainFlags: readonly boolean[], available: number, minimum: { readonly mainVideo: number; readonly other: number }): number[] {
  const nominal = mainFlags.map((main) => (main ? timelineRowHeight.mainVideo : timelineRowHeight.other));
  const floors = mainFlags.map((main) => (main ? minimum.mainVideo : minimum.other));
  const deficit = nominal.reduce((sum, height) => sum + height, 0) - available;
  const slack = nominal.reduce((sum, height, index) => sum + height - (floors[index] ?? height), 0);
  if (available <= 0 || deficit <= 0 || slack <= 0) return nominal;
  const share = Math.min(1, deficit / slack);
  return nominal.map((height, index) => {
    const floor = floors[index] ?? height;
    return Math.max(floor, height - Math.ceil((height - floor) * share));
  });
}

/** The track of the one selected clip when the keyframe lane is on; otherwise null. */
export function keyframeLaneTrackIdFor(timeline: Timeline, visible: boolean, selectedItemIds: readonly string[]): string | null {
  const [itemId] = selectedItemIds;
  if (!visible || selectedItemIds.length !== 1 || itemId === undefined) return null;
  return timeline.tracks.find((track) => track.items.some((item) => item.id === itemId))?.id ?? null;
}

/** Items that intersect the render window, plus any persistent ids (selected or focused). */
export function visibleTimelineItems(
  items: readonly TimelineItem[],
  window: TimelineViewportWindow,
  persistentItemIds: ReadonlySet<string>,
): TimelineItem[] {
  return items.filter((item) => timelineItemIntersectsWindow(item, window, persistentItemIds));
}

/**
 * The row a dragged clip centred at `y` (tracks-area pixels) is over and, near a row edge or
 * outside every row, the band-ordered index a new track would be inserted at.
 */
export function trackTargetAtY(rows: readonly TimelineRow[], y: number): ClipTrackTarget | null {
  const first = rows[0];
  const last = rows.at(-1);
  if (!first || !last) return null;
  if (y < first.top) return { trackId: first.track.id, insertIndex: 0 };
  const index = rows.reduce((found, row, candidate) => (y >= row.top ? candidate : found), -1);
  const row = rows[index];
  if (!row) return { trackId: last.track.id, insertIndex: rows.length };
  // Below the row (past the last row, or over the keyframe lane after it): insert after it.
  if (y >= row.top + row.height) return { trackId: row.track.id, insertIndex: index + 1 };
  const zone = Math.min(insertZonePixels, row.height / 4);
  if (y - row.top < zone) return { trackId: row.track.id, insertIndex: index };
  if (row.top + row.height - y <= zone) return { trackId: row.track.id, insertIndex: index + 1 };
  return { trackId: row.track.id, insertIndex: null };
}

/** Row and clip boxes for `itemsInMarquee`, in tracks-area pixels. */
export function marqueeGeometry(geometry: Pick<TimelineGeometry, "rows" | "pixelsPerSecond">): MarqueeGeometry {
  return {
    rows: geometry.rows.map((row) => ({ trackId: row.track.id, top: row.top, bottom: row.top + row.height })),
    items: geometry.rows.flatMap((row) =>
      row.track.items.map((item) => ({
        itemId: item.id,
        trackId: row.track.id,
        left: item.startSeconds * geometry.pixelsPerSecond,
        right: (item.startSeconds + item.durationSeconds) * geometry.pixelsPerSecond,
      })),
    ),
    verticalInset: timelineClipInset,
  };
}

/** The zoom percent that fits `durationSeconds` plus the trailing space into the viewport. */
export function fitZoom(durationSeconds: number, viewportWidth: number): number {
  if (!(durationSeconds > 0)) return zoomBounds.default;
  const pixelsPerSecond = Math.max(1, viewportWidth - trailingPixels) / durationSeconds;
  return clamp((pixelsPerSecond / basePixelsPerSecond) * 100, zoomBounds.min, zoomBounds.max);
}

interface MeasuredSize {
  readonly width: number;
  readonly height: number;
}

/** Client size of the scroll container, tracked with ResizeObserver where available. */
function useMeasuredSize(ref: RefObject<HTMLElement | null>): MeasuredSize {
  const [size, setSize] = useState<MeasuredSize>({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return undefined;
    const measure = () =>
      setSize((current) =>
        current.width === element.clientWidth && current.height === element.clientHeight ? current : { width: element.clientWidth, height: element.clientHeight },
      );
    measure();
    if (typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return size;
}

/** Geometry of the active timeline for a scroll container that also holds the header column. */
export function useTimelineGeometry(scrollerRef: RefObject<HTMLElement | null>, mobile: boolean): TimelineGeometry {
  const timeline = useEditorStore((state) => state.project.timeline);
  const keyframeLaneTrackId = useEditorStore((state) => keyframeLaneTrackIdFor(state.project.timeline, state.keyframesVisible, state.selectedItemIds));
  const zoomPercent = useEditorStore((state) => state.zoomPercent);
  const scrollLeft = useEditorStore((state) => state.scrollLeft);
  const measured = useMeasuredSize(scrollerRef);
  const headerWidth = mobile ? timelineHeaderWidth.mobile : timelineHeaderWidth.desktop;
  const viewportWidth = measured.width > headerWidth ? measured.width - headerWidth : fallbackViewportWidth;
  const viewportHeight = measured.height;
  return useMemo(
    () => computeTimelineGeometry({ timeline, zoomPercent, scrollLeft, viewportWidth, viewportHeight, mobile, keyframeLaneTrackId }),
    [timeline, zoomPercent, scrollLeft, viewportWidth, viewportHeight, mobile, keyframeLaneTrackId],
  );
}
