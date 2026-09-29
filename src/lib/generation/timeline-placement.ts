import { roundTimelineSeconds } from "@/lib/format";
import type {
  MediaGenerationRequest,
  MediaGenerationTimelineSourceRange,
  MediaGenerationTimelineTargets,
} from "@/lib/generation/types";
import { mediaDisplayName } from "@/lib/media/names";
import { isVisualTimelineSourceItem } from "@/lib/preview/viewer-context";
import type {
  GeneratedAsset,
  GenerationPlacementIntent,
  MediaAsset,
  ProjectAction,
  VideoProject,
} from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { timelineMediaItemId } from "@/lib/timeline-ops/ids";
import { stringProperty } from "@/lib/timeline-ops/item-properties";
import type { TimelineRangeSelection } from "@/lib/timeline-ops/navigation";

export function generatedAssetPlacementContextLabel(
  placementIntent: GenerationPlacementIntent | null | undefined,
) {
  if (placementIntent === "timeline") {
    return "Timeline target";
  }

  if (placementIntent === "library") {
    return "Library";
  }

  if (placementIntent?.startsWith("replace:")) {
    return "Replacement target";
  }

  return null;
}

export function timelineGenerationTrackForKind(
  project: VideoProject,
  targetKind: "video" | "audio",
) {
  return (
    project.timeline.tracks.find(
      (track) => track.kind === targetKind && !track.locked,
    ) ?? null
  );
}

/**
 * The timeline clip for a media asset at `startSeconds`: canonical item id, display label and
 * the full source range. Stills and assets without a duration last 4 s. Null for missing media.
 */
export function mediaTimelineItem(
  project: VideoProject,
  mediaId: string,
  startSeconds: number,
): TimelineItem | null {
  const media = project.media.find((asset) => asset.id === mediaId) ?? null;
  if (!media) {
    return null;
  }

  const durationSeconds = roundTimelineSeconds(
    media.durationSeconds > 0 ? media.durationSeconds : 4,
  );
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return null;
  }

  return {
    id: timelineMediaItemId(project, media.id),
    kind: media.kind === "audio" ? "audio_clip" : "video_clip",
    startSeconds: roundTimelineSeconds(startSeconds),
    durationSeconds,
    source: { type: "media", mediaId: media.id },
    label: mediaDisplayName(media),
    properties: {
      sourceIn: 0,
      sourceOut: durationSeconds,
    },
  };
}

export function mediaTimelineAction(
  project: VideoProject,
  mediaId: string,
): { action: ProjectAction; itemId: string } | null {
  const media = project.media.find((asset) => asset.id === mediaId) ?? null;
  if (!media) {
    return null;
  }

  const targetKind = media.kind === "audio" ? "audio" : "video";
  const targetTrack = timelineGenerationTrackForKind(project, targetKind);
  if (!targetTrack) {
    return null;
  }

  const startSeconds = targetTrack.items.reduce(
    (endSeconds, item) => Math.max(endSeconds, item.startSeconds + item.durationSeconds),
    0,
  );
  const item = mediaTimelineItem(project, media.id, startSeconds);
  if (!item) {
    return null;
  }

  return {
    itemId: item.id,
    action: {
      type: "addItems",
      targetTrackId: targetTrack.id,
      items: [item],
    },
  };
}

export function generatedOutputTimelineItemId(mediaId: string, role: "primary" | "audio" = "primary") {
  const slug = mediaId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  const roleSuffix = role === "audio" ? "-audio" : "";
  return `generated-output-${slug || "media"}${roleSuffix}-${Date.now().toString(36)}`;
}

export function generatedTimelinePlaceholderItemId(assetId: string) {
  const slug = assetId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return `generated-placeholder-${slug || "asset"}`;
}

export function generatedAssetPlacementIntent(
  asset: VideoProject["generatedAssets"][number] | null,
): GenerationPlacementIntent {
  return asset?.placementIntent ?? "library";
}

export function generatedReplacementPlacementIntent(
  itemId: string,
): GenerationPlacementIntent {
  return `replace:${itemId}`;
}

export function generatedComposerPlacementForItem(
  item: TimelineItem | null,
): GenerationPlacementIntent {
  return item ? `replace:${item.id}` : "library";
}

export function generatedPlacementLabel(asset: VideoProject["generatedAssets"][number]) {
  if (asset.placementIntent === "timeline") {
    return "Timeline target";
  }

  if (asset.placementIntent?.startsWith("replace:")) {
    return "Replacement target";
  }

  return "Library";
}

export function timelineGenerationTargetForKind(
  project: VideoProject,
  targetKind: "video" | "audio",
) {
  const targetTrack = timelineGenerationTrackForKind(project, targetKind);
  if (!targetTrack) {
    return null;
  }

  const startSeconds = roundTimelineSeconds(
    targetTrack.items.reduce(
      (endSeconds, item) => Math.max(endSeconds, item.startSeconds + item.durationSeconds),
      0,
    ),
  );

  return {
    trackName: targetTrack.name,
    startSeconds,
  };
}

export function mediaGenerationTargetKind(kind: MediaGenerationRequest["kind"]): "video" | "audio" {
  return kind === "audio" ? "audio" : "video";
}

export function generatedTimelineStartSecondsFromSettings(
  settings: GeneratedAsset["settings"],
  fallbackStartSeconds: number,
) {
  const startSeconds = settings.timelineStartSeconds ?? settings.videoSourceStartSeconds;
  if (
    typeof startSeconds === "number" &&
    Number.isFinite(startSeconds) &&
    startSeconds >= 0
  ) {
    return roundTimelineSeconds(startSeconds);
  }
  return fallbackStartSeconds;
}

export function timelineItemAcceptsGeneratedOutputMedia(
  item: TimelineItem | null,
  media: MediaAsset | null,
) {
  if (!item || !media) {
    return false;
  }

  const outputType = generatedOutputMediaType(media);
  return item.kind === "audio_clip" ? outputType === "audio" : outputType !== "audio";
}

type GeneratedOutputMediaType = "audio" | "image" | "video";

export function generatedOutputMediaType(media: MediaAsset): GeneratedOutputMediaType {
  if (media.kind === "audio") {
    return "audio";
  }
  if (media.kind === "image") {
    return "image";
  }
  if (media.kind === "video") {
    return "video";
  }

  const cleanPath = media.relativePath.split(/[?#]/, 1)[0] ?? media.relativePath;
  const extension = cleanPath.split(".").at(-1)?.toLowerCase() ?? "";
  if (["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus"].includes(extension)) {
    return "audio";
  }
  if (
    ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif", "avif"].includes(
      extension,
    )
  ) {
    return "image";
  }
  return "video";
}

export function generatedTimelinePlaceholderAction(
  project: VideoProject,
  assetId: string,
  request: MediaGenerationRequest,
): ProjectAction | null {
  if (request.placementIntent !== "timeline") {
    return null;
  }

  const targetKind = mediaGenerationTargetKind(request.kind);
  const targetTrack = timelineGenerationTrackForKind(project, targetKind);
  if (!targetTrack) {
    return null;
  }

  const requestedDurationSeconds = request.settings.durationSeconds;
  if (requestedDurationSeconds === null) {
    return null;
  }

  const durationSeconds = roundTimelineSeconds(requestedDurationSeconds);
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return null;
  }

  const fallbackStartSeconds = roundTimelineSeconds(
    targetTrack.items.reduce(
      (endSeconds, item) => Math.max(endSeconds, item.startSeconds + item.durationSeconds),
      0,
    ),
  );
  const startSeconds = generatedTimelineStartSecondsFromSettings(
    request.settings,
    fallbackStartSeconds,
  );
  const item: TimelineItem = {
    id: generatedTimelinePlaceholderItemId(assetId),
    kind: targetKind === "audio" ? "audio_clip" : "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "generated", artifactId: assetId },
    label: request.name?.trim() || (targetKind === "audio" ? "Queued audio" : "Queued generation"),
    properties: {
      generatedAssetId: assetId,
      generatedTimelinePlaceholder: true,
      sourceIn: 0,
      sourceOut: durationSeconds,
    },
  };

  return {
    type: "addItems",
    targetTrackId: targetTrack.id,
    items: [item],
  };
}

export function generatedTimelinePlaceholderForAsset(
  project: VideoProject,
  assetId: string,
): { track: TimelineTrack; item: TimelineItem } | null {
  for (const track of project.timeline.tracks) {
    const item =
      track.items.find(
        (candidate) =>
          stringProperty(candidate, "generatedAssetId") === assetId &&
          candidate.properties.generatedTimelinePlaceholder === true,
      ) ?? null;
    if (item) {
      return { track, item };
    }
  }

  return null;
}

export function generatedTimelineOutputItem(
  generatedAsset: VideoProject["generatedAssets"][number],
  media: MediaAsset,
  itemId: string,
  itemKind: "audio_clip" | "image_clip" | "video_clip",
  startSeconds: number,
  durationSeconds: number,
  linkGroupId: string | null = null,
) {
  const properties: TimelineItem["properties"] = {
    generatedAssetId: generatedAsset.id,
    generatedOutputMediaId: media.id,
    ...(linkGroupId ? { linkGroupId } : {}),
  };
  if (itemKind !== "image_clip") {
    properties.sourceIn = 0;
    properties.sourceOut = durationSeconds;
  }

  return {
    id: itemId,
    kind: itemKind,
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: media.id },
    label: generatedTimelineOutputLabel(generatedAsset, media),
    properties,
  } satisfies TimelineItem;
}

export function generatedTimelineOutputLabel(
  generatedAsset: VideoProject["generatedAssets"][number],
  media: MediaAsset,
) {
  return generatedAsset.name?.trim() || media.id;
}

export function generatedOutputTimelineActions(
  project: VideoProject,
  mediaId: string,
): { actions: ProjectAction[]; itemId: string } | null {
  const generatedAsset =
    project.generatedAssets.find((asset) =>
      asset.outputs.some((output) => output.mediaId === mediaId),
    ) ?? null;
  const output =
    generatedAsset?.outputs.find((candidate) => candidate.mediaId === mediaId) ?? null;
  const media = project.media.find((asset) => asset.id === mediaId) ?? null;
  if (!generatedAsset || !output || !media) {
    return null;
  }

  const durationSeconds = roundTimelineSeconds(output.durationSeconds);
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return null;
  }

  const outputType = generatedOutputMediaType(media);
  const itemKind =
    outputType === "audio"
      ? "audio_clip"
      : outputType === "image"
        ? "image_clip"
        : "video_clip";
  const primaryItemId = generatedOutputTimelineItemId(media.id);
  const audioTrack =
    outputType === "video" && generatedAsset.settings.generateAudio === true
      ? timelineGenerationTrackForKind(project, "audio")
      : null;
  const linkGroupId = audioTrack ? `link-${primaryItemId}` : null;

  const insertionActions = (
    targetTrackId: string,
    startSeconds: number,
    itemDurationSeconds: number,
  ): ProjectAction[] => {
    const primaryItem = generatedTimelineOutputItem(
      generatedAsset,
      media,
      primaryItemId,
      itemKind,
      startSeconds,
      itemDurationSeconds,
      linkGroupId,
    );
    const actions: ProjectAction[] = [
      {
        type: "addItems",
        targetTrackId,
        items: [primaryItem],
      },
    ];
    if (audioTrack && linkGroupId) {
      actions.push({
        type: "addItems",
        targetTrackId: audioTrack.id,
        items: [
          generatedTimelineOutputItem(
            generatedAsset,
            media,
            generatedOutputTimelineItemId(media.id, "audio"),
            "audio_clip",
            startSeconds,
            itemDurationSeconds,
            linkGroupId,
          ),
        ],
      });
    }
    return actions;
  };

  const placeholder = generatedTimelinePlaceholderForAsset(project, generatedAsset.id);
  if (placeholder) {
    return {
      itemId: primaryItemId,
      actions: [
        {
          type: "removeItems",
          itemIds: [placeholder.item.id],
        },
        ...insertionActions(
          placeholder.track.id,
          placeholder.item.startSeconds,
          placeholder.item.durationSeconds,
        ),
      ],
    };
  }

  const targetKind = outputType === "audio" ? "audio" : "video";
  const targetTrack = project.timeline.tracks.find(
    (track) => track.kind === targetKind && !track.locked,
  );
  if (!targetTrack) {
    return null;
  }

  const fallbackStartSeconds = roundTimelineSeconds(
    targetTrack.items.reduce(
      (endSeconds, item) => Math.max(endSeconds, item.startSeconds + item.durationSeconds),
      0,
    ),
  );
  const startSeconds = generatedTimelineStartSecondsFromSettings(
    generatedAsset.settings,
    fallbackStartSeconds,
  );
  return {
    itemId: primaryItemId,
    actions: insertionActions(targetTrack.id, startSeconds, durationSeconds),
  };
}

export function timelineGenerationTargets(project: VideoProject): MediaGenerationTimelineTargets {
  const videoTarget = timelineGenerationTargetForKind(project, "video");
  const audioTarget = timelineGenerationTargetForKind(project, "audio");

  return {
    ...(videoTarget ? { image: videoTarget, video: videoTarget } : {}),
    ...(audioTarget ? { audio: audioTarget } : {}),
  };
}

export function timelineGenerationSourceRange(
  project: VideoProject,
  selectedRange: TimelineRangeSelection | null,
): MediaGenerationTimelineSourceRange | null {
  if (
    selectedRange &&
    selectedRange.endSeconds > selectedRange.startSeconds &&
    timelineHasVisualSourceInRange(project, selectedRange.startSeconds, selectedRange.endSeconds)
  ) {
    return {
      label: "Selected timeline range",
      startSeconds: roundTimelineSeconds(selectedRange.startSeconds),
      endSeconds: roundTimelineSeconds(selectedRange.endSeconds),
    };
  }

  const visualEndSeconds = timelineVisualSourceEndSeconds(project);
  const endSeconds = roundTimelineSeconds(
    Math.max(project.timeline.durationSeconds, visualEndSeconds),
  );
  if (endSeconds <= 0 || !timelineHasVisualSourceInRange(project, 0, endSeconds)) {
    return null;
  }

  return {
    label: "Whole timeline",
    startSeconds: 0,
    endSeconds,
  };
}

export function timelineVisualSourceEndSeconds(project: VideoProject) {
  return project.timeline.tracks
    .filter((track) => track.kind === "video" && !track.locked)
    .flatMap((track) => track.items)
    .filter(isVisualTimelineSourceItem)
    .reduce(
      (endSeconds, item) => Math.max(endSeconds, item.startSeconds + item.durationSeconds),
      0,
    );
}

export function timelineHasVisualSourceInRange(
  project: VideoProject,
  startSeconds: number,
  endSeconds: number,
) {
  return project.timeline.tracks
    .filter((track) => track.kind === "video" && !track.locked)
    .some((track) =>
      track.items
        .filter(isVisualTimelineSourceItem)
        .some(
          (item) =>
            Math.min(item.startSeconds + item.durationSeconds, endSeconds) >
            Math.max(item.startSeconds, startSeconds),
        ),
    );
}
