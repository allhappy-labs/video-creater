import { isVisualTimelineSourceItem } from "@/lib/preview/viewer-context";
import type { VideoProject } from "@/lib/project";
import { isTemplateTimelineItem, type TimelineItem } from "@/lib/timeline";
import { isVisualOpacityItem } from "@/lib/timeline-ops/item-properties";

/** The Properties tab set for a selection; `transition` is a selected transition badge. */
export type SelectionKind =
  | "none"
  | "visual"
  | "audio"
  | "text"
  | "caption"
  | "template"
  | "transition"
  | "multiple";

/** Properties that the Multiple tab can edit on every selected item at once. */
export interface CommonPropertySupport {
  readonly opacity: boolean;
  readonly volume: boolean;
  readonly effects: boolean;
}

/** Existing selected items in selection order, without duplicates. */
export function selectedTimelineItems(project: VideoProject, itemIds: readonly string[]): TimelineItem[] {
  const byId = new Map(
    project.timeline.tracks.flatMap((track) => track.items.map((item) => [item.id, item] as const)),
  );
  return [...new Set(itemIds)].flatMap((itemId) => {
    const item = byId.get(itemId);
    return item ? [item] : [];
  });
}

/**
 * Single-item kind, in the legacy inspector's precedence: caption, then template, then text
 * overlay. Everything else that is not an audio clip (including Lottie, generated, nested
 * timeline and HyperFrame clips) uses the visual tabs.
 */
function itemSelectionKind(item: TimelineItem): SelectionKind {
  if (item.kind === "caption") return "caption";
  if (isTemplateTimelineItem(item)) return "template";
  if (item.kind === "overlay" && item.source.type === "text") return "text";
  if (item.kind === "audio_clip") return "audio";
  return "visual";
}

/** Whether `transitionId` names a transition on the active timeline. */
export function activeTimelineHasTransition(project: VideoProject, transitionId: string | null): transitionId is string {
  return (
    transitionId !== null &&
    project.timeline.tracks.some((track) => (track.transitions ?? []).some((transition) => transition.id === transitionId))
  );
}

/**
 * The selection's kind. Item and transition selection are exclusive in the store; a selected
 * transition on the active timeline wins when no item is selected.
 */
export function selectionKind(project: VideoProject, itemIds: readonly string[], transitionId: string | null = null): SelectionKind {
  const items = selectedTimelineItems(project, itemIds);
  if (items.length === 0 && activeTimelineHasTransition(project, transitionId)) return "transition";
  const [only] = items;
  if (!only) return "none";
  return items.length > 1 ? "multiple" : itemSelectionKind(only);
}

export function commonPropertySupport(
  project: VideoProject,
  itemIds: readonly string[],
): CommonPropertySupport {
  const items = selectedTimelineItems(project, itemIds);
  if (items.length === 0) return { opacity: false, volume: false, effects: false };
  return {
    opacity: items.every(isVisualOpacityItem),
    volume: items.every((item) => item.kind === "audio_clip"),
    // Legacy batch effects: visual source clips only, never nested timeline clips.
    effects: items.every((item) => isVisualTimelineSourceItem(item) && item.source.type !== "timeline"),
  };
}
