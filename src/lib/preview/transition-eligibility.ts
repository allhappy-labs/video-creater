import type { MediaAsset } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack } from "@/lib/timeline";
import type { TransitionMediaSources } from "@/lib/timeline-ops/transitions";
import { planTrackTransitions, type PreviewItemTransitions } from "./transition-frame";

/**
 * Which clip transitions the render can draw, so the editor preview skips the ones it cannot. Render
 * preparation (`prepare_project_for_render_cancellable` in `src-tauri/src/precompose/mod.rs`) runs
 * before the render plan (`plan_clip_transitions` in `src-tauri/src/render_pipeline/transition_plan.rs`)
 * and removes some transitions from the prepared project:
 *
 * - Denoised audio (`audio_denoise.rs` `collect_tasks` / `denoise_amount`): an audio clip with an
 *   enabled `audio.denoise` effect renders from an intermediate covering only the clip, so it has no
 *   handles and `plan_clip_transitions` drops its transitions. They are `dropped`.
 * - Flattened composites (`flatten.rs` `plan_groups` / `rich_items` / `rewrite_dependent_intervals`
 *   and `flatten_transitions.rs`): clips with a richer blend or prepared effects (`richer_blend`,
 *   `has_prepared_effects`, read after LUT preparation bakes the LUT and the effects before it) are
 *   composited with every enabled video track beneath them. Groups cover the rich clips' spans,
 *   extended by their flattenable transitions (`FlattenTransitions::plan`), and widen over every
 *   flattenable transition window they touch (`widen_groups_over_transitions`). A transition is
 *   flattenable when both clips are media-backed video (or Lottie, prepared to video first) on a
 *   video track (`pair_is_flattenable`). Clips beneath a group are split around it, and a transition
 *   survives only when its outgoing clip kept its end and its incoming clip kept its start
 *   (`retarget_split_transitions`). A removed flattenable transition is baked into the composite
 *   (`flattened`: the preview still draws it, and the prepared frame covers both clips and the dip
 *   solid); any other removed transition is `dropped` and the render hard-cuts.
 *
 * LUT and Lottie prepared sources include their clips' transition handles, so they keep their
 * transitions. Transition ids are namespaced by nested expansion, so ids identify transitions across
 * the expanded timeline.
 */
export interface TransitionEligibility {
  /** Transitions the render never draws: the preview cuts instead. */
  readonly droppedTransitionIds: ReadonlySet<string>;
  /** Transitions baked into a flattened composite, drawn by its prepared frames. */
  readonly flattenedTransitionIds: ReadonlySet<string>;
}

const GROUP_EPSILON_SECONDS = 1e-9;
const emptyIds: ReadonlySet<string> = new Set();

/** Mirrors Rust `richer_blend`: blend modes only a flattened composite can draw. */
const richerBlendModes: ReadonlySet<string> = new Set([
  "source", "add", "darken", "multiply", "colorBurn", "lighten", "screen", "colorDodge", "overlay",
  "softLight", "hardLight", "difference", "exclusion", "hue", "saturation", "color", "luminosity",
]);

/** Mirrors Rust `effects::canonical_effect_order`. */
const canonicalEffectOrder: readonly string[] = [
  "color.exposure", "color.contrast", "color.highlightsShadows", "color.blacksWhites", "color.temperature",
  "color.vibrance", "color.saturation", "color.wheels", "color.curves", "color.hueCurves", "color.lut",
  "detail.clarity", "key.chroma", "blur.gaussian", "blur.sharpen", "blur.noiseReduction", "audio.denoise",
  "blur.motion", "stylize.grain", "stylize.vignette", "stylize.glow",
];
const lutEffectRank = canonicalEffectOrder.indexOf("color.lut");

type PlannedTransition = NonNullable<PreviewItemTransitions["tail"]>;

interface RichSpan {
  readonly trackIndex: number;
  readonly itemId: string;
  readonly start: number;
  readonly end: number;
}

interface FlattenGroup {
  start: number;
  end: number;
  topTrackIndex: number;
  topItemId: string;
}

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function effectsOf(item: TimelineItem): readonly unknown[] | null {
  return Array.isArray(item.properties.effects) ? item.properties.effects : null;
}

/** Mirrors Rust `denoise_amount`: an effect typed `audio.denoise` (or legacy `type`) not disabled. */
function usesDenoisedIntermediate(item: TimelineItem): boolean {
  return item.kind === "audio_clip" && (effectsOf(item) ?? []).some((effect) => {
    const fields = record(effect);
    if (!fields) return false;
    const kind = "effectType" in fields ? fields.effectType : fields.type;
    return kind === "audio.denoise" && fields.enabled !== false;
  });
}

/** Mirrors the LUT reason of Rust `classify_item`, for media LUT preparation accepts. */
function hasPreparedLut(item: TimelineItem, mediaKind: MediaAsset["kind"] | undefined): boolean {
  if (mediaKind !== "video" && mediaKind !== "lottie") return false;
  const grade = record(item.properties.colorGrade);
  if (grade && grade.lut !== undefined && grade.lut !== null) return true;
  return (effectsOf(item) ?? []).some((effect) => {
    const fields = record(effect);
    return fields?.effectType === "color.lut" && fields.enabled !== false;
  });
}

/** Mirrors Rust `rich_items`: `richer_blend` or `has_prepared_effects` after LUT preparation. */
function isRichItem(item: TimelineItem, mediaById: ReadonlyMap<string, MediaAsset>): boolean {
  const blendMode = item.properties.blendMode;
  if (typeof blendMode === "string" && richerBlendModes.has(blendMode)) return true;
  const mediaKind = item.source.type === "media" ? mediaById.get(item.source.mediaId)?.kind : undefined;
  const lutBaked = item.source.type === "media" && hasPreparedLut(item, mediaKind);
  const remaining = (effectsOf(item) ?? []).filter((effect) => {
    if (!lutBaked) return true;
    const effectType = record(effect)?.effectType;
    if (typeof effectType !== "string") return true;
    const rank = canonicalEffectOrder.indexOf(effectType);
    return rank === -1 || rank > lutEffectRank;
  });
  if (remaining.some((effect) => record(effect)?.enabled !== false)) return true;
  const grade = lutBaked ? null : record(item.properties.colorGrade);
  return grade !== null && Object.keys(grade).some((key) => key !== "lut");
}

/** Mirrors Rust `pair_is_flattenable`. */
function isFlattenablePair(track: TimelineTrack, planned: PlannedTransition, mediaById: ReadonlyMap<string, MediaAsset>): boolean {
  if (track.kind !== "video") return false;
  return [planned.leftItemId, planned.rightItemId].every((id) => {
    const source = track.items.find((item) => item.id === id)?.source;
    const kind = source?.type === "media" ? mediaById.get(source.mediaId)?.kind : undefined;
    return kind === "video" || kind === "lottie";
  });
}

/** A clip on a track, like Rust `ClipKey`; the numeric index cannot contain the separator. */
function itemKey(trackIndex: number, itemId: string): string {
  return `${trackIndex}/${itemId}`;
}

function windowEnd(planned: PlannedTransition): number {
  return planned.startSeconds + planned.durationSeconds;
}

/** Mirrors Rust `plan_groups` before widening: one group per run of the topmost active rich clip. */
function planGroups(rich: readonly RichSpan[]): FlattenGroup[] {
  const times = rich.flatMap(({ start, end }) => [start, end]).sort((left, right) => left - right);
  const groups: FlattenGroup[] = [];
  let previousTime: number | undefined;
  for (const end of times) {
    const start = previousTime;
    if (start === end) continue;
    previousTime = end;
    if (start === undefined || !Number.isFinite(start) || !Number.isFinite(end) || end <= start) continue;
    let top: RichSpan | undefined;
    for (const span of rich) {
      if (span.start <= start && span.end > start && (!top || span.trackIndex >= top.trackIndex)) top = span;
    }
    if (!top) continue;
    const previous = groups[groups.length - 1];
    if (previous && previous.topTrackIndex === top.trackIndex && previous.topItemId === top.itemId && Math.abs(previous.end - start) <= GROUP_EPSILON_SECONDS) {
      previous.end = end;
      continue;
    }
    groups.push({ start, end, topTrackIndex: top.trackIndex, topItemId: top.itemId });
  }
  return groups;
}

/** Mirrors Rust `widen_groups_over_transitions`. */
function widenGroups(initial: FlattenGroup[], windows: readonly { readonly trackIndex: number; readonly planned: PlannedTransition }[]): FlattenGroup[] {
  let groups = initial;
  if (windows.length === 0) return groups;
  for (;;) {
    let changed = false;
    for (const group of groups) {
      for (const { trackIndex, planned } of windows) {
        if (trackIndex > group.topTrackIndex) continue;
        const start = planned.startSeconds;
        const end = windowEnd(planned);
        if (start < group.end - GROUP_EPSILON_SECONDS && end > group.start + GROUP_EPSILON_SECONDS && (start < group.start || end > group.end)) {
          group.start = Math.min(group.start, start);
          group.end = Math.max(group.end, end);
          changed = true;
        }
      }
    }
    const merged: FlattenGroup[] = [];
    for (const group of [...groups].sort((left, right) => left.start - right.start)) {
      const previous = merged[merged.length - 1];
      if (previous && group.start < previous.end - GROUP_EPSILON_SECONDS) {
        previous.end = Math.max(previous.end, group.end);
        if (group.topTrackIndex > previous.topTrackIndex) {
          previous.topTrackIndex = group.topTrackIndex;
          previous.topItemId = group.topItemId;
        }
        changed = true;
      } else {
        merged.push(group);
      }
    }
    groups = merged;
    if (!changed) return groups;
  }
}

/**
 * Mirrors Rust `split_item_outside_intervals` and the kept edges of `rewrite_dependent_intervals`:
 * whether the segments left outside `intervals` keep the item's start and end. Null when unsplit.
 */
function keptEdges(item: TimelineItem, intervals: readonly (readonly [number, number])[]): { start: boolean; end: boolean } | null {
  const itemStart = item.startSeconds;
  const itemEnd = item.startSeconds + item.durationSeconds;
  let remaining: [number, number][] = [[itemStart, itemEnd]];
  for (const [cutStart, cutEnd] of intervals) {
    remaining = remaining.flatMap(([start, end]): [number, number][] => {
      if (cutEnd <= start || cutStart >= end) return [[start, end]];
      return [
        ...(cutStart > start ? [[start, Math.min(cutStart, end)] as [number, number]] : []),
        ...(cutEnd < end ? [[Math.max(cutEnd, start), end] as [number, number]] : []),
      ];
    });
  }
  const [only] = remaining;
  if (remaining.length === 1 && only && Math.abs(only[0] - itemStart) <= GROUP_EPSILON_SECONDS && Math.abs(only[1] - itemEnd) <= GROUP_EPSILON_SECONDS) {
    return null;
  }
  const segments = remaining.filter(([start, end]) => end - start > GROUP_EPSILON_SECONDS);
  const first = segments[0];
  const last = segments[segments.length - 1];
  return {
    start: first !== undefined && Math.abs(first[0] - itemStart) <= GROUP_EPSILON_SECONDS,
    end: last !== undefined && Math.abs(last[1] - itemEnd) <= GROUP_EPSILON_SECONDS,
  };
}

/** A flattened composite's span and the topmost track it composites, like Rust `FlattenGroup`. */
export interface FlattenGroupSpan {
  readonly start: number;
  readonly end: number;
  readonly topTrackIndex: number;
}

interface FlattenPlan {
  readonly groups: readonly FlattenGroup[];
  readonly flattenableIds: ReadonlySet<string>;
}

/**
 * Mirrors Rust `plan_groups` and `widen_groups_over_transitions` for the nested-expanded `timeline`:
 * the flattened composite groups and the flattenable transitions that widened them.
 */
function flattenGroups(timeline: Timeline, sources: TransitionMediaSources, frameSeconds: number): FlattenPlan {
  const tracks = timeline.tracks;
  const mediaById = new Map(sources.media.map((asset) => [asset.id, asset]));
  const hasRichItem = tracks.some((track) => track.enabled !== false && track.kind === "video" && track.items.some((item) => isRichItem(item, mediaById)));
  if (!hasRichItem) return { groups: [], flattenableIds: emptyIds };
  const flattenable: { readonly trackIndex: number; readonly planned: PlannedTransition }[] = [];
  const flattenableByItem = new Map<string, { head: number; tail: number }>();
  tracks.forEach((track, trackIndex) => {
    for (const { tail } of planTrackTransitions(track, sources, frameSeconds).values()) {
      if (!tail || tail.audio || !isFlattenablePair(track, tail, mediaById)) continue;
      flattenable.push({ trackIndex, planned: tail });
      const half = tail.durationSeconds / 2;
      flattenableByItem.set(itemKey(trackIndex, tail.leftItemId), { head: flattenableByItem.get(itemKey(trackIndex, tail.leftItemId))?.head ?? 0, tail: half });
      flattenableByItem.set(itemKey(trackIndex, tail.rightItemId), { head: half, tail: flattenableByItem.get(itemKey(trackIndex, tail.rightItemId))?.tail ?? 0 });
    }
  });

  const rich: RichSpan[] = tracks.flatMap((track, trackIndex) =>
    track.enabled === false || track.kind !== "video"
      ? []
      : track.items.filter((item) => isRichItem(item, mediaById)).map((item) => {
          const handles = flattenableByItem.get(itemKey(trackIndex, item.id));
          return {
            trackIndex,
            itemId: item.id,
            start: item.startSeconds - (handles?.head ?? 0),
            end: item.startSeconds + item.durationSeconds + (handles?.tail ?? 0),
          };
        }),
  );
  return { groups: widenGroups(planGroups(rich), flattenable), flattenableIds: new Set(flattenable.map(({ planned }) => planned.id)) };
}

/**
 * The flattened composite groups active at `playheadSeconds`. Rust `collect_dependencies` composites
 * every enabled video track at or below `topTrackIndex` inside the group, so the group's prepared
 * frames stand in for those clips even when no transition is involved.
 */
export function flattenGroupsAt(timeline: Timeline, sources: TransitionMediaSources, frameSeconds: number, playheadSeconds: number): FlattenGroupSpan[] {
  return flattenGroups(timeline, sources, frameSeconds)
    .groups.filter((group) => playheadSeconds >= group.start && playheadSeconds < group.end)
    .map(({ start, end, topTrackIndex }) => ({ start, end, topTrackIndex }));
}

/** The transitions of the nested-expanded `timeline` the render drops or bakes into flattened composites. */
export function transitionEligibility(timeline: Timeline, sources: TransitionMediaSources, frameSeconds: number): TransitionEligibility {
  return planPreviewTransitionEligibility(timeline, sources, frameSeconds).eligibility;
}

/** Calculate the playhead-independent eligibility and flattened spans together, once per plan. */
export function planPreviewTransitionEligibility(timeline: Timeline, sources: TransitionMediaSources, frameSeconds: number): {
  eligibility: TransitionEligibility;
  flattenedGroups: readonly FlattenGroupSpan[];
} {
  const flattenPlan = flattenGroups(timeline, sources, frameSeconds);
  return {
    eligibility: eligibilityFromFlattenPlan(timeline, flattenPlan),
    flattenedGroups: flattenPlan.groups.map(({ start, end, topTrackIndex }) => ({ start, end, topTrackIndex })),
  };
}

function eligibilityFromFlattenPlan(timeline: Timeline, { groups, flattenableIds }: FlattenPlan): TransitionEligibility {
  const tracks = timeline.tracks;
  if (!tracks.some((track) => track.enabled !== false && track.transitions?.length)) {
    return { droppedTransitionIds: emptyIds, flattenedTransitionIds: emptyIds };
  }
  const dropped = new Set<string>();
  const flattened = new Set<string>();
  for (const track of tracks) {
    const denoisedIds = new Set(track.items.filter(usesDenoisedIntermediate).map((item) => item.id));
    for (const transition of track.transitions ?? []) {
      const denoised = denoisedIds.has(transition.leftItemId) || denoisedIds.has(transition.rightItemId);
      if (denoised) dropped.add(transition.id);
    }
  }
  tracks.forEach((track, trackIndex) => {
    if (track.enabled === false || track.kind !== "video" || !track.transitions?.length) return;
    const intervals = groups.filter((group) => trackIndex <= group.topTrackIndex).map((group) => [group.start, group.end] as const);
    if (intervals.length === 0) return;
    const edges = new Map(track.items.map((item) => [item.id, keptEdges(item, intervals)]));
    for (const transition of track.transitions) {
      const leftKeptEnd = edges.get(transition.leftItemId)?.end ?? true;
      const rightKeptStart = edges.get(transition.rightItemId)?.start ?? true;
      if (leftKeptEnd && rightKeptStart) continue;
      (flattenableIds.has(transition.id) ? flattened : dropped).add(transition.id);
    }
  });
  return { droppedTransitionIds: dropped, flattenedTransitionIds: flattened };
}
