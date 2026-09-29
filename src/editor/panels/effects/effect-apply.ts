import { isVisualTimelineSourceItem } from "@/lib/preview/viewer-context";
import type { ProjectActionEffect, VideoProject, VisualEffectDescriptor } from "@/lib/project";
import { effectsAction, itemEffects } from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { nextCatalogEffectInstanceId } from "@/lib/timeline-ops/item-properties";

/** The catalog fields applying an effect needs; a drag payload carries exactly these. */
export interface ApplicableEffect {
  readonly id: string;
  readonly displayName: string;
  readonly params: readonly { readonly key: string; readonly defaultValue: number }[];
  /** Set for effects that need a resource (for example a LUT file) entered in Properties. */
  readonly resourceKey?: string | null;
}

const noVisualTargetReason = "Select a video or image clip";
const multipleTargetsReason = "Select one video or image clip at a time";
const lockedTargetReason = "Unlock the track to apply effects";

/**
 * Catalog entries the Effects tab offers. Color effects belong to the Look section (the backend
 * rejects them in `updateItemEffects`), and audio effects to the Voice tab.
 */
export function panelEffects(catalog: readonly VisualEffectDescriptor[]): VisualEffectDescriptor[] {
  return catalog.filter((effect) => !effect.colorEffect && !effect.id.startsWith("color.") && !effect.id.startsWith("audio."));
}

/** Unique categories, sorted. */
export function effectCategories(effects: readonly VisualEffectDescriptor[]): string[] {
  return [...new Set(effects.map((effect) => effect.category))].sort();
}

/** Legacy search: trimmed, locale-lowercased substring over id, name, category and resource key. */
export function effectMatchesSearch(effect: VisualEffectDescriptor, query: string): boolean {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return true;
  return [effect.id, effect.displayName, effect.category, effect.resourceKey]
    .filter((value): value is string => typeof value === "string" && value.length > 0)
    .some((value) => value.toLocaleLowerCase().includes(normalized));
}

function locateItem(project: VideoProject, itemId: string) {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, locked: track.locked };
  }
  return null;
}

/** Visual source clips take effects; nested timeline clips never do (legacy batch rule). */
function acceptsEffects(item: TimelineItem): boolean {
  return isVisualTimelineSourceItem(item) && item.source.type !== "timeline";
}

function itemTarget(project: VideoProject, itemId: string): { readonly item: TimelineItem } | { readonly blocked: string } {
  const located = locateItem(project, itemId);
  if (!located || !acceptsEffects(located.item)) return { blocked: noVisualTargetReason };
  if (located.locked) return { blocked: lockedTargetReason };
  return { item: located.item };
}

/** The clip `+` applies to: exactly one selected visual clip on an unlocked track. */
export function effectTarget(project: VideoProject, selectedItemIds: readonly string[]): { readonly item: TimelineItem } | { readonly blocked: string } {
  const ids = [...new Set(selectedItemIds)];
  const [only] = ids;
  if (only === undefined) return { blocked: noVisualTargetReason };
  if (ids.length > 1) {
    const allVisual = ids.every((id) => {
      const located = locateItem(project, id);
      return located !== null && acceptsEffects(located.item);
    });
    return { blocked: allVisual ? multipleTargetsReason : noVisualTargetReason };
  }
  return itemTarget(project, only);
}

/** Effect types already on the item. */
export function appliedEffectTypes(item: TimelineItem): ReadonlySet<string> {
  return new Set(itemEffects(item).map((effect) => effect.effectType));
}

/**
 * One `updateItemEffects` that appends an enabled instance with the descriptor defaults to the
 * item's effects. An effect type the item already has is refused, as in the legacy catalog.
 */
export function planEffectApply(project: VideoProject, itemId: string, effect: ApplicableEffect): CommandResult {
  const target = itemTarget(project, itemId);
  if ("blocked" in target) return target;
  const existing = itemEffects(target.item);
  if (existing.some((candidate) => candidate.effectType === effect.id)) {
    return { blocked: `${effect.displayName} is already applied` };
  }
  const instance: ProjectActionEffect = {
    effectInstanceId: nextCatalogEffectInstanceId(effect.id, existing),
    effectType: effect.id,
    enabled: true,
    params: Object.fromEntries(effect.params.map((param) => [param.key, param.defaultValue])),
  };
  return { actions: [effectsAction([target.item.id], [...existing, instance])] };
}
