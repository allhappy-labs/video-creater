import type { ProjectAction, ProjectActionEffect, VisualEffectDescriptor } from "@/lib/project";
import { itemEffects } from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";

/** One effect instance on one selected item. */
interface EffectTarget {
  readonly item: TimelineItem;
  readonly effect: ProjectActionEffect;
}

/** A row in the Effects section: one instance (single item) or one shared effect type (multiple). */
export interface EffectRow {
  readonly id: string;
  readonly effectType: string;
  readonly targets: readonly EffectTarget[];
}

export interface EffectParamControl {
  readonly key: string;
  readonly label: string;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly defaultValue: number;
}

/** Audio denoise is edited in the Voice tab, never as a visual effect row. */
const hiddenEffectTypes: ReadonlySet<string> = new Set(["audio.denoise"]);

const curveMidpoint: EffectParamControl = { key: "curveMidpoint", label: "Master midpoint", min: 0, max: 1, step: 0.01, defaultValue: 0.5 };

/** Legacy inspector extras for the curve effects, whose catalog entries carry no numeric params. */
const hueCurveControls: readonly EffectParamControl[] = [
  { key: "targetHue", label: "Target hue (°)", min: 0, max: 360, step: 1, defaultValue: 0 },
  { key: "hueShift", label: "Hue shift (°)", min: -30, max: 30, step: 0.5, defaultValue: 0 },
  { key: "satScale", label: "Saturation", min: 0, max: 2, step: 0.02, defaultValue: 1 },
  { key: "lumShift", label: "Luminance", min: -0.5, max: 0.5, step: 0.01, defaultValue: 0 },
];

function visibleEffects(item: TimelineItem): ProjectActionEffect[] {
  return itemEffects(item).filter((effect) => !hiddenEffectTypes.has(effect.effectType));
}

/**
 * One item: every effect instance. Several items: the effect types every item has (the
 * intersection), each targeting the first instance of that type on each item.
 */
export function effectRows(items: readonly TimelineItem[]): EffectRow[] {
  const [first, ...rest] = items;
  if (!first) return [];
  if (rest.length === 0) {
    return visibleEffects(first).map((effect) => ({
      id: effect.effectInstanceId,
      effectType: effect.effectType,
      targets: [{ item: first, effect }],
    }));
  }
  const types = [...new Set(visibleEffects(first).map((effect) => effect.effectType))];
  return types.flatMap((effectType): EffectRow[] => {
    const targets = items.flatMap((item): EffectTarget[] => {
      const effect = visibleEffects(item).find((candidate) => candidate.effectType === effectType);
      return effect ? [{ item, effect }] : [];
    });
    return targets.length === items.length ? [{ id: effectType, effectType, targets }] : [];
  });
}

export function effectParamControls(
  effectType: string,
  descriptor: VisualEffectDescriptor | undefined,
): EffectParamControl[] {
  if (effectType === "color.curves") return [curveMidpoint];
  if (effectType === "color.hueCurves") return [...hueCurveControls];
  return (descriptor?.params ?? []).map((param) => ({
    key: param.key,
    label: param.unit ? `${param.label} (${param.unit})` : param.label,
    min: param.min,
    max: param.max,
    step: Math.max(0.01, (param.max - param.min) / 100),
    defaultValue: param.defaultValue,
  }));
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function finite(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function hueTarget(effect: ProjectActionEffect): Record<string, unknown> {
  const targets = effect.params.targets;
  return (Array.isArray(targets) ? record(targets[0]) : null) ?? {};
}

/** The stored numeric value, the curve extras' nested values, or the control default. */
export function effectParamValue(effect: ProjectActionEffect, effectType: string, control: EffectParamControl): number {
  if (effectType === "color.curves") {
    const master = effect.params.masterCurve;
    const midpoint = Array.isArray(master) && Array.isArray(master[1]) ? finite(master[1][1]) : null;
    return midpoint ?? control.defaultValue;
  }
  if (effectType === "color.hueCurves") return finite(hueTarget(effect)[control.key]) ?? control.defaultValue;
  return finite(effect.params[control.key]) ?? control.defaultValue;
}

/** Legacy storage: curves keep a three-point master curve, hue curves a single target entry. */
export function withEffectParam(effect: ProjectActionEffect, key: string, value: number): ProjectActionEffect {
  if (effect.effectType === "color.curves") {
    return { ...effect, params: { masterCurve: [[0, 0], [0.5, value], [1, 1]] } };
  }
  if (effect.effectType === "color.hueCurves") {
    const current = Object.fromEntries(
      hueCurveControls.map((control) => [control.key, effectParamValue(effect, effect.effectType, control)]),
    );
    return { ...effect, params: { targets: [{ ...current, [key]: value }] } };
  }
  return { ...effect, params: { ...effect.params, [key]: value } };
}

/**
 * One `updateItemEffects` per target item with that item's effect list rebuilt: the target
 * instance is replaced by `update(effect)`, or removed when it returns null.
 */
export function effectRowActions(
  row: EffectRow,
  update: (effect: ProjectActionEffect) => ProjectActionEffect | null,
): ProjectAction[] {
  return row.targets.map(({ item, effect }) => ({
    type: "updateItemEffects",
    itemIds: [item.id],
    effects: itemEffects(item).flatMap((candidate) => {
      if (candidate.effectInstanceId !== effect.effectInstanceId) return [candidate];
      const next = update(candidate);
      return next ? [next] : [];
    }),
  }));
}
