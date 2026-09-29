import type { ProjectActionKeyframe, ProjectActionKeyframeProperty } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty } from "@/lib/timeline-ops/item-properties";

export interface KeyframePropertyConfig<Property extends string = ProjectActionKeyframeProperty> {
  property: Property;
  label: string;
  minimum: number;
  maximum: number;
  step: number;
  defaultValue: number;
}

export type VisualMotionKeyframeProperty =
  | "positionX"
  | "positionY"
  | "scale"
  | "rotationDegrees";

export function motionKeyframeDefault(item: TimelineItem | null, property: VisualMotionKeyframeProperty) {
  const fallback = property === "scale" ? 1 : 0;
  return item ? (numberProperty(item, property)?.toString() ?? fallback.toString()) : fallback.toString();
}

export function motionKeyframeBounds(property: VisualMotionKeyframeProperty) {
  if (property === "scale") return { min: 0.01, max: 100, step: 0.01 };
  if (property === "rotationDegrees") return { min: -360, max: 360, step: 1 };
  return { min: -10000, max: 10000, step: 1 };
}

export const visualKeyframePropertyConfigs: readonly KeyframePropertyConfig[] = [
  { property: "opacity", label: "Opacity", minimum: 0, maximum: 1, step: 0.05, defaultValue: 1 },
  { property: "positionX", label: "Position X", minimum: -10000, maximum: 10000, step: 1, defaultValue: 0 },
  { property: "positionY", label: "Position Y", minimum: -10000, maximum: 10000, step: 1, defaultValue: 0 },
  { property: "scale", label: "Scale", minimum: 0.01, maximum: 100, step: 0.01, defaultValue: 1 },
  { property: "rotationDegrees", label: "Rotation", minimum: -360, maximum: 360, step: 1, defaultValue: 0 },
  { property: "cropTop", label: "Crop top", minimum: 0, maximum: 0.95, step: 0.01, defaultValue: 0 },
  { property: "cropRight", label: "Crop right", minimum: 0, maximum: 0.95, step: 0.01, defaultValue: 0 },
  { property: "cropBottom", label: "Crop bottom", minimum: 0, maximum: 0.95, step: 0.01, defaultValue: 0 },
  { property: "cropLeft", label: "Crop left", minimum: 0, maximum: 0.95, step: 0.01, defaultValue: 0 },
];

export const audioKeyframePropertyConfigs: readonly KeyframePropertyConfig[] = [
  { property: "volumeDb", label: "Volume dB", minimum: -60, maximum: 24, step: 0.5, defaultValue: 0 },
];

export function inspectorKeyframesByProperty(item: TimelineItem) {
  const stored = item.properties.keyframes;
  if (!stored || typeof stored !== "object" || Array.isArray(stored)) return {};
  const result: Partial<Record<ProjectActionKeyframeProperty, ProjectActionKeyframe[]>> = {};
  for (const property of [
    "opacity",
    "volumeDb",
    "positionX",
    "positionY",
    "scale",
    "rotationDegrees",
    "cropTop",
    "cropRight",
    "cropBottom",
    "cropLeft",
  ] as const) {
    const points = (stored as Record<string, unknown>)[property];
    if (!Array.isArray(points)) continue;
    result[property] = points.flatMap((point) => {
      if (!point || typeof point !== "object" || Array.isArray(point)) return [];
      const record = point as Record<string, unknown>;
      const easing =
        typeof record.easing === "string" &&
        ["linear", "hold", "easeIn", "easeOut", "easeInOut", "smooth"].includes(
          record.easing,
        )
          ? (record.easing as NonNullable<ProjectActionKeyframe["easing"]>)
          : undefined;
      return typeof record.atSeconds === "number" &&
        Number.isFinite(record.atSeconds) &&
        typeof record.value === "number" &&
        Number.isFinite(record.value)
        ? [{
            atSeconds: record.atSeconds,
            value: record.value,
            ...(easing === undefined ? {} : { easing }),
          }]
        : [];
    });
  }
  return result;
}

export function inspectorEffectParameterKeyframes(
  item: TimelineItem,
  effectInstanceId: string,
  parameterKey: string,
): ProjectActionKeyframe[] {
  const stored = item.properties.effectParameterKeyframes;
  if (!stored || typeof stored !== "object" || Array.isArray(stored)) return [];
  const instance = (stored as Record<string, unknown>)[effectInstanceId];
  if (!instance || typeof instance !== "object" || Array.isArray(instance)) return [];
  const points = (instance as Record<string, unknown>)[parameterKey];
  if (!Array.isArray(points)) return [];
  return points.flatMap((point) => {
    if (!point || typeof point !== "object" || Array.isArray(point)) return [];
    const record = point as Record<string, unknown>;
    if (typeof record.atSeconds !== "number" || typeof record.value !== "number") return [];
    const easing =
      typeof record.easing === "string" &&
      ["linear", "hold", "easeIn", "easeOut", "easeInOut", "smooth"].includes(
        record.easing,
      )
        ? (record.easing as NonNullable<ProjectActionKeyframe["easing"]>)
        : "linear";
    return [{
      atSeconds: record.atSeconds,
      value: record.value,
      easing,
    }];
  });
}

export function keyframePropertyConfigsForItem(item: TimelineItem) {
  const configs = item.kind === "audio_clip" ? audioKeyframePropertyConfigs : visualKeyframePropertyConfigs;
  return configs.map((config) => ({
    ...config,
    defaultValue: numberProperty(item, config.property) ?? config.defaultValue,
  }));
}

export function sortedKeyframes(keyframes: readonly ProjectActionKeyframe[] | undefined) {
  return [...(keyframes ?? [])].sort((left, right) => left.atSeconds - right.atSeconds);
}

export function suggestedValue<Property extends string>(
  config: KeyframePropertyConfig<Property>,
  keyframes: readonly ProjectActionKeyframe[],
  atSeconds: number,
) {
  if (keyframes.length === 0) return config.defaultValue;
  const exact = keyframes.find((keyframe) => keyframe.atSeconds === atSeconds);
  if (exact) return exact.value;
  const previous = keyframes.filter((keyframe) => keyframe.atSeconds < atSeconds).at(-1);
  const next = keyframes.find((keyframe) => keyframe.atSeconds > atSeconds);
  if (!previous) return next?.value ?? config.defaultValue;
  if (!next || previous.easing === "hold") return previous.value;
  const progress = (atSeconds - previous.atSeconds) / (next.atSeconds - previous.atSeconds);
  return previous.value + (next.value - previous.value) * progress;
}

