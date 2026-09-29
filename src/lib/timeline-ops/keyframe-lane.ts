import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectAction, ProjectActionKeyframe, ProjectActionKeyframeProperty } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import {
  inspectorKeyframesByProperty,
  keyframePropertyConfigsForItem,
  sortedKeyframes,
  suggestedValue,
  type KeyframePropertyConfig,
} from "./keyframes";

const timeStepSeconds = 0.1;
const largeTimeStepSeconds = 1;
const largeValueStepFactor = 10;

interface KeyframePoint {
  readonly atSeconds: number;
  readonly value: number;
}

export interface KeyframeLaneModel {
  readonly configs: readonly KeyframePropertyConfig[];
  readonly config: KeyframePropertyConfig;
  /** Keyframes of `config.property`, sorted by clip-local time. */
  readonly keyframes: readonly ProjectActionKeyframe[];
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}

/** The lane for a clip: its keyframe configs, the preferred property (else the first) and its keyframes. */
export function keyframeLaneFor(item: TimelineItem, preferred: ProjectActionKeyframeProperty): KeyframeLaneModel | null {
  const configs = keyframePropertyConfigsForItem(item);
  const config = configs.find((candidate) => candidate.property === preferred) ?? configs[0];
  if (!config) return null;
  return { configs, config, keyframes: sortedKeyframes(inspectorKeyframesByProperty(item)[config.property]) };
}

/** A keyframe moved by a drag: time clamped to the clip, value to the config bounds, both to 3 dp. */
export function draggedKeyframe(
  item: TimelineItem,
  config: KeyframePropertyConfig,
  from: KeyframePoint,
  deltaSeconds: number,
  deltaValue: number,
): KeyframePoint {
  return {
    atSeconds: roundTimelineSeconds(clamp(from.atSeconds + deltaSeconds, 0, item.durationSeconds)),
    value: roundTimelineSeconds(clamp(from.value + deltaValue, config.minimum, config.maximum)),
  };
}

/**
 * One batch for a keyframe edit: `moveItemKeyframe` when the time changed, then
 * `upsertItemKeyframe` at the new time when the value changed (the easing is kept).
 */
export function keyframeEditActions(
  itemId: string,
  property: ProjectActionKeyframeProperty,
  from: ProjectActionKeyframe,
  to: KeyframePoint,
): ProjectAction[] {
  const actions: ProjectAction[] = [];
  if (to.atSeconds !== from.atSeconds) {
    actions.push({ type: "moveItemKeyframe", itemId, property, fromSeconds: from.atSeconds, toSeconds: to.atSeconds });
  }
  if (to.value !== from.value) {
    const easing = from.easing ? { easing: from.easing } : {};
    actions.push({ type: "upsertItemKeyframe", itemId, property, keyframe: { atSeconds: to.atSeconds, value: to.value, ...easing } });
  }
  return actions;
}

/**
 * Keyboard editing of a focused diamond: Delete or Backspace removes it; Arrow Left/Right
 * move it by 0.1 s (1 s with Shift); Arrow Up/Down change the value by the config step
 * (10 steps with Shift). Null for other keys.
 */
export function keyframeKeyboardEdit(
  item: TimelineItem,
  config: KeyframePropertyConfig,
  keyframe: KeyframePoint,
  event: { readonly key: string; readonly shiftKey: boolean },
): KeyframePoint | "delete" | null {
  switch (event.key) {
    case "Delete":
    case "Backspace":
      return "delete";
    case "ArrowLeft":
    case "ArrowRight": {
      const step = event.shiftKey ? largeTimeStepSeconds : timeStepSeconds;
      return draggedKeyframe(item, config, keyframe, event.key === "ArrowLeft" ? -step : step, 0);
    }
    case "ArrowUp":
    case "ArrowDown": {
      const step = config.step * (event.shiftKey ? largeValueStepFactor : 1);
      return draggedKeyframe(item, config, keyframe, 0, event.key === "ArrowDown" ? -step : step);
    }
    default:
      return null;
  }
}

/** Adds a linear keyframe at a clip-local time with the value the curve has there. */
export function addKeyframeAction(
  item: TimelineItem,
  config: KeyframePropertyConfig,
  keyframes: readonly ProjectActionKeyframe[],
  atSeconds: number,
): ProjectAction {
  const time = roundTimelineSeconds(clamp(atSeconds, 0, item.durationSeconds));
  const value = roundTimelineSeconds(clamp(suggestedValue(config, keyframes, time), config.minimum, config.maximum));
  return { type: "upsertItemKeyframe", itemId: item.id, property: config.property, keyframe: { atSeconds: time, value, easing: "linear" } };
}
