import { generatedAssetForTimelineItem } from "@/lib/generation/assets";
import type { TimelinePreviewCanvasState } from "@/lib/preview/canvas-geometry";
import type { VideoProject } from "@/lib/project";
import { isTemplateTimelineItem, type TimelineItem } from "@/lib/timeline";

export type ViewerContextKind =
  | "visual"
  | "caption"
  | "text"
  | "template"
  | "lottie"
  | "generated";

export function isVisualTimelineSourceItem(item: TimelineItem) {
  return (
    item.kind === "video_clip" ||
    item.kind === "image_clip" ||
    item.kind === "lottie_clip" ||
    item.kind === "generated_clip"
  );
}

export function viewerContextKindForItem(
  project: VideoProject,
  item: TimelineItem | null,
): ViewerContextKind | null {
  if (!item || item.kind === "audio_clip") {
    return null;
  }
  if (item.kind === "lottie_clip") {
    return "lottie";
  }
  if (item.kind === "caption") {
    return "caption";
  }
  if (item.kind === "overlay" && item.source.type === "text" && !isTemplateTimelineItem(item)) {
    return "text";
  }
  if (generatedAssetForTimelineItem(project, item)) {
    return "generated";
  }
  if (isTemplateTimelineItem(item)) {
    return "template";
  }
  if (
    isVisualTimelineSourceItem(item) ||
    item.kind === "overlay" ||
    item.kind === "hyperframe_scene"
  ) {
    return "visual";
  }
  return null;
}

export function viewerContextToolbarPlacementForItem(
  item: TimelineItem | null,
  kind: ViewerContextKind | null,
): "top" | "bottom" {
  if (kind === "caption") {
    // Center captions begin at 42% of the canvas, below the compact top tool lane.
    return item?.properties.captionPlacement === "upper" ? "bottom" : "top";
  }

  // Ordinary text overlays use the compositor's top-default geometry; other visuals keep bottom.
  return "bottom";
}

export function initialSelectedTimelineItem(project: VideoProject) {
  const items = project.timeline.tracks.flatMap((track) => track.items);

  return (
    items.find((item) => generatedAssetForTimelineItem(project, item) !== null) ??
    items.find(
      (item) =>
        item.kind === "video_clip" ||
        item.kind === "audio_clip" ||
        item.kind === "hyperframe_scene",
    ) ??
    items[0] ??
    null
  );
}

export function resolveContextToolbarPlacement(
  preferredPlacement: "top" | "bottom",
  canvasState: TimelinePreviewCanvasState,
): "top" | "bottom" | null {
  if (canvasState.issueState === "retry") {
    return null;
  }

  const preferredOccupied =
    preferredPlacement === "top" ? canvasState.topOccupied : canvasState.bottomOccupied;
  if (!preferredOccupied) {
    return preferredPlacement;
  }

  const alternatePlacement = preferredPlacement === "top" ? "bottom" : "top";
  const alternateOccupied =
    alternatePlacement === "top" ? canvasState.topOccupied : canvasState.bottomOccupied;
  return alternateOccupied ? null : alternatePlacement;
}
