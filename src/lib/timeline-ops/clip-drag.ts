import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { itemAllowedOnTrack, type Timeline, type TimelineItem, type TimelineItemKind, type TimelineTrack } from "@/lib/timeline";
import {
  evaluateTimelineMove,
  type TimelineEditState,
  type TimelineResolvedPlacement,
} from "@/lib/timeline-edit-evaluator";
import {
  resolveTimelineSnap,
  timelineSnapTargets,
  type StickyTimelineSnap,
  type TimelineSnapProbe,
} from "@/lib/timeline-snap";
import { cloneItemsForTracks, withEmptyTrackRemovals, type CommandResult } from "./clip-commands";
import { newTrackId, planDropTarget, type AssetKind, type DropTargetPlan } from "./dynamic-tracks";
import { duplicateTimelineItemId } from "./ids";
import { stringProperty } from "./item-properties";
import { orderedTracksByBand } from "./track-bands";

/** Pointer travel (either axis) before a press on a clip becomes a move. */
export const moveThresholdPixels = 3;
/** Shortest clip the blade can split and the shortest a trim can leave. */
export const minimumClipDurationSeconds = 0.1;

const unavailableDestination = "Destination unavailable";
const duplicateProbePrefix = "duplicate-probe:";

/** Magnetic snapping to edit points and the playhead; pass `null` when snapping is off. */
export interface ClipSnapOptions {
  readonly playheadSeconds: number;
  readonly pixelsPerSecond: number;
  readonly sticky: StickyTimelineSnap | null;
}

/**
 * Where the dragged lead clip is over: the row under it and, when it sits on a row boundary
 * or outside every row, the band-ordered index a new track would be inserted at.
 */
export interface ClipTrackTarget {
  readonly trackId: string;
  readonly insertIndex: number | null;
}

export interface ClipMoveInput {
  readonly timeline: Timeline;
  readonly leadItemId: string;
  /** The move group from `clipMoveGroup`; it includes the lead. */
  readonly itemIds: readonly string[];
  /** Raw pointer travel in seconds since the press. */
  readonly deltaSeconds: number;
  readonly target: ClipTrackTarget;
  /** Alt-drag: copies land at the placements and the originals stay. */
  readonly duplicate: boolean;
  readonly snap: ClipSnapOptions | null;
}

type CreateTrackPlan = Extract<DropTargetPlan, { kind: "create" }>;

interface ClipMoveNewTrack {
  readonly createTrack: CreateTrackPlan["createTrack"];
  readonly reorderTrack: CreateTrackPlan["reorderTrack"];
  /** Band-ordered row index the new track will occupy. */
  readonly insertIndex: number;
}

export interface ClipMovePreview {
  readonly state: TimelineEditState;
  readonly placements: readonly TimelineResolvedPlacement[];
  /** User-facing reason for a rejected move. */
  readonly reason: string | null;
  /** Existing row that shows the target feedback; null when the target is a new track. */
  readonly targetTrackId: string | null;
  readonly newTrack: ClipMoveNewTrack | null;
  readonly guideSeconds: number | null;
  readonly sticky: StickyTimelineSnap | null;
}

interface ItemLocation {
  readonly item: TimelineItem;
  readonly track: TimelineTrack;
}

function itemLocations(timeline: Timeline): Map<string, ItemLocation> {
  return new Map(timeline.tracks.flatMap((track) => track.items.map((item) => [item.id, { item, track }] as const)));
}

function linkGroupId(item: TimelineItem): string | null {
  return stringProperty(item, "linkGroupId");
}

/**
 * The items a press on `leadItemId` moves: the whole selection when the lead is part of it,
 * otherwise the lead alone, plus every clip linked to a member. Null when any of them sits
 * on a locked track (the press only selects).
 */
export function clipMoveGroup(timeline: Timeline, selectedItemIds: readonly string[], leadItemId: string): string[] | null {
  const locations = itemLocations(timeline);
  const seeds = selectedItemIds.includes(leadItemId) ? selectedItemIds : [leadItemId];
  const groupIds = new Set(seeds.flatMap((itemId) => {
    const item = locations.get(itemId)?.item;
    const groupId = item ? linkGroupId(item) : null;
    return groupId ? [groupId] : [];
  }));
  const ids = [...new Set([
    ...seeds.filter((itemId) => locations.has(itemId)),
    ...[...locations.values()].filter(({ item }) => {
      const groupId = linkGroupId(item);
      return groupId !== null && groupIds.has(groupId);
    }).map(({ item }) => item.id),
  ])];
  if (ids.length === 0) return null;
  return ids.some((itemId) => locations.get(itemId)?.track.locked) ? null : ids;
}

function editPointsExcluding(timeline: Timeline, excludedItemIds: ReadonlySet<string>): number[] {
  const points = [0, timeline.durationSeconds];
  for (const track of timeline.tracks) {
    for (const item of track.items) {
      if (!excludedItemIds.has(item.id)) points.push(item.startSeconds, item.startSeconds + item.durationSeconds);
    }
  }
  return points;
}

/** Snaps the probes to edit points (excluding `excludedItemIds`) and the playhead. */
export function snapClipProbes(
  timeline: Timeline,
  probes: readonly TimelineSnapProbe[],
  excludedItemIds: ReadonlySet<string>,
  snap: ClipSnapOptions | null,
) {
  if (!snap) return { deltaSeconds: 0, guideSeconds: null, sticky: null };
  const result = resolveTimelineSnap({
    probes,
    targets: timelineSnapTargets(editPointsExcluding(timeline, excludedItemIds), snap.playheadSeconds),
    pixelsPerSecond: snap.pixelsPerSecond,
    sticky: snap.sticky,
  });
  return result.guideSeconds === null
    ? { deltaSeconds: 0, guideSeconds: null, sticky: null }
    : { deltaSeconds: result.deltaSeconds, guideSeconds: result.guideSeconds, sticky: result.sticky };
}

function assetKindForItem(kind: TimelineItemKind): AssetKind {
  switch (kind) {
    case "audio_clip":
      return "audio";
    case "image_clip":
      return "image";
    case "lottie_clip":
      return "lottie";
    case "overlay":
      return "text";
    case "caption":
      return "caption";
    case "hyperframe_scene":
      return "background";
    case "video_clip":
    case "generated_clip":
      return "video";
  }
}

/** The timeline with the planned track inserted where `createTrack` (and `reorderTrack`) put it. */
function timelineWithTrack(timeline: Timeline, plan: CreateTrackPlan): Timeline {
  const tracks = [...timeline.tracks];
  const { createTrack, reorderTrack } = plan;
  const afterIndex = createTrack.afterTrackId === undefined ? -1 : tracks.findIndex((track) => track.id === createTrack.afterTrackId);
  const index = reorderTrack
    ? tracks.findIndex((track) => track.id === reorderTrack.targetTrackId)
    : afterIndex >= 0 ? afterIndex + 1 : tracks.length;
  tracks.splice(Math.max(0, index), 0, createTrack.track);
  return { ...timeline, tracks };
}

type TrackResolution =
  | {
      readonly kind: "resolved";
      readonly timeline: Timeline;
      readonly targetTrackIds: Readonly<Record<string, string>>;
      readonly feedbackTrackId: string | null;
      readonly newTrack: ClipMoveNewTrack | null;
    }
  | { readonly kind: "rejected"; readonly reason: string; readonly feedbackTrackId: string | null };

interface ResolveTracksInput {
  readonly timeline: Timeline;
  readonly lead: ItemLocation;
  readonly group: readonly ItemLocation[];
  readonly target: ClipTrackTarget;
  readonly startSeconds: number;
}

function resolveTargetTracks({ timeline, lead, group, target, startSeconds }: ResolveTracksInput): TrackResolution {
  const ordered = orderedTracksByBand(timeline);
  const rowOf = (trackId: string) => ordered.findIndex((track) => track.id === trackId);
  const leadGroupId = linkGroupId(lead.item);
  const pinned = ({ item }: ItemLocation) =>
    item.id !== lead.item.id &&
    ((leadGroupId !== null && linkGroupId(item) === leadGroupId) || !itemAllowedOnTrack(item.kind, lead.track.kind));
  const movable = group.filter((location) => !pinned(location));
  const stay = Object.fromEntries(group.map(({ item, track }) => [item.id, track.id]));
  const resolved = (targetTrackId: string | null, feedbackTrackId: string | null, over = timeline, newTrack: ClipMoveNewTrack | null = null): TrackResolution => ({
    kind: "resolved",
    timeline: over,
    targetTrackIds: targetTrackId === null
      ? stay
      : { ...stay, ...Object.fromEntries(movable.map(({ item }) => [item.id, targetTrackId])) },
    feedbackTrackId,
    newTrack,
  });

  const singleTrack = movable.every(({ track }) => track.id === lead.track.id);
  if (!singleTrack) {
    const rowDelta = rowOf(target.trackId) - rowOf(lead.track.id);
    const targetTrackIds: Record<string, string> = { ...stay };
    for (const { item, track } of movable) {
      const destination = ordered[rowOf(track.id) + rowDelta];
      if (!destination) return { kind: "rejected", reason: unavailableDestination, feedbackTrackId: target.trackId };
      targetTrackIds[item.id] = destination.id;
    }
    return { kind: "resolved", timeline, targetTrackIds, feedbackTrackId: target.trackId, newTrack: null };
  }

  const leadRow = rowOf(lead.track.id);
  const groupIds = new Set(group.map(({ item }) => item.id));
  const aloneOnTrack = lead.track.items.every((item) => groupIds.has(item.id));
  if (target.insertIndex !== null && aloneOnTrack && (target.insertIndex === leadRow || target.insertIndex === leadRow + 1)) {
    return resolved(null, lead.track.id);
  }
  const hovered = target.insertIndex === null ? ordered.find((track) => track.id === target.trackId) : undefined;
  if (target.insertIndex === null) {
    if (!hovered) return { kind: "rejected", reason: unavailableDestination, feedbackTrackId: null };
    if (hovered.id === lead.track.id) return resolved(null, lead.track.id);
    if (itemAllowedOnTrack(lead.item.kind, hovered.kind)) return resolved(hovered.id, hovered.id);
  }

  const plan = planDropTarget({
    timeline,
    assetKind: assetKindForItem(lead.item.kind),
    hoveredTrackId: hovered?.id ?? null,
    insertIndex: target.insertIndex,
    startSeconds,
    durationSeconds: lead.item.durationSeconds,
    newTrackId: newTrackId(timeline, assetKindForItem(lead.item.kind)),
  });
  if (plan.kind === "invalid") return { kind: "rejected", reason: plan.reason, feedbackTrackId: hovered?.id ?? null };
  if (plan.kind === "existing") return resolved(plan.trackId, plan.trackId);
  const withTrack = timelineWithTrack(timeline, plan);
  const insertIndex = orderedTracksByBand(withTrack).findIndex((track) => track.id === plan.trackId);
  return resolved(plan.trackId, null, withTrack, {
    createTrack: plan.createTrack,
    reorderTrack: plan.reorderTrack,
    insertIndex,
  });
}

/** Unlocked copies of the group on probe tracks, so a duplicate collides with the originals. */
function withDuplicateProbes(timeline: Timeline, group: readonly ItemLocation[]): Timeline {
  const groupIds = new Set(group.map(({ item }) => item.id));
  const probeTracks = timeline.tracks.flatMap((track) => {
    const items = track.items.filter((item) => groupIds.has(item.id));
    return items.length === 0
      ? []
      : [{
          ...track,
          id: `${duplicateProbePrefix}${track.id}`,
          locked: false,
          items: items.map((item) => ({ ...item, id: `${duplicateProbePrefix}${item.id}` })),
        }];
  });
  return { ...timeline, tracks: [...timeline.tracks, ...probeTracks] };
}

interface RejectedPreviewInput {
  readonly reason: string;
  readonly targetTrackId: string | null;
  /** Where the clips would have landed, so the target can show the rejection. */
  readonly placements: readonly TimelineResolvedPlacement[];
  readonly guideSeconds: number | null;
  readonly sticky: StickyTimelineSnap | null;
}

function rejectedPreview({ reason, targetTrackId, placements, guideSeconds, sticky }: RejectedPreviewInput): ClipMovePreview {
  return { state: "rejected", placements, reason, targetTrackId, newTrack: null, guideSeconds, sticky };
}

/**
 * Live evaluation of a clip drag: time delta clamped so no clip starts before zero, magnetic
 * snap of every group edge, target tracks (existing, a new track via `planDropTarget`, or a
 * row shift for multi-track groups) and the evaluator's collision and lock checks.
 */
export function evaluateClipMove(input: ClipMoveInput): ClipMovePreview {
  const locations = itemLocations(input.timeline);
  const lead = locations.get(input.leadItemId);
  if (!lead) {
    return rejectedPreview({ reason: "That clip no longer exists.", targetTrackId: null, placements: [], guideSeconds: null, sticky: null });
  }
  const group = [...new Set([input.leadItemId, ...input.itemIds])].flatMap((itemId) => {
    const location = locations.get(itemId);
    return location ? [location] : [];
  });
  const earliestStart = Math.min(...group.map(({ item }) => item.startSeconds));
  const rawDelta = Math.max(-earliestStart, input.deltaSeconds);
  const snap = snapClipProbes(
    input.timeline,
    group.flatMap(({ item }) => [
      { seconds: item.startSeconds + rawDelta, edge: "start" as const },
      { seconds: item.startSeconds + item.durationSeconds + rawDelta, edge: "end" as const },
    ]),
    new Set(group.map(({ item }) => item.id)),
    input.snap,
  );
  const deltaSeconds = Math.max(-earliestStart, rawDelta + snap.deltaSeconds);
  const proposedStartSeconds = roundTimelineSeconds(lead.item.startSeconds + deltaSeconds);

  const tracks = resolveTargetTracks({ timeline: input.timeline, lead, group, target: input.target, startSeconds: proposedStartSeconds });
  const attempted = (targetTrackIds: Readonly<Record<string, string>>) =>
    group.map(({ item, track }) => ({
      itemId: item.id,
      trackId: targetTrackIds[item.id] ?? track.id,
      startSeconds: roundTimelineSeconds(item.startSeconds + deltaSeconds),
      durationSeconds: item.durationSeconds,
    }));
  const snapResult = { guideSeconds: snap.guideSeconds, sticky: snap.sticky };
  if (tracks.kind === "rejected") {
    const feedback = tracks.feedbackTrackId;
    const placements = attempted(feedback === null ? {} : { [lead.item.id]: feedback });
    return rejectedPreview({ reason: tracks.reason, targetTrackId: feedback, placements, ...snapResult });
  }

  const evaluationId = (itemId: string) => (input.duplicate ? `${duplicateProbePrefix}${itemId}` : itemId);
  const evaluation = evaluateTimelineMove({
    timeline: input.duplicate ? withDuplicateProbes(tracks.timeline, group) : tracks.timeline,
    itemId: evaluationId(lead.item.id),
    itemIds: group.map(({ item }) => evaluationId(item.id)),
    targetTrackId: tracks.targetTrackIds[lead.item.id] ?? lead.track.id,
    targetTrackIds: Object.fromEntries(Object.entries(tracks.targetTrackIds).map(([itemId, trackId]) => [evaluationId(itemId), trackId])),
    proposedStartSeconds,
    guideSeconds: snap.guideSeconds,
  });
  const existingTrackId = (trackId: string | undefined) =>
    trackId !== undefined && input.timeline.tracks.some((track) => track.id === trackId) ? trackId : null;
  if (evaluation.state === "rejected") {
    return {
      ...rejectedPreview({
        reason: evaluation.reason?.message.split(duplicateProbePrefix).join("") ?? unavailableDestination,
        targetTrackId: existingTrackId(evaluation.reason?.trackId) ?? tracks.feedbackTrackId,
        placements: attempted(tracks.targetTrackIds),
        ...snapResult,
      }),
      newTrack: tracks.newTrack,
    };
  }
  return {
    state: evaluation.state,
    placements: evaluation.placements.map((placement) => ({ ...placement, itemId: placement.itemId.replace(duplicateProbePrefix, "") })),
    reason: null,
    targetTrackId: tracks.newTrack ? null : tracks.feedbackTrackId,
    newTrack: tracks.newTrack,
    guideSeconds: snap.guideSeconds,
    sticky: snap.sticky,
  };
}

/**
 * One batch for a finished drag: the created track (and its reorder), then `moveItems` or,
 * for an Alt-drag duplicate, `addItems` with copies, then removal of emptied tracks.
 */
export function planClipMoveCommit(project: VideoProject, preview: ClipMovePreview, duplicate: boolean): CommandResult {
  if (preview.state === "rejected") return { blocked: preview.reason ?? unavailableDestination };
  const locations = itemLocations(project.timeline);
  const placements = preview.placements.filter((placement) => locations.has(placement.itemId));
  const changed = placements.filter((placement) => {
    const location = locations.get(placement.itemId);
    return location?.track.id !== placement.trackId || location.item.startSeconds !== placement.startSeconds;
  });
  if (changed.length === 0) return { actions: [] };

  const actions: ProjectAction[] = [];
  if (preview.newTrack) {
    actions.push(preview.newTrack.createTrack);
    if (preview.newTrack.reorderTrack) actions.push(preview.newTrack.reorderTrack);
  }
  if (duplicate) {
    const itemsByTrack = cloneItemsForTracks(
      project,
      placements.flatMap((placement) => {
        const source = locations.get(placement.itemId)?.item;
        return source ? [{ source, targetTrackId: placement.trackId, startSeconds: placement.startSeconds }] : [];
      }),
      { newId: (itemId) => duplicateTimelineItemId(project, itemId), labelSuffix: "copy", linkPrefix: "link-copy" },
    );
    for (const [targetTrackId, items] of itemsByTrack) actions.push({ type: "addItems", targetTrackId, items });
    return { actions };
  }
  actions.push({
    type: "moveItems",
    moves: changed.map((placement) => ({ itemId: placement.itemId, targetTrackId: placement.trackId, startSeconds: placement.startSeconds })),
  });
  return withEmptyTrackRemovals(project, actions);
}

/**
 * Where a blade click on `itemId` splits: the pointer time, snapped when snapping is on and
 * rounded to milliseconds. Null on or outside the clip edges and for clips of 0.1 s or less.
 */
export function bladeSplitSeconds(timeline: Timeline, itemId: string, pointerSeconds: number, snap: ClipSnapOptions | null): number | null {
  const item = itemLocations(timeline).get(itemId)?.item;
  if (!item || item.durationSeconds <= minimumClipDurationSeconds || !Number.isFinite(pointerSeconds)) return null;
  const snapped = snapClipProbes(timeline, [{ seconds: pointerSeconds, edge: "start" }], new Set([itemId]), snap);
  const seconds = roundTimelineSeconds(pointerSeconds + snapped.deltaSeconds);
  return seconds > item.startSeconds && seconds < item.startSeconds + item.durationSeconds ? seconds : null;
}
