import {
  itemAllowedOnTrack,
  type Timeline,
  type TimelineItem,
  type TimelineItemKind,
  type TimelinePatch,
} from "./timeline";
import { headHandleSeconds, isReversedItem } from "./timeline-ops/reverse";
import {
  resolveTimelineSnap,
  type StickyTimelineSnap,
  type TimelineSnapProbe,
  type TimelineSnapTarget,
} from "./timeline-snap";

const collisionEpsilonSeconds = 0.001;

export type TimelineEditState = "accepted" | "clamped" | "rejected";
type TimelineEditReasonCode = "track_locked" | "track_incompatible" | "collision";

interface TimelineEditReason {
  code: TimelineEditReasonCode;
  message: string;
  trackId: string;
  blockingItemId: string | null;
}

export interface TimelineResolvedPlacement {
  itemId: string;
  trackId: string;
  startSeconds: number;
  durationSeconds: number;
}

export interface TimelineMoveEvaluation {
  state: TimelineEditState;
  placements: readonly TimelineResolvedPlacement[];
  patches: readonly { itemId: string; targetTrackId: string; startSeconds: number }[];
  reason: TimelineEditReason | null;
  guideSeconds: number | null;
}

export interface TimelineResizeEvaluation {
  state: TimelineEditState;
  placement: TimelineResolvedPlacement;
  patch: TimelinePatch | null;
  reason: TimelineEditReason | null;
  guideSeconds: number | null;
}

interface TimelineEditSnapInput {
  probes: readonly TimelineSnapProbe[];
  targets: readonly TimelineSnapTarget[];
  pixelsPerSecond: number;
  sticky?: StickyTimelineSnap | null;
}

export interface EvaluateTimelineMoveInput {
  timeline: Timeline;
  itemId: string;
  itemIds?: readonly string[];
  targetTrackId: string;
  targetTrackIds?: Readonly<Record<string, string>>;
  proposedStartSeconds: number;
  guideSeconds: number | null;
  snap?: TimelineEditSnapInput;
}

export interface EvaluateTimelineResizeInput {
  timeline: Timeline;
  itemId: string;
  edge: "left" | "right";
  proposedStartSeconds: number;
  proposedDurationSeconds: number;
  guideSeconds: number | null;
  snap?: TimelineEditSnapInput;
  /** Full underlying source duration in source-time seconds, when a future known source supplies it. */
  sourceDurationSeconds?: number;
}

interface TimelineItemLocation {
  item: TimelineItem;
  trackId: string;
}

function resolvedSnap(
  guideSeconds: number | null,
  snap: TimelineEditSnapInput | undefined,
) {
  if (!snap) return { deltaSeconds: 0, guideSeconds };
  const result = resolveTimelineSnap(snap);
  return {
    deltaSeconds: result.guideSeconds === null ? 0 : result.deltaSeconds,
    guideSeconds: result.guideSeconds ?? guideSeconds,
  };
}

function roundSeconds(seconds: number) {
  return Number(seconds.toFixed(3));
}

function numericItemProperty(item: TimelineItem, key: string) {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function itemPlaybackSpeed(item: TimelineItem) {
  const speed = numericItemProperty(item, "speed") ?? 1;
  return speed > 0 ? speed : 1;
}

function sourceRange(item: TimelineItem) {
  const sourceIn = numericItemProperty(item, "sourceIn");
  const sourceOut = numericItemProperty(item, "sourceOut");
  return sourceIn !== null && sourceOut !== null ? { sourceIn, sourceOut } : null;
}

function itemLocations(timeline: Timeline) {
  return timeline.tracks.flatMap((track) =>
    track.items.map((item) => ({ item, trackId: track.id })),
  );
}

function findItem(timeline: Timeline, itemId: string) {
  return itemLocations(timeline).find((location) => location.item.id === itemId) ?? null;
}

function itemKindTrackMessage(kind: TimelineItemKind) {
  switch (kind) {
    case "audio_clip": return "Audio clips stay on Audio tracks";
    case "caption": return "Captions stay on Caption tracks";
    case "hyperframe_scene": return "HyperFrame scenes stay on HyperFrame tracks";
    case "overlay": return "Overlays stay on Overlay tracks";
    default: return "Video clips stay on Video tracks";
  }
}

function trackReason(
  item: TimelineItem,
  targetTrackId: string,
  timeline: Timeline,
): TimelineEditReason | null {
  const track = timeline.tracks.find((candidate) => candidate.id === targetTrackId);
  if (track?.locked) {
    return { code: "track_locked", message: "Track locked", trackId: targetTrackId, blockingItemId: null };
  }
  if (!track || !itemAllowedOnTrack(item.kind, track.kind)) {
    return {
      code: "track_incompatible",
      message: itemKindTrackMessage(item.kind),
      trackId: targetTrackId,
      blockingItemId: null,
    };
  }
  return null;
}

function intervalOverlapSeconds(
  startA: number,
  durationA: number,
  startB: number,
  durationB: number,
) {
  const overlap = Math.min(startA + durationA, startB + durationB) - Math.max(startA, startB);
  return overlap > collisionEpsilonSeconds ? overlap : 0;
}

function canonicalOverlapSeconds(
  first: TimelineItemLocation,
  second: TimelineItemLocation,
) {
  if (first.trackId !== second.trackId) return 0;
  return intervalOverlapSeconds(
    first.item.startSeconds,
    first.item.durationSeconds,
    second.item.startSeconds,
    second.item.durationSeconds,
  );
}

function collisionReason(
  timeline: Timeline,
  placements: readonly TimelineResolvedPlacement[],
  movingItemIds: ReadonlySet<string>,
) {
  const locations = itemLocations(timeline);
  const locationByItemId = new Map(locations.map((location) => [location.item.id, location]));
  const stationary = locations.filter((location) => !movingItemIds.has(location.item.id));
  const pairs = placements.flatMap((placement, index) => [
    ...stationary.map((location) => ({
      placement,
      otherPlacement: {
        itemId: location.item.id,
        trackId: location.trackId,
        startSeconds: location.item.startSeconds,
        durationSeconds: location.item.durationSeconds,
      } satisfies TimelineResolvedPlacement,
    })),
    ...placements.slice(index + 1).map((otherPlacement) => ({ placement, otherPlacement })),
  ]);

  for (const { placement, otherPlacement } of pairs) {
    if (placement.trackId !== otherPlacement.trackId) continue;
    const proposedOverlap = intervalOverlapSeconds(
      placement.startSeconds,
      placement.durationSeconds,
      otherPlacement.startSeconds,
      otherPlacement.durationSeconds,
    );
    if (proposedOverlap === 0) continue;
    const first = locationByItemId.get(placement.itemId);
    const second = locationByItemId.get(otherPlacement.itemId);
    const canonicalOverlap = first && second ? canonicalOverlapSeconds(first, second) : 0;
    if (proposedOverlap <= canonicalOverlap + Number.EPSILON) continue;
    const blocker = locationByItemId.get(otherPlacement.itemId);
    return {
      code: "collision" as const,
      message: `Overlaps ${blocker?.item.label ?? otherPlacement.itemId}`,
      trackId: placement.trackId,
      blockingItemId: otherPlacement.itemId,
    };
  }
  return null;
}

function rejectedMove(reason: TimelineEditReason, guideSeconds: number | null): TimelineMoveEvaluation {
  return { state: "rejected", placements: [], patches: [], reason, guideSeconds };
}

function rejectedResize(
  placement: TimelineResolvedPlacement,
  reason: TimelineEditReason,
  guideSeconds: number | null,
): TimelineResizeEvaluation {
  return { state: "rejected", placement, patch: null, reason, guideSeconds };
}

export function evaluateTimelineMove(input: EvaluateTimelineMoveInput): TimelineMoveEvaluation {
  const snap = resolvedSnap(input.guideSeconds, input.snap);
  const guideSeconds = snap.guideSeconds;
  const lead = findItem(input.timeline, input.itemId);
  if (!lead) {
    return rejectedMove({
      code: "track_incompatible",
      message: "Item is unavailable",
      trackId: input.targetTrackId,
      blockingItemId: null,
    }, guideSeconds);
  }
  const groupItemIds = [...new Set(input.itemIds?.length ? input.itemIds : [input.itemId])];
  if (!groupItemIds.includes(input.itemId)) groupItemIds.unshift(input.itemId);
  const locations = groupItemIds.map((itemId) => findItem(input.timeline, itemId));
  if (locations.some((location) => location === null)) {
    return rejectedMove({
      code: "track_incompatible",
      message: "Item is unavailable",
      trackId: input.targetTrackId,
      blockingItemId: null,
    }, guideSeconds);
  }
  const movingLocations = locations as TimelineItemLocation[];
  const deltaSeconds = input.proposedStartSeconds + snap.deltaSeconds - lead.item.startSeconds;
  const placements = movingLocations.map((location) => {
    const trackId = input.targetTrackIds?.[location.item.id] ??
      (location.item.id === input.itemId ? input.targetTrackId : location.trackId);
    return {
      itemId: location.item.id,
      trackId,
      startSeconds: roundSeconds(location.item.startSeconds + deltaSeconds),
      durationSeconds: location.item.durationSeconds,
    };
  });
  for (const location of movingLocations) {
    const targetTrackId = placements.find((placement) => placement.itemId === location.item.id)?.trackId ?? location.trackId;
    const reason = trackReason(location.item, targetTrackId, input.timeline);
    if (reason) return rejectedMove(reason, guideSeconds);
  }
  const reason = collisionReason(input.timeline, placements, new Set(groupItemIds));
  if (reason) return rejectedMove(reason, guideSeconds);
  return {
    state: "accepted",
    placements,
    patches: placements.map((placement) => ({
      itemId: placement.itemId,
      targetTrackId: placement.trackId,
      startSeconds: placement.startSeconds,
    })),
    reason: null,
    guideSeconds,
  };
}

function resizeBounds(input: EvaluateTimelineResizeInput, item: TimelineItem) {
  const minimumDurationSeconds = 0.1;
  const endSeconds = item.startSeconds + item.durationSeconds;
  const range = sourceRange(item);
  const speed = itemPlaybackSpeed(item);
  const sourceDurationSeconds = Number.isFinite(input.sourceDurationSeconds)
    ? Math.max(0, input.sourceDurationSeconds ?? 0)
    : null;
  if (range !== null && isReversedItem(item)) {
    // Reversed clips extend right towards source 0 and left towards the media end.
    const window = { ...range, speed, reverse: true };
    return {
      minimumDurationSeconds,
      maximumDurationSeconds: Math.max(minimumDurationSeconds, range.sourceOut / speed),
      minimumStartSeconds: sourceDurationSeconds === null
        ? 0
        : Math.max(0, item.startSeconds - headHandleSeconds(window, sourceDurationSeconds)),
      maximumStartSeconds: endSeconds - minimumDurationSeconds,
    };
  }
  return {
    minimumDurationSeconds,
    maximumDurationSeconds: sourceDurationSeconds !== null && range !== null
      ? Math.max(minimumDurationSeconds, (sourceDurationSeconds - range.sourceIn) / speed)
      : Number.POSITIVE_INFINITY,
    minimumStartSeconds: range === null
      ? 0
      : Math.max(0, item.startSeconds - range.sourceIn / speed),
    maximumStartSeconds: endSeconds - minimumDurationSeconds,
  };
}

function clampResizePlacement(
  input: EvaluateTimelineResizeInput,
  canonicalPlacement: TimelineResolvedPlacement,
  item: TimelineItem,
  snapDeltaSeconds: number,
) {
  const bounds = resizeBounds(input, item);
  if (input.edge === "right") {
    const durationSeconds = Math.max(
      bounds.minimumDurationSeconds,
      Math.min(input.proposedDurationSeconds + snapDeltaSeconds, bounds.maximumDurationSeconds),
    );
    return {
      placement: {
        ...canonicalPlacement,
        durationSeconds: roundSeconds(durationSeconds),
      },
      bounds,
    };
  }
  const endSeconds = canonicalPlacement.startSeconds + canonicalPlacement.durationSeconds;
  const startSeconds = Math.max(
    bounds.minimumStartSeconds,
    Math.min(input.proposedStartSeconds + snapDeltaSeconds, bounds.maximumStartSeconds),
  );
  return {
    placement: {
      ...canonicalPlacement,
      startSeconds: roundSeconds(startSeconds),
      durationSeconds: roundSeconds(endSeconds - startSeconds),
    },
    bounds,
  };
}

function resizePatch(
  item: TimelineItem,
  edge: EvaluateTimelineResizeInput["edge"],
  placement: TimelineResolvedPlacement,
): TimelinePatch {
  const range = sourceRange(item);
  if (edge === "right" && range === null) {
    return { type: "resizeItem", itemId: item.id, durationSeconds: placement.durationSeconds };
  }
  const speed = itemPlaybackSpeed(item);
  if (range === null) {
    return {
      type: "trimItem",
      itemId: item.id,
      startSeconds: placement.startSeconds,
      durationSeconds: placement.durationSeconds,
    };
  }
  if (isReversedItem(item)) {
    const [sourceIn, sourceOut] = edge === "left"
      ? [range.sourceIn, range.sourceOut + (item.startSeconds - placement.startSeconds) * speed]
      : [range.sourceOut - placement.durationSeconds * speed, range.sourceOut];
    return {
      type: "trimItem",
      itemId: item.id,
      startSeconds: placement.startSeconds,
      durationSeconds: placement.durationSeconds,
      sourceIn: roundSeconds(sourceIn),
      sourceOut: roundSeconds(sourceOut),
    };
  }
  return {
    type: "trimItem",
    itemId: item.id,
    startSeconds: placement.startSeconds,
    durationSeconds: placement.durationSeconds,
    sourceIn: edge === "left"
      ? roundSeconds(range.sourceIn + (placement.startSeconds - item.startSeconds) * speed)
      : roundSeconds(range.sourceIn),
    sourceOut: edge === "left"
      ? roundSeconds(range.sourceOut)
      : roundSeconds(range.sourceIn + placement.durationSeconds * speed),
  };
}

function placementMatchesCanonical(
  placement: TimelineResolvedPlacement,
  canonicalPlacement: TimelineResolvedPlacement,
) {
  return placement.startSeconds === canonicalPlacement.startSeconds &&
    placement.durationSeconds === canonicalPlacement.durationSeconds;
}

function collisionClampCandidates(
  input: EvaluateTimelineResizeInput,
  timeline: Timeline,
  canonicalPlacement: TimelineResolvedPlacement,
  desiredPlacement: TimelineResolvedPlacement,
  bounds: ReturnType<typeof resizeBounds>,
) {
  const stationary = timeline.tracks
    .find((track) => track.id === canonicalPlacement.trackId)
    ?.items.filter((item) => item.id !== canonicalPlacement.itemId) ?? [];
  if (input.edge === "right") {
    const durations = [
      desiredPlacement.durationSeconds,
      canonicalPlacement.durationSeconds,
      ...stationary.map((item) => item.startSeconds - canonicalPlacement.startSeconds),
    ].filter((duration) =>
      duration >= bounds.minimumDurationSeconds && duration <= bounds.maximumDurationSeconds,
    );
    return [...new Set(durations.map(roundSeconds))]
      .sort((first, second) => second - first)
      .map((durationSeconds) => ({ ...desiredPlacement, durationSeconds }));
  }
  const starts = [
    desiredPlacement.startSeconds,
    canonicalPlacement.startSeconds,
    ...stationary.map((item) => item.startSeconds + item.durationSeconds),
  ].filter((start) => start >= bounds.minimumStartSeconds && start <= bounds.maximumStartSeconds);
  const endSeconds = canonicalPlacement.startSeconds + canonicalPlacement.durationSeconds;
  return [...new Set(starts.map(roundSeconds))]
    .sort((first, second) => first - second)
    .map((startSeconds) => ({
      ...desiredPlacement,
      startSeconds,
      durationSeconds: roundSeconds(endSeconds - startSeconds),
    }));
}

export function evaluateTimelineResize(input: EvaluateTimelineResizeInput): TimelineResizeEvaluation {
  const snap = resolvedSnap(input.guideSeconds, input.snap);
  const guideSeconds = snap.guideSeconds;
  const location = findItem(input.timeline, input.itemId);
  if (!location) {
    const placement = { itemId: input.itemId, trackId: "", startSeconds: input.proposedStartSeconds, durationSeconds: input.proposedDurationSeconds };
    return rejectedResize(placement, {
      code: "track_incompatible",
      message: "Item is unavailable",
      trackId: "",
      blockingItemId: null,
    }, guideSeconds);
  }
  const canonicalPlacement: TimelineResolvedPlacement = {
    itemId: location.item.id,
    trackId: location.trackId,
    startSeconds: location.item.startSeconds,
    durationSeconds: location.item.durationSeconds,
  };
  const trackError = trackReason(location.item, location.trackId, input.timeline);
  if (trackError) return rejectedResize(canonicalPlacement, trackError, guideSeconds);
  const { placement: desiredPlacement, bounds } = clampResizePlacement(
    input,
    canonicalPlacement,
    location.item,
    snap.deltaSeconds,
  );
  const desiredReason = collisionReason(
    input.timeline,
    [desiredPlacement],
    new Set([input.itemId]),
  );
  if (!desiredReason) {
    const requestedStartSeconds = input.edge === "right"
      ? canonicalPlacement.startSeconds
      : input.proposedStartSeconds + snap.deltaSeconds;
    const requestedDurationSeconds = input.edge === "right"
      ? input.proposedDurationSeconds + snap.deltaSeconds
      : input.proposedDurationSeconds - snap.deltaSeconds;
    return {
      state: desiredPlacement.startSeconds === requestedStartSeconds &&
          desiredPlacement.durationSeconds === requestedDurationSeconds
        ? "accepted" : "clamped",
      placement: desiredPlacement,
      patch: placementMatchesCanonical(desiredPlacement, canonicalPlacement)
        ? null
        : resizePatch(location.item, input.edge, desiredPlacement),
      reason: null,
      guideSeconds,
    };
  }
  const clampedPlacement = collisionClampCandidates(
    input,
    input.timeline,
    canonicalPlacement,
    desiredPlacement,
    bounds,
  ).find((candidate) =>
    collisionReason(input.timeline, [candidate], new Set([input.itemId])) === null,
  );
  if (!clampedPlacement) return rejectedResize(canonicalPlacement, desiredReason, guideSeconds);
  const revalidationReason = collisionReason(
    input.timeline,
    [clampedPlacement],
    new Set([input.itemId]),
  );
  if (revalidationReason) return rejectedResize(canonicalPlacement, revalidationReason, guideSeconds);
  return {
    state: "clamped",
    placement: clampedPlacement,
    patch: placementMatchesCanonical(clampedPlacement, canonicalPlacement)
      ? null
      : resizePatch(location.item, input.edge, clampedPlacement),
    reason: desiredReason,
    guideSeconds,
  };
}
