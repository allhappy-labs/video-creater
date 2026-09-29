import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectAction, ProjectActionKeyframe, ProjectActionKeyframeProperty } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty } from "@/lib/timeline-ops/item-properties";
import {
  audioKeyframePropertyConfigs,
  inspectorKeyframesByProperty,
  sortedKeyframes,
  suggestedValue,
  visualKeyframePropertyConfigs,
  type KeyframePropertyConfig,
} from "@/lib/timeline-ops/keyframes";

/** Keyframe properties that Properties controls can animate (the ones with lane configs). */
export type AnimatableProperty = Exclude<ProjectActionKeyframeProperty, "scaleX" | "scaleY">;

export interface KeyframeState {
  /** The property has a keyframe lane, so value edits upsert instead of setting the static value. */
  readonly keyframed: boolean;
  /** Item-relative playhead the ◇ button and value edits use. */
  readonly localSeconds: number;
  readonly atPlayhead: ProjectActionKeyframe | null;
  /** Value shown at the playhead: interpolated from keyframes, else the static value or default. */
  readonly value: number;
}

const keyframeTimeToleranceSeconds = 0.000_5;

/** Legacy clip-local playhead: clamp(playhead - start, 0, duration), rounded like the lane editor. */
export function clipLocalPlayhead(item: TimelineItem, playheadSeconds: number): number {
  if (!Number.isFinite(playheadSeconds)) return 0;
  const local = Math.min(Math.max(playheadSeconds - item.startSeconds, 0), Math.max(item.durationSeconds, 0));
  return roundTimelineSeconds(local);
}

function propertyConfig(item: TimelineItem, property: AnimatableProperty): KeyframePropertyConfig {
  const config = [...visualKeyframePropertyConfigs, ...audioKeyframePropertyConfigs].find(
    (candidate) => candidate.property === property,
  );
  if (!config) throw new Error(`No keyframe configuration for ${property}.`);
  return { ...config, defaultValue: numberProperty(item, property) ?? config.defaultValue };
}

export function keyframeStateAtPlayhead(
  item: TimelineItem,
  property: AnimatableProperty,
  playheadSeconds: number,
): KeyframeState {
  const lane = inspectorKeyframesByProperty(item)[property];
  const keyframes = sortedKeyframes(lane);
  const localSeconds = clipLocalPlayhead(item, playheadSeconds);
  return {
    // Legacy `visualItemHasKeyframes`: a stored lane array counts, even before it has points.
    keyframed: lane !== undefined,
    localSeconds,
    atPlayhead:
      keyframes.find((keyframe) => Math.abs(keyframe.atSeconds - localSeconds) <= keyframeTimeToleranceSeconds) ??
      null,
    value: suggestedValue(propertyConfig(item, property), keyframes, localSeconds),
  };
}

/**
 * The ◇ button: removes the keyframe at the playhead when there is one, otherwise adds one at
 * the item-relative playhead with `value` (or the current value), clamped and rounded like the
 * legacy lane editor's "Add at playhead".
 */
export function toggleKeyframeAction(
  item: TimelineItem,
  property: AnimatableProperty,
  playheadSeconds: number,
  value?: number,
): ProjectAction {
  const state = keyframeStateAtPlayhead(item, property, playheadSeconds);
  if (state.atPlayhead) {
    return { type: "deleteItemKeyframe", itemId: item.id, property, atSeconds: state.atPlayhead.atSeconds };
  }
  const config = propertyConfig(item, property);
  const clamped = Math.min(Math.max(value ?? state.value, config.minimum), config.maximum);
  return {
    type: "upsertItemKeyframe",
    itemId: item.id,
    property,
    keyframe: { atSeconds: state.localSeconds, value: roundTimelineSeconds(clamped), easing: "linear" },
  };
}

/**
 * A value edit on a keyframed property upserts at the playhead (keeping the easing of a keyframe
 * already there). Returns null when the property is not keyframed, so the caller sets the
 * static value instead.
 */
export function keyframedEditAction(
  item: TimelineItem,
  property: AnimatableProperty,
  playheadSeconds: number,
  value: number,
): ProjectAction | null {
  const state = keyframeStateAtPlayhead(item, property, playheadSeconds);
  if (!state.keyframed) return null;
  const easing = state.atPlayhead?.easing;
  return {
    type: "upsertItemKeyframe",
    itemId: item.id,
    property,
    keyframe: {
      atSeconds: state.atPlayhead?.atSeconds ?? state.localSeconds,
      value,
      ...(easing ? { easing } : {}),
    },
  };
}
