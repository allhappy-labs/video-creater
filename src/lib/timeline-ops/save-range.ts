import { roundTimelineSeconds } from "@/lib/format";
import type { VideoProject } from "@/lib/project";
import type { TimelineRangeSelection } from "@/lib/timeline-ops/navigation";

export const saveRangeUnavailableReason = "Select a clip or mark in and out points";

interface SaveRangeContext {
  readonly selectedItemIds: readonly string[];
  /** I/O marks in timeline seconds. */
  readonly rangeIn: number | null;
  readonly rangeOut: number | null;
}

type SaveRangeTarget =
  | { readonly range: TimelineRangeSelection; readonly source: "selection" | "marks" }
  | { readonly blocked: string };

/**
 * The timeline range "Save range as media" renders. Selected clips on the active timeline win, using
 * the span from the earliest start to the latest end. Otherwise both I/O marks must be set, with
 * the out point after the in point; the range stops at the end of the timeline.
 */
export function saveRangeTarget(project: VideoProject, context: SaveRangeContext): SaveRangeTarget {
  const selected = new Set(context.selectedItemIds);
  const items = project.timeline.tracks.flatMap((track) => track.items.filter((item) => selected.has(item.id)));
  if (items.length > 0) {
    const startSeconds = roundTimelineSeconds(Math.min(...items.map((item) => item.startSeconds)));
    const endSeconds = roundTimelineSeconds(Math.max(...items.map((item) => item.startSeconds + item.durationSeconds)));
    if (endSeconds > startSeconds) return { range: { startSeconds, endSeconds }, source: "selection" };
  }
  const { rangeIn, rangeOut } = context;
  if (rangeIn !== null && rangeOut !== null) {
    const startSeconds = roundTimelineSeconds(rangeIn);
    const endSeconds = roundTimelineSeconds(Math.min(rangeOut, project.timeline.durationSeconds));
    if (endSeconds > startSeconds) return { range: { startSeconds, endSeconds }, source: "marks" };
  }
  return { blocked: saveRangeUnavailableReason };
}
