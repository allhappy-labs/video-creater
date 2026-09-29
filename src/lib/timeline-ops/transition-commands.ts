import type { ProjectAction, VideoProject } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack, TimelineTransition, TransitionKind } from "@/lib/timeline";
import type { CommandResult } from "./clip-commands";
import { timelineTransitionId } from "./ids";
import {
  applyTransitionAction,
  cutsOnTrack,
  formatTransitionMaxSeconds,
  formatTransitionSeconds,
  nearestCut,
  TRANSITION_SECONDS_EPSILON,
  transitionBounds,
  transitionFrameSeconds,
  type TransitionCut,
} from "./transitions";

/**
 * Editor planners for clip transitions: where the Effects tab `+`, a tile drop and the context menu
 * add a transition, and the single actions the badge, Properties and Delete commit.
 */

export const transitionKinds: readonly TransitionKind[] = ["crossfade", "dipToBlack", "dipToWhite", "wipe"];

export const transitionKindLabels: Readonly<Record<TransitionKind, string>> = {
  crossfade: "Crossfade",
  dipToBlack: "Dip to black",
  dipToWhite: "Dip to white",
  wipe: "Wipe",
};

/** The duration a new transition asks for before it is shortened to the cut's maximum. */
const defaultTransitionSeconds = 0.5;

/** Why nothing can take a transition: the `+` tooltip and the context menu reason. */
export const noCutReason = "Place two clips next to each other first";

/** A tile dropped this close to a cut, in screen pixels, lands on it. */
const transitionDropTolerancePixels = 12;

interface LocatedTransition {
  readonly track: TimelineTrack;
  readonly transition: TimelineTransition;
}

/** The transition and its track on the active timeline. */
export function locateTransition(timeline: Timeline, transitionId: string | null): LocatedTransition | null {
  if (transitionId === null) return null;
  for (const track of timeline.tracks) {
    const transition = (track.transitions ?? []).find((candidate) => candidate.id === transitionId);
    if (transition) return { track, transition };
  }
  return null;
}

/** "Crossfade transition, 0.5s": the badge's accessible name. */
export function transitionName(transition: Pick<TimelineTransition, "kind" | "durationSeconds">): string {
  return `${transitionKindLabels[transition.kind]} transition, ${formatTransitionSeconds(transition.durationSeconds)}s`;
}

function pairItems(track: TimelineTrack, leftItemId: string, rightItemId: string) {
  const left = track.items.find((item) => item.id === leftItemId);
  const right = track.items.find((item) => item.id === rightItemId);
  return left && right ? { left, right } : null;
}

export interface TransitionDurationRange {
  /** One frame. */
  readonly min: number;
  /** `min(5, 2 * handles, clip durations)`. */
  readonly max: number;
}

/** The durations `transition` accepts where it stands, or null when its clips are gone. */
export function transitionDurationRange(project: VideoProject, track: TimelineTrack, transition: TimelineTransition): TransitionDurationRange | null {
  const pair = pairItems(track, transition.leftItemId, transition.rightItemId);
  if (!pair) return null;
  return { min: transitionFrameSeconds(project), max: transitionBounds(project, pair.left, pair.right).maxSeconds };
}

export function clampTransitionSeconds(range: TransitionDurationRange, seconds: number): number {
  return Math.min(Math.max(seconds, range.min), Math.max(range.min, range.max));
}

type AddTransitionPlan =
  | {
      readonly actions: ProjectAction[];
      readonly transitionId: string;
      /** The toast when the transition was shortened to fit, or null at the default duration. */
      readonly shortenedMessage: string | null;
    }
  | { readonly blocked: string };

/** The default duration, or the longest the cut allows, floored to hundredths when that stays a frame or more. */
function fittedDuration(maxSeconds: number, frameSeconds: number): number {
  if (maxSeconds >= defaultTransitionSeconds - TRANSITION_SECONDS_EPSILON) return defaultTransitionSeconds;
  const floored = Math.floor(maxSeconds * 100 + TRANSITION_SECONDS_EPSILON) / 100;
  return floored >= frameSeconds - TRANSITION_SECONDS_EPSILON ? floored : maxSeconds;
}

function shortenedMessage(seconds: number, left: TimelineItem, right: TimelineItem, project: VideoProject): string {
  const { limit } = transitionBounds(project, left, right);
  const reason = limit === "leftHandle" || limit === "rightHandle" ? "not enough unused media" : "the clips are too short";
  return `Shortened to ${formatTransitionMaxSeconds(seconds)}s — ${reason}`;
}

/**
 * Adds a `kind` transition on `cut` at `min(0.5, max)`. A cut that already has a transition changes
 * its type instead. A cut whose maximum is under one frame is refused with the validation message.
 */
export function planAddTransition(project: VideoProject, cut: TransitionCut, kind: TransitionKind): AddTransitionPlan {
  const track = project.timeline.tracks.find((candidate) => candidate.id === cut.trackId);
  const pair = track ? pairItems(track, cut.leftItemId, cut.rightItemId) : null;
  if (!track || !pair) return { blocked: noCutReason };
  if (track.locked) return { blocked: "Unlock the track to add a transition." };
  if (cut.transitionId !== null) {
    const result = planTransitionKind(project, cut.transitionId, kind);
    return "blocked" in result ? result : { ...result, transitionId: cut.transitionId, shortenedMessage: null };
  }
  const frame = transitionFrameSeconds(project);
  const { maxSeconds } = transitionBounds(project, pair.left, pair.right);
  const blockedDuration = maxSeconds < frame - TRANSITION_SECONDS_EPSILON;
  const durationSeconds = blockedDuration ? defaultTransitionSeconds : fittedDuration(maxSeconds, frame);
  const transition: TimelineTransition = {
    id: timelineTransitionId(project, cut.leftItemId, cut.rightItemId),
    leftItemId: cut.leftItemId,
    rightItemId: cut.rightItemId,
    kind,
    durationSeconds,
  };
  const action: ProjectAction = { type: "addTransition", trackId: track.id, transition };
  const result = applyTransitionAction(project, action);
  if ("error" in result) return { blocked: result.error.message };
  const shortened = durationSeconds < defaultTransitionSeconds - TRANSITION_SECONDS_EPSILON;
  return {
    actions: [action],
    transitionId: transition.id,
    shortenedMessage: shortened ? shortenedMessage(durationSeconds, pair.left, pair.right, project) : null,
  };
}

function trackOfItem(timeline: Timeline, itemId: string | undefined): TimelineTrack | undefined {
  return itemId === undefined ? undefined : timeline.tracks.find((track) => track.items.some((item) => item.id === itemId));
}

interface TransitionTargetContext {
  readonly selectedTransitionId: string | null;
  readonly selectedItemIds: readonly string[];
  readonly selectedTrackId: string | null;
  readonly playheadSeconds: number;
}

/**
 * The cut the Effects tab `+` acts on: the selected transition's cut, else the cut nearest the
 * playhead on the selected clip's track (or the selected track), else on any track.
 */
export function transitionAddTarget(project: VideoProject, context: TransitionTargetContext): TransitionCut | null {
  const frame = transitionFrameSeconds(project);
  const selected = locateTransition(project.timeline, context.selectedTransitionId);
  if (selected) {
    const cut = cutsOnTrack(selected.track, frame).find((candidate) => candidate.transitionId === selected.transition.id);
    if (cut) return cut;
  }
  const trackId = trackOfItem(project.timeline, context.selectedItemIds[0])?.id ?? context.selectedTrackId ?? undefined;
  return nearestCut(project.timeline, context.playheadSeconds, trackId, frame);
}

/** The cut on `track` within the drop tolerance of `seconds` at this zoom, nearest first. */
export function cutNearTime(project: VideoProject, track: TimelineTrack, seconds: number, pixelsPerSecond: number): TransitionCut | null {
  const toleranceSeconds = transitionDropTolerancePixels / Math.max(pixelsPerSecond, TRANSITION_SECONDS_EPSILON);
  const cut = nearestCut({ ...project.timeline, tracks: [track] }, seconds, track.id, transitionFrameSeconds(project));
  return cut && Math.abs(cut.seconds - seconds) <= toleranceSeconds + TRANSITION_SECONDS_EPSILON ? cut : null;
}

/** The cut at either edge of `itemId` nearest `seconds` (where the clip was right-clicked). */
export function cutAdjacentToItem(project: VideoProject, itemId: string, seconds: number): TransitionCut | null {
  const track = trackOfItem(project.timeline, itemId);
  if (!track) return null;
  const cuts = cutsOnTrack(track, transitionFrameSeconds(project)).filter(
    (cut) => cut.leftItemId === itemId || cut.rightItemId === itemId,
  );
  return cuts.reduce<TransitionCut | null>(
    (nearest, cut) => (!nearest || Math.abs(cut.seconds - seconds) < Math.abs(nearest.seconds - seconds) ? cut : nearest),
    null,
  );
}

function editable(project: VideoProject, transitionId: string): LocatedTransition | { readonly blocked: string } {
  const located = locateTransition(project.timeline, transitionId);
  if (!located) return { blocked: "That transition no longer exists." };
  if (located.track.locked) return { blocked: "Unlock the track to edit this transition." };
  return located;
}

function checked(project: VideoProject, action: ProjectAction & { type: "updateTransition" | "removeTransition" }): CommandResult {
  const result = applyTransitionAction(project, action);
  return "error" in result ? { blocked: result.error.message } : { actions: [action] };
}

/** Sets the duration, clamped to one frame and the cut's maximum; an unchanged duration is an empty batch. */
export function planTransitionDuration(project: VideoProject, transitionId: string, seconds: number): CommandResult {
  const located = editable(project, transitionId);
  if ("blocked" in located) return located;
  const { track, transition } = located;
  const range = transitionDurationRange(project, track, transition);
  if (!range) return { blocked: "That transition no longer exists." };
  const durationSeconds = clampTransitionSeconds(range, seconds);
  if (Math.abs(durationSeconds - transition.durationSeconds) <= TRANSITION_SECONDS_EPSILON) return { actions: [] };
  return checked(project, { type: "updateTransition", trackId: track.id, transitionId, durationSeconds });
}

export function planTransitionKind(project: VideoProject, transitionId: string, kind: TransitionKind): CommandResult {
  const located = editable(project, transitionId);
  if ("blocked" in located) return located;
  if (located.transition.kind === kind) return { actions: [] };
  return checked(project, { type: "updateTransition", trackId: located.track.id, transitionId, kind });
}

export function planRemoveTransition(project: VideoProject, transitionId: string): CommandResult {
  const located = editable(project, transitionId);
  if ("blocked" in located) return located;
  return checked(project, { type: "removeTransition", trackId: located.track.id, transitionId });
}
