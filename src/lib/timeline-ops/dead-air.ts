import { roundTimelineSeconds } from "@/lib/format";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty } from "./item-properties";
import { isReversedItem, timelineRangeForSource } from "./reverse";

/** A dead-air span of one timeline item, in absolute timeline seconds and item-relative ratios. */
export interface DeadAirRange {
  readonly startSeconds: number;
  readonly endSeconds: number;
  /** 0..1 offset of `startSeconds` within the item, rounded to 4 decimals. */
  readonly startRatio: number;
  /** 0..1 offset of `endSeconds` within the item, rounded to 4 decimals. */
  readonly endRatio: number;
}

type MediaSilenceRange = NonNullable<VideoProject["mediaSilenceRanges"]>[number];

/**
 * Maps `project.mediaSilenceRanges` for the item's media through its source range
 * (`sourceIn`, `sourceOut` or `sourceIn + duration * speed`), speed and direction onto the timeline.
 * Ranges are clipped to the item bounds, sorted and merged where they overlap or touch.
 */
export function deadAirRangesForItem(project: VideoProject, item: TimelineItem): DeadAirRange[] {
  if (item.source.type !== "media") return [];
  const mediaId = item.source.mediaId;
  const speed = itemSpeed(item);
  if (speed === null || !(item.durationSeconds > 0)) return [];

  const sourceIn = numberProperty(item, "sourceIn") ?? 0;
  const sourceOut = numberProperty(item, "sourceOut") ?? sourceIn + item.durationSeconds * speed;
  const sourceWindow = { sourceIn, sourceOut, speed, reverse: isReversedItem(item) };
  const itemStart = item.startSeconds;
  const itemEnd = item.startSeconds + item.durationSeconds;

  const spans = (project.mediaSilenceRanges ?? [])
    .filter((range) => range.mediaId === mediaId && validSilenceRange(range))
    .flatMap((range) => {
      const clippedIn = Math.max(range.sourceIn, sourceIn);
      const clippedOut = Math.min(range.sourceOut, sourceOut);
      if (clippedOut <= clippedIn) return [];
      const [timelineStart, timelineEnd] = timelineRangeForSource(sourceWindow, itemStart, clippedIn, clippedOut);
      const start = Math.max(itemStart, timelineStart);
      const end = Math.min(itemEnd, timelineEnd);
      return end > start ? [{ start, end }] : [];
    })
    .sort((left, right) => left.start - right.start);

  const merged: Array<{ start: number; end: number }> = [];
  for (const span of spans) {
    const last = merged[merged.length - 1];
    if (last && span.start <= last.end) {
      last.end = Math.max(last.end, span.end);
    } else {
      merged.push({ ...span });
    }
  }

  return merged.map(({ start, end }) => ({
    startSeconds: roundTimelineSeconds(start),
    endSeconds: roundTimelineSeconds(end),
    startRatio: roundRatio((start - itemStart) / item.durationSeconds),
    endRatio: roundRatio((end - itemStart) / item.durationSeconds),
  }));
}

function itemSpeed(item: TimelineItem): number | null {
  const value = item.properties.speed;
  if (value === undefined) return 1;
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : null;
}

function validSilenceRange(range: MediaSilenceRange) {
  return (
    range.mediaId.trim().length > 0 &&
    Number.isFinite(range.sourceIn) &&
    Number.isFinite(range.sourceOut) &&
    Number.isFinite(range.confidence) &&
    range.sourceOut > range.sourceIn
  );
}

function roundRatio(ratio: number) {
  return Number(Math.min(1, Math.max(0, ratio)).toFixed(4));
}
