import { roundTimelineSeconds } from "@/lib/format";
import { falAuraSrModelId, falVideoUpscalerModelId } from "@/lib/generation/provider-rules";
import type {
  MediaGenerationRequest,
  SourceClipGenerationContext,
  SourceClipUpscaleContext,
  SourceClipVideoAudioContext,
  SourceClipVideoAudioKind,
} from "@/lib/generation/types";
import { aspectRatioLabel } from "@/lib/media/names";
import type { GeneratedAsset, MediaAsset } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty } from "@/lib/timeline-ops/item-properties";

export const upscaleGenerationPrompt =
  "Upscale this clip while preserving composition, timing, motion, and subject details.";

export const videoToMusicGenerationPrompt = "Generate music that fits the video.";

export const videoToSfxGenerationPrompt = "Create matching sound for the video.";

export function generatedVariationId(assetId: string) {
  return `${assetId}-variation-${Date.now().toString(36)}`;
}

export function generatedVariationSetId(assetId: string, index: number) {
  return `${assetId}-variation-${Date.now().toString(36)}-${(index + 1).toString()}`;
}

export function generatedMediaAssetId() {
  return `generated-media-${Date.now().toString(36)}`;
}

export function upscaleGenerationRequest(
  media: MediaAsset,
  context?: SourceClipUpscaleContext,
): MediaGenerationRequest {
  const width = media.width && media.width > 0 ? media.width * 2 : 1920;
  const height = media.height && media.height > 0 ? media.height * 2 : 1080;
  const modelId = media.kind === "video" ? falVideoUpscalerModelId : falAuraSrModelId;
  const sourceSpan =
    context &&
    Number.isFinite(context.sourceIn) &&
    Number.isFinite(context.sourceOut) &&
    context.sourceOut > context.sourceIn
      ? {
          sourceIn: roundTimelineSeconds(context.sourceIn),
          sourceOut: roundTimelineSeconds(context.sourceOut),
          durationSeconds: roundTimelineSeconds(context.sourceOut - context.sourceIn),
        }
      : null;
  const durationSeconds =
    sourceSpan?.durationSeconds ??
    (media.durationSeconds > 0 ? roundTimelineSeconds(media.durationSeconds) : 4);

  return {
    kind: "generated",
    name: null,
    targetFolderId: null,
    placementIntent: "library",
    prompt: upscaleGenerationPrompt,
    model: {
      provider: "fal.ai",
      id: modelId,
    },
    references: {
      mediaIds: [media.id],
      ...(media.kind === "video" && sourceSpan ? { sourceVideoMediaRef: media.id } : {}),
      firstFrameMediaId: null,
      lastFrameMediaId: null,
    },
    settings: {
      width,
      height,
      durationSeconds,
      fps: media.fps ?? 24,
      aspectRatio: aspectRatioLabel(width, height),
      ...(media.kind === "video" && sourceSpan
        ? {
            videoSourceStartSeconds: sourceSpan.sourceIn,
            videoSourceEndSeconds: sourceSpan.sourceOut,
          }
        : {}),
    },
  };
}

export function videoAudioGenerationRequest(
  media: MediaAsset,
  kind: SourceClipVideoAudioKind,
  context: SourceClipVideoAudioContext,
): MediaGenerationRequest {
  const sourceSpan =
    typeof context.sourceIn === "number" &&
    typeof context.sourceOut === "number" &&
    Number.isFinite(context.sourceIn) &&
    Number.isFinite(context.sourceOut) &&
    context.sourceOut > context.sourceIn
      ? {
          sourceIn: roundTimelineSeconds(context.sourceIn),
          sourceOut: roundTimelineSeconds(context.sourceOut),
          durationSeconds: roundTimelineSeconds(context.sourceOut - context.sourceIn),
        }
      : null;
  const durationSeconds =
    sourceSpan?.durationSeconds ??
    (context.durationSeconds > 0
      ? roundTimelineSeconds(context.durationSeconds)
      : media.durationSeconds > 0
        ? roundTimelineSeconds(media.durationSeconds)
        : 4);
  const isMusic = kind === "music";

  return {
    kind: "audio",
    name: isMusic ? "Generated music" : "Generated sound effects",
    targetFolderId: null,
    placementIntent: "timeline",
    prompt: isMusic ? videoToMusicGenerationPrompt : videoToSfxGenerationPrompt,
    model: {
      provider: "fal.ai",
      id: isMusic ? "sonilo/v1.1/video-to-music" : "mirelo-ai/sfx-v1.5/video-to-audio",
    },
    references: {
      mediaIds: [media.id],
      sourceVideoMediaRef: media.id,
      firstFrameMediaId: null,
      lastFrameMediaId: null,
    },
    settings: {
      width: null,
      height: null,
      durationSeconds,
      fps: null,
      aspectRatio: null,
      category: isMusic ? "music" : "sfx",
      timelineStartSeconds: roundTimelineSeconds(context.timelineStartSeconds),
      ...(sourceSpan
        ? {
            videoSourceStartSeconds: sourceSpan.sourceIn,
            videoSourceEndSeconds: sourceSpan.sourceOut,
          }
        : {}),
    },
  };
}

const importedVideoEditMaxDurationSeconds = 10;
const fourKVideoHeight = 2160;
const upscaleModelIds = new Set([falAuraSrModelId, falVideoUpscalerModelId]);

export function importedUpscaleLimitReasonForMedia(mediaAsset: MediaAsset | null) {
  if (!mediaAsset) {
    return null;
  }
  if (mediaAsset.kind === "video") {
    if (!mediaAsset.height || mediaAsset.height <= 0) {
      return "Loading video metadata...";
    }
    if (mediaAsset.height >= fourKVideoHeight) {
      return "Already 4K or higher";
    }
  }
  return null;
}

export function generatedUpscaleLimitReasonForAsset(generatedAsset: GeneratedAsset | null) {
  return generatedAsset && upscaleModelIds.has(generatedAsset.model.id)
    ? "Already upscaled"
    : null;
}

export function sourceClipUpscaleContextForItem(
  item: TimelineItem | null,
  mediaAsset: MediaAsset | null,
): SourceClipUpscaleContext | undefined {
  if (!item) {
    return undefined;
  }
  const sourceInValue = numberProperty(item, "sourceIn");
  const sourceOutValue = numberProperty(item, "sourceOut");
  if (
    sourceInValue === null ||
    sourceOutValue === null ||
    sourceOutValue <= sourceInValue
  ) {
    return undefined;
  }
  if (
    mediaAsset?.durationSeconds &&
    mediaAsset.durationSeconds > 0 &&
    sourceInValue <= 0.001 &&
    sourceOutValue >= mediaAsset.durationSeconds - 0.001
  ) {
    return undefined;
  }
  return {
    itemId: item.id,
    sourceIn: sourceInValue,
    sourceOut: sourceOutValue,
  };
}

export function sourceClipGenerationContextForItem(
  item: TimelineItem | null,
): SourceClipGenerationContext | null {
  if (!item) {
    return null;
  }
  const context: SourceClipGenerationContext = {
    itemId: item.id,
    timelineStartSeconds: item.startSeconds,
    durationSeconds: item.durationSeconds,
  };
  const sourceInValue = numberProperty(item, "sourceIn");
  const sourceOutValue = numberProperty(item, "sourceOut");
  if (
    sourceInValue !== null &&
    sourceOutValue !== null &&
    sourceOutValue > sourceInValue
  ) {
    context.sourceIn = sourceInValue;
    context.sourceOut = sourceOutValue;
  }
  return context;
}

export function importedVideoEditLimitReasonForMedia(
  mediaAsset: MediaAsset | null,
  sourceClipGenerationContext: SourceClipGenerationContext | null,
) {
  if (mediaAsset?.kind !== "video") {
    return null;
  }
  const duration =
    sourceClipGenerationContext?.durationSeconds ?? mediaAsset.durationSeconds;
  if (
    !Number.isFinite(duration) ||
    duration <= importedVideoEditMaxDurationSeconds
  ) {
    return null;
  }
  return `Edit supports up to ${importedVideoEditMaxDurationSeconds}s (this is ${Math.round(duration)}s)`;
}
