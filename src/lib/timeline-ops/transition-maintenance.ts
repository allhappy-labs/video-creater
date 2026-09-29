import type { ProjectTimeline, VideoProject } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack, TimelineTransition } from "@/lib/timeline";
import { isReversedItem } from "./reverse";
import {
  itemsAreAdjacent,
  transitionFrameSeconds,
  transitionMaxDuration,
  transitionPairError,
  TRANSITION_SECONDS_EPSILON,
} from "@/lib/timeline-ops/transitions";

/**
 * Keeps stored transitions valid after any local project action. Mirrors
 * `src-tauri/src/project/transitions/maintenance.rs`, run in the same order as
 * `apply_project_action`:
 *
 * 1. {@link retargetTransitionsToSplitTails}, when the action did not switch the active timeline.
 *    Splits, ripple deletes, inserts and overwrites keep the original id on the left-hand piece of a
 *    cut clip, so a transition on that clip's end moves to the new right-hand piece.
 * 2. {@link maintainTransitions} on the active timeline and every library timeline: drop
 *    transitions whose clips are missing, no longer adjacent or eligible, already covered by an
 *    earlier transition on the same cut, or without room for one frame; otherwise clamp the
 *    duration into `[one frame, maximum]`.
 *
 * Every step returns its input unchanged (by reference) when nothing changes.
 */

function sameSource(item: TimelineItem, original: TimelineItem): boolean {
  const a = item.source;
  const b = original.source;
  switch (a.type) {
    case "media":
      return b.type === "media" && a.mediaId === b.mediaId;
    case "generated":
      return b.type === "generated" && a.artifactId === b.artifactId;
    case "timeline":
      return b.type === "timeline" && a.timelineId === b.timelineId;
    case "text":
      return b.type === "text" && a.text === b.text;
  }
}

/** Whether `item` ends at the source second `original` ends at (`sourceIn` for reversed clips). */
function sameEndSource(item: TimelineItem, original: TimelineItem): boolean {
  const key = isReversedItem(original) ? "sourceIn" : "sourceOut";
  const itemOut = item.properties[key];
  const originalOut = original.properties[key];
  const itemNumber = typeof itemOut === "number" ? itemOut : null;
  const originalNumber = typeof originalOut === "number" ? originalOut : null;
  if (itemNumber === null || originalNumber === null) return itemNumber === originalNumber;
  return Math.abs(itemNumber - originalNumber) <= TRANSITION_SECONDS_EPSILON;
}

/**
 * Moves a transition's left clip to the new right-hand piece of that clip when the action cut it in
 * two. A candidate piece is on the same track, did not exist in `before`, has the original clip's
 * kind, source and direction, ends at the same source edge (`sourceOut`, or `sourceIn` when
 * reversed), and is adjacent to the transition's right clip.
 */
function retargetTransitionsToSplitTails(before: Timeline, after: Timeline, frameSeconds: number): Timeline {
  let beforeIds: Set<string> | null = null;
  let changed = false;
  const tracks = after.tracks.map((track) => {
    const transitions = track.transitions;
    if (!transitions?.length) return track;
    const beforeTrack = before.tracks.find((entry) => entry.id === track.id);
    if (!beforeTrack) return track;
    const find = (itemId: string) => track.items.find((item) => item.id === itemId);
    let trackChanged = false;
    const next = transitions.map((transition) => {
      const right = find(transition.rightItemId);
      if (!right) return transition;
      const left = find(transition.leftItemId);
      if (left && itemsAreAdjacent(left, right, frameSeconds)) return transition;
      const original = beforeTrack.items.find((item) => item.id === transition.leftItemId);
      if (!original) return transition;
      const knownIds = (beforeIds ??= new Set(
        before.tracks.flatMap((entry) => entry.items.map((item) => item.id)),
      ));
      const tail = track.items.find(
        (item) =>
          !knownIds.has(item.id) &&
          item.kind === original.kind &&
          sameSource(item, original) &&
          isReversedItem(item) === isReversedItem(original) &&
          sameEndSource(item, original) &&
          itemsAreAdjacent(item, right, frameSeconds),
      );
      if (!tail) return transition;
      trackChanged = true;
      return { ...transition, leftItemId: tail.id };
    });
    if (!trackChanged) return track;
    changed = true;
    return { ...track, transitions: next };
  });
  return changed ? { ...after, tracks } : after;
}

function maintainedTrackTransitions(project: VideoProject, track: TimelineTrack): TimelineTrack {
  const transitions = track.transitions;
  if (!transitions?.length) return track;
  const frame = transitionFrameSeconds(project);
  const usedLeft = new Set<string>();
  const usedRight = new Set<string>();
  const find = (itemId: string) => track.items.find((item) => item.id === itemId);
  let changed = false;
  const maintained: TimelineTransition[] = [];
  for (const transition of transitions) {
    const left = find(transition.leftItemId);
    const right = find(transition.rightItemId);
    const max = left && right ? transitionMaxDuration(project, left, right) : 0;
    if (
      !left ||
      !right ||
      transitionPairError(left, right, frame) ||
      usedLeft.has(left.id) ||
      usedRight.has(right.id) ||
      !Number.isFinite(transition.durationSeconds) ||
      max < frame - TRANSITION_SECONDS_EPSILON
    ) {
      changed = true;
      continue;
    }
    let durationSeconds = transition.durationSeconds;
    if (durationSeconds > max + TRANSITION_SECONDS_EPSILON) durationSeconds = max;
    else if (durationSeconds < frame - TRANSITION_SECONDS_EPSILON) durationSeconds = frame;
    usedLeft.add(left.id);
    usedRight.add(right.id);
    if (durationSeconds === transition.durationSeconds) {
      maintained.push(transition);
    } else {
      changed = true;
      maintained.push({ ...transition, durationSeconds });
    }
  }
  if (!changed) return track;
  const { transitions: _previous, ...rest } = track;
  return maintained.length > 0 ? { ...rest, transitions: maintained } : rest;
}

function maintainedTimeline(project: VideoProject, timeline: Timeline): Timeline {
  let changed = false;
  const tracks = timeline.tracks.map((track) => {
    const next = maintainedTrackTransitions(project, track);
    if (next !== track) changed = true;
    return next;
  });
  return changed ? { ...timeline, tracks } : timeline;
}

/**
 * Drops or clamps transitions that edits made invalid, on the active timeline and every library
 * timeline. A library entry that shares the active timeline object receives the maintained active
 * timeline, so the two stay in sync.
 */
export function maintainTransitions(project: VideoProject): VideoProject {
  const timeline = maintainedTimeline(project, project.timeline);
  let changed = timeline !== project.timeline;
  const timelines = project.timelines?.map((entry): ProjectTimeline => {
    const next = entry.timeline === project.timeline ? timeline : maintainedTimeline(project, entry.timeline);
    if (next === entry.timeline) return entry;
    changed = true;
    return { ...entry, timeline: next };
  });
  if (!changed) return project;
  return { ...project, timeline, ...(timelines ? { timelines } : {}) };
}

/**
 * The transition maintenance `apply_project_action` runs after every action: re-target split tails
 * when the active timeline did not change, then drop or clamp across all timelines.
 */
export function maintainTransitionsAfterAction(before: VideoProject, after: VideoProject): VideoProject {
  let project = after;
  if ((before.activeTimelineId ?? "main") === (after.activeTimelineId ?? "main")) {
    const timeline = retargetTransitionsToSplitTails(
      before.timeline,
      after.timeline,
      transitionFrameSeconds(after),
    );
    if (timeline !== after.timeline) {
      const activeId = after.activeTimelineId ?? "main";
      project = {
        ...after,
        timeline,
        ...(after.timelines
          ? {
              timelines: after.timelines.map((entry) =>
                entry.id === activeId && entry.timeline === after.timeline ? { ...entry, timeline } : entry,
              ),
            }
          : {}),
      };
    }
  }
  return maintainTransitions(project);
}
