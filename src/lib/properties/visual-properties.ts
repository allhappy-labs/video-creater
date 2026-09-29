import type {
  ProjectAction,
  ProjectActionColorGrade,
  ProjectActionEffect,
  ProjectActionVisualCrop,
  ProjectActionVisualTransform,
  VideoProject,
} from "@/lib/project";
import { clipLocalPlayhead, keyframedEditAction, keyframeStateAtPlayhead } from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import { setItemSpeed, type CommandResult } from "@/lib/timeline-ops/clip-commands";
import {
  colorGradeNumberProperty,
  isVisualOpacityItem,
  numberProperty,
  projectActionEffectsForItem,
  transformBooleanProperty,
  transformNumberProperty,
  visualBlendModeProperty,
  type VisualBlendMode,
} from "@/lib/timeline-ops/item-properties";
import { motionKeyframeBounds, type VisualMotionKeyframeProperty } from "@/lib/timeline-ops/keyframes";

export interface VisualTransform {
  readonly centerX: number;
  readonly centerY: number;
  readonly width: number;
  readonly height: number;
  readonly flipHorizontal: boolean;
  readonly flipVertical: boolean;
}

export type VisualMotion = Readonly<Record<VisualMotionKeyframeProperty, number>>;

export type VisualCrop = Readonly<Required<ProjectActionVisualCrop>>;

export interface VisualFades {
  readonly fadeInSeconds: number;
  readonly fadeOutSeconds: number;
}

/** The color grade controls the Properties panel edits. */
export type ColorGradeValues = Readonly<
  Required<{ [Key in "exposure" | "contrast" | "saturation" | "temperature" | "tint"]: number }>
>;

interface Range {
  readonly min: number;
  readonly max: number;
  readonly step: number;
}

/** Legacy inspector input ranges. */
export const visualPropertyRanges = {
  opacity: { min: 0, max: 1, step: 0.05 },
  center: { min: 0, max: 1, step: 0.01 },
  size: { min: 0.01, max: 1, step: 0.01 },
  crop: { min: 0, max: 0.99, step: 0.01 },
  fade: { min: 0, max: Number.POSITIVE_INFINITY, step: 0.1 },
  speed: { min: 0.1, max: 8, step: 0.1 },
} as const satisfies Record<string, Range>;

/** Legacy inspector grade ranges; temperature (Kelvin) and tint use the backend validation ranges. */
export const colorGradeRanges = {
  exposure: { min: -3, max: 3, step: 0.05, defaultValue: 0 },
  contrast: { min: 0.5, max: 1.5, step: 0.05, defaultValue: 1 },
  saturation: { min: 0, max: 2, step: 0.05, defaultValue: 1 },
  temperature: { min: 2000, max: 11000, step: 100, defaultValue: 6500 },
  tint: { min: -100, max: 100, step: 1, defaultValue: 0 },
} as const satisfies Record<keyof ColorGradeValues, Range & { defaultValue: number }>;

const motionDefaults: VisualMotion = { positionX: 0, positionY: 0, scale: 1, rotationDegrees: 0 };
const cropSides = ["cropTop", "cropRight", "cropBottom", "cropLeft"] as const;
const notVisualMessage = "Select a visual clip to change this property.";

function inRange(value: number | null, min: number, max: number): value is number {
  return value !== null && value >= min && value <= max;
}

function finiteInRange(value: number | undefined, min: number, max: number) {
  return value === undefined || (Number.isFinite(value) && value >= min && value <= max);
}

export function visualTransform(item: TimelineItem): VisualTransform {
  const center = (key: string) => {
    const value = transformNumberProperty(item, key);
    return inRange(value, 0, 1) ? value : 0.5;
  };
  const size = (key: string) => {
    const value = transformNumberProperty(item, key);
    return value !== null && value > 0 && value <= 1 ? value : 1;
  };
  return {
    centerX: center("centerX"),
    centerY: center("centerY"),
    width: size("width"),
    height: size("height"),
    flipHorizontal: transformBooleanProperty(item, "flipHorizontal"),
    flipVertical: transformBooleanProperty(item, "flipVertical"),
  };
}

/** Static motion values (the keyframe lane bases); out-of-bounds values fall back to defaults. */
export function visualMotion(item: TimelineItem): VisualMotion {
  const read = (property: VisualMotionKeyframeProperty) => {
    const bounds = motionKeyframeBounds(property);
    const value = numberProperty(item, property);
    return inRange(value, bounds.min, bounds.max) ? value : motionDefaults[property];
  };
  return {
    positionX: read("positionX"),
    positionY: read("positionY"),
    scale: read("scale"),
    rotationDegrees: read("rotationDegrees"),
  };
}

export function visualOpacity(item: TimelineItem): number {
  const value = numberProperty(item, "opacity");
  return inRange(value, 0, 1) ? value : 1;
}

export function visualBlendMode(item: TimelineItem): VisualBlendMode {
  return visualBlendModeProperty(item);
}

export function visualCrop(item: TimelineItem): VisualCrop {
  const read = (side: (typeof cropSides)[number]) => {
    const value = numberProperty(item, side);
    return value !== null && value >= 0 && value < 1 ? value : 0;
  };
  return { cropTop: read("cropTop"), cropRight: read("cropRight"), cropBottom: read("cropBottom"), cropLeft: read("cropLeft") };
}

export function visualFades(item: TimelineItem): VisualFades {
  const read = (key: string) => {
    const value = numberProperty(item, key);
    return value !== null && value >= 0 ? value : 0;
  };
  return { fadeInSeconds: read("fadeInSeconds"), fadeOutSeconds: read("fadeOutSeconds") };
}

export function visualSpeed(item: TimelineItem): number {
  const value = numberProperty(item, "speed");
  return value !== null && value > 0 ? value : 1;
}

export function colorGrade(item: TimelineItem): ColorGradeValues {
  const read = (key: keyof ColorGradeValues) => {
    const range = colorGradeRanges[key];
    const value = colorGradeNumberProperty(item, key);
    return inRange(value, range.min, range.max) ? value : range.defaultValue;
  };
  return {
    exposure: read("exposure"),
    contrast: read("contrast"),
    saturation: read("saturation"),
    temperature: read("temperature"),
    tint: read("tint"),
  };
}

/** Well-formed effects with stable instance ids; malformed entries are dropped. */
export function itemEffects(item: TimelineItem): ProjectActionEffect[] {
  return projectActionEffectsForItem(item);
}

export function transformAction(itemId: string, transform: ProjectActionVisualTransform): CommandResult {
  if (Object.values(transform).every((value) => value === undefined)) {
    return { blocked: "Change at least one transform value." };
  }
  if (!finiteInRange(transform.centerX, 0, 1) || !finiteInRange(transform.centerY, 0, 1)) {
    return { blocked: "Center must be between 0 and 1." };
  }
  for (const size of [transform.width, transform.height]) {
    if (size !== undefined && !(Number.isFinite(size) && size > 0 && size <= 1)) {
      return { blocked: "Size must be greater than 0 and at most 1." };
    }
  }
  return { actions: [{ type: "updateVisualClipTransform", itemId, transform }] };
}

/**
 * Position, scale or rotation. Legacy rotation rule, applied to every motion property: when the
 * property is keyframed, upsert at the clip-local playhead; otherwise set the static property,
 * removing it at its default (0, or 1 for scale).
 */
export function motionActions(
  item: TimelineItem,
  property: VisualMotionKeyframeProperty,
  value: number,
  playheadSeconds: number,
): CommandResult {
  if (!isVisualOpacityItem(item)) return { blocked: notVisualMessage };
  const bounds = motionKeyframeBounds(property);
  if (!finiteInRange(value, bounds.min, bounds.max)) {
    const label = property === "scale" ? "Scale" : property === "rotationDegrees" ? "Rotation" : "Position";
    return { blocked: `${label} must be between ${bounds.min.toString()} and ${bounds.max.toString()}.` };
  }
  const keyframed = keyframedEditAction(item, property, playheadSeconds, value);
  if (keyframed) return { actions: [keyframed] };
  const isDefault = value === motionDefaults[property];
  return {
    actions: [
      {
        type: "updateItemProperties",
        updates: [{ itemId: item.id, set: isDefault ? {} : { [property]: value }, remove: isDefault ? [property] : [] }],
      },
    ],
  };
}

export function opacityActions(item: TimelineItem, opacity: number, playheadSeconds: number): CommandResult {
  if (!isVisualOpacityItem(item)) return { blocked: notVisualMessage };
  if (!finiteInRange(opacity, 0, 1)) return { blocked: "Opacity must be between 0 and 1." };
  const keyframed = keyframedEditAction(item, "opacity", playheadSeconds, opacity);
  return { actions: [keyframed ?? { type: "updateVisualClipOpacity", itemId: item.id, opacity }] };
}

/** Legacy: "over" (Normal) removes `blendMode`; other modes set it. */
export function blendModeAction(itemId: string, blendMode: VisualBlendMode): ProjectAction {
  const isNormal = blendMode === "over";
  return {
    type: "updateItemProperties",
    updates: [{ itemId, set: isNormal ? {} : { blendMode }, remove: isNormal ? ["blendMode"] : [] }],
  };
}

/**
 * Sides missing from `crop` keep their current values. Legacy rule: when any side is keyframed,
 * all four sides upsert at the clip-local playhead; otherwise one `updateVisualClipCrop`.
 */
export function cropActions(item: TimelineItem, crop: ProjectActionVisualCrop, playheadSeconds: number): CommandResult {
  if (!isVisualOpacityItem(item)) return { blocked: notVisualMessage };
  const merged = { ...visualCrop(item), ...crop };
  if (cropSides.some((side) => !(Number.isFinite(merged[side]) && merged[side] >= 0 && merged[side] < 1))) {
    return { blocked: "Each crop side must be at least 0 and less than 1." };
  }
  if (merged.cropLeft + merged.cropRight >= 1 || merged.cropTop + merged.cropBottom >= 1) {
    return { blocked: "Opposite crop sides must leave part of the frame visible." };
  }
  if (cropSides.some((side) => keyframeStateAtPlayhead(item, side, playheadSeconds).keyframed)) {
    const atSeconds = clipLocalPlayhead(item, playheadSeconds);
    return {
      actions: cropSides.map((side) => ({
        type: "upsertItemKeyframe",
        itemId: item.id,
        property: side,
        keyframe: { atSeconds, value: merged[side] },
      })),
    };
  }
  return { actions: [{ type: "updateVisualClipCrop", itemId: item.id, crop }] };
}

export function fadesAction(item: TimelineItem, fadeInSeconds: number, fadeOutSeconds: number): CommandResult {
  if (!isVisualOpacityItem(item)) return { blocked: notVisualMessage };
  if (!Number.isFinite(fadeInSeconds) || !Number.isFinite(fadeOutSeconds) || fadeInSeconds < 0 || fadeOutSeconds < 0) {
    return { blocked: "Fades must be 0 seconds or longer." };
  }
  if (fadeInSeconds + fadeOutSeconds > item.durationSeconds) {
    return { blocked: "Fade in and fade out together can't be longer than the clip." };
  }
  return { actions: [{ type: "updateVisualClipFades", itemId: item.id, fadeInSeconds, fadeOutSeconds }] };
}

/**
 * The clip's speed action (`updateVisualClipSpeed` or `updateAudioClipSpeed`, also for linked
 * partners) plus the rescaled `resizeItems`, via the timeline speed command.
 */
export function speedActions(project: VideoProject, itemId: string, speed: number): CommandResult {
  return setItemSpeed(project, itemId, speed);
}

export function colorGradeAction(itemIds: readonly string[], grade: Partial<ColorGradeValues>): CommandResult {
  for (const key of Object.keys(colorGradeRanges) as (keyof ColorGradeValues)[]) {
    const range = colorGradeRanges[key];
    if (!finiteInRange(grade[key], range.min, range.max)) {
      const label = key.charAt(0).toUpperCase() + key.slice(1);
      return { blocked: `${label} must be between ${range.min.toString()} and ${range.max.toString()}.` };
    }
  }
  const projectGrade: ProjectActionColorGrade = { ...grade };
  return { actions: [{ type: "updateItemColorGrade", itemIds, reset: false, grade: projectGrade }] };
}

/** Section reset: clears the whole stored grade. */
export function resetColorGradeAction(itemIds: readonly string[]): ProjectAction {
  return { type: "updateItemColorGrade", itemIds, reset: true, grade: {} };
}

export function effectsAction(itemIds: readonly string[], effects: readonly ProjectActionEffect[]): ProjectAction {
  return { type: "updateItemEffects", itemIds, effects };
}

export function removeEffectAction(item: TimelineItem, effectInstanceId: string): ProjectAction {
  return effectsAction(
    [item.id],
    itemEffects(item).filter((effect) => effect.effectInstanceId !== effectInstanceId),
  );
}
