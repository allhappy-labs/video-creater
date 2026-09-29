import { roundTimelineSeconds } from "@/lib/format";
import {
  applyProjectActionLocally,
  projectActionFromTimelinePatch,
  type ProjectAction,
  type ProjectActionRippleDeleteRange,
  type VideoProject,
} from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack } from "@/lib/timeline";
import { evaluateTimelineMove, evaluateTimelineResize } from "@/lib/timeline-edit-evaluator";
import { emptyTrackRemovals } from "@/lib/timeline-ops/dynamic-tracks";
import { duplicateTimelineItemId } from "@/lib/timeline-ops/ids";
import { isVisualOpacityItem, numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";
import { timelineGapAtSeconds } from "@/lib/timeline-ops/navigation";
import { isReversedItem, isReversibleItem } from "@/lib/timeline-ops/reverse";
import { rippleTrackIdsForItem } from "@/lib/timeline-ops/silence";

/** Actions to apply as one batch (one undo step), or user-facing copy explaining why not. */
export type CommandResult = { readonly actions: ProjectAction[] } | { readonly blocked: string };

interface ItemLocation {
  readonly item: TimelineItem;
  readonly track: TimelineTrack;
}

const minimumSpeed = 0.1;
const maximumSpeed = 8;
const fallbackFps = 30;
const timeEpsilonSeconds = 0.000_5;

/** Selected items that still exist, deduplicated, in stored track order then item order. */
function locateItems(timeline: Timeline, itemIds: readonly string[]): ItemLocation[] {
  const wanted = new Set(itemIds);
  return timeline.tracks.flatMap((track) =>
    track.items.filter((item) => wanted.has(item.id)).map((item) => ({ item, track })),
  );
}

function locateItem(timeline: Timeline, itemId: string): ItemLocation | null {
  return locateItems(timeline, [itemId])[0] ?? null;
}

/** `base`, else `base-2`, `base-3`, ...; the returned id is added to `taken`. */
function reserveUniqueId(base: string, taken: Set<string>): string {
  let candidate = base;
  let suffix = 2;
  while (taken.has(candidate)) {
    candidate = `${base}-${suffix.toString()}`;
    suffix += 1;
  }
  taken.add(candidate);
  return candidate;
}

function projectItemIds(project: VideoProject): Set<string> {
  return new Set(project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)));
}

function projectLinkGroupIds(project: VideoProject): Set<string> {
  return new Set(
    project.timeline.tracks.flatMap((track) =>
      track.items.flatMap((item) => {
        const groupId = stringProperty(item, "linkGroupId");
        return groupId ? [groupId] : [];
      }),
    ),
  );
}

/** Appends the `removeTracks` action for tracks the batch leaves empty. */
export function withEmptyTrackRemovals(project: VideoProject, actions: ProjectAction[]): CommandResult {
  const after = actions.reduce(applyProjectActionLocally, project);
  const removal = emptyTrackRemovals(project.timeline, after.timeline);
  return { actions: removal ? [...actions, removal] : actions };
}

interface ItemClonePlacement {
  readonly source: TimelineItem;
  readonly targetTrackId: string;
  readonly startSeconds: number;
}

/**
 * Clones items for duplicate and paste (legacy rules): deep-cloned source and properties,
 * `<label> <labelSuffix>`, ids from `newId` made unique across the project and the batch.
 * When more than one member of a link group is cloned, the clones share a new group
 * `<linkPrefix>-<first clone id>`; a lone member loses its `linkGroupId`.
 * Returns one `addItems`-ready group per target track, in first-seen order.
 */
export function cloneItemsForTracks(
  project: VideoProject,
  placements: readonly ItemClonePlacement[],
  options: {
    readonly newId: (sourceItemId: string) => string;
    readonly labelSuffix: string;
    readonly linkPrefix: string;
  },
): Map<string, TimelineItem[]> {
  const takenItemIds = projectItemIds(project);
  const takenGroupIds = projectLinkGroupIds(project);
  const groupCounts = new Map<string, number>();
  for (const { source } of placements) {
    const groupId = stringProperty(source, "linkGroupId");
    if (groupId) groupCounts.set(groupId, (groupCounts.get(groupId) ?? 0) + 1);
  }
  const cloneGroupIds = new Map<string, string>();
  const itemsByTrack = new Map<string, TimelineItem[]>();
  for (const { source, targetTrackId, startSeconds } of placements) {
    const id = reserveUniqueId(options.newId(source.id), takenItemIds);
    const properties = structuredClone(source.properties);
    const groupId = stringProperty(source, "linkGroupId");
    if (groupId && (groupCounts.get(groupId) ?? 0) > 1) {
      let cloneGroupId = cloneGroupIds.get(groupId);
      if (!cloneGroupId) {
        cloneGroupId = reserveUniqueId(`${options.linkPrefix}-${id}`, takenGroupIds);
        cloneGroupIds.set(groupId, cloneGroupId);
      }
      properties.linkGroupId = cloneGroupId;
    } else {
      delete properties.linkGroupId;
    }
    const clone: TimelineItem = {
      ...source,
      id,
      startSeconds,
      label: `${source.label} ${options.labelSuffix}`,
      source: structuredClone(source.source),
      properties,
    };
    itemsByTrack.set(targetTrackId, [...(itemsByTrack.get(targetTrackId) ?? []), clone]);
  }
  return itemsByTrack;
}

/** Legacy right-half id: `${itemId}-split-${round(splitSeconds * 1000)}`. */
export function defaultSplitItemId(itemId: string, splitSeconds: number): string {
  return `${itemId}-split-${Math.round(splitSeconds * 1000).toString()}`;
}

/**
 * Splits every selected, unlocked clip the playhead is strictly inside. `newId` names the
 * right halves (suffixed -2, -3 on collision). Link groups with two or more split members
 * re-link their right halves to a new group `link-split-<ms>` in the same batch.
 */
export function splitAtPlayhead(
  project: VideoProject,
  itemIds: readonly string[],
  playheadSeconds: number,
  newId: (itemId: string) => string,
): CommandResult {
  const selected = locateItems(project.timeline, itemIds);
  if (selected.length === 0) return { blocked: "Select a clip to split." };
  const unlocked = selected.filter(({ track }) => !track.locked);
  if (unlocked.length === 0) return { blocked: "Unlock the selected tracks to split these clips." };
  const splitSeconds = Number.isFinite(playheadSeconds) ? roundTimelineSeconds(playheadSeconds) : Number.NaN;
  const splittable = unlocked.filter(
    ({ item }) => splitSeconds > item.startSeconds && splitSeconds < item.startSeconds + item.durationSeconds,
  );
  if (splittable.length === 0) return { blocked: "Move the playhead over the selected clip to split." };

  const takenItemIds = projectItemIds(project);
  const rightIds = new Map(
    splittable.map(({ item }) => [item.id, reserveUniqueId(newId(item.id), takenItemIds)] as const),
  );
  const actions: ProjectAction[] = [
    {
      type: "splitItems",
      splits: splittable.map(({ item }) => ({
        itemId: item.id,
        newItemId: rightIds.get(item.id) ?? item.id,
        splitSeconds,
      })),
    },
  ];

  const members = new Map<string, string[]>();
  for (const { item } of splittable) {
    const groupId = stringProperty(item, "linkGroupId");
    if (groupId) members.set(groupId, [...(members.get(groupId) ?? []), item.id]);
  }
  const takenGroupIds = projectLinkGroupIds(project);
  const splitToken = Math.round(splitSeconds * 1000).toString();
  const updates = [...members.values()].flatMap((groupItemIds) => {
    if (groupItemIds.length < 2) return [];
    const rightGroupId = reserveUniqueId(`link-split-${splitToken}`, takenGroupIds);
    return groupItemIds.map((itemId) => ({
      itemId: rightIds.get(itemId) ?? itemId,
      set: { linkGroupId: rightGroupId },
      remove: [],
    }));
  });
  if (updates.length > 0) actions.push({ type: "updateItemProperties", updates });
  return { actions };
}

export function deleteItems(project: VideoProject, itemIds: readonly string[]): CommandResult {
  const selected = locateItems(project.timeline, itemIds);
  if (selected.length === 0) return { blocked: "Select a clip to delete." };
  if (selected.some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to remove these clips." };
  }
  return withEmptyTrackRemovals(project, [
    { type: "removeItems", itemIds: selected.map(({ item }) => item.id) },
  ]);
}

interface Interval {
  start: number;
  end: number;
}

function mergeIntervals(intervals: readonly Interval[]): Interval[] {
  const sorted = [...intervals].sort((left, right) => left.start - right.start || left.end - right.end);
  const merged: Interval[] = [];
  for (const interval of sorted) {
    const last = merged[merged.length - 1];
    if (last && interval.start <= last.end + timeEpsilonSeconds) {
      last.end = Math.max(last.end, interval.end);
    } else {
      merged.push({ ...interval });
    }
  }
  return merged;
}

/**
 * Ripple deletes the selection. Each clip's range closes on its own track and on the tracks
 * of its linked clips (legacy `rippleTrackIdsForItem`); every unlocked sync-locked track
 * closes the union of all selected ranges. Ranges are merged per track so the reducer never
 * shifts one track twice for the same time, then grouped into one range per span.
 */
export function rippleDeleteItems(project: VideoProject, itemIds: readonly string[]): CommandResult {
  const selected = locateItems(project.timeline, itemIds);
  if (selected.length === 0) return { blocked: "Select a clip to ripple delete." };
  if (selected.some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to ripple delete these clips." };
  }

  const tracks = project.timeline.tracks;
  const unlockedTrackIds = new Set(tracks.filter((track) => !track.locked).map((track) => track.id));
  const intervalsByTrack = new Map<string, Interval[]>();
  const addInterval = (trackId: string, interval: Interval) => {
    if (!unlockedTrackIds.has(trackId)) return;
    intervalsByTrack.set(trackId, [...(intervalsByTrack.get(trackId) ?? []), interval]);
  };
  const selectedIntervals = selected.map(({ item, track }) => {
    const interval = {
      start: roundTimelineSeconds(item.startSeconds),
      end: roundTimelineSeconds(item.startSeconds + item.durationSeconds),
    };
    for (const trackId of rippleTrackIdsForItem(project, item, track.id)) addInterval(trackId, interval);
    return interval;
  });
  for (const track of tracks) {
    if (track.syncLocked) for (const interval of selectedIntervals) addInterval(track.id, interval);
  }

  const ranges = new Map<string, { startSeconds: number; endSeconds: number; trackIds: string[] }>();
  for (const track of tracks) {
    for (const { start, end } of mergeIntervals(intervalsByTrack.get(track.id) ?? [])) {
      const key = `${start.toString()}:${end.toString()}`;
      const range = ranges.get(key) ?? { startSeconds: start, endSeconds: end, trackIds: [] };
      range.trackIds.push(track.id);
      ranges.set(key, range);
    }
  }
  const orderedRanges: ProjectActionRippleDeleteRange[] = [...ranges.values()].sort(
    (left, right) => left.startSeconds - right.startSeconds || left.endSeconds - right.endSeconds,
  );
  return withEmptyTrackRemovals(project, [{ type: "rippleDeleteRanges", ranges: orderedRanges }]);
}

/**
 * Closes the empty gap under `seconds` on a track (`timelineGapAtSeconds`): clips starting at
 * or after the gap end shift left by the gap on that track and on every unlocked sync-locked
 * track. Blocked when a shifted clip would overlap another clip.
 */
export function deleteGapAt(project: VideoProject, trackId: string, seconds: number): CommandResult {
  const gapTrack = project.timeline.tracks.find((track) => track.id === trackId);
  if (!gapTrack) return { blocked: "That track no longer exists." };
  if (gapTrack.locked) return { blocked: "Unlock the track to delete this gap." };
  const gap = timelineGapAtSeconds(gapTrack, seconds);
  const gapSeconds = gap ? gap.endSeconds - gap.startSeconds : 0;
  if (!gap || gapSeconds <= 0) return { blocked: "Move the playhead into an empty gap to delete it." };

  const moves = project.timeline.tracks
    .filter((track) => track.id === trackId || (track.syncLocked === true && !track.locked))
    .flatMap((track) =>
      track.items
        .filter((item) => item.startSeconds >= gap.endSeconds)
        .map((item) => ({
          itemId: item.id,
          targetTrackId: track.id,
          startSeconds: roundTimelineSeconds(item.startSeconds - gapSeconds),
        })),
    );
  const lead = moves[0];
  if (!lead) return { blocked: "Move the playhead into an empty gap to delete it." };
  const evaluation = evaluateTimelineMove({
    timeline: project.timeline,
    itemId: lead.itemId,
    itemIds: moves.map((move) => move.itemId),
    targetTrackId: lead.targetTrackId,
    proposedStartSeconds: lead.startSeconds,
    guideSeconds: null,
  });
  if (evaluation.state === "rejected") {
    return { blocked: `Can't close this gap. ${evaluation.reason?.message ?? "Clips would overlap"}.` };
  }
  return { actions: [{ type: "moveItems", moves }] };
}

/**
 * Moves the selection by whole frames at `renderSettings.fps`, clamped so the earliest clip
 * stays at or after 0. The legacy toolbar nudged by ±0.25 s; the redesign nudges by frames.
 */
export function nudgeItems(project: VideoProject, itemIds: readonly string[], frames: number): CommandResult {
  const selected = locateItems(project.timeline, itemIds);
  if (selected.length === 0) return { blocked: "Select a clip to nudge." };
  if (selected.some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to nudge these clips." };
  }
  if (!Number.isFinite(frames) || Math.trunc(frames) === 0) return { blocked: "Nudge by at least one frame." };

  const fps = project.renderSettings.fps > 0 ? project.renderSettings.fps : fallbackFps;
  const earliestStart = Math.min(...selected.map(({ item }) => item.startSeconds));
  const deltaSeconds = Math.max(Math.trunc(frames) / fps, -earliestStart);
  if (Math.abs(deltaSeconds) < timeEpsilonSeconds) {
    return { blocked: "The selected clips are already at the start of the timeline." };
  }
  const [lead] = selected;
  if (!lead) return { blocked: "Select a clip to nudge." };
  const evaluation = evaluateTimelineMove({
    timeline: project.timeline,
    itemId: lead.item.id,
    itemIds: selected.map(({ item }) => item.id),
    targetTrackId: lead.track.id,
    proposedStartSeconds: lead.item.startSeconds + deltaSeconds,
    guideSeconds: null,
  });
  if (evaluation.state === "rejected") return { blocked: evaluation.reason?.message ?? "Can't move these clips there." };
  return {
    actions: [
      {
        type: "moveItems",
        moves: evaluation.patches.map((patch) => ({ ...patch, startSeconds: Math.max(0, patch.startSeconds) })),
      },
    ],
  };
}

function sourceMedia(project: VideoProject, item: TimelineItem) {
  const mediaId = item.source.type === "media" ? item.source.mediaId : null;
  return mediaId === null ? undefined : project.media.find((candidate) => candidate.id === mediaId);
}

/** Timeline seconds the item can play before running out of source media, or Infinity. */
function maximumDurationForSource(project: VideoProject, item: TimelineItem): number {
  const media = sourceMedia(project, item);
  if (!media || !(media.durationSeconds > 0)) return Number.POSITIVE_INFINITY;
  const speed = itemSpeed(item);
  const sourceIn = numberProperty(item, "sourceIn") ?? 0;
  const sourceOut = numberProperty(item, "sourceOut");
  // A reversed clip's right edge reads towards source 0.
  if (isReversedItem(item) && sourceOut !== null) return Math.max(0, sourceOut / speed);
  return Math.max(0, (media.durationSeconds - sourceIn) / speed);
}

function itemSpeed(item: TimelineItem): number {
  const speed = numberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

function collisionBlocked(message: string) {
  return { blocked: `${message}. Make room after this clip first.` };
}

/**
 * Sets a clip's duration from its right edge through `evaluateTimelineResize`. Clips with a
 * source range trim `sourceOut` (`trimItems`); others resize (`resizeItems`). The duration
 * is silently clamped to the remaining source media; collisions block.
 */
export function setItemDuration(project: VideoProject, itemId: string, durationSeconds: number): CommandResult {
  const location = locateItem(project.timeline, itemId);
  if (!location) return { blocked: "That clip no longer exists." };
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return { blocked: "Enter a duration longer than zero." };
  }
  if (location.track.locked) return { blocked: "Unlock the track to change this clip's duration." };

  const { item } = location;
  const media = sourceMedia(project, item);
  const evaluation = evaluateTimelineResize({
    timeline: project.timeline,
    itemId,
    edge: "right",
    proposedStartSeconds: item.startSeconds,
    proposedDurationSeconds: roundTimelineSeconds(Math.min(durationSeconds, maximumDurationForSource(project, item))),
    guideSeconds: null,
    ...(media ? { sourceDurationSeconds: media.durationSeconds } : {}),
  });
  if (evaluation.reason?.code === "collision") return collisionBlocked(evaluation.reason.message);
  if (evaluation.state === "rejected") return { blocked: evaluation.reason?.message ?? "Can't change this clip's duration." };
  return { actions: evaluation.patch ? [projectActionFromTimelinePatch(evaluation.patch)] : [] };
}

/**
 * Sets a visual or audio clip's playback speed (0.1x–8x). Like the legacy inspector, the duration
 * is rescaled to `duration * currentSpeed / speed` (3 dp) so the source range stays the same. Speed
 * follows links: every visual or audio clip sharing the clip's `linkGroupId` (text partners
 * excluded, as in MCP `set_clip_properties`) gets the same speed and its own rescaled duration. All
 * speed actions and one `resizeItems` go in one batch; the first blocked partner blocks the batch.
 */
export function setItemSpeed(project: VideoProject, itemId: string, speed: number): CommandResult {
  const location = locateItem(project.timeline, itemId);
  if (!location) return { blocked: "That clip no longer exists." };
  const { item, track } = location;
  if (!isSpeedItem(item)) return { blocked: "Speed is available for video, image and audio clips." };
  if (!Number.isFinite(speed) || speed < minimumSpeed || speed > maximumSpeed) {
    return { blocked: "Enter a speed between 0.1x and 8x." };
  }
  if (track.locked) return { blocked: "Unlock the track to change this clip's speed." };
  if (speed === itemSpeed(item)) return { actions: [] };

  const speedActions: ProjectAction[] = [];
  const resizes: { itemId: string; durationSeconds: number }[] = [];
  for (const target of [location, ...linkedSpeedPartners(project, item)]) {
    const targetSpeed = itemSpeed(target.item);
    if (speed === targetSpeed) continue;
    if (target.track.locked) return { blocked: "Unlock the linked clip's track to change its speed." };
    const newDurationSeconds = roundTimelineSeconds((target.item.durationSeconds * targetSpeed) / speed);
    if (newDurationSeconds <= 0) return { blocked: "This clip is too short for that speed." };
    const evaluation = evaluateTimelineResize({
      timeline: project.timeline,
      itemId: target.item.id,
      edge: "right",
      proposedStartSeconds: target.item.startSeconds,
      proposedDurationSeconds: newDurationSeconds,
      guideSeconds: null,
    });
    if (evaluation.reason?.code === "collision") return collisionBlocked(evaluation.reason.message);
    if (evaluation.state === "rejected") return { blocked: evaluation.reason?.message ?? "Can't change this clip's speed." };
    speedActions.push(
      target.item.kind === "audio_clip"
        ? { type: "updateAudioClipSpeed", itemId: target.item.id, speed }
        : { type: "updateVisualClipSpeed", itemId: target.item.id, speed },
    );
    resizes.push({ itemId: target.item.id, durationSeconds: newDurationSeconds });
  }
  return { actions: [...speedActions, { type: "resizeItems", resizes }] };
}

function isSpeedItem(item: TimelineItem) {
  return isVisualOpacityItem(item) || item.kind === "audio_clip";
}

/** Visual and audio clips sharing `item`'s link group, excluding text partners (speed and reverse follow them). */
function linkedSpeedPartners(project: VideoProject, item: TimelineItem) {
  const groupId = stringProperty(item, "linkGroupId");
  if (groupId === null) return [];
  return project.timeline.tracks.flatMap((track) =>
    track.items
      .filter((partner) => partner.id !== item.id && stringProperty(partner, "linkGroupId") === groupId)
      .filter((partner) => isSpeedItem(partner) && partner.kind !== "overlay" && partner.source.type !== "text")
      .map((partner) => ({ item: partner, track })),
  );
}

/**
 * Reverse playback for a video clip of video media or a media audio clip. Like speed, it follows
 * links: every partner that can play reversed gets the same direction, in one batch.
 */
export function setItemReverse(project: VideoProject, itemId: string, reverse: boolean): CommandResult {
  const location = locateItem(project.timeline, itemId);
  if (!location) return { blocked: "That clip no longer exists." };
  if (!isReversibleItem(project, location.item)) return { blocked: "Reverse is available for video and audio clips." };
  if (location.track.locked) return { blocked: "Unlock the track to reverse this clip." };
  const partners = linkedSpeedPartners(project, location.item).filter((partner) => isReversibleItem(project, partner.item));
  const actions: ProjectAction[] = [];
  for (const target of [location, ...partners]) {
    if (isReversedItem(target.item) === reverse) continue;
    if (target.track.locked) return { blocked: "Unlock the linked clip's track to reverse it." };
    actions.push({ type: "updateClipReverse", itemId: target.item.id, reverse });
  }
  return { actions };
}

/** Existing ids in the caller's order, without duplicates. */
function existingIdsInOrder(project: VideoProject, itemIds: readonly string[]) {
  const existing = projectItemIds(project);
  return [...new Set(itemIds)].filter((itemId) => existing.has(itemId));
}

export function linkItems(project: VideoProject, itemIds: readonly string[], linkGroupId: string): CommandResult {
  const ids = existingIdsInOrder(project, itemIds);
  if (ids.length < 2) return { blocked: "Select at least two clips to link." };
  if (locateItems(project.timeline, ids).some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to link these clips." };
  }
  const groupId = linkGroupId.trim();
  if (groupId.length === 0 || groupId.length > 128) return { blocked: "Link group ids need 1 to 128 characters." };
  return { actions: [{ type: "linkItems", itemIds: ids, linkGroupId: groupId }] };
}

export function unlinkItems(project: VideoProject, itemIds: readonly string[]): CommandResult {
  const ids = existingIdsInOrder(project, itemIds);
  if (ids.length === 0) return { blocked: "Select a clip to unlink." };
  const selected = locateItems(project.timeline, ids);
  if (!selected.some(({ item }) => stringProperty(item, "linkGroupId"))) {
    return { blocked: "The selected clips are not linked." };
  }
  if (selected.some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to unlink these clips." };
  }
  return { actions: [{ type: "unlinkItems", itemIds: ids }] };
}

/** Replaces a nested timeline item with its contents; the emptied wrapper track is removed. */
export function decomposeNested(project: VideoProject, itemId: string): CommandResult {
  const location = locateItem(project.timeline, itemId);
  if (!location) return { blocked: "That clip no longer exists." };
  const { item, track } = location;
  if (item.source.type !== "timeline") return { blocked: "Select one nested timeline sequence to decompose." };
  if (track.locked) return { blocked: "Unlock the selected track to decompose this sequence." };
  const { timelineId } = item.source;
  if (!project.timelines?.some((entry) => entry.id === timelineId)) {
    return { blocked: "The nested timeline no longer exists." };
  }
  if (Object.keys(item.properties).length > 0) {
    return { blocked: "Clear this sequence's clip settings before you decompose it." };
  }
  return withEmptyTrackRemovals(project, [{ type: "decomposeTimelineItem", itemId }]);
}

/**
 * Duplicates the selection on its own tracks, shifted by the selection span (a single clip
 * lands right after itself). Ids and labels follow the legacy "<id>-copy" / "<label> copy".
 */
export function duplicateItems(project: VideoProject, itemIds: readonly string[]): CommandResult {
  const selected = locateItems(project.timeline, itemIds);
  if (selected.length === 0) return { blocked: "Select a clip to duplicate." };
  if (selected.some(({ track }) => track.locked)) {
    return { blocked: "Unlock the selected tracks to duplicate these clips." };
  }
  const selectionStart = Math.min(...selected.map(({ item }) => item.startSeconds));
  const selectionEnd = Math.max(...selected.map(({ item }) => item.startSeconds + item.durationSeconds));
  const offsetSeconds = selectionEnd - selectionStart;
  const itemsByTrack = cloneItemsForTracks(
    project,
    selected.map(({ item, track }) => ({
      source: item,
      targetTrackId: track.id,
      startSeconds: roundTimelineSeconds(item.startSeconds + offsetSeconds),
    })),
    { newId: (sourceItemId) => duplicateTimelineItemId(project, sourceItemId), labelSuffix: "copy", linkPrefix: "link-copy" },
  );
  return {
    actions: [...itemsByTrack].map(([targetTrackId, items]) => ({ type: "addItems", targetTrackId, items })),
  };
}
