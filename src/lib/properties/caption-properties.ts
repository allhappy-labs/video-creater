import {
  captionRepairActionForItem,
  captionStyleProperties,
  captionWordTokens,
  type CaptionMotionPreset,
  type CaptionPlacement,
  type CaptionStylePreset,
  type CaptionWordAnimation,
  type CaptionWordAnimationPreset,
} from "@/lib/captions/caption-items";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";

/** "Applies to: All captions / Only this". */
export type CaptionStyleScope = "all" | "this";

const captionPlacements: readonly CaptionPlacement[] = ["lower", "center", "upper"];
const captionStylePresets: readonly CaptionStylePreset[] = ["boldReadableLower", "kineticFocus", "centeredMinimal"];
const captionMotionPresets: readonly CaptionMotionPreset[] = ["snap-pop-v1", "pulse-emphasis-v2", "soft-depth-card-v2"];
const captionWordAnimationPresets: readonly CaptionWordAnimationPreset[] = ["sequentialPop", "groupPulse", "karaokeFade"];

const defaultWordStaggerSeconds = 0.06;
const maximumWordStaggerSeconds = 0.5;

function oneOf<Value extends string>(values: readonly Value[], candidate: string | null, fallback: Value): Value {
  return values.find((value) => value === candidate) ?? fallback;
}

export function captionPlacement(item: TimelineItem): CaptionPlacement {
  return oneOf(captionPlacements, stringProperty(item, "captionPlacement"), "lower");
}

export function captionStylePreset(item: TimelineItem): CaptionStylePreset {
  return oneOf(captionStylePresets, stringProperty(item, "stylePreset"), "boldReadableLower");
}

export function captionMotionPreset(item: TimelineItem): CaptionMotionPreset {
  return oneOf(captionMotionPresets, stringProperty(item, "motionPresetId"), "snap-pop-v1");
}

export function captionWordAnimationPreset(item: TimelineItem): CaptionWordAnimationPreset {
  return oneOf(captionWordAnimationPresets, stringProperty(item, "captionWordAnimationPreset"), "sequentialPop");
}

export function captionWordStaggerSeconds(item: TimelineItem): number {
  const value = numberProperty(item, "captionWordStaggerSeconds");
  return value !== null && value >= 0 && value <= maximumWordStaggerSeconds ? value : defaultWordStaggerSeconds;
}

/** Legacy caption group: every caption with the same `captionGroupId`, else just the cue. */
export function captionGroupItems(project: VideoProject, itemId: string): TimelineItem[] {
  const captions = project.timeline.tracks.flatMap((track) => track.items).filter((item) => item.kind === "caption");
  const selected = captions.find((item) => item.id === itemId);
  if (!selected) return [];
  const groupId = stringProperty(selected, "captionGroupId");
  return groupId ? captions.filter((item) => stringProperty(item, "captionGroupId") === groupId) : [selected];
}

function scopedCaptionItems(project: VideoProject, itemId: string, scope: CaptionStyleScope) {
  const group = captionGroupItems(project, itemId);
  return scope === "all" ? group : group.filter((item) => item.id === itemId);
}

/**
 * Legacy `applyCaptionText`: a caption repair when the cue is exactly one transcript word and the
 * new text is one word (see `captionRepairActionForItem`), else `editCaptionText`, which keeps the
 * cue's timing and the transcript. The text is sent untrimmed, but blank text is rejected.
 */
export function captionTextAction(item: TimelineItem, text: string, project: Pick<VideoProject, "transcripts">): CommandResult {
  if (item.kind !== "caption") return { blocked: "Select a caption to edit its text." };
  if (text.trim().length === 0) return { blocked: "Caption text can't be empty." };
  return {
    actions: [captionRepairActionForItem(item, text, project) ?? { type: "editCaptionText", itemId: item.id, text }],
  };
}

/**
 * Style, position and animation edits for the "Applies to" scope, as one `updateItemProperties`
 * batch. `null` values remove the property.
 */
export function captionStyleActions(
  project: VideoProject,
  itemId: string,
  scope: CaptionStyleScope,
  properties: Readonly<Record<string, unknown>>,
): CommandResult {
  const items = scopedCaptionItems(project, itemId, scope);
  if (items.length === 0) return { blocked: "Select a caption to change its style." };
  const entries = Object.entries(properties);
  if (entries.length === 0) return { blocked: "Change at least one caption style." };
  const set = Object.fromEntries(entries.filter(([, value]) => value !== null));
  const remove = entries.filter(([, value]) => value === null).map(([key]) => key);
  return {
    actions: [
      {
        type: "updateItemProperties",
        updates: items.map((item) => ({ itemId: item.id, set: { ...set }, remove: [...remove] })),
      },
    ],
  };
}

/** Style preset grid: the preset's full property set from `captionStyleProperties`. */
export function captionPresetActions(
  project: VideoProject,
  itemId: string,
  scope: CaptionStyleScope,
  preset: CaptionStylePreset,
): CommandResult {
  return captionStyleActions(project, itemId, scope, captionStyleProperties(preset));
}

function emphasizedWordIndices(item: TimelineItem): number[] {
  const stored = item.properties.emphasizedWordIndices;
  return Array.isArray(stored) ? stored.filter((value): value is number => typeof value === "number") : [];
}

function storedWordTiming(item: TimelineItem, wordIndex: number) {
  const timings = Array.isArray(item.properties.captionWordTimings) ? item.properties.captionWordTimings : [];
  const timing: unknown = timings.find(
    (candidate: unknown) =>
      candidate !== null &&
      typeof candidate === "object" &&
      (candidate as Record<string, unknown>).wordIndex === wordIndex,
  );
  const record = (timing ?? {}) as Record<string, unknown>;
  return {
    start: typeof record.startSeconds === "number" ? record.startSeconds : 0,
    end: typeof record.endSeconds === "number" ? record.endSeconds : item.durationSeconds,
  };
}

function clampStagger(staggerSeconds: number) {
  return Number.isFinite(staggerSeconds) ? Math.max(0, Math.min(maximumWordStaggerSeconds, staggerSeconds)) : 0;
}

/** Pre-cut `applyCaptionWordAnimationPreset` math for one cue's emphasized words. */
export function captionWordAnimations(
  item: TimelineItem,
  preset: CaptionWordAnimationPreset,
  staggerSeconds: number,
): CaptionWordAnimation[] {
  const words = captionWordTokens(item.source.type === "text" ? item.source.text : item.label);
  const stagger = clampStagger(staggerSeconds);
  return emphasizedWordIndices(item).flatMap((wordIndex, order): CaptionWordAnimation[] => {
    if (!words[wordIndex]) return [];
    const { start, end } = storedWordTiming(item, wordIndex);
    const offset = preset === "groupPulse" ? 0 : Math.min(order * stagger, Math.max(0, end - start) * 0.25);
    const enterStartSeconds = Math.min(end, start + offset);
    const span = Math.max(0, end - enterStartSeconds);
    return [
      {
        wordIndex,
        enterStartSeconds,
        enterEndSeconds: enterStartSeconds + span * 0.2,
        holdEndSeconds: enterStartSeconds + span * 0.8,
        exitEndSeconds: end,
        emphasisScale: preset === "karaokeFade" ? 1 : preset === "groupPulse" ? 1.08 : 1.14,
        emphasisColor: preset === "karaokeFade" ? "#ffffff" : "#ffcf5a",
        emphasisOpacity: preset === "karaokeFade" ? 0.82 : 1,
        easing: preset === "sequentialPop" ? "outBack" : "outQuad",
      },
    ];
  });
}

/**
 * Word animation preset for the scope. Pre-cut applied it to one cue; the Animation tab's
 * "Applies to" scope extends it to the group, with each cue's animations built from its own
 * words and timings, in one batch.
 */
export function captionWordAnimationPresetActions(
  project: VideoProject,
  itemId: string,
  scope: CaptionStyleScope,
  preset: CaptionWordAnimationPreset,
  staggerSeconds: number,
): CommandResult {
  const items = scopedCaptionItems(project, itemId, scope);
  if (items.length === 0) return { blocked: "Select a caption to animate its words." };
  const stagger = clampStagger(staggerSeconds);
  return {
    actions: [
      {
        type: "updateItemProperties",
        updates: items.map((item) => ({
          itemId: item.id,
          set: {
            captionWordAnimations: captionWordAnimations(item, preset, stagger),
            captionWordAnimationPreset: preset,
            captionWordStaggerSeconds: stagger,
          },
          remove: [],
        })),
      },
    ],
  };
}
