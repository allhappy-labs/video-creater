import type { ProjectAction } from "@/lib/project";
import {
  canonicalTimelineItemKindForMediaKind,
  itemAllowedOnTrack,
  type Timeline,
  type TimelineItemKind,
  type TimelineTrack,
  type TrackKind,
} from "@/lib/timeline";
import { evaluateTimelineMove } from "@/lib/timeline-edit-evaluator";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import {
  orderedTracksByBand,
  trackBand,
  trackBandOrder,
  trackKindDisplayName,
  type TrackBand,
} from "@/lib/timeline-ops/track-bands";

export type AssetKind = "video" | "image" | "audio" | "text" | "caption" | "template" | "background" | "lottie";

type CreateTrackAction = Extract<ProjectAction, { type: "createTrack" }>;
type ReorderTrackAction = Extract<ProjectAction, { type: "reorderTrack" }>;
type RemoveTracksAction = Extract<ProjectAction, { type: "removeTracks" }>;

export interface DropTargetInput {
  readonly timeline: Timeline;
  readonly assetKind: AssetKind;
  /** Track under the pointer, or null when between tracks / outside. */
  readonly hoveredTrackId: string | null;
  /** Insert position when between tracks: index in band-ordered tracks. Null with no hovered track appends. */
  readonly insertIndex: number | null;
  readonly startSeconds: number;
  readonly durationSeconds: number;
  readonly newTrackId: string; // caller-supplied deterministic id
}

export type DropTargetPlan =
  | { readonly kind: "existing"; readonly trackId: string }
  | {
      readonly kind: "create";
      readonly createTrack: CreateTrackAction;
      /**
       * `createTrack` can only insert after a track. When the new track must sit before a
       * track that is stored first, this follow-up action moves it into place. Apply both,
       * in order, in the same batch.
       */
      readonly reorderTrack: ReorderTrackAction | null;
      readonly trackId: string;
    }
  | { readonly kind: "invalid"; readonly reason: string };

const trackKinds: readonly TrackKind[] = ["video", "hyperframe_scene", "overlay", "caption", "audio"];

function itemKindForAsset(assetKind: AssetKind): TimelineItemKind {
  switch (assetKind) {
    case "video":
    case "image":
    case "audio":
    case "lottie":
      return canonicalTimelineItemKindForMediaKind(assetKind);
    case "text":
    case "template":
      return "overlay";
    case "caption":
      return "caption";
    case "background":
      return "hyperframe_scene";
  }
}

function trackKindForItem(itemKind: TimelineItemKind): TrackKind {
  const kind = trackKinds.find((candidate) => itemAllowedOnTrack(itemKind, candidate));
  if (!kind) throw new Error(`No track kind accepts ${itemKind} items`);
  return kind;
}

/** Runs the move evaluator for a probe item placed on its own track and moved onto the hovered track. */
function evaluateDropOnTrack(input: DropTargetInput, itemKind: TimelineItemKind, trackKind: TrackKind, trackId: string) {
  const probeItemId = `drop-probe:${input.newTrackId}`;
  const probeTrack: TimelineTrack = {
    id: input.newTrackId,
    name: trackKindDisplayName(trackKind),
    kind: trackKind,
    locked: false,
    items: [
      {
        id: probeItemId,
        kind: itemKind,
        startSeconds: input.startSeconds,
        durationSeconds: input.durationSeconds,
        source: { type: "text", text: "" },
        label: "Drop",
        properties: {},
      },
    ],
  };
  return evaluateTimelineMove({
    timeline: { ...input.timeline, tracks: [...input.timeline.tracks, probeTrack] },
    itemId: probeItemId,
    targetTrackId: trackId,
    proposedStartSeconds: input.startSeconds,
    guideSeconds: null,
  });
}

function tracksInBand(timeline: Timeline, band: TrackBand) {
  return timeline.tracks.filter((track) => trackBand(track.kind) === band);
}

/** Builds the actions that insert `track` at position `bandIndex` among the tracks of its band. */
function createTrackPlan(timeline: Timeline, track: TimelineTrack, bandIndex: number): DropTargetPlan {
  const bandTracks = tracksInBand(timeline, trackBand(track.kind));
  const before = bandTracks[bandIndex];
  const createTrack: CreateTrackAction = { type: "createTrack", track };
  let reorderTrack: ReorderTrackAction | null = null;
  if (before) {
    const storedIndex = timeline.tracks.indexOf(before);
    const previous = timeline.tracks[storedIndex - 1];
    if (previous) {
      createTrack.afterTrackId = previous.id;
    } else {
      reorderTrack = { type: "reorderTrack", trackId: track.id, targetTrackId: before.id, placement: "before" };
    }
  } else {
    const last = bandTracks.at(-1);
    if (last) createTrack.afterTrackId = last.id;
  }
  return { kind: "create", createTrack, reorderTrack, trackId: track.id };
}

export function planDropTarget(input: DropTargetInput): DropTargetPlan {
  const { timeline } = input;
  if (timeline.tracks.some((track) => track.id === input.newTrackId)) {
    return { kind: "invalid", reason: `Track ${input.newTrackId} already exists` };
  }
  const itemKind = itemKindForAsset(input.assetKind);
  const trackKind = trackKindForItem(itemKind);
  const band = trackBand(trackKind);
  const newTrack: TimelineTrack = {
    id: input.newTrackId,
    name: trackKindDisplayName(trackKind),
    kind: trackKind,
    locked: false,
    enabled: true,
    items: [],
  };
  const bandTracks = tracksInBand(timeline, band);
  const hovered = timeline.tracks.find((track) => track.id === input.hoveredTrackId);

  if (hovered) {
    const evaluation = evaluateDropOnTrack(input, itemKind, trackKind, hovered.id);
    if (evaluation.reason?.code === "track_locked") return { kind: "invalid", reason: "Track is locked" };
    if (evaluation.state === "accepted") return { kind: "existing", trackId: hovered.id };

    // Incompatible or colliding: create next to the hovered track, inside the right band.
    const hoveredBand = trackBand(hovered.kind);
    if (hoveredBand === band) {
      return createTrackPlan(timeline, newTrack, bandTracks.indexOf(hovered));
    }
    const aboveHovered = trackBandOrder.indexOf(band) < trackBandOrder.indexOf(hoveredBand);
    return createTrackPlan(timeline, newTrack, aboveHovered ? bandTracks.length : 0);
  }

  // Between tracks: clamp the band-ordered insert position into the item's band.
  const bandStart = timeline.tracks.filter(
    (track) => trackBandOrder.indexOf(trackBand(track.kind)) < trackBandOrder.indexOf(band),
  ).length;
  const position = input.insertIndex ?? timeline.tracks.length;
  const bandIndex = Math.min(Math.max(position - bandStart, 0), bandTracks.length);
  return createTrackPlan(timeline, newTrack, bandIndex);
}

/** The asset kind whose items land on tracks of `kind`. */
const assetKindForTrack: Readonly<Record<TrackKind, AssetKind>> = {
  video: "video",
  hyperframe_scene: "background",
  overlay: "text",
  caption: "caption",
  audio: "audio",
};

/** An unused track id `track-<asset kind>-<n>`, counting up from the track count plus one. */
export function newTrackId(timeline: Timeline, assetKind: AssetKind): string {
  const taken = new Set(timeline.tracks.map((track) => track.id));
  const base = `track-${assetKind}`;
  let suffix = timeline.tracks.length + 1;
  while (taken.has(`${base}-${suffix.toString()}`)) suffix += 1;
  return `${base}-${suffix.toString()}`;
}

/**
 * An empty track of the same kind directly above or below `trackId` in display order, as
 * `createTrack` plus the `reorderTrack` it needs when it must sit before the first stored track.
 */
export function planAdjacentTrack(timeline: Timeline, trackId: string, placement: "above" | "below"): CommandResult {
  const ordered = orderedTracksByBand(timeline);
  const index = ordered.findIndex((track) => track.id === trackId);
  const track = ordered[index];
  if (!track) return { blocked: "That track no longer exists." };
  const assetKind = assetKindForTrack[track.kind];
  const plan = planDropTarget({
    timeline,
    assetKind,
    hoveredTrackId: null,
    insertIndex: placement === "above" ? index : index + 1,
    startSeconds: 0,
    durationSeconds: 0,
    newTrackId: newTrackId(timeline, assetKind),
  });
  if (plan.kind !== "create") return { blocked: plan.kind === "invalid" ? plan.reason : "Can't add a track here." };
  return { actions: plan.reorderTrack ? [plan.createTrack, plan.reorderTrack] : [plan.createTrack] };
}

function trackHasItems(track: TimelineTrack | undefined) {
  return (track?.items.length ?? 0) > 0;
}

/**
 * The `removeTracks` action for tracks this edit emptied: they had items in `before` and
 * none in `after`. Tracks that were already empty stay. Locked tracks stay, because
 * `removeTracks` rejects them, and at least one video track always remains.
 */
export function emptyTrackRemovals(before: Timeline, after: Timeline): RemoveTracksAction | null {
  const beforeById = new Map(before.tracks.map((track) => [track.id, track]));
  const ordered = orderedTracksByBand(after);
  const emptied = ordered.filter(
    (track) => !track.locked && !trackHasItems(track) && trackHasItems(beforeById.get(track.id)),
  );
  const videoTracks = ordered.filter((track) => track.kind === "video");
  const keptVideo = videoTracks.length > 0 && videoTracks.every((track) => emptied.includes(track))
    ? videoTracks[0]
    : null;
  const trackIds = emptied.filter((track) => track !== keptVideo).map((track) => track.id);
  return trackIds.length > 0 ? { type: "removeTracks", trackIds } : null;
}
