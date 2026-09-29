import { roundTimelineSeconds } from "@/lib/format";
import { motionPresetCatalog, type MotionPresetId } from "@/lib/motion-presets";
import type { CanonicalProjectActionKeyframeEasing, ProjectAction, ProjectActionKeyframe } from "@/lib/project";
import { keyframeStateAtPlayhead } from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { isVisualOpacityItem, stringProperty } from "@/lib/timeline-ops/item-properties";
import { inspectorKeyframesByProperty, visualKeyframePropertyConfigs } from "@/lib/timeline-ops/keyframes";

export type AnimationPhase = "in" | "out" | "loop";

export interface AnimationPreset {
  readonly id: MotionPresetId;
  readonly phase: AnimationPhase;
  readonly label: string;
  readonly description: string;
}

type AnimatedProperty = "opacity" | "positionX" | "positionY" | "scale" | "rotationDegrees";

/** A lane shape: [progress through the window 0–1, value derived from the resting value]. */
interface PresetCurve {
  readonly property: AnimatedProperty;
  readonly points: readonly (readonly [number, (rest: number) => number])[];
}

interface ClipPresetDefinition {
  readonly phase: AnimationPhase;
  readonly curves: readonly PresetCurve[];
}

const fadeIn: PresetCurve = { property: "opacity", points: [[0, () => 0], [1, (rest) => rest]] };
const fadeOut: PresetCurve = { property: "opacity", points: [[0, (rest) => rest], [1, () => 0]] };

/**
 * Clip keyframe shapes for the catalog presets that read as an entrance, exit or loop. Pre-cut
 * applied clip motion as whole `setItemKeyframes` lanes and graphics motion as `motionPresetId`;
 * clip presets keep the lane path so preview and export animate them the same way.
 */
const clipPresetDefinitions: Partial<Record<MotionPresetId, ClipPresetDefinition>> = {
  "slide-fade-up-v1": {
    phase: "in",
    curves: [fadeIn, { property: "positionY", points: [[0, (rest) => rest + 80], [1, (rest) => rest]] }],
  },
  "spring-pop-v2": {
    phase: "in",
    curves: [
      fadeIn,
      { property: "scale", points: [[0, (rest) => rest * 0.6], [0.7, (rest) => rest * 1.08], [1, (rest) => rest]] },
    ],
  },
  "slide-rotate-settle-v2": {
    phase: "in",
    curves: [
      fadeIn,
      { property: "positionX", points: [[0, (rest) => rest - 160], [1, (rest) => rest]] },
      { property: "rotationDegrees", points: [[0, (rest) => rest - 8], [1, (rest) => rest]] },
    ],
  },
  "exit-snap-v2": {
    phase: "out",
    curves: [
      fadeOut,
      { property: "scale", points: [[0, (rest) => rest], [1, (rest) => rest * 0.9]] },
      { property: "positionY", points: [[0, (rest) => rest], [1, (rest) => rest - 40]] },
    ],
  },
  "pulse-emphasis-v2": {
    phase: "loop",
    curves: [{ property: "scale", points: [[0, (rest) => rest], [0.5, (rest) => rest * 1.06], [1, (rest) => rest]] }],
  },
};

const phasePropertyKeys: Readonly<Record<AnimationPhase, string>> = {
  in: "animationInPresetId",
  out: "animationOutPresetId",
  loop: "animationLoopPresetId",
};

const phaseEasing: Readonly<Record<AnimationPhase, CanonicalProjectActionKeyframeEasing>> = {
  in: "easeOut",
  out: "easeIn",
  loop: "easeInOut",
};

const maximumWindowSeconds = 0.5;
const loopCycleSeconds = 1;
const timeToleranceSeconds = 0.000_5;

export function animationPresets(phase: AnimationPhase): AnimationPreset[] {
  return motionPresetCatalog.flatMap((preset) =>
    clipPresetDefinitions[preset.id]?.phase === phase
      ? [{ id: preset.id, phase, label: preset.label, description: preset.description }]
      : [],
  );
}

export function appliedAnimationPreset(item: TimelineItem, phase: AnimationPhase): MotionPresetId | null {
  const stored = stringProperty(item, phasePropertyKeys[phase]);
  const preset = motionPresetCatalog.find((candidate) => candidate.id === stored);
  return preset && clipPresetDefinitions[preset.id]?.phase === phase ? preset.id : null;
}

/** Item-relative [start, end] the phase animates; in/out use up to 0.5 s or half the clip. */
function phaseWindow(item: TimelineItem, phase: AnimationPhase): readonly [number, number] {
  if (phase === "loop") return [0, item.durationSeconds];
  const length = roundTimelineSeconds(Math.min(maximumWindowSeconds, item.durationSeconds / 2));
  return phase === "in" ? [0, length] : [roundTimelineSeconds(item.durationSeconds - length), item.durationSeconds];
}

function keyframesOutsideWindow(item: TimelineItem, property: AnimatedProperty, window: readonly [number, number]) {
  return (inspectorKeyframesByProperty(item)[property] ?? []).filter(
    (keyframe) =>
      keyframe.atSeconds < window[0] - timeToleranceSeconds || keyframe.atSeconds > window[1] + timeToleranceSeconds,
  );
}

function presetKeyframes(
  item: TimelineItem,
  phase: AnimationPhase,
  curve: PresetCurve,
  window: readonly [number, number],
): ProjectActionKeyframe[] {
  const restAt = phase === "out" ? window[0] : window[1];
  const rest = keyframeStateAtPlayhead(item, curve.property, item.startSeconds + (phase === "loop" ? 0 : restAt)).value;
  const config = visualKeyframePropertyConfigs.find((candidate) => candidate.property === curve.property);
  const clamp = (value: number) => Math.min(Math.max(value, config?.minimum ?? value), config?.maximum ?? value);
  const cycles = phase === "loop" ? Math.max(1, Math.round((window[1] - window[0]) / loopCycleSeconds)) : 1;
  const cycleLength = (window[1] - window[0]) / cycles;
  const byTime = new Map<number, ProjectActionKeyframe>();
  for (let cycle = 0; cycle < cycles; cycle += 1) {
    for (const [progress, valueFor] of curve.points) {
      const atSeconds = roundTimelineSeconds(window[0] + (cycle + progress) * cycleLength);
      byTime.set(atSeconds, { atSeconds, value: roundTimelineSeconds(clamp(valueFor(rest))), easing: phaseEasing[phase] });
    }
  }
  return [...byTime.values()];
}

function sortByTime(keyframes: readonly ProjectActionKeyframe[]) {
  return [...keyframes].sort((left, right) => left.atSeconds - right.atSeconds);
}

/** Lanes that only a previously applied preset animated lose their keyframes in its window. */
function clearedLaneActions(
  item: TimelineItem,
  phase: AnimationPhase,
  previous: MotionPresetId | null,
  keep: ReadonlySet<AnimatedProperty>,
): ProjectAction[] {
  const definition = previous ? clipPresetDefinitions[previous] : undefined;
  if (!definition) return [];
  const lanes = inspectorKeyframesByProperty(item);
  const window = phaseWindow(item, phase);
  return definition.curves.flatMap((curve): ProjectAction[] =>
    keep.has(curve.property) || lanes[curve.property] === undefined
      ? []
      : [
          {
            type: "setItemKeyframes",
            itemId: item.id,
            property: curve.property,
            keyframes: phase === "loop" ? [] : keyframesOutsideWindow(item, curve.property, window),
          },
        ],
  );
}

/**
 * In, out and loop presets for visual clips and text overlays: one `setItemKeyframes` per
 * animated lane (keyframes outside the phase window are kept; loop replaces the lane), plus the
 * recorded preset id so the grid can show the selection and "None" can clear it.
 */
export function animationPresetActions(item: TimelineItem, presetId: MotionPresetId): CommandResult {
  if (!isVisualOpacityItem(item)) return { blocked: "Animation presets apply to visual clips and text overlays." };
  const definition = clipPresetDefinitions[presetId];
  if (!definition) return { blocked: "That motion preset isn't available as a clip animation." };
  if (!(item.durationSeconds > 0)) return { blocked: "This clip is too short to animate." };
  const { phase } = definition;
  const window = phaseWindow(item, phase);
  const laneActions = definition.curves.map(
    (curve): ProjectAction => ({
      type: "setItemKeyframes",
      itemId: item.id,
      property: curve.property,
      keyframes: sortByTime([
        ...(phase === "loop" ? [] : keyframesOutsideWindow(item, curve.property, window)),
        ...presetKeyframes(item, phase, curve, window),
      ]),
    }),
  );
  const animated = new Set(definition.curves.map((curve) => curve.property));
  return {
    actions: [
      ...laneActions,
      ...clearedLaneActions(item, phase, appliedAnimationPreset(item, phase), animated),
      {
        type: "updateItemProperties",
        updates: [{ itemId: item.id, set: { [phasePropertyKeys[phase]]: presetId }, remove: [] }],
      },
    ],
  };
}

/** "None" in a phase grid: remove the recorded preset's keyframes in its window and its id. */
export function clearAnimationPresetActions(item: TimelineItem, phase: AnimationPhase): CommandResult {
  const previous = appliedAnimationPreset(item, phase);
  if (!previous) return { actions: [] };
  return {
    actions: [
      ...clearedLaneActions(item, phase, previous, new Set()),
      { type: "updateItemProperties", updates: [{ itemId: item.id, set: {}, remove: [phasePropertyKeys[phase]] }] },
    ],
  };
}

/** Graphics motion (templates, captions): the pre-cut `motionPresetId` property update. */
export function motionPresetAction(itemIds: readonly string[], presetId: MotionPresetId): ProjectAction {
  return {
    type: "updateItemProperties",
    updates: itemIds.map((itemId) => ({ itemId, set: { motionPresetId: presetId }, remove: [] })),
  };
}
