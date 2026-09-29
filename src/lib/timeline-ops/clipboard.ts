import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { itemAllowedOnTrack, type Timeline, type TimelineItem } from "@/lib/timeline";
import { cloneItemsForTracks, type CommandResult } from "@/lib/timeline-ops/clip-commands";
import { duplicateTimelineItemId } from "@/lib/timeline-ops/ids";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";

interface TimelineClipboardEntry {
  /** Track the item was copied from. */
  readonly trackId: string;
  /** Display-order track distance from the first copied clip's track. */
  readonly trackOffset: number;
  /** Seconds from the earliest copied clip start (3 dp). */
  readonly startOffset: number;
  /** Deep clone of the copied item. */
  readonly item: TimelineItem;
}

export interface TimelineClipboard {
  readonly entries: readonly TimelineClipboardEntry[];
  /** Span from the earliest start to the latest end of the copied clips. */
  readonly durationSeconds: number;
}

export type PasteMode = "paste" | "insert";

/**
 * Copies clips (locked tracks included). Entries are sorted by display track index, then
 * start, then id; offsets are relative to the first entry's track and the earliest start.
 * Track indices use band order (`orderedTracksByBand`), which is the order the redesigned
 * timeline shows; the legacy editor used stored order, which matched its display.
 */
export function copyItems(timeline: Timeline, itemIds: readonly string[]): TimelineClipboard | null {
  const wanted = new Set(itemIds);
  const captures = orderedTracksByBand(timeline).flatMap((track, trackIndex) =>
    track.items.filter((item) => wanted.has(item.id)).map((item) => ({ item, trackId: track.id, trackIndex })),
  );
  captures.sort(
    (first, second) =>
      first.trackIndex - second.trackIndex ||
      first.item.startSeconds - second.item.startSeconds ||
      first.item.id.localeCompare(second.item.id),
  );
  const [firstCapture] = captures;
  if (!firstCapture) return null;
  const earliestStart = Math.min(...captures.map(({ item }) => item.startSeconds));
  return {
    entries: captures.map(({ item, trackId, trackIndex }) => ({
      trackId,
      trackOffset: trackIndex - firstCapture.trackIndex,
      startOffset: roundTimelineSeconds(item.startSeconds - earliestStart),
      item: structuredClone(item),
    })),
    durationSeconds: Math.max(
      ...captures.map(({ item }) => item.startSeconds - earliestStart + item.durationSeconds),
    ),
  };
}

/**
 * Plans a paste ("paste": `addItems`, which overwrites what is under the clips) or a paste
 * insert ("insert": `insertItems`, which ripples later clips on each target track) at the
 * playhead. Each entry lands on the display track at anchor index + `trackOffset`, where the
 * anchor is the first entry's original track. When that track is gone (for example removed
 * after a cut emptied it), the anchor falls back to the first unlocked track that accepts the
 * first clip. Ids come from `newId` (default `duplicateTimelineItemId`), made unique.
 */
export function pastePlan(
  project: VideoProject,
  clipboard: TimelineClipboard,
  playheadSeconds: number,
  mode: PasteMode,
  newId: (sourceItemId: string) => string = (sourceItemId) => duplicateTimelineItemId(project, sourceItemId),
): CommandResult {
  const [firstEntry] = clipboard.entries;
  if (!firstEntry) return { blocked: "Copy a clip first." };

  const tracks = orderedTracksByBand(project.timeline);
  let anchorIndex = tracks.findIndex((track) => track.id === firstEntry.trackId);
  if (anchorIndex < 0) {
    anchorIndex = tracks.findIndex((track) => !track.locked && itemAllowedOnTrack(firstEntry.item.kind, track.kind));
  }
  const pasteSeconds = roundTimelineSeconds(Number.isFinite(playheadSeconds) ? Math.max(0, playheadSeconds) : 0);

  const placements = [];
  for (const entry of clipboard.entries) {
    const target = anchorIndex < 0 ? undefined : tracks[anchorIndex + entry.trackOffset];
    if (!target) return { blocked: "There aren't enough matching tracks to paste these clips." };
    if (target.locked) return { blocked: "Unlock the destination tracks to paste." };
    if (!itemAllowedOnTrack(entry.item.kind, target.kind)) {
      return { blocked: "These clips can't go on the destination tracks." };
    }
    placements.push({
      source: entry.item,
      targetTrackId: target.id,
      // insertItems lays clips out from insertSeconds; the offset only keeps their order.
      startSeconds: mode === "paste" ? roundTimelineSeconds(pasteSeconds + entry.startOffset) : entry.startOffset,
    });
  }

  const itemsByTrack = cloneItemsForTracks(project, placements, {
    newId,
    labelSuffix: mode === "paste" ? "copy" : "insert",
    linkPrefix: "link-paste",
  });
  const actions: ProjectAction[] = [...itemsByTrack].map(([targetTrackId, items]) =>
    mode === "paste"
      ? { type: "addItems", targetTrackId, items }
      : { type: "insertItems", targetTrackId, insertSeconds: pasteSeconds, items },
  );
  return { actions };
}
