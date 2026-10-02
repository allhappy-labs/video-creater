import type { TimelineItem, TimelineTrack, TimelineTransition, TransitionKind } from "@/lib/timeline";
import {
  TRANSITION_SECONDS_EPSILON,
  transitionMaxDuration,
  transitionPairError,
  type TransitionMediaSources,
} from "@/lib/timeline-ops/transitions";
import { handleSourceSeconds, isReversedItem } from "@/lib/timeline-ops/reverse";

/**
 * Clip transitions in the editor preview. The canonical timeline never overlaps the two clips of a
 * transition; like the render plan (`src-tauri/src/render_pipeline/transition_plan.rs`), the preview
 * extends both clips into their unused media across the window `[cut - d/2, cut + d/2)`, where the
 * cut is the right clip's start:
 *
 * - the left (outgoing) clip stays active `d/2` past its canonical end, sampling source
 *   `sourceOut + (t - leftEnd) * speed`;
 * - the right (incoming) clip is active `d/2` before its canonical start, sampling source
 *   `sourceIn - (rightStart - t) * speed`.
 *
 * Fades, opacity and motion keyframes stay aligned to the canonical item: keyframes hold their edge
 * values in the handles, and fades clamp to [0, 1] (a fade-in reads 0 before the canonical start, a
 * fade-out reads 0 after the canonical end). Transition opacity multiplies with them.
 *
 * Progress is `p = (t - windowStart) / d`, 0 at the window start. At the window end only the right
 * clip remains, at its canonical start. Per kind, with the outgoing clip drawn beneath the incoming
 * clip in the track's z-slot:
 *
 * - `crossfade`: outgoing opacity 1, incoming opacity `p` on top. For opaque layers this is exactly
 *   the linear mix `(1 - p) * outgoing + p * incoming`.
 * - `dipToBlack` / `dipToWhite`: an opaque black or white solid beneath both clips for the whole
 *   window; outgoing opacity `max(0, 1 - 2p)`, incoming opacity `max(0, 2p - 1)`. Both are 0 at the
 *   cut, so the solid shows alone there.
 * - `wipe`: outgoing opacity 1, incoming opacity 1 on top, clipped in canvas space to the left `p`
 *   of the output frame (`clip-path: inset(0 (1 - p) * 100% 0 0)`), revealing left to right.
 *
 * Audio pairs use an equal-power crossfade: outgoing gain `cos(p * pi / 2)`, incoming `sin(p * pi / 2)`.
 *
 * Transitions are rechecked like the render plan: the pair must be on the same enabled track of
 * matching kind (visual clips on a video track, audio clips on an audio track), source-backed and
 * adjacent within a frame; the duration clamps to the remaining handles; and a clip end extended by
 * one transition is not extended again.
 */

/** Keeps extended source ranges inside the media, like the render plan's `HANDLE_MARGIN_SECONDS`. */
const HANDLE_MARGIN_SECONDS = 1e-9;

type PreviewTransitionRole = "outgoing" | "incoming";

/** A transition the render would draw on a track, with its duration clamped to the handles. */
interface PlannedPreviewTransition {
  readonly id: string;
  readonly kind: TransitionKind;
  readonly leftItemId: string;
  readonly rightItemId: string;
  readonly audio: boolean;
  readonly startSeconds: number;
  readonly durationSeconds: number;
}

/** The transitions that extend one clip: `head` before its canonical start, `tail` after its end. */
export interface PreviewItemTransitions {
  readonly head: PlannedPreviewTransition | null;
  readonly tail: PlannedPreviewTransition | null;
}

/** A transition drawn at the playhead. */
export interface TimelinePreviewTransition {
  readonly transitionId: string;
  readonly kind: TransitionKind;
  readonly leftItemId: string;
  readonly rightItemId: string;
  readonly startSeconds: number;
  readonly durationSeconds: number;
  readonly progress: number;
  /** The opaque solid drawn beneath both clips for dips, otherwise null. */
  readonly solidColor: "black" | "white" | null;
  /**
   * Present when canonical preparation bakes the transition into a flattened composite, whose
   * prepared frames then draw both clips and the solid (see `transition-eligibility.ts`).
   */
  readonly flattened?: true;
}

/** How a visual layer takes part in a transition at the playhead. */
export interface TimelinePreviewLayerTransition {
  readonly transitionId: string;
  readonly kind: TransitionKind;
  readonly role: PreviewTransitionRole;
  readonly progress: number;
  /** The transition's opacity factor, already multiplied into the layer opacity. */
  readonly opacity: number;
  /** Wipe only: the fraction of the canvas width hidden from the right. */
  readonly wipeInsetRight?: number;
}

/** How an audio layer takes part in a transition at the playhead. */
export interface TimelinePreviewAudioTransition {
  readonly transitionId: string;
  readonly role: PreviewTransitionRole;
  readonly progress: number;
  /** The equal-power gain factor, already multiplied into the layer gain. */
  readonly gain: number;
}

const visualItemKinds: ReadonlySet<TimelineItem["kind"]> = new Set(["video_clip", "image_clip", "generated_clip"]);

function planTransition(
  sources: TransitionMediaSources,
  track: TimelineTrack,
  transition: TimelineTransition,
  frameSeconds: number,
  itemsById: ReadonlyMap<string, TimelineItem>,
): PlannedPreviewTransition | null {
  const left = itemsById.get(transition.leftItemId);
  const right = itemsById.get(transition.rightItemId);
  if (!left || !right || left.id === right.id || transitionPairError(left, right, frameSeconds)) return null;
  const audio = left.kind === "audio_clip";
  if (audio ? track.kind !== "audio" : track.kind !== "video" || !visualItemKinds.has(left.kind)) return null;
  const maxSeconds = transitionMaxDuration(sources, left, right) - HANDLE_MARGIN_SECONDS;
  const durationSeconds = Math.min(transition.durationSeconds, maxSeconds);
  if (!Number.isFinite(durationSeconds) || durationSeconds < frameSeconds - TRANSITION_SECONDS_EPSILON) return null;
  return {
    id: transition.id,
    kind: transition.kind,
    leftItemId: left.id,
    rightItemId: right.id,
    audio,
    startSeconds: right.startSeconds - durationSeconds / 2,
    durationSeconds,
  };
}

/**
 * The drawable transitions of `track`, keyed by the clips they extend. Empty for tracks without any.
 * `droppedTransitionIds` are transitions render preparation removes; they neither draw nor claim a
 * clip end.
 */
export function planTrackTransitions(
  track: TimelineTrack,
  sources: TransitionMediaSources,
  frameSeconds: number,
  droppedTransitionIds: ReadonlySet<string> = new Set(),
): ReadonlyMap<string, PreviewItemTransitions> {
  const byItem = new Map<string, PreviewItemTransitions>();
  if (track.enabled === false || !track.transitions?.length) return byItem;
  const itemsById = new Map<string, TimelineItem>();
  // Preserve find()'s first-match behavior even for malformed duplicate IDs.
  for (const item of track.items) if (!itemsById.has(item.id)) itemsById.set(item.id, item);
  for (const transition of track.transitions) {
    if (droppedTransitionIds.has(transition.id)) continue;
    const planned = planTransition(sources, track, transition, frameSeconds, itemsById);
    if (!planned) continue;
    const empty: PreviewItemTransitions = { head: null, tail: null };
    if (byItem.get(planned.leftItemId)?.tail || byItem.get(planned.rightItemId)?.head) continue;
    byItem.set(planned.leftItemId, { ...(byItem.get(planned.leftItemId) ?? empty), tail: planned });
    byItem.set(planned.rightItemId, { ...(byItem.get(planned.rightItemId) ?? empty), head: planned });
  }
  return byItem;
}

function windowContains(transition: PlannedPreviewTransition, seconds: number): boolean {
  return seconds >= transition.startSeconds && seconds < transition.startSeconds + transition.durationSeconds;
}

function progressAt(transition: PlannedPreviewTransition, seconds: number): number {
  return Math.max(0, Math.min(1, (seconds - transition.startSeconds) / transition.durationSeconds));
}

/** Whether `item` is drawn at `seconds`: its canonical span, extended by its transition handles. */
export function isItemActiveWithTransitions(
  item: TimelineItem,
  transitions: PreviewItemTransitions | undefined,
  seconds: number,
): boolean {
  const head = transitions?.head ? transitions.head.durationSeconds / 2 : 0;
  const tail = transitions?.tail ? transitions.tail.durationSeconds / 2 : 0;
  return seconds >= item.startSeconds - head && seconds < item.startSeconds + item.durationSeconds + tail;
}

/** The transition `item` takes part in at `seconds`, with its role and progress, or null. */
export function itemTransitionAt(
  transitions: PreviewItemTransitions | undefined,
  seconds: number,
): { readonly transition: PlannedPreviewTransition; readonly role: PreviewTransitionRole; readonly progress: number } | null {
  if (transitions?.head && windowContains(transitions.head, seconds)) {
    return { transition: transitions.head, role: "incoming", progress: progressAt(transitions.head, seconds) };
  }
  if (transitions?.tail && windowContains(transitions.tail, seconds)) {
    return { transition: transitions.tail, role: "outgoing", progress: progressAt(transitions.tail, seconds) };
  }
  return null;
}

/** The transitions of one track drawn at `seconds`, in timeline order, marking the flattened ones. */
export function activeTrackTransitions(
  byItem: ReadonlyMap<string, PreviewItemTransitions>,
  seconds: number,
  flattenedTransitionIds: ReadonlySet<string> = new Set(),
): TimelinePreviewTransition[] {
  const active: TimelinePreviewTransition[] = [];
  for (const { tail } of byItem.values()) {
    if (!tail || tail.audio || !windowContains(tail, seconds)) continue;
    active.push({
      transitionId: tail.id,
      kind: tail.kind,
      leftItemId: tail.leftItemId,
      rightItemId: tail.rightItemId,
      startSeconds: tail.startSeconds,
      durationSeconds: tail.durationSeconds,
      progress: progressAt(tail, seconds),
      solidColor: tail.kind === "dipToBlack" ? "black" : tail.kind === "dipToWhite" ? "white" : null,
      ...(flattenedTransitionIds.has(tail.id) ? { flattened: true as const } : {}),
    });
  }
  return active.sort((left, right) => left.startSeconds - right.startSeconds);
}

/** The visual transition state of a layer: the per-kind opacity factor and the wipe inset. */
export function visualLayerTransition(
  transition: Pick<PlannedPreviewTransition, "id" | "kind">,
  role: PreviewTransitionRole,
  progress: number,
): TimelinePreviewLayerTransition {
  const base = { transitionId: transition.id, kind: transition.kind, role, progress };
  switch (transition.kind) {
    case "crossfade":
      return { ...base, opacity: role === "incoming" ? progress : 1 };
    case "dipToBlack":
    case "dipToWhite":
      return { ...base, opacity: role === "incoming" ? Math.max(0, 2 * progress - 1) : Math.max(0, 1 - 2 * progress) };
    case "wipe":
      return role === "incoming" ? { ...base, opacity: 1, wipeInsetRight: 1 - progress } : { ...base, opacity: 1 };
  }
}

/** The equal-power audio transition state: `cos(p * pi / 2)` outgoing, `sin(p * pi / 2)` incoming. */
export function audioLayerTransition(
  transition: Pick<PlannedPreviewTransition, "id">,
  role: PreviewTransitionRole,
  progress: number,
): TimelinePreviewAudioTransition {
  const angle = (progress * Math.PI) / 2;
  return { transitionId: transition.id, role, progress, gain: role === "incoming" ? Math.sin(angle) : Math.cos(angle) };
}

/** The canvas-space clip path for a wiping layer, or undefined. */
export function transitionClipPath(transition: Pick<TimelinePreviewLayerTransition, "wipeInsetRight"> | undefined) {
  if (transition?.wipeInsetRight === undefined) return undefined;
  return `inset(0 ${Math.round(transition.wipeInsetRight * 100_000) / 1000}% 0 0)`;
}

function finiteNumber(item: TimelineItem, key: string): number | null {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/**
 * The source time of `item` at `seconds` inside a transition handle, clamped to the media, or null
 * outside the handles. The tail continues from `sourceOut`; the head runs back from `sourceIn` (mirrored
 * for reversed clips).
 */
export function transitionHandleSourceSeconds(
  item: TimelineItem,
  seconds: number,
  playbackSpeed: number,
  mediaDurationSeconds: number,
): number | null {
  const sourceIn = finiteNumber(item, "sourceIn") ?? 0;
  const end = item.startSeconds + item.durationSeconds;
  if (seconds >= item.startSeconds && seconds < end) return null;
  const sourceOut = finiteNumber(item, "sourceOut") ?? sourceIn + item.durationSeconds * playbackSpeed;
  const window = { sourceIn, sourceOut, speed: playbackSpeed, reverse: isReversedItem(item) };
  const source = seconds < item.startSeconds
    ? handleSourceSeconds(window, "head", item.startSeconds - seconds)
    : handleSourceSeconds(window, "tail", seconds - end);
  const max = mediaDurationSeconds > 0 ? mediaDurationSeconds : Number.POSITIVE_INFINITY;
  return Math.max(0, Math.min(source, max));
}

/**
 * `track`'s transitions carried onto its nested-expanded copy: ids gain the expansion namespace
 * like the items, and durations are retimed by the nested playback speed. Transitions whose clips
 * were dropped are skipped. Mirrors Rust `carry_transitions_through_expansion`.
 */
export function carryTransitionsThroughExpansion(
  track: TimelineTrack,
  expandedItems: readonly TimelineItem[],
  namespace: string,
  playbackSpeed: number,
): TimelineTransition[] {
  if (!track.transitions?.length) return [];
  const presentIds = new Set(expandedItems.map((item) => item.id));
  const expandedId = (id: string) => (namespace === "root" ? id : `${namespace}:${id}`);
  return (track.transitions ?? []).flatMap((transition) => {
    const leftItemId = expandedId(transition.leftItemId);
    const rightItemId = expandedId(transition.rightItemId);
    if (!presentIds.has(leftItemId) || !presentIds.has(rightItemId)) return [];
    return [
      {
        id: expandedId(transition.id),
        leftItemId,
        rightItemId,
        kind: transition.kind,
        durationSeconds: transition.durationSeconds / playbackSpeed,
      },
    ];
  });
}
