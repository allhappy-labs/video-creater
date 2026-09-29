import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";

/**
 * Reversed clip playback. Mirrors `src-tauri/src/project/reverse.rs`.
 *
 * A clip plays its source window `[sourceIn, sourceOut]` at `speed`. Forward clips read
 * `sourceIn + t * speed` at clip-local timeline second `t`; reversed clips (`properties.reverse`)
 * read `sourceOut - t * speed`. Handles swap (a reversed head handle is
 * `(mediaDuration - sourceOut) / speed`, its tail handle `sourceIn / speed`), the left part of a
 * split reversed clip keeps `sourceOut`, and trimming a reversed clip's left edge moves `sourceOut`
 * while its right edge moves `sourceIn`.
 */

type ClipReverseAction = Extract<ProjectAction, { type: "updateClipReverse" }>;

export interface SourceWindow {
  readonly sourceIn: number;
  readonly sourceOut: number;
  readonly speed: number;
  readonly reverse: boolean;
}

/** Whether `item` plays its source window backwards. */
export function isReversedItem(item: Pick<TimelineItem, "properties">): boolean {
  return item.properties.reverse === true;
}

/**
 * Whether `item` can play reversed: a video clip of video media, or an audio clip of project media
 * (the clips render preparation can reverse).
 */
export function isReversibleItem(project: Pick<VideoProject, "media">, item: TimelineItem): boolean {
  const { source } = item;
  if (source.type !== "media") return false;
  if (item.kind === "audio_clip") return true;
  return item.kind === "video_clip" && project.media.some((media) => media.id === source.mediaId && media.kind === "video");
}

/** The source second read at clip-local `localSeconds`; outside the clip it continues into the handles. */
export function sourceSecondsAt(window: SourceWindow, localSeconds: number): number {
  return window.reverse
    ? window.sourceOut - localSeconds * window.speed
    : window.sourceIn + localSeconds * window.speed;
}

/** The clip-local second at which `sourceSeconds` plays: the inverse of `sourceSecondsAt`. */
export function localSecondsForSource(window: SourceWindow, sourceSeconds: number): number {
  return (window.reverse ? window.sourceOut - sourceSeconds : sourceSeconds - window.sourceIn) / window.speed;
}

/**
 * The ordered timeline range over which source `[sourceStart, sourceEnd]` plays in a clip starting at
 * `clipStartSeconds`: forward `start + (a - sourceIn) / speed` to `start + (b - sourceIn) / speed`,
 * reversed `start + (sourceOut - b) / speed` to `start + (sourceOut - a) / speed`.
 */
export function timelineRangeForSource(
  window: SourceWindow,
  clipStartSeconds: number,
  sourceStartSeconds: number,
  sourceEndSeconds: number,
): [number, number] {
  const start = clipStartSeconds + localSecondsForSource(window, sourceStartSeconds);
  const end = clipStartSeconds + localSecondsForSource(window, sourceEndSeconds);
  return [Math.min(start, end), Math.max(start, end)];
}

/**
 * The source second `outsideSeconds` into a transition handle: the head (before the clip) continues
 * from the edge the clip starts at, the tail (after the clip) from the edge it ends at.
 */
export function handleSourceSeconds(window: SourceWindow, handle: "head" | "tail", outsideSeconds: number): number {
  const source = outsideSeconds * window.speed;
  return (handle === "head") !== window.reverse ? window.sourceIn - source : window.sourceOut + source;
}

/** Unused media before the clip's start, in timeline seconds. */
export function headHandleSeconds(window: SourceWindow, mediaDurationSeconds: number): number {
  const source = window.reverse ? mediaDurationSeconds - window.sourceOut : window.sourceIn;
  return Math.max(source / window.speed, 0);
}

/** Unused media after the clip's end, in timeline seconds. */
export function tailHandleSeconds(window: SourceWindow, mediaDurationSeconds: number): number {
  const source = window.reverse ? window.sourceIn : mediaDurationSeconds - window.sourceOut;
  return Math.max(source / window.speed, 0);
}

/**
 * The source window of the clip-local part `[start, end]` of a clip that lasted `originalDuration`.
 * The edge a part shares with the original clip keeps the stored value.
 */
export function sourceSubrange(
  window: SourceWindow,
  startSeconds: number,
  endSeconds: number,
  originalDurationSeconds: number,
): [number, number] {
  const keepsEnd = endSeconds >= originalDurationSeconds;
  if (window.reverse) {
    return [
      keepsEnd ? window.sourceIn : window.sourceOut - endSeconds * window.speed,
      window.sourceOut - startSeconds * window.speed,
    ];
  }
  return [
    window.sourceIn + startSeconds * window.speed,
    keepsEnd ? window.sourceOut : window.sourceIn + endSeconds * window.speed,
  ];
}

/** The source window after `edge` moves so the clip gains `durationDeltaSeconds` (negative shrinks it). */
export function trimmedSourceRange(
  window: SourceWindow,
  edge: "left" | "right",
  durationDeltaSeconds: number,
): [number, number] {
  const sourceDelta = durationDeltaSeconds * window.speed;
  const movesSourceIn = (edge === "left") !== window.reverse;
  return movesSourceIn
    ? [window.sourceIn - sourceDelta, window.sourceOut]
    : [window.sourceIn, window.sourceOut + sourceDelta];
}

/**
 * Sets or clears `reverse` on a clip `isReversibleItem` accepts, returning the updated
 * tracks. Mirrors Rust `update_clip_reverse`: missing items, locked tracks and other clips throw.
 */
export function applyClipReverse(project: VideoProject, action: ClipReverseAction): TimelineTrack[] {
  const track = project.timeline.tracks.find((candidate) => candidate.items.some((item) => item.id === action.itemId));
  const item = track?.items.find((candidate) => candidate.id === action.itemId);
  if (!track || !item) throw new Error(`Timeline item ${action.itemId} was not found.`);
  if (track.locked) throw new Error(`Track ${track.id} is locked.`);
  const { source } = item;
  if (item.kind === "video_clip" && source.type === "media" && !project.media.some((media) => media.id === source.mediaId)) {
    throw new Error(`Media ${source.mediaId} was not found.`);
  }
  if (!isReversibleItem(project, item)) throw new Error(`Timeline item ${action.itemId} cannot be reversed.`);
  return project.timeline.tracks.map((candidate) =>
    candidate === track
      ? {
          ...candidate,
          items: candidate.items.map((entry) => {
            if (entry.id !== action.itemId) return entry;
            const properties = { ...entry.properties };
            if (action.reverse) properties.reverse = true;
            else delete properties.reverse;
            return { ...entry, properties };
          }),
        }
      : candidate,
  );
}
