import { generatedAssetForTimelineItem } from "@/lib/generation/assets";
import { mediaDisplayName } from "@/lib/media/names";
import { selectedTimelineItems, selectionKind, type SelectionKind } from "@/lib/preview/selection-kind";
import type { VideoProject } from "@/lib/project";
import { locateTransition, transitionKindLabels } from "@/lib/timeline-ops/transition-commands";
import { timelineItemHasAudio } from "@/lib/timeline-ops/automation";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";

export type PropertyTabId =
  | "video"
  | "audio"
  | "speed"
  | "animation"
  | "basic"
  | "voice"
  | "text"
  | "style"
  | "position"
  | "content"
  | "effects"
  | "common"
  | "transition"
  | "ai";

export interface PropertyTab {
  readonly id: PropertyTabId;
  readonly label: string;
}

export interface PropertiesSelection {
  readonly kind: SelectionKind;
  /** Header text: the item label, else its media name, else "N items". */
  readonly name: string;
  readonly tabs: readonly PropertyTab[];
}

const tab = (id: PropertyTabId, label: string): PropertyTab => ({ id, label });

/**
 * Tab sets per selection kind. Audio clips have a Speed tab like visual clips, and show AI only for
 * generated audio.
 */
export function propertyTabsForKind(kind: SelectionKind, hasAudio: boolean, generated = false): readonly PropertyTab[] {
  switch (kind) {
    case "visual":
      return [tab("video", "Video"), ...(hasAudio ? [tab("audio", "Audio")] : []), tab("speed", "Speed"), tab("animation", "Animation"), tab("ai", "AI")];
    case "audio":
      return [tab("basic", "Basic"), tab("voice", "Voice"), tab("speed", "Speed"), ...(generated ? [tab("ai", "AI")] : [])];
    case "text":
    case "caption":
      return [tab("text", "Text"), tab("style", "Style"), tab("position", "Position"), tab("animation", "Animation")];
    case "template":
      return [tab("content", "Content"), tab("style", "Style"), tab("animation", "Animation"), tab("effects", "Effects")];
    case "multiple":
      return [tab("common", "Common")];
    case "transition":
      return [tab("transition", "Transition")];
    case "none":
      return [];
  }
}

/** The Properties header and tabs for the current selection, or null when nothing is shown. */
export function propertiesSelection(project: VideoProject, itemIds: readonly string[], transitionId: string | null = null): PropertiesSelection | null {
  const items = selectedTimelineItems(project, itemIds);
  const kind = selectionKind(project, itemIds, transitionId);
  if (kind === "transition") {
    const located = locateTransition(project.timeline, transitionId);
    return located ? { kind, name: `${transitionKindLabels[located.transition.kind]} transition`, tabs: propertyTabsForKind(kind, false) } : null;
  }
  const [only] = items;
  const tabs = propertyTabsForKind(
    kind,
    only !== undefined && timelineItemHasAudio(project, only),
    kind === "audio" && only !== undefined && generatedAssetForTimelineItem(project, only) !== null,
  );
  if (!only || tabs.length === 0) return null;
  if (items.length > 1) return { kind, name: `${items.length.toString()} items`, tabs };
  const mediaId = timelineItemSourceMediaId(only);
  const media = mediaId ? project.media.find((candidate) => candidate.id === mediaId) : undefined;
  const name = only.label.trim() || (media ? mediaDisplayName(media) : "") || "Untitled item";
  return { kind, name, tabs };
}
