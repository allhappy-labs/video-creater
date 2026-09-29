import {
  generationModelAcceptsTypedAudioReferences,
  generationModelAcceptsTypedVisualReferences,
  generationModelLabel,
  generationModelReferenceLimits,
  generationModelSupportsReferenceMedia,
} from "@/lib/generation/provider-rules";
import type {
  GenerationModel,
  GenerationModelOption,
  GenerationReferencePromptTag,
  MediaGenerationMode,
  TypedGenerationReferenceMediaRefs,
} from "@/lib/generation/types";
import type { MediaAsset } from "@/lib/project";

export function generationReferencePromptTags(
  mediaIds: readonly string[],
  mediaAssets: readonly MediaAsset[],
): GenerationReferencePromptTag[] {
  const counts = { Image: 0, Video: 0, Audio: 0 };
  const tags: GenerationReferencePromptTag[] = [];
  for (const mediaId of mediaIds) {
    const asset = mediaAssets.find((candidate) => candidate.id === mediaId);
    const kindLabel =
      asset?.kind === "video" || asset?.kind === "generated"
        ? "Video"
        : asset?.kind === "audio"
          ? "Audio"
          : asset?.kind === "image"
            ? "Image"
            : null;
    if (!kindLabel) {
      continue;
    }
    counts[kindLabel] += 1;
    tags.push({
      mediaId,
      kindLabel,
      tag: `@${kindLabel}${counts[kindLabel]}`,
    });
  }
  return tags;
}

export function trailingReferenceTagQuery(prompt: string) {
  const match = /(^|[\s\n])@([A-Za-z0-9]*)$/.exec(prompt);
  return match?.[2] ?? null;
}

export function promptWithInsertedReferenceTag(prompt: string, tag: string) {
  if (/(^|[\s\n])@[A-Za-z0-9]*$/.test(prompt)) {
    return prompt.replace(/(^|[\s\n])@[A-Za-z0-9]*$/, `$1${tag} `);
  }
  return `${prompt}${prompt.endsWith(" ") || prompt.length === 0 ? "" : " "}${tag} `;
}

export function isVisualMediaAsset(asset: MediaAsset) {
  return asset.kind !== "audio";
}

export function isFrameReferenceMediaAsset(asset: MediaAsset) {
  return asset.kind === "image";
}

export function isSourceVideoMediaAsset(asset: MediaAsset) {
  return asset.kind === "video";
}

export function generationReferenceMediaForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  media: readonly MediaAsset[],
) {
  return media.filter((asset) => {
    if (mode === "image") {
      return generationModelSupportsReferenceMedia(mode, model) && isFrameReferenceMediaAsset(asset);
    }
    if (asset.kind === "audio") {
      return generationModelAcceptsTypedAudioReferences(mode, model);
    }
    return isVisualMediaAsset(asset);
  });
}

export function typedGenerationReferenceMediaRefs(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  mediaIds: readonly string[],
  media: readonly MediaAsset[],
) {
  const mediaById = new Map(media.map((asset) => [asset.id, asset]));
  const referenceImageMediaRefs: string[] = [];
  const referenceVideoMediaRefs: string[] = [];
  const referenceAudioMediaRefs: string[] = [];

  for (const mediaId of mediaIds) {
    const asset = mediaById.get(mediaId);
    if (!asset) {
      continue;
    }
    if (asset.kind === "audio") {
      if (generationModelAcceptsTypedAudioReferences(mode, model)) {
        referenceAudioMediaRefs.push(mediaId);
      }
      continue;
    }
    if (!generationModelAcceptsTypedVisualReferences(mode, model)) {
      continue;
    }
    if (asset.kind === "video" || asset.kind === "generated") {
      referenceVideoMediaRefs.push(mediaId);
    } else {
      referenceImageMediaRefs.push(mediaId);
    }
  }

  return {
    referenceImageMediaRefs,
    referenceVideoMediaRefs,
    referenceAudioMediaRefs,
  };
}

function referenceLimitMessage(
  displayName: string,
  noun: "image" | "video" | "audio",
  count: number,
  max: number,
) {
  if (count <= max) {
    return null;
  }
  if (max === 0) {
    return `${displayName} does not accept ${noun} references`;
  }
  const suffix = max === 1 ? "" : "s";
  return `${displayName} accepts at most ${max} ${noun} reference${suffix}`;
}

export function generationReferenceLimitMessage(
  mode: MediaGenerationMode,
  model: GenerationModel,
  references: TypedGenerationReferenceMediaRefs,
  mediaAssets: readonly MediaAsset[],
) {
  const limits = generationModelReferenceLimits(mode, model);
  if (!limits) {
    return null;
  }
  const displayName = generationModelLabel(model);
  return (
    (limits.requiresReferenceImage && references.referenceImageMediaRefs.length === 0
      ? `${displayName} requires an image reference`
      : null) ??
    referenceLimitMessage(
      displayName,
      "image",
      references.referenceImageMediaRefs.length,
      limits.maxReferenceImages,
    ) ??
    referenceLimitMessage(
      displayName,
      "video",
      references.referenceVideoMediaRefs.length,
      limits.maxReferenceVideos,
    ) ??
    referenceLimitMessage(
      displayName,
      "audio",
      references.referenceAudioMediaRefs.length,
      limits.maxReferenceAudios,
    ) ??
    (limits.maxTotalReferences !== null &&
    references.referenceImageMediaRefs.length +
      references.referenceVideoMediaRefs.length +
      references.referenceAudioMediaRefs.length >
      limits.maxTotalReferences
      ? `${displayName} accepts at most ${limits.maxTotalReferences} references total`
      : null) ??
    referenceDurationLimitMessage(
      "video",
      references.referenceVideoMediaRefs,
      mediaAssets,
      limits.maxCombinedVideoRefSeconds,
    ) ??
    referenceDurationLimitMessage(
      "audio",
      references.referenceAudioMediaRefs,
      mediaAssets,
      limits.maxCombinedAudioRefSeconds,
    )
  );
}

function referenceDurationLimitMessage(
  noun: "video" | "audio",
  mediaRefs: readonly string[],
  mediaAssets: readonly MediaAsset[],
  maxSeconds: number | null,
) {
  if (maxSeconds === null) {
    return null;
  }
  const totalSeconds = mediaRefs.reduce((total, mediaId) => {
    const asset = mediaAssets.find((candidate) => candidate.id === mediaId);
    return total + Math.max(asset?.durationSeconds ?? 0, 0);
  }, 0);
  return totalSeconds > maxSeconds
    ? `Combined ${noun} reference duration exceeds ${maxSeconds}s`
    : null;
}
