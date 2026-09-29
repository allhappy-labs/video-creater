import { roundGenerationTimelineSeconds } from "@/lib/format";
import type {
  GenerationModel,
  GenerationModelCatalog,
  GenerationModelOption,
  GenerationReferenceLimits,
  MediaGenerationMode,
  MediaGenerationTimelineSourceRange,
} from "@/lib/generation/types";
import type { GeneratedAsset } from "@/lib/project";

export const falFluxSchnellModelId = "fal-ai/flux/schnell";
export const falWanTextToVideoModelId = "fal-ai/wan-25-preview/text-to-video";
const mockCompletableFalModelIds = new Set([
  falFluxSchnellModelId,
  falWanTextToVideoModelId,
  "sonilo/v1.1/text-to-music",
]);
export const falNanoBananaProEditModelId = "fal-ai/nano-banana-pro/edit";
export const falWanImageToVideoModelId = "fal-ai/wan/v2.7/image-to-video";
export const falWanReferenceToVideoModelId = "fal-ai/wan/v2.7/reference-to-video";
export const falWanVideoToVideoModelId = "fal-ai/wan/v2.2-a14b/video-to-video";
export const falKlingStandardTextToVideoModelId =
  "fal-ai/kling-video/v3/standard/text-to-video";
export const falKlingProImageToVideoModelId =
  "fal-ai/kling-video/v3/pro/image-to-video";
export const falKlingProMotionControlModelId =
  "fal-ai/kling-video/v3/pro/motion-control";
export const falKrea2TurboModelId = "fal-ai/krea-2/turbo";
export const falRecraftV3TextToImageModelId = "fal-ai/recraft/v3/text-to-image";
export const falAuraSrModelId = "fal-ai/aura-sr";
export const falVideoUpscalerModelId = "fal-ai/video-upscaler";
export const falSoniloTextToMusicModelId = "sonilo/v1.1/text-to-music";
const falSoniloVideoToMusicModelId = "sonilo/v1.1/video-to-music";
const falMireloVideoToAudioModelId = "mirelo-ai/sfx-v1.5/video-to-audio";
export const replicateSeedanceModelId = "bytedance/seedance-2.0";
export const replicateSeedanceFastModelId = "bytedance/seedance-2.0-fast";
export const openAiGptImage2ModelId = "gpt-image-2";
export const openAiGptImageEditModelId = "gpt-image-1.5";
export const openAiTtsModelId = "gpt-4o-mini-tts";
export const elevenLabsTtsModelId = "elevenlabs-tts-v3";
export const elevenLabsMusicModelId = "elevenlabs-music";
export const minimaxMusicModelId = "minimax-music-v2.6";
export const googleGeminiTtsModelId = "gemini-3.1-flash-tts-preview";
export const googleLyriaModelId = "lyria3-pro";
export const xAiGrokImageQualityModelId = "grok-imagine-image-quality";
export const xAiGrokVideoModelId = "grok-imagine-video";
export const googleVeo31FastModelId = "veo3.1-fast";

export const organizeMediaPrompt = [
  "Organize the current project media into clear folders using media metadata and content.",
  "Do not delete any media.",
  "Do not add, remove, move, trim, or otherwise change timeline items.",
  "Return reviewed project actions only; do not mutate canonical files directly.",
].join(" ");

const generationProviderDisplayNames: Readonly<Record<string, string>> = {
  "fal.ai": "fal.ai",
  replicate: "Replicate",
  openai: "OpenAI",
  xai: "xAI",
  elevenlabs: "ElevenLabs",
  google: "Google",
  minimax: "MiniMax",
};

export function generationProviderDisplayName(provider: string) {
  const normalizedProvider = provider.trim().toLowerCase();
  return generationProviderDisplayNames[normalizedProvider] ?? provider.trim();
}

export function timelineTargetMode(mode: MediaGenerationMode): MediaGenerationMode {
  return mode === "audio" ? "audio" : "video";
}

export function timelineTargetKindLabel(mode: MediaGenerationMode) {
  return mode === "audio" ? "audio" : "video";
}

export function generationModeLabel(mode: MediaGenerationMode) {
  return mode.charAt(0).toUpperCase() + mode.slice(1);
}

export const defaultGenerationModels = {
  image: { provider: "openai", id: openAiGptImage2ModelId },
  video: { provider: "replicate", id: replicateSeedanceFastModelId },
  audio: { provider: "fal.ai", id: "bytedance/seed-audio-1.0" },
} satisfies Record<MediaGenerationMode, GenerationModel>;

export const generationModelOptions: Record<MediaGenerationMode, readonly GenerationModelOption[]> = {
  image: [
    defaultGenerationModels.image,
    { provider: "fal.ai", id: falNanoBananaProEditModelId },
    { provider: "fal.ai", id: falFluxSchnellModelId },
    { provider: "fal.ai", id: falKrea2TurboModelId },
    { provider: "fal.ai", id: falRecraftV3TextToImageModelId },
    { provider: "replicate", id: "black-forest-labs/flux-schnell" },
    { provider: "replicate", id: "black-forest-labs/flux-dev" },
    { provider: "replicate", id: "black-forest-labs/flux-1.1-pro" },
    { provider: "replicate", id: "black-forest-labs/flux-1.1-pro-ultra" },
    { provider: "openai", id: openAiGptImageEditModelId },
    { provider: "xai", id: xAiGrokImageQualityModelId },
  ],
  video: [
    defaultGenerationModels.video,
    { provider: "fal.ai", id: falWanTextToVideoModelId },
    { provider: "fal.ai", id: falWanImageToVideoModelId },
    { provider: "fal.ai", id: falWanReferenceToVideoModelId },
    { provider: "fal.ai", id: falWanVideoToVideoModelId },
    { provider: "fal.ai", id: falKlingStandardTextToVideoModelId },
    { provider: "fal.ai", id: falKlingProImageToVideoModelId },
    { provider: "fal.ai", id: falKlingProMotionControlModelId },
    { provider: "replicate", id: replicateSeedanceModelId },
    { provider: "xai", id: xAiGrokVideoModelId },
    { provider: "google", id: googleVeo31FastModelId },
  ],
  audio: [
    defaultGenerationModels.audio,
    { provider: "fal.ai", id: falSoniloTextToMusicModelId },
    { provider: "fal.ai", id: falSoniloVideoToMusicModelId },
    { provider: "fal.ai", id: falMireloVideoToAudioModelId },
    { provider: "openai", id: openAiTtsModelId },
    { provider: "elevenlabs", id: elevenLabsTtsModelId },
    { provider: "elevenlabs", id: elevenLabsMusicModelId },
    { provider: "minimax", id: minimaxMusicModelId },
    { provider: "google", id: googleGeminiTtsModelId },
    { provider: "google", id: googleLyriaModelId },
  ],
};

export const defaultGenerationModelValues: Record<MediaGenerationMode, string> = {
  image: generationModelValue(defaultGenerationModels.image),
  video: generationModelValue(defaultGenerationModels.video),
  audio: generationModelValue(defaultGenerationModels.audio),
};

export function generationModelValue(model: GenerationModel) {
  return `${model.provider}:${model.id}`;
}

export function generationRequestModel(model: GenerationModel): GenerationModel {
  return {
    provider: model.provider,
    id: model.id,
  };
}

export function generationModelOptionsFromCatalog(
  catalog: GenerationModelCatalog | null | undefined,
): Record<MediaGenerationMode, readonly GenerationModelOption[]> {
  return {
    image: catalog?.image?.length ? catalog.image : generationModelOptions.image,
    video: catalog?.video?.length ? catalog.video : generationModelOptions.video,
    audio: catalog?.audio?.length ? catalog.audio : generationModelOptions.audio,
  };
}

export function generationModelFromValue(
  mode: MediaGenerationMode,
  value: string,
  options: Record<MediaGenerationMode, readonly GenerationModelOption[]> = generationModelOptions,
) {
  return (
    options[mode].find((model) => generationModelValue(model) === value) ??
    options[mode][0] ??
    defaultGenerationModels[mode]
  );
}

export function selectedGenerationModel(
  mode: MediaGenerationMode,
  value?: string,
  options: Record<MediaGenerationMode, readonly GenerationModelOption[]> = generationModelOptions,
) {
  return value
    ? generationModelFromValue(mode, value, options)
    : (options[mode][0] ?? defaultGenerationModels[mode]);
}

export function isGenerationModelValueForMode(
  mode: MediaGenerationMode,
  value: string,
  options: Record<MediaGenerationMode, readonly GenerationModelOption[]> = generationModelOptions,
) {
  return options[mode].some(
    (model) => generationModelValue(model) === value,
  );
}

export function generatedAssetHasRerunnableModel(
  asset: GeneratedAsset,
  catalog: GenerationModelCatalog | null | undefined,
  options: Record<MediaGenerationMode, readonly GenerationModelOption[]>,
) {
  const mode = generationModeFromAsset(asset);
  const modelValue = generationModelValue(asset.model);
  if (isGenerationModelValueForMode(mode, modelValue, options)) {
    return true;
  }

  if (catalog) {
    return Boolean(
      catalog.upscale?.some(
        (model) => generationModelValue(model) === modelValue,
      ),
    );
  }

  return (
    asset.model.provider === "fal.ai" &&
    (asset.model.id === falAuraSrModelId || asset.model.id === falVideoUpscalerModelId)
  );
}

export function generationModelLabel(model: GenerationModelOption) {
  if (model.displayName?.trim()) {
    return model.displayName.trim();
  }
  if (model.provider === "fal.ai" && model.id.startsWith("fal-ai/")) {
    return model.id;
  }

  return `${model.provider}/${model.id}`;
}

export function canCompleteWithMockWorker(asset: GeneratedAsset) {
  return (
    (asset.model.provider === "fal.ai" &&
      mockCompletableFalModelIds.has(asset.model.id)) ||
    (asset.model.provider === "openai" && asset.model.id === openAiGptImage2ModelId) ||
    (asset.model.provider === "xai" && asset.model.id === xAiGrokImageQualityModelId)
  );
}

export function generationModeFromAsset(asset: Pick<GeneratedAsset, "kind" | "model" | "settings">): MediaGenerationMode {
  if (asset.kind === "image") {
    return "image";
  }

  if (
    asset.kind === "audio" ||
    asset.model.provider === "elevenlabs" ||
    (asset.settings.width === null && asset.settings.height === null)
  ) {
    return "audio";
  }

  if (asset.settings.durationSeconds === null && asset.settings.fps === null) {
    return "image";
  }

  return "video";
}

export function generationModelSupportsReferenceMedia(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "image" && model.supportsImageReference !== undefined) {
    return model.supportsImageReference;
  }
  if (mode === "video" && model.supportsReferences !== undefined) {
    return model.supportsReferences;
  }
  if (mode === "audio") {
    return false;
  }

  if (mode === "image" && model.provider === "replicate") {
    return false;
  }

  if (
    mode === "image" &&
    model.provider === "fal.ai" &&
    (model.id === falKrea2TurboModelId ||
      model.id === falRecraftV3TextToImageModelId)
  ) {
    return false;
  }

  if (
    mode === "image" &&
    model.provider === "openai" &&
    model.id === openAiGptImage2ModelId
  ) {
    return false;
  }

  return true;
}

export function generationModelUsesTypedImageReferences(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "image" && model.supportsImageReference !== undefined) {
    return model.supportsImageReference;
  }
  return (
    mode === "image" &&
    ((model.provider === "fal.ai" && model.id === falNanoBananaProEditModelId) ||
      (model.provider === "openai" && model.id === openAiGptImageEditModelId) ||
      (model.provider === "xai" && model.id === xAiGrokImageQualityModelId))
  );
}

export function generationModelRequiresSelectedSourceVideo(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "video" && model.requiresSourceVideo !== undefined) {
    return model.requiresSourceVideo;
  }
  return (
    model.provider === "fal.ai" &&
    ((mode === "audio" &&
      (model.id === falSoniloVideoToMusicModelId ||
        model.id === falMireloVideoToAudioModelId)) ||
      (mode === "video" &&
        (model.id === falWanVideoToVideoModelId ||
          model.id === falKlingProMotionControlModelId)))
  );
}

export function generationModelAcceptsSelectedSourceVideo(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "video" && model.supportsSourceVideo !== undefined) {
    return model.supportsSourceVideo || Boolean(model.requiresSourceVideo);
  }
  return (
    generationModelRequiresSelectedSourceVideo(mode, model) ||
    (mode === "video" &&
      model.provider === "fal.ai" &&
      model.id === falWanVideoToVideoModelId) ||
    (mode === "video" &&
      model.provider === "xai" &&
      model.id === xAiGrokVideoModelId)
  );
}

export function generationModelPreservesSelectedSourceVideoSettings(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  return (
    mode === "video" &&
    model.provider === "fal.ai" &&
    model.id === falWanVideoToVideoModelId
  );
}

export function videoInputAudioCategory(model: GenerationModel): "music" | "sfx" {
  return model.id === falMireloVideoToAudioModelId ? "sfx" : "music";
}

export function validGenerationTimelineSourceRange(
  mode: MediaGenerationMode,
  model: GenerationModel,
  range: MediaGenerationTimelineSourceRange | null | undefined,
) {
  if (!generationModelRequiresSelectedSourceVideo(mode, model) || !range) {
    return null;
  }
  if (
    !Number.isFinite(range.startSeconds) ||
    !Number.isFinite(range.endSeconds) ||
    range.startSeconds < 0 ||
    range.endSeconds <= range.startSeconds
  ) {
    return null;
  }

  return {
    label: range.label,
    startSeconds: roundGenerationTimelineSeconds(range.startSeconds),
    endSeconds: roundGenerationTimelineSeconds(range.endSeconds),
  };
}

export function generationModelSupportsInstrumental(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "audio" && model.supportsInstrumental !== undefined) {
    return model.supportsInstrumental;
  }
  return (
    mode === "audio" &&
    ((model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId) ||
      (model.provider === "fal.ai" && model.id === falSoniloTextToMusicModelId) ||
      (model.provider === "minimax" && model.id === minimaxMusicModelId) ||
      (model.provider === "google" && model.id === googleLyriaModelId))
  );
}

export function generationModelSupportsAudioToggle(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode !== "video") {
    return false;
  }
  if (
    model.provider === "fal.ai" &&
    [
      falWanTextToVideoModelId,
      falWanImageToVideoModelId,
      falWanReferenceToVideoModelId,
      falKlingStandardTextToVideoModelId,
      falKlingProImageToVideoModelId,
      falKlingProMotionControlModelId,
    ].includes(model.id)
  ) {
    return true;
  }
  if (
    model.provider === "replicate" &&
    [replicateSeedanceModelId, replicateSeedanceFastModelId].includes(model.id)
  ) {
    return true;
  }
  return model.provider === "google" && model.id === googleVeo31FastModelId;
}

export function generationModelSupportsLyrics(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "audio" && model.supportsLyrics !== undefined) {
    return model.supportsLyrics;
  }
  return (
    mode === "audio" &&
    ((model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId) ||
      (model.provider === "minimax" && model.id === minimaxMusicModelId) ||
      (model.provider === "google" && model.id === googleLyriaModelId))
  );
}

export function generationModelSupportsStyleInstructions(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "audio" && model.supportsStyleInstructions !== undefined) {
    return model.supportsStyleInstructions;
  }
  return (
    mode === "audio" &&
    ((model.provider === "openai" && model.id === openAiTtsModelId) ||
      (model.provider === "elevenlabs" && model.id === elevenLabsTtsModelId) ||
      (model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId) ||
      (model.provider === "fal.ai" && model.id === falSoniloTextToMusicModelId) ||
      (model.provider === "minimax" && model.id === minimaxMusicModelId) ||
      (model.provider === "google" && model.id === googleGeminiTtsModelId) ||
      (model.provider === "google" && model.id === googleLyriaModelId))
  );
}

export function generationModelMinPromptLength(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (
    mode === "audio" &&
    Number.isFinite(model.minPromptLength) &&
    model.minPromptLength !== undefined &&
    model.minPromptLength >= 0
  ) {
    return Math.floor(model.minPromptLength);
  }
  if (
    mode === "audio" &&
    model.provider === "minimax" &&
    model.id === minimaxMusicModelId
  ) {
    return 10;
  }
  return 1;
}

export function generationPromptReadinessMessage(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  prompt: string,
) {
  const minLength = generationModelMinPromptLength(mode, model);
  const promptLength = prompt.trim().length;
  if (promptLength >= minLength) {
    return null;
  }
  if (promptLength === 0 && minLength <= 1) {
    return "Prompt required";
  }
  return `Prompt needs ${minLength} characters`;
}

export function generationAudioPromptCategory(model: GenerationModelOption) {
  if (model.category?.trim()) {
    return model.category.trim();
  }
  if (
    (model.provider === "openai" && model.id === openAiTtsModelId) ||
    (model.provider === "elevenlabs" && model.id === elevenLabsTtsModelId) ||
    (model.provider === "google" && model.id === googleGeminiTtsModelId)
  ) {
    return "tts";
  }
  if (
    (model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId) ||
    (model.provider === "fal.ai" && model.id === falSoniloTextToMusicModelId) ||
    (model.provider === "minimax" && model.id === minimaxMusicModelId) ||
    (model.provider === "google" && model.id === googleLyriaModelId) ||
    (model.provider === "fal.ai" && model.id === falSoniloVideoToMusicModelId)
  ) {
    return "music";
  }
  return "sfx";
}

export function generationPromptPlaceholder(
  mode: MediaGenerationMode,
  model: GenerationModel,
) {
  if (mode === "image") {
    return "Describe the image";
  }
  if (mode === "video") {
    return "Describe the video";
  }
  const minLength = generationModelMinPromptLength(mode, model);
  const promptHint = minLength > 1 ? ` (min ${minLength} chars)` : "";
  const category = generationAudioPromptCategory(model);
  if (category === "tts") {
    return `Text to speak${promptHint}`;
  }
  if (category === "music") {
    return `Describe the music style or mood${promptHint}`;
  }
  return `Describe the sound${promptHint}`;
}

export function generationAgentModePrompt(
  mode: MediaGenerationMode,
  model: GenerationModel,
) {
  if (mode !== "audio" || generationAudioPromptCategory(model) !== "music") {
    return null;
  }
  if (model.provider === "fal.ai" && model.id === falSoniloVideoToMusicModelId) {
    return "Score my timeline with music that matches the visuals. Use a video-to-music model on the full timeline span so the music follows the edit, and place it on an audio track.";
  }
  return "Generate music for my timeline and place it on an audio track aligned to the edit.";
}

export function generationModelRequiresFirstFrame(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "video" && model.supportsFirstFrame !== undefined) {
    return model.supportsFirstFrame && model.requiresReferenceImage === true;
  }
  return (
    mode === "video" &&
    model.provider === "fal.ai" &&
    (model.id === falWanImageToVideoModelId ||
      model.id === falKlingProImageToVideoModelId)
  );
}

export function generationModelSupportsFrameReferences(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (
    mode === "video" &&
    (model.supportsFirstFrame !== undefined || model.supportsLastFrame !== undefined)
  ) {
    return Boolean(model.supportsFirstFrame || model.supportsLastFrame);
  }
  return (
    mode === "video" &&
    ((model.provider === "fal.ai" &&
      (model.id === falWanImageToVideoModelId ||
        model.id === falKlingProImageToVideoModelId)) ||
      (model.provider === "replicate" &&
        (model.id === replicateSeedanceModelId ||
          model.id === replicateSeedanceFastModelId)) ||
      (model.provider === "xai" && model.id === xAiGrokVideoModelId) ||
      (model.provider === "google" && model.id === googleVeo31FastModelId))
  );
}

export function generationModelUsesExclusiveFrameReferenceModes(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "video" && model.framesAndReferencesExclusive !== undefined) {
    return model.framesAndReferencesExclusive;
  }
  return (
    mode === "video" &&
    ((model.provider === "replicate" &&
      (model.id === replicateSeedanceModelId ||
        model.id === replicateSeedanceFastModelId)) ||
      (model.provider === "xai" && model.id === xAiGrokVideoModelId))
  );
}

export function generationModelAcceptsTypedAudioReferences(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  const limits = generationModelReferenceLimits(mode, model);
  if (limits && model.supportsReferences !== undefined) {
    return model.supportsReferences && limits.maxReferenceAudios > 0;
  }
  return (
    mode === "video" &&
    ((model.provider === "fal.ai" &&
      (model.id === falWanTextToVideoModelId ||
        model.id === falWanImageToVideoModelId)) ||
      (model.provider === "replicate" &&
        (model.id === replicateSeedanceModelId ||
          model.id === replicateSeedanceFastModelId)))
  );
}

export function generationModelAcceptsTypedVisualReferences(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "image" && model.supportsImageReference !== undefined) {
    return (
      model.supportsImageReference &&
      (model.maxReferenceImages === undefined || model.maxReferenceImages > 0)
    );
  }
  const limits = generationModelReferenceLimits(mode, model);
  if (limits && model.supportsReferences !== undefined) {
    return (
      model.supportsReferences &&
      (limits.maxReferenceImages > 0 || limits.maxReferenceVideos > 0)
    );
  }
  return (
    mode === "video" &&
    ((model.provider === "fal.ai" && model.id === falWanReferenceToVideoModelId) ||
      (model.provider === "fal.ai" && model.id === falWanVideoToVideoModelId) ||
      (model.provider === "fal.ai" &&
        model.id === falKlingProMotionControlModelId) ||
      (model.provider === "replicate" &&
      (model.id === replicateSeedanceModelId ||
        model.id === replicateSeedanceFastModelId)) ||
      (model.provider === "xai" && model.id === xAiGrokVideoModelId) ||
      (model.provider === "google" && model.id === googleVeo31FastModelId))
  );
}

export function generationModelReferenceLimits(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
): GenerationReferenceLimits | null {
  if (mode !== "video") {
    return null;
  }
  if (
    model.requiresReferenceImage !== undefined ||
    model.maxReferenceImages !== undefined ||
    model.maxReferenceVideos !== undefined ||
    model.maxReferenceAudios !== undefined ||
    model.maxTotalReferences !== undefined ||
    model.maxCombinedVideoRefSeconds !== undefined ||
    model.maxCombinedAudioRefSeconds !== undefined
  ) {
    return {
      requiresReferenceImage: model.requiresReferenceImage ?? false,
      maxReferenceImages: model.maxReferenceImages ?? 0,
      maxReferenceVideos: model.maxReferenceVideos ?? 0,
      maxReferenceAudios: model.maxReferenceAudios ?? 0,
      maxTotalReferences: model.maxTotalReferences ?? null,
      maxCombinedVideoRefSeconds: model.maxCombinedVideoRefSeconds ?? null,
      maxCombinedAudioRefSeconds: model.maxCombinedAudioRefSeconds ?? null,
    };
  }
  if (model.provider === "fal.ai") {
    if (model.id === falWanTextToVideoModelId) {
      return {
        requiresReferenceImage: false,
        maxReferenceImages: 0,
        maxReferenceVideos: 0,
        maxReferenceAudios: 1,
        maxTotalReferences: 1,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falWanImageToVideoModelId) {
      return {
        requiresReferenceImage: false,
        maxReferenceImages: 2,
        maxReferenceVideos: 1,
        maxReferenceAudios: 1,
        maxTotalReferences: null,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falWanReferenceToVideoModelId) {
      return {
        requiresReferenceImage: true,
        maxReferenceImages: 4,
        maxReferenceVideos: 3,
        maxReferenceAudios: 0,
        maxTotalReferences: null,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falKlingStandardTextToVideoModelId) {
      return {
        requiresReferenceImage: false,
        maxReferenceImages: 0,
        maxReferenceVideos: 0,
        maxReferenceAudios: 0,
        maxTotalReferences: 0,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falKlingProImageToVideoModelId) {
      return {
        requiresReferenceImage: false,
        maxReferenceImages: 2,
        maxReferenceVideos: 0,
        maxReferenceAudios: 0,
        maxTotalReferences: null,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falKlingProMotionControlModelId) {
      return {
        requiresReferenceImage: true,
        maxReferenceImages: 1,
        maxReferenceVideos: 0,
        maxReferenceAudios: 0,
        maxTotalReferences: null,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
    if (model.id === falWanVideoToVideoModelId) {
      return {
        requiresReferenceImage: false,
        maxReferenceImages: 1,
        maxReferenceVideos: 0,
        maxReferenceAudios: 0,
        maxTotalReferences: null,
        maxCombinedVideoRefSeconds: null,
        maxCombinedAudioRefSeconds: null,
      };
    }
  }
  if (
    model.provider === "replicate" &&
    (model.id === replicateSeedanceModelId ||
      model.id === replicateSeedanceFastModelId)
  ) {
    return {
      requiresReferenceImage: false,
      maxReferenceImages: 4,
      maxReferenceVideos: 3,
      maxReferenceAudios: 3,
      maxTotalReferences: 6,
      maxCombinedVideoRefSeconds: 15,
      maxCombinedAudioRefSeconds: 15,
    };
  }
  if (model.provider === "xai" && model.id === xAiGrokVideoModelId) {
    return {
      requiresReferenceImage: false,
      maxReferenceImages: 7,
      maxReferenceVideos: 0,
      maxReferenceAudios: 0,
      maxTotalReferences: 7,
      maxCombinedVideoRefSeconds: null,
      maxCombinedAudioRefSeconds: null,
    };
  }
  if (model.provider === "google" && model.id === googleVeo31FastModelId) {
    return {
      requiresReferenceImage: false,
      maxReferenceImages: 3,
      maxReferenceVideos: 0,
      maxReferenceAudios: 0,
      maxTotalReferences: 3,
      maxCombinedVideoRefSeconds: null,
      maxCombinedAudioRefSeconds: null,
    };
  }
  return null;
}
