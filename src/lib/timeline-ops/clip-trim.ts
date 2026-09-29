import { roundTimelineSeconds } from "@/lib/format";
import {
  planProjectRippleTrim,
  projectActionFromTimelinePatch,
  type ProjectAction,
  type ProjectActionRippleTrim,
  type VideoProject,
} from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import {
  evaluateTimelineResize,
  type TimelineEditState,
  type TimelineResolvedPlacement,
} from "@/lib/timeline-edit-evaluator";
import type { StickyTimelineSnap } from "@/lib/timeline-snap";
import { minimumClipDurationSeconds, snapClipProbes, type ClipSnapOptions } from "./clip-drag";
import type { TimelineRippleTrimRequest } from "./navigation";
import { rippleTrackIdsForItem } from "./silence";

export type ClipTrimEdge = "left" | "right";

export interface ClipTrimInput {
  readonly project: VideoProject;
  readonly itemId: string;
  readonly edge: ClipTrimEdge;
  /** Raw pointer travel in seconds since the press. */
  readonly deltaSeconds: number;
  readonly snap: ClipSnapOptions | null;
}

export interface ClipTrimPreview {
  readonly state: TimelineEditState;
  readonly placement: TimelineResolvedPlacement;
  /** `trimItems` or `resizeItems`; null when nothing changes or the trim is rejected. */
  readonly action: ProjectAction | null;
  /** Why the edge stopped short or was rejected. */
  readonly reason: string | null;
  readonly guideSeconds: number | null;
  readonly sticky: StickyTimelineSnap | null;
}

export interface ClipRippleTrimPreview {
  readonly action: ProjectActionRippleTrim | null;
  /** Planned start and duration of every clip the ripple trim changes. */
  readonly placements: ReadonlyMap<string, { readonly startSeconds: number; readonly durationSeconds: number }>;
  readonly blocked: string | null;
}

function locate(project: VideoProject, itemId: string) {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, track };
  }
  return null;
}

/** Source media length for clips that play a video or audio file, for the right-edge clamp. */
function sourceDurationSeconds(project: VideoProject, item: TimelineItem): number | null {
  if (item.source.type !== "media") return null;
  const { mediaId } = item.source;
  const media = project.media.find((candidate) => candidate.id === mediaId);
  return media && (media.kind === "video" || media.kind === "audio") && media.durationSeconds > 0 ? media.durationSeconds : null;
}

/**
 * Live evaluation of dragging one trim handle: the edge snaps to edit points and the
 * playhead, the right edge keeps at least 0.1 s, and `evaluateTimelineResize` clamps the
 * edge to the source range and neighbouring clips.
 */
export function evaluateClipTrim(input: ClipTrimInput): ClipTrimPreview {
  const { project, itemId, edge } = input;
  const location = locate(project, itemId);
  const timeline = project.timeline;
  if (!location) {
    const placement = { itemId, trackId: "", startSeconds: 0, durationSeconds: 0 };
    return { state: "rejected", placement, action: null, reason: "That clip no longer exists.", guideSeconds: null, sticky: null };
  }
  const { item } = location;
  const endSeconds = item.startSeconds + item.durationSeconds;
  const edgeSeconds = (edge === "left" ? item.startSeconds : endSeconds) + input.deltaSeconds;
  const snap = snapClipProbes(timeline, [{ seconds: edgeSeconds, edge: edge === "left" ? "start" : "end" }], new Set([itemId]), input.snap);
  const snappedEdge = edgeSeconds + snap.deltaSeconds;
  const proposedStartSeconds = edge === "left" ? roundTimelineSeconds(snappedEdge) : item.startSeconds;
  const proposedDurationSeconds = edge === "left"
    ? roundTimelineSeconds(endSeconds - proposedStartSeconds)
    : roundTimelineSeconds(Math.max(item.startSeconds + minimumClipDurationSeconds, snappedEdge) - item.startSeconds);
  const mediaDuration = sourceDurationSeconds(project, item);
  const evaluation = evaluateTimelineResize({
    timeline,
    itemId,
    edge,
    proposedStartSeconds,
    proposedDurationSeconds,
    guideSeconds: snap.guideSeconds,
    ...(mediaDuration === null ? {} : { sourceDurationSeconds: mediaDuration }),
  });
  const fallbackReason =
    evaluation.state === "clamped" ? "Resize limited to the available range" : evaluation.state === "rejected" ? "Invalid resize" : null;
  return {
    state: evaluation.state,
    placement: evaluation.placement,
    action: evaluation.state !== "rejected" && evaluation.patch ? projectActionFromTimelinePatch(evaluation.patch) : null,
    reason: evaluation.reason?.message ?? fallbackReason,
    guideSeconds: evaluation.guideSeconds,
    sticky: snap.sticky,
  };
}

/**
 * Plans a ripple trim (Shift on a trim handle): the edge moves by the rounded delta, kept to
 * at least 0.1 s of clip, linked clips follow, and sync-locked tracks outside the link group
 * shift with the edit. The dry run from `planProjectRippleTrim` drives the live preview.
 */
export function evaluateClipRippleTrim(project: VideoProject, request: TimelineRippleTrimRequest): ClipRippleTrimPreview {
  const empty = new Map<string, { startSeconds: number; durationSeconds: number }>();
  const location = locate(project, request.itemId);
  if (!location) return { action: null, placements: empty, blocked: "That clip no longer exists." };
  const { item, track } = location;
  const shortening = item.durationSeconds - minimumClipDurationSeconds;
  const clamped = request.edge === "left" ? Math.min(request.deltaSeconds, shortening) : Math.max(request.deltaSeconds, -shortening);
  const deltaSeconds = roundTimelineSeconds(clamped);
  if (deltaSeconds === 0 || !Number.isFinite(deltaSeconds)) return { action: null, placements: empty, blocked: null };

  const linkedTrackIds = new Set(rippleTrackIdsForItem(project, item, track.id));
  const action: ProjectActionRippleTrim = {
    type: "rippleTrimItem",
    itemId: item.id,
    edge: request.edge,
    deltaSeconds,
    propagateLinked: true,
    syncLockedTrackIds: project.timeline.tracks
      .filter((candidate) => candidate.syncLocked === true && !linkedTrackIds.has(candidate.id))
      .map((candidate) => candidate.id),
  };
  try {
    const plan = planProjectRippleTrim(project, action);
    const starts = new Map(project.timeline.tracks.flatMap((entry) => entry.items.map((clip) => [clip.id, clip] as const)));
    const placements = new Map<string, { startSeconds: number; durationSeconds: number }>();
    for (const resize of plan.resizes) {
      const clip = starts.get(resize.itemId);
      if (clip) placements.set(resize.itemId, { startSeconds: clip.startSeconds, durationSeconds: roundTimelineSeconds(resize.durationSeconds) });
    }
    for (const shift of plan.shifts) {
      const clip = starts.get(shift.itemId);
      if (clip) placements.set(shift.itemId, { startSeconds: roundTimelineSeconds(shift.startSeconds), durationSeconds: clip.durationSeconds });
    }
    return { action, placements, blocked: null };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { action: null, placements: empty, blocked: `Ripple trim blocked: ${message}` };
  }
}
