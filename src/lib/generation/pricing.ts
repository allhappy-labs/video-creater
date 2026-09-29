import {
  defaultGenerationModels,
  elevenLabsMusicModelId,
  elevenLabsTtsModelId,
  falFluxSchnellModelId,
  falKlingProImageToVideoModelId,
  falKlingProMotionControlModelId,
  falKlingStandardTextToVideoModelId,
  falKrea2TurboModelId,
  falNanoBananaProEditModelId,
  falRecraftV3TextToImageModelId,
  falWanImageToVideoModelId,
  falWanReferenceToVideoModelId,
  falWanTextToVideoModelId,
  generationModelValue,
  googleGeminiTtsModelId,
  googleLyriaModelId,
  googleVeo31FastModelId,
  minimaxMusicModelId,
  openAiGptImage2ModelId,
  openAiGptImageEditModelId,
  openAiTtsModelId,
  replicateSeedanceFastModelId,
  replicateSeedanceModelId,
  xAiGrokImageQualityModelId,
  xAiGrokVideoModelId,
} from "@/lib/generation/provider-rules";
import {
  fallbackVideoResolutionOption,
  generationDurationBoundsForModel,
  generationDurationOptionsForModel,
  generationQualityOptionsForModel,
  generationResolutionOptionsForModel,
  imageGenerationProviderResolution,
  selectedBoundedGenerationDurationSeconds,
  selectedGenerationQualityValue,
  selectedGenerationResolutionValue,
  selectedImageCount,
} from "@/lib/generation/settings-options";
import type {
  AudioGenerationPricing,
  GenerationCreditRateTable,
  GenerationModel,
  GenerationModelOption,
  GenerationResolutionOption,
  MediaGenerationMode,
} from "@/lib/generation/types";

export const mockGenerationCreditBalance = 4400;

export function selectedGenerationCost(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  durationValue: string,
  resolutionValue: string,
  imageCountValue: string,
  generateAudio: boolean,
  prompt: string,
  qualityValue: string,
) {
  const durationOptions = generationDurationOptionsForModel(mode, model);
  const durationBounds = generationDurationBoundsForModel(mode, model);
  const duration =
    durationOptions.find((option) => option.value === durationValue) ??
    durationOptions[0];
  const boundedDurationSeconds = selectedBoundedGenerationDurationSeconds(
    durationValue,
    durationBounds,
  );
  const resolutionOptions = generationResolutionOptionsForModel(mode, model);
  const resolution =
    resolutionOptions.find(
      (option) =>
        option.value ===
        selectedGenerationResolutionValue(resolutionValue, resolutionOptions),
    ) ?? resolutionOptions[0];

  if (mode === "video") {
    if (!duration) {
      return "varies";
    }
    return videoGenerationCost(
      model,
      duration.seconds,
      resolution ?? fallbackVideoResolutionOption,
      generateAudio,
    );
  }

  if (mode === "image") {
    return imageGenerationCost(
      model,
      resolution ?? fallbackVideoResolutionOption,
      selectedGenerationQualityValue(
        qualityValue,
        generationQualityOptionsForModel(mode, model),
      ) ?? null,
      selectedImageCount(mode, model, imageCountValue),
    );
  }

  return audioGenerationCost(
    model,
    prompt,
    duration?.seconds ?? boundedDurationSeconds,
  );
}

export function formatGenerationCreditEstimate(estimatedCredits: number | "varies") {
  if (estimatedCredits === "varies") {
    return "Est. varies";
  }
  return `Est. ${estimatedCredits} ${
    estimatedCredits === 1 ? "credit" : "credits"
  }`;
}

function generationPricingKey(model: GenerationModel) {
  return generationModelValue(model);
}

function resolveGenerationRate(
  rates: GenerationCreditRateTable,
  key: string | null | undefined,
) {
  if (key && rates[key] !== undefined) {
    return rates[key];
  }
  return rates[""];
}

function ceilGenerationCredits(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }
  return Math.ceil(value);
}

const videoGenerationCreditRates: Record<
  string,
  {
    creditsPerSecond: GenerationCreditRateTable;
    audioDiscountRate?: GenerationCreditRateTable;
  }
> = {
  [generationModelValue(defaultGenerationModels.video)]: {
    creditsPerSecond: { "480p": 0.7, "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falWanTextToVideoModelId })]: {
    creditsPerSecond: { "480p": 0.7, "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falWanImageToVideoModelId })]: {
    creditsPerSecond: { "480p": 0.7, "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falWanReferenceToVideoModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falKlingStandardTextToVideoModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falKlingProImageToVideoModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "fal.ai", id: falKlingProMotionControlModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "replicate", id: replicateSeedanceFastModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "replicate", id: replicateSeedanceModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1 },
    audioDiscountRate: { "": 0.8 },
  },
  [generationModelValue({ provider: "xai", id: xAiGrokVideoModelId })]: {
    creditsPerSecond: { "480p": 1, "720p": 1 },
  },
  [generationModelValue({ provider: "google", id: googleVeo31FastModelId })]: {
    creditsPerSecond: { "720p": 1, "1080p": 1, "4k": 2 },
    audioDiscountRate: { "": 0.8 },
  },
};

function videoGenerationCost(
  model: GenerationModelOption,
  durationSeconds: number,
  resolution: GenerationResolutionOption,
  generateAudio: boolean,
) {
  const pricing =
    model.creditsPerSecond
      ? {
          creditsPerSecond: model.creditsPerSecond,
          audioDiscountRate: model.audioDiscountRate,
        }
      : videoGenerationCreditRates[generationPricingKey(model)];
  if (!pricing) {
    return "varies";
  }
  let rate = resolveGenerationRate(
    pricing.creditsPerSecond,
    resolution.providerResolution,
  );
  if (rate === undefined) {
    return "varies";
  }
  if (!generateAudio && pricing.audioDiscountRate) {
    const discount =
      resolveGenerationRate(pricing.audioDiscountRate, resolution.providerResolution) ??
      1;
    rate *= discount;
  }
  return ceilGenerationCredits(rate * durationSeconds);
}

const imageGenerationCreditsPerImage: Record<
  string,
  GenerationCreditRateTable
> = {
  [generationModelValue({ provider: "fal.ai", id: falFluxSchnellModelId })]: { "": 1 },
  [generationModelValue({ provider: "fal.ai", id: falNanoBananaProEditModelId })]: {
    "1K": 1,
    "2K": 2,
    "4K": 4,
  },
  [generationModelValue({ provider: "fal.ai", id: falKrea2TurboModelId })]: {
    "": 1,
  },
  [generationModelValue({ provider: "fal.ai", id: falRecraftV3TextToImageModelId })]: {
    realistic_image: 2,
    digital_illustration: 2,
    vector_illustration: 2,
  },
  [generationModelValue({ provider: "replicate", id: "black-forest-labs/flux-schnell" })]: {
    "": 1,
  },
  [generationModelValue({ provider: "replicate", id: "black-forest-labs/flux-dev" })]: {
    "": 2,
  },
  [generationModelValue({ provider: "replicate", id: "black-forest-labs/flux-1.1-pro" })]: {
    "": 3,
  },
  [generationModelValue({
    provider: "replicate",
    id: "black-forest-labs/flux-1.1-pro-ultra",
  })]: {
    "": 4,
  },
  [generationModelValue({ provider: "openai", id: openAiGptImage2ModelId })]: {
    "": 2,
  },
  [generationModelValue({ provider: "openai", id: openAiGptImageEditModelId })]: {
    "": 2,
  },
  [generationModelValue({ provider: "xai", id: xAiGrokImageQualityModelId })]: {
    "": 2,
  },
};

function imageGenerationCost(
  model: GenerationModelOption,
  resolution: GenerationResolutionOption,
  quality: string | null,
  imageCount: number,
) {
  const pricing =
    model.creditsPerImage ??
    imageGenerationCreditsPerImage[generationPricingKey(model)];
  if (!pricing) {
    return "varies";
  }
  const providerResolution = imageGenerationProviderResolution(model, resolution);
  const matrixRate = quality
    ? resolveGenerationRate(pricing, `${providerResolution}|${quality}`)
    : undefined;
  const rate =
    matrixRate ??
    (quality ? resolveGenerationRate(pricing, quality) : undefined) ??
    resolveGenerationRate(pricing, providerResolution);
  if (rate === undefined) {
    return "varies";
  }
  return ceilGenerationCredits(rate * Math.max(1, imageCount));
}

const audioGenerationPricing: Record<string, AudioGenerationPricing> = {
  [generationModelValue({ provider: "openai", id: openAiTtsModelId })]: {
    mode: "perThousandChars",
    rate: 15,
  },
  [generationModelValue({ provider: "elevenlabs", id: elevenLabsTtsModelId })]: {
    mode: "perThousandChars",
    rate: 15,
  },
  [generationModelValue({ provider: "elevenlabs", id: elevenLabsMusicModelId })]: {
    mode: "flat",
    price: 10,
  },
  [generationModelValue({ provider: "minimax", id: minimaxMusicModelId })]: {
    mode: "flat",
    price: 10,
  },
  [generationModelValue({ provider: "google", id: googleGeminiTtsModelId })]: {
    mode: "perThousandChars",
    rate: 15,
  },
  [generationModelValue({ provider: "google", id: googleLyriaModelId })]: {
    mode: "flat",
    price: 10,
  },
};

function audioGenerationCost(
  model: GenerationModelOption,
  prompt: string,
  durationSeconds: number | null,
) {
  const pricing =
    model.audioPricing ?? audioGenerationPricing[generationPricingKey(model)];
  if (!pricing) {
    return "varies";
  }
  if (pricing.mode === "flat") {
    return ceilGenerationCredits(pricing.price);
  }
  if (pricing.mode === "perSecond") {
    return durationSeconds
      ? ceilGenerationCredits(pricing.rate * durationSeconds)
      : "varies";
  }
  const characterCount = prompt.trim().length;
  if (characterCount <= 0) {
    return "varies";
  }
  return ceilGenerationCredits(pricing.rate * (characterCount / 1000));
}
