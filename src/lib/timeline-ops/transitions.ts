import type { ProjectAction, VideoProject } from "@/lib/project";
import type {
  Timeline,
  TimelineItem,
  TimelineItemKind,
  TimelineTrack,
  TimelineTransition,
} from "@/lib/timeline";
import { headHandleSeconds, isReversedItem, tailHandleSeconds, type SourceWindow } from "./reverse";

/**
 * Clip-to-clip transitions stored on a track and centered on a cut. Mirrors
 * `src-tauri/src/project/transitions.rs`: handle math, duration bounds, validation messages, and the
 * three transition actions.
 *
 * A clip's handles are its unused source media in timeline seconds: the left clip's tail handle is
 * `(sourceDuration - sourceOut) / speed` and the right clip's head handle is `sourceIn / speed`
 * (swapped for reversed clips, see `./reverse`).
 * The maximum duration is `min(5, 2 * leftHandle, 2 * rightHandle, left.duration, right.duration)`
 * and the minimum is one frame.
 */

const MAX_TRANSITION_SECONDS = 5;
export const TRANSITION_SECONDS_EPSILON = 0.000_001;

/** Which constraint sets a transition's maximum duration. Ties report the first in this order. */
type TransitionLimit = "cap" | "leftHandle" | "rightHandle" | "leftDuration" | "rightDuration";

interface TransitionBounds {
  readonly maxSeconds: number;
  readonly limit: TransitionLimit;
}

type TransitionErrorCode =
  | "invalidTransition"
  | "itemNotFound"
  | "trackNotFound"
  | "trackLocked"
  | "transitionNotFound";

/** A rejected transition: `message` matches the Rust `ProjectActionError` display text. */
export interface TransitionError {
  readonly code: TransitionErrorCode;
  readonly message: string;
}

export type TransitionAction = Extract<
  ProjectAction,
  { type: "addTransition" | "updateTransition" | "removeTransition" }
>;

/** The media a clip's handles are measured against. */
export type TransitionMediaSources = Pick<VideoProject, "media" | "generatedAssets">;

/** One frame at the project frame rate (24 fps when the setting is invalid). */
export function transitionFrameSeconds(project: Pick<VideoProject, "renderSettings">): number {
  const fps = project.renderSettings.fps;
  return Number.isFinite(fps) && fps > 0 ? 1 / fps : 1 / 24;
}

/** The maximum duration between `left` and `right`, and the constraint that sets it. */
export function transitionBounds(
  project: TransitionMediaSources,
  left: TimelineItem,
  right: TimelineItem,
): TransitionBounds {
  const doubled = (handle: number | null) => (handle === null ? Infinity : 2 * handle);
  const candidates: readonly (readonly [number, TransitionLimit])[] = [
    [MAX_TRANSITION_SECONDS, "cap"],
    [doubled(itemTailHandleSeconds(project, left)), "leftHandle"],
    [doubled(itemHeadHandleSeconds(project, right)), "rightHandle"],
    [left.durationSeconds, "leftDuration"],
    [right.durationSeconds, "rightDuration"],
  ];
  return candidates.reduce<TransitionBounds>(
    (best, [raw, limit]) => {
      const seconds = Number.isFinite(raw) ? Math.max(raw, 0) : raw === Infinity ? raw : 0;
      return seconds < best.maxSeconds - TRANSITION_SECONDS_EPSILON ? { maxSeconds: seconds, limit } : best;
    },
    { maxSeconds: Infinity, limit: "cap" },
  );
}

/** `min(5, 2 * leftHandle, 2 * rightHandle, left.duration, right.duration)`. */
export function transitionMaxDuration(project: TransitionMediaSources, left: TimelineItem, right: TimelineItem): number {
  return transitionBounds(project, left, right).maxSeconds;
}

/** Unused source media after the clip's end, in timeline seconds; `null` is unlimited. */
function itemTailHandleSeconds(project: TransitionMediaSources, item: TimelineItem): number | null {
  const sourceDuration = itemSourceMediaDuration(project, item);
  return sourceDuration === null ? null : tailHandleSeconds(itemSourceWindow(item), sourceDuration);
}

/** Unused source media before the clip's start, in timeline seconds; `null` is unlimited. */
function itemHeadHandleSeconds(project: TransitionMediaSources, item: TimelineItem): number | null {
  const sourceDuration = itemSourceMediaDuration(project, item);
  return sourceDuration === null ? null : headHandleSeconds(itemSourceWindow(item), sourceDuration);
}

function finiteProperty(item: TimelineItem, key: string): number | null {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function itemSourceWindow(item: TimelineItem): SourceWindow {
  const rawSpeed = item.properties.speed;
  const speed = typeof rawSpeed === "number" && Number.isFinite(rawSpeed) && rawSpeed > 0 ? rawSpeed : 1;
  const sourceIn = finiteProperty(item, "sourceIn") ?? 0;
  const sourceOut = finiteProperty(item, "sourceOut") ?? sourceIn + item.durationSeconds * speed;
  return { sourceIn, sourceOut, speed, reverse: isReversedItem(item) };
}

/** The finite source duration a clip can run out of, or `null` when unlimited. */
function itemSourceMediaDuration(project: TransitionMediaSources, item: TimelineItem): number | null {
  if (item.kind === "image_clip") return null;
  let mediaId: string;
  let outputDuration: number | null = null;
  switch (item.source.type) {
    case "media":
      mediaId = item.source.mediaId;
      break;
    case "generated": {
      const { artifactId } = item.source;
      const asset = project.generatedAssets.find(
        (candidate) => candidate.id === artifactId && candidate.status === "completed",
      );
      const output = asset?.outputs[0];
      if (!output) return null;
      mediaId = output.mediaId;
      outputDuration = output.durationSeconds;
      break;
    }
    case "timeline":
    case "text":
      return null;
  }
  const media = project.media.find((candidate) => candidate.id === mediaId);
  if (media?.kind === "image") return null;
  const duration = media ? media.durationSeconds : outputDuration;
  return duration !== null && Number.isFinite(duration) && duration > 0 ? duration : null;
}

const visualTransitionKinds: ReadonlySet<TimelineItemKind> = new Set(["video_clip", "image_clip", "generated_clip"]);

function isTransitionItemKind(kind: TimelineItemKind): boolean {
  return visualTransitionKinds.has(kind) || kind === "audio_clip";
}

function invalid(message: string): TransitionError {
  return { code: "invalidTransition", message };
}

function label(item: TimelineItem): string {
  return item.label.trim() || "this clip";
}

function roundHalfAwayFromZero(value: number): number {
  return Math.sign(value) * Math.round(Math.abs(value));
}

/** Seconds for messages: `1.0`, `0.4`, `0.25`. */
export function formatTransitionSeconds(seconds: number): string {
  const hundredths = roundHalfAwayFromZero(seconds * 100);
  return (hundredths / 100).toFixed(hundredths % 10 === 0 ? 1 : 2);
}

/** A maximum rounded down so the reported value is always accepted. */
export function formatTransitionMaxSeconds(seconds: number): string {
  return formatTransitionSeconds(Math.floor(seconds * 100 + TRANSITION_SECONDS_EPSILON) / 100);
}

/** Whether `left` ends where `right` starts, within one frame. */
export function itemsAreAdjacent(left: TimelineItem, right: TimelineItem, frameSeconds: number): boolean {
  return (
    left.startSeconds < right.startSeconds &&
    Math.abs(left.startSeconds + left.durationSeconds - right.startSeconds) <=
      frameSeconds + TRANSITION_SECONDS_EPSILON
  );
}

/**
 * Structural checks shared by validation and maintenance: eligible kinds and sources, no
 * visual/audio mix, and adjacency within one frame.
 */
export function transitionPairError(
  left: TimelineItem,
  right: TimelineItem,
  frameSeconds: number,
): TransitionError | null {
  const kindMessage = "Transitions only work between video, image, generated, or audio clips.";
  if (!isTransitionItemKind(left.kind) || !isTransitionItemKind(right.kind)) return invalid(kindMessage);
  for (const item of [left, right]) {
    if (item.source.type === "timeline") return invalid("Transitions don't support nested sequences yet.");
    if (item.source.type === "text") return invalid(kindMessage);
  }
  if (visualTransitionKinds.has(left.kind) !== visualTransitionKinds.has(right.kind)) {
    return invalid("Transitions need two visual clips or two audio clips, not a mix.");
  }
  if (!itemsAreAdjacent(left, right, frameSeconds)) {
    return invalid(`${label(left)} must end where ${label(right)} starts to add a transition.`);
  }
  return null;
}

function findInTimeline(timeline: Timeline, itemId: string): TimelineItem | undefined {
  for (const track of timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return item;
  }
  return undefined;
}

/**
 * Validates a transition against `track`: both items on the track, compatible kinds, adjacency, no
 * other transition on the cut, and duration bounds. Returns `null` when valid. Transition id
 * uniqueness is checked by the actions.
 */
export function validateTransition(
  project: VideoProject,
  track: TimelineTrack,
  transition: TimelineTransition,
): TransitionError | null {
  if (transition.leftItemId === transition.rightItemId) {
    return invalid("A transition needs two different clips.");
  }
  const onTrack = (itemId: string) => track.items.find((item) => item.id === itemId);
  const left = onTrack(transition.leftItemId);
  const right = onTrack(transition.rightItemId);
  if (!left || !right) {
    const anywhere = (itemId: string) => onTrack(itemId) ?? findInTimeline(project.timeline, itemId);
    const leftAnywhere = anywhere(transition.leftItemId);
    if (!leftAnywhere) return itemNotFound(transition.leftItemId);
    const rightAnywhere = anywhere(transition.rightItemId);
    if (!rightAnywhere) return itemNotFound(transition.rightItemId);
    return invalid(
      `${label(leftAnywhere)} and ${label(rightAnywhere)} must be on the same track to add a transition.`,
    );
  }
  return validateTransitionItems(project, track, transition, left, right);
}

function itemNotFound(itemId: string): TransitionError {
  return { code: "itemNotFound", message: `timeline item was not found: ${itemId}` };
}

function validateTransitionItems(
  project: VideoProject,
  track: TimelineTrack,
  transition: TimelineTransition,
  left: TimelineItem,
  right: TimelineItem,
): TransitionError | null {
  const frame = transitionFrameSeconds(project);
  const pairError = transitionPairError(left, right, frame);
  if (pairError) return pairError;
  if (
    (track.transitions ?? []).some(
      (other) =>
        other.id !== transition.id && (other.leftItemId === left.id || other.rightItemId === right.id),
    )
  ) {
    return invalid(`${label(left)} and ${label(right)} already have a transition.`);
  }

  const duration = transition.durationSeconds;
  if (!Number.isFinite(duration) || duration < frame - TRANSITION_SECONDS_EPSILON) {
    return invalid(`Transition duration must be at least one frame (${formatTransitionSeconds(frame)}s).`);
  }
  if (duration > MAX_TRANSITION_SECONDS + TRANSITION_SECONDS_EPSILON) {
    return invalid(`Transition duration cannot be longer than ${formatTransitionSeconds(MAX_TRANSITION_SECONDS)}s.`);
  }
  const bounds = transitionBounds(project, left, right);
  if (duration <= bounds.maxSeconds + TRANSITION_SECONDS_EPSILON) return null;
  const tail = `for a ${formatTransitionSeconds(duration)}s transition. Maximum is ${formatTransitionMaxSeconds(bounds.maxSeconds)}s.`;
  switch (bounds.limit) {
    case "leftHandle":
      return invalid(`Not enough unused media after ${label(left)} ${tail}`);
    case "rightHandle":
      return invalid(`Not enough unused media before ${label(right)} ${tail}`);
    case "leftDuration":
    case "cap":
      return invalid(`${label(left)} is too short ${tail}`);
    case "rightDuration":
      return invalid(`${label(right)} is too short ${tail}`);
  }
}

function editableTrackIndex(project: VideoProject, trackId: string): number | TransitionError {
  const index = project.timeline.tracks.findIndex((track) => track.id === trackId);
  if (index < 0) return { code: "trackNotFound", message: `timeline track was not found: ${trackId}` };
  if (project.timeline.tracks[index]?.locked) {
    return { code: "trackLocked", message: `track is locked: ${trackId}` };
  }
  return index;
}

function transitionNotFound(transitionId: string): TransitionError {
  return { code: "transitionNotFound", message: `transition was not found: ${transitionId}` };
}

function replaceTrackTransitions(
  tracks: readonly TimelineTrack[],
  trackIndex: number,
  transitions: TimelineTransition[],
): TimelineTrack[] {
  return tracks.map((track, index) => {
    if (index !== trackIndex) return track;
    const { transitions: _previous, ...rest } = track;
    return transitions.length > 0 ? { ...rest, transitions } : rest;
  });
}

/**
 * Applies `addTransition`, `updateTransition` or `removeTransition` to the active timeline. Returns
 * the next tracks, or the error the Rust action reports; the project is never mutated.
 */
export function applyTransitionAction(
  project: VideoProject,
  action: TransitionAction,
): { readonly tracks: TimelineTrack[] } | { readonly error: TransitionError } {
  const trackIndex = editableTrackIndex(project, action.trackId);
  if (typeof trackIndex !== "number") return { error: trackIndex };
  const tracks = project.timeline.tracks;
  const track = tracks[trackIndex]!;
  const transitions = track.transitions ?? [];

  if (action.type === "addTransition") {
    const { transition } = action;
    if (transition.id.trim() === "") return { error: invalid("Transition id cannot be empty.") };
    if (tracks.some((candidate) => (candidate.transitions ?? []).some((existing) => existing.id === transition.id))) {
      return { error: invalid(`A transition with id ${transition.id} already exists.`) };
    }
    const error = validateTransition(project, track, transition);
    return error ? { error } : { tracks: replaceTrackTransitions(tracks, trackIndex, [...transitions, transition]) };
  }

  const index = transitions.findIndex((transition) => transition.id === action.transitionId);
  const existing = transitions[index];
  if (!existing) return { error: transitionNotFound(action.transitionId) };
  if (action.type === "removeTransition") {
    return { tracks: replaceTrackTransitions(tracks, trackIndex, transitions.filter((_, i) => i !== index)) };
  }
  const updated: TimelineTransition = {
    ...existing,
    ...(action.kind !== undefined ? { kind: action.kind } : {}),
    ...(action.durationSeconds !== undefined ? { durationSeconds: action.durationSeconds } : {}),
  };
  const error = validateTransition(project, track, updated);
  return error
    ? { error }
    : { tracks: replaceTrackTransitions(tracks, trackIndex, transitions.map((t, i) => (i === index ? updated : t))) };
}

/** An adjacent pair of clips on a track that can take a transition. */
export interface TransitionCut {
  readonly trackId: string;
  readonly leftItemId: string;
  readonly rightItemId: string;
  /** The cut a transition centers on: the right clip's start. */
  readonly seconds: number;
  /** The transition already stored on exactly this pair, if any. */
  readonly transitionId: string | null;
}

/**
 * Adjacent pairs on `track`, in timeline order, that pass the structural transition checks: eligible
 * kinds and sources, no visual/audio mix, and a gap of at most one frame. Handles are not checked.
 */
export function cutsOnTrack(track: TimelineTrack, frameSeconds = 1 / 24): TransitionCut[] {
  const items = [...track.items].sort((left, right) => left.startSeconds - right.startSeconds);
  const cuts: TransitionCut[] = [];
  for (let index = 1; index < items.length; index += 1) {
    const left = items[index - 1]!;
    const right = items[index]!;
    if (transitionPairError(left, right, frameSeconds)) continue;
    const existing = (track.transitions ?? []).find(
      (transition) => transition.leftItemId === left.id && transition.rightItemId === right.id,
    );
    cuts.push({
      trackId: track.id,
      leftItemId: left.id,
      rightItemId: right.id,
      seconds: right.startSeconds,
      transitionId: existing?.id ?? null,
    });
  }
  return cuts;
}

/** The cut closest to `seconds`, optionally on one track; the earliest track and cut win ties. */
export function nearestCut(
  timeline: Timeline,
  seconds: number,
  trackId?: string,
  frameSeconds = 1 / 24,
): TransitionCut | null {
  let nearest: TransitionCut | null = null;
  for (const track of timeline.tracks) {
    if (trackId !== undefined && track.id !== trackId) continue;
    for (const cut of cutsOnTrack(track, frameSeconds)) {
      if (!nearest || Math.abs(cut.seconds - seconds) < Math.abs(nearest.seconds - seconds)) nearest = cut;
    }
  }
  return nearest;
}

/**
 * The timeline span a transition covers, centered on its cut (the right clip's start), or `null`
 * when the right clip is not on `track`.
 */
export function transitionWindow(
  transition: TimelineTransition,
  track: TimelineTrack,
): { readonly start: number; readonly end: number } | null {
  const right = track.items.find((item) => item.id === transition.rightItemId);
  if (!right) return null;
  const half = transition.durationSeconds / 2;
  return { start: right.startSeconds - half, end: right.startSeconds + half };
}
