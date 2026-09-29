import {
  elevenLabsMusicModelId,
  elevenLabsTtsModelId,
  falFluxSchnellModelId,
  falKlingProImageToVideoModelId,
  falKlingProMotionControlModelId,
  falKlingStandardTextToVideoModelId,
  falKrea2TurboModelId,
  falNanoBananaProEditModelId,
  falRecraftV3TextToImageModelId,
  falSoniloTextToMusicModelId,
  falWanImageToVideoModelId,
  falWanReferenceToVideoModelId,
  falWanTextToVideoModelId,
  falWanVideoToVideoModelId,
  generationModelSupportsAudioToggle,
  generationModelSupportsInstrumental,
  generationModelSupportsLyrics,
  generationModelSupportsStyleInstructions,
  googleGeminiTtsModelId,
  googleLyriaModelId,
  googleVeo31FastModelId,
  minimaxMusicModelId,
  openAiGptImage2ModelId,
  openAiGptImageEditModelId,
  openAiTtsModelId,
  replicateSeedanceFastModelId,
  replicateSeedanceModelId,
  xAiGrokVideoModelId,
} from "@/lib/generation/provider-rules";
import type {
  GenerationDurationBounds,
  GenerationDurationOption,
  GenerationModel,
  GenerationModelOption,
  GenerationResolutionOption,
  MediaGenerationMode,
} from "@/lib/generation/types";
import type { GeneratedAssetSettings } from "@/lib/project";

export function generationDurationValue(
  seconds: number | null,
  options: readonly GenerationDurationOption[] = generationDurationOptions,
) {
  return options.find((option) => option.seconds === seconds)?.value ?? null;
}

export function boundedGenerationDurationValue(seconds: number | null) {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) {
    return null;
  }
  return String(Math.round(seconds));
}

export function generationResolutionValue(
  width: number | null,
  height: number | null,
  options: readonly GenerationResolutionOption[] = generationResolutionOptions,
) {
  return (
    options.find(
      (option) => option.width === width && option.height === height,
    )?.value ??
    generationResolutionValueFromDimensions(width, height, options) ??
    null
  );
}

function generationResolutionValueFromDimensions(
  width: number | null,
  height: number | null,
  options: readonly GenerationResolutionOption[],
) {
  if (!width || !height) {
    return null;
  }
  const longestEdge = Math.max(width, height);
  if (longestEdge >= 3000 && options.some((option) => option.value === "4k")) {
    return "4k";
  }
  if (longestEdge >= 1500 && options.some((option) => option.value === "1080p")) {
    return "1080p";
  }
  if (longestEdge <= 900 && options.some((option) => option.value === "480p")) {
    return "480p";
  }
  if (options.some((option) => option.value === "720p")) {
    return "720p";
  }
  return null;
}

export function selectedGenerationResolutionValue(
  value: string,
  options: readonly GenerationResolutionOption[],
) {
  if (options.length === 0) {
    return "";
  }
  if (options.some((option) => option.value === value)) {
    return value;
  }
  const currentOption = generationResolutionOptions.find(
    (option) => option.value === value,
  );
  const providerResolutionOption = currentOption
    ? options.find((option) => option.value === currentOption.providerResolution)
    : null;
  if (providerResolutionOption) {
    return providerResolutionOption.value;
  }
  const matchingOption = currentOption
    ? options.find(
      (option) =>
        option.width === currentOption.width && option.height === currentOption.height,
    )
    : null;
  return matchingOption?.value ?? options[0]?.value ?? "";
}

function videoResolutionBasePixels(providerResolution: string) {
  if (providerResolution === "4k") {
    return 2160;
  }
  const match = providerResolution.match(/^(\d+)p$/);
  const pixels = match?.[1];
  return pixels ? Number.parseInt(pixels, 10) : null;
}

function roundedEvenPixel(value: number) {
  return Math.max(1, Math.round(value / 2) * 2);
}

function dimensionsForAspectResolution(
  aspectRatio: string | undefined,
  providerResolution: string,
) {
  const basePixels = videoResolutionBasePixels(providerResolution);
  const [widthRatio, heightRatio] = (aspectRatio ?? "").split(":").map(Number);
  if (
    !basePixels ||
    widthRatio === undefined ||
    heightRatio === undefined ||
    !Number.isFinite(widthRatio) ||
    !Number.isFinite(heightRatio) ||
    widthRatio <= 0 ||
    heightRatio <= 0
  ) {
    return null;
  }
  if (widthRatio >= heightRatio) {
    return {
      width: roundedEvenPixel(basePixels * (widthRatio / heightRatio)),
      height: basePixels,
    };
  }
  return {
    width: basePixels,
    height: roundedEvenPixel(basePixels * (heightRatio / widthRatio)),
  };
}

export function isGenerationAspectRatio(value: string | null): value is string {
  return value !== null && allGenerationAspectRatioOptions.some((option) => option === value);
}

const generationDurationOptions = [
  { value: "4s", seconds: 4 },
  { value: "8s", seconds: 8 },
  { value: "12s", seconds: 12 },
] as const satisfies readonly GenerationDurationOption[];

function durationOptionsForSeconds(seconds: readonly number[]) {
  return seconds.map((value) => ({ value: `${value}s`, seconds: value }));
}

const generationAspectRatioOptions = ["16:9", "9:16", "1:1"] as const;
const falKreaRecraftImageAspectRatioOptions = ["1:1", "16:9", "9:16", "4:3", "3:4"] as const;
const falNanoBananaImageAspectRatioOptions = [
  "auto",
  "21:9",
  "16:9",
  "3:2",
  "4:3",
  "5:4",
  "1:1",
  "4:5",
  "3:4",
  "2:3",
  "9:16",
] as const;
const expandedVideoAspectRatioOptions = ["16:9", "9:16", "1:1", "4:3", "3:4"] as const;
const xAiVideoAspectRatioOptions = [
  "16:9",
  "9:16",
  "1:1",
  "4:3",
  "3:4",
  "3:2",
  "2:3",
] as const;
const googleVideoAspectRatioOptions = ["16:9", "9:16"] as const;
const allGenerationAspectRatioOptions = Array.from(
  new Set([
      ...generationAspectRatioOptions,
      ...falKreaRecraftImageAspectRatioOptions,
      ...falNanoBananaImageAspectRatioOptions,
      ...expandedVideoAspectRatioOptions,
      ...xAiVideoAspectRatioOptions,
    ...googleVideoAspectRatioOptions,
  ]),
);

const openAiTtsVoiceOptions = [
  "alloy",
  "ash",
  "ballad",
  "cedar",
  "coral",
  "echo",
  "fable",
  "marin",
  "nova",
  "onyx",
  "sage",
  "shimmer",
  "verse",
] as const;

const googleGeminiTtsVoiceOptions = [
  "Zephyr",
  "Puck",
  "Charon",
  "Kore",
  "Fenrir",
  "Leda",
  "Orus",
  "Aoede",
  "Callirrhoe",
  "Autonoe",
  "Enceladus",
  "Iapetus",
  "Umbriel",
  "Algieba",
  "Despina",
  "Erinome",
  "Algenib",
  "Rasalgethi",
  "Laomedeia",
  "Achernar",
  "Alnilam",
  "Schedar",
  "Gacrux",
  "Pulcherrima",
  "Achird",
  "Zubenelgenubi",
  "Vindemiatrix",
  "Sadachbia",
  "Sadaltager",
  "Sulafat",
] as const;

export function generationDurationOptionsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
): readonly GenerationDurationOption[] {
  if (mode === "audio") {
    return [];
  }
  if (mode !== "video") {
    return generationDurationOptions;
  }
  if (model.durations?.length) {
    return durationOptionsForSeconds(model.durations);
  }
  if (
    model.provider === "fal.ai" &&
    (model.id === falWanTextToVideoModelId ||
      model.id === falWanImageToVideoModelId ||
      model.id === falKlingStandardTextToVideoModelId ||
      model.id === falKlingProImageToVideoModelId ||
      model.id === falKlingProMotionControlModelId)
  ) {
    return durationOptionsForSeconds([5, 10]);
  }
  if (model.provider === "fal.ai" && model.id === falWanReferenceToVideoModelId) {
    return durationOptionsForSeconds([2, 3, 4, 5, 6, 7, 8, 9, 10]);
  }
  if (model.provider === "fal.ai" && model.id === falWanVideoToVideoModelId) {
    return durationOptionsForSeconds([5, 10]);
  }
  if (
    model.provider === "replicate" &&
    (model.id === replicateSeedanceModelId ||
      model.id === replicateSeedanceFastModelId)
  ) {
    return durationOptionsForSeconds([5, 10]);
  }
  if (model.provider === "google" && model.id === googleVeo31FastModelId) {
    return durationOptionsForSeconds([8]);
  }
  if (model.provider === "xai" && model.id === xAiGrokVideoModelId) {
    return durationOptionsForSeconds([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
  }
  return generationDurationOptions;
}

export function generationDurationBoundsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
): GenerationDurationBounds | null {
  if (mode !== "audio") {
    return null;
  }
  if (
    Number.isFinite(model.minSeconds) &&
    Number.isFinite(model.maxSeconds) &&
    model.minSeconds !== undefined &&
    model.maxSeconds !== undefined &&
    model.minSeconds > 0 &&
    model.maxSeconds >= model.minSeconds
  ) {
    return {
      minSeconds: model.minSeconds,
      maxSeconds: model.maxSeconds,
      defaultSeconds: Math.min(
        model.maxSeconds,
        Math.max(model.minSeconds, 90),
      ),
    };
  }
  if (model.provider === "fal.ai" && model.id === falSoniloTextToMusicModelId) {
    return { minSeconds: 1, maxSeconds: 600, defaultSeconds: 90 };
  }
  if (model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId) {
    return { minSeconds: 3, maxSeconds: 600, defaultSeconds: 90 };
  }
  return null;
}

export function selectedBoundedGenerationDurationSeconds(
  value: string,
  bounds: GenerationDurationBounds | null,
) {
  if (!bounds) {
    return null;
  }
  const trimmed = value.trim();
  const parsed = Number.parseFloat(trimmed);
  const rawSeconds =
    Number.isFinite(parsed) && !trimmed.endsWith("s")
      ? parsed
      : bounds.defaultSeconds;
  return Math.min(
    bounds.maxSeconds,
    Math.max(bounds.minSeconds, Math.round(rawSeconds)),
  );
}

export function selectedBoundedGenerationDurationValue(
  value: string,
  bounds: GenerationDurationBounds | null,
) {
  const seconds = selectedBoundedGenerationDurationSeconds(value, bounds);
  return seconds === null ? "" : String(seconds);
}

export function generationAspectRatioOptionsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "image") {
    if (model.aspectRatios?.length) {
      return model.aspectRatios;
    }
    if (
      model.provider === "fal.ai" &&
      (model.id === falKrea2TurboModelId ||
        model.id === falRecraftV3TextToImageModelId)
    ) {
      return falKreaRecraftImageAspectRatioOptions;
    }
    if (model.provider === "fal.ai" && model.id === falNanoBananaProEditModelId) {
      return falNanoBananaImageAspectRatioOptions;
    }
    return generationAspectRatioOptions;
  }
  if (mode !== "video") {
    return [];
  }
  if (model.aspectRatios?.length) {
    return model.aspectRatios;
  }
  if (
    model.provider === "fal.ai" &&
    model.id === falWanReferenceToVideoModelId
  ) {
    return expandedVideoAspectRatioOptions;
  }
  if (model.provider === "fal.ai" && model.id === falWanVideoToVideoModelId) {
    return ["auto", "16:9", "9:16", "1:1"] as const;
  }
  if (model.provider === "xai" && model.id === xAiGrokVideoModelId) {
    return xAiVideoAspectRatioOptions;
  }
  if (model.provider === "google" && model.id === googleVeo31FastModelId) {
    return googleVideoAspectRatioOptions;
  }
  return generationAspectRatioOptions;
}

const generationResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "1280x720", width: 1280, height: 720, providerResolution: "720p" },
  { value: "1536x864", width: 1536, height: 864, providerResolution: "1080p" },
  { value: "1080x1920", width: 1080, height: 1920, providerResolution: "1080p" },
  { value: "1024x1024", width: 1024, height: 1024, providerResolution: "720p" },
] as const;

const falKrea2TurboResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "1024x1024", width: 1024, height: 1024, providerResolution: "1024x1024" },
  { value: "1024x576", width: 1024, height: 576, providerResolution: "1024x576" },
  { value: "576x1024", width: 576, height: 1024, providerResolution: "576x1024" },
  { value: "1024x768", width: 1024, height: 768, providerResolution: "1024x768" },
  { value: "768x1024", width: 768, height: 1024, providerResolution: "768x1024" },
] as const;

const falRecraftV3ResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "1024x1024", width: 1024, height: 1024, providerResolution: "1024x1024" },
  { value: "1280x720", width: 1280, height: 720, providerResolution: "1280x720" },
  { value: "720x1280", width: 720, height: 1280, providerResolution: "720x1280" },
  { value: "1280x960", width: 1280, height: 960, providerResolution: "1280x960" },
  { value: "960x1280", width: 960, height: 1280, providerResolution: "960x1280" },
] as const;

const falRecraftV3QualityOptions = [
  "realistic_image",
  "digital_illustration",
  "vector_illustration",
] as const;

export function generationQualityOptionsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
): readonly string[] {
  if (mode === "image" && model.qualities?.length) {
    return model.qualities;
  }
  if (
    mode === "image" &&
    model.provider === "fal.ai" &&
    model.id === falRecraftV3TextToImageModelId
  ) {
    return falRecraftV3QualityOptions;
  }

  return [];
}

export function selectedGenerationQualityValue(
  value: string,
  options: readonly string[],
) {
  if (options.length === 0) {
    return null;
  }
  return options.includes(value) ? value : options[options.length - 1];
}

export function generationQualityLabel(value: string) {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function imageResolutionDisplayLabel(option: GenerationResolutionOption) {
  if (option.width === option.height) {
    return "Square";
  }
  const orientation = option.width > option.height ? "Landscape" : "Portrait";
  const longEdge = Math.max(option.width, option.height);
  const tier =
    longEdge === 3840
      ? "4K"
      : longEdge === 2560
        ? "2K"
        : longEdge === 1920
          ? "1080p"
          : longEdge === 1024 || longEdge === 1536
            ? ""
            : `${longEdge}p`;

  return tier ? `${orientation} ${tier}` : orientation;
}

export function generationResolutionLabel(
  mode: MediaGenerationMode,
  option: GenerationResolutionOption,
) {
  if (option.label) {
    return option.label;
  }
  return mode === "image" ? imageResolutionDisplayLabel(option) : option.value;
}

const falWanVideoResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "480p", width: 854, height: 480, providerResolution: "480p" },
  { value: "720p", width: 1280, height: 720, providerResolution: "720p" },
  { value: "1080p", width: 1920, height: 1080, providerResolution: "1080p" },
] as const;

const standardVideoResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "720p", width: 1280, height: 720, providerResolution: "720p" },
  { value: "1080p", width: 1920, height: 1080, providerResolution: "1080p" },
] as const;

const falWanVideoToVideoResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "480p", width: 854, height: 480, providerResolution: "480p" },
  { value: "580p", width: 1032, height: 580, providerResolution: "580p" },
  { value: "720p", width: 1280, height: 720, providerResolution: "720p" },
] as const;

const xAiGrokVideoResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "480p", width: 854, height: 480, providerResolution: "480p" },
  { value: "720p", width: 1280, height: 720, providerResolution: "720p" },
] as const;

const googleVeoResolutionOptions: readonly GenerationResolutionOption[] = [
  { value: "720p", width: 1280, height: 720, providerResolution: "720p" },
  { value: "1080p", width: 1920, height: 1080, providerResolution: "1080p" },
  { value: "4k", label: "4K", width: 3840, height: 2160, providerResolution: "4k" },
] as const;
export const fallbackVideoResolutionOption: GenerationResolutionOption = {
  value: "720p",
  width: 1280,
  height: 720,
  providerResolution: "720p",
};

function catalogResolutionOption(value: string): GenerationResolutionOption {
  const dimensions = /^(\d+)x(\d+)$/i.exec(value);
  const width = dimensions?.[1];
  const height = dimensions?.[2];
  if (width && height) {
    return {
      value,
      width: Number.parseInt(width, 10),
      height: Number.parseInt(height, 10),
      providerResolution: value,
    };
  }
  if (value === "1K") {
    return { value, width: 1024, height: 1024, providerResolution: value };
  }
  if (value === "2K") {
    return { value, width: 2048, height: 2048, providerResolution: value };
  }
  if (value === "4K") {
    return {
      value,
      label: "4K",
      width: 3840,
      height: 2160,
      providerResolution: value,
    };
  }
  if (value === "480p") {
    return { value, width: 854, height: 480, providerResolution: value };
  }
  if (value === "580p") {
    return { value, width: 1032, height: 580, providerResolution: value };
  }
  if (value === "720p") {
    return { value, width: 1280, height: 720, providerResolution: value };
  }
  if (value === "1080p") {
    return { value, width: 1920, height: 1080, providerResolution: value };
  }
  if (value === "4k") {
    return {
      value,
      label: "4K",
      width: 3840,
      height: 2160,
      providerResolution: value,
    };
  }
  return {
    value,
    width: fallbackVideoResolutionOption.width,
    height: fallbackVideoResolutionOption.height,
    providerResolution: value,
  };
}

function catalogResolutionOptions(
  model: GenerationModelOption,
): readonly GenerationResolutionOption[] | null {
  if (!model.resolutions?.length) {
    return null;
  }
  return model.resolutions.map(catalogResolutionOption);
}

export function generationResolutionOptionsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode === "video" || mode === "image") {
    const catalogOptions = catalogResolutionOptions(model);
    if (catalogOptions) {
      return catalogOptions;
    }
  }
  if (
    mode === "video" &&
    model.provider === "fal.ai" &&
    (model.id === falWanTextToVideoModelId ||
      model.id === falWanImageToVideoModelId)
  ) {
    return falWanVideoResolutionOptions;
  }
  if (mode === "video" && model.provider === "fal.ai" && model.id === falWanReferenceToVideoModelId) {
    return standardVideoResolutionOptions;
  }
  if (mode === "video" && model.provider === "fal.ai" && model.id === falWanVideoToVideoModelId) {
    return falWanVideoToVideoResolutionOptions;
  }
  if (
    mode === "video" &&
    model.provider === "fal.ai" &&
    (model.id === falKlingStandardTextToVideoModelId ||
      model.id === falKlingProImageToVideoModelId ||
      model.id === falKlingProMotionControlModelId)
  ) {
    return [];
  }
  if (
    mode === "video" &&
    model.provider === "replicate" &&
    (model.id === replicateSeedanceModelId ||
      model.id === replicateSeedanceFastModelId)
  ) {
    return standardVideoResolutionOptions;
  }
  if (mode === "video" && model.provider === "xai" && model.id === xAiGrokVideoModelId) {
    return xAiGrokVideoResolutionOptions;
  }
  if (mode === "video" && model.provider === "google" && model.id === googleVeo31FastModelId) {
    return googleVeoResolutionOptions;
  }
  if (mode === "image" && model.provider === "fal.ai" && model.id === falKrea2TurboModelId) {
    return falKrea2TurboResolutionOptions;
  }
  if (
    mode === "image" &&
    model.provider === "fal.ai" &&
    model.id === falRecraftV3TextToImageModelId
  ) {
    return falRecraftV3ResolutionOptions;
  }

  return generationResolutionOptions;
}

export function selectedGenerationSettings(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  durationValue: string,
  aspectRatio: string,
  resolutionValue: string,
  imageCountValue: string,
  generateAudio: boolean,
  instrumental: boolean,
  voice: string,
  lyrics: string,
  styleInstructions: string,
  qualityValue: string,
): GeneratedAssetSettings {
  const durationOptions = generationDurationOptionsForModel(mode, model);
  const durationBounds = generationDurationBoundsForModel(mode, model);
  const duration =
    durationOptions.find(
      (option) => option.value === durationValue,
    ) ?? durationOptions[0];
  const boundedDurationSeconds = selectedBoundedGenerationDurationSeconds(
    durationValue,
    durationBounds,
  );
  const aspectRatioOptions = generationAspectRatioOptionsForModel(mode, model);
  const selectedAspectRatio = aspectRatioOptions.some(
    (option) => option === aspectRatio,
  )
    ? aspectRatio
    : aspectRatioOptions[0] ?? null;
  const resolutionOptions = generationResolutionOptionsForModel(mode, model);
  const resolution =
    resolutionOptions.find(
      (option) =>
        option.value === selectedGenerationResolutionValue(resolutionValue, resolutionOptions),
    ) ?? resolutionOptions[0];
  const selectedResolution = resolution ?? fallbackVideoResolutionOption;

  if (mode === "audio") {
    const trimmedLyrics = lyrics.trim();
    const trimmedStyleInstructions = styleInstructions.trim();
    const catalogAudioCategory = model.category?.trim();
    if (catalogAudioCategory) {
      return {
        width: null,
        height: null,
        durationSeconds: duration?.seconds ?? boundedDurationSeconds,
        fps: null,
        aspectRatio: null,
        resolution: null,
        generateAudio: null,
        category: catalogAudioCategory,
        ...(generationVoiceOptionsForModel(mode, model).length > 0 ? { voice } : {}),
        ...(generationModelSupportsInstrumental(mode, model) ? { instrumental } : {}),
        ...(generationModelSupportsLyrics(mode, model) && trimmedLyrics
          ? { lyrics: trimmedLyrics }
          : {}),
        ...(generationModelSupportsStyleInstructions(mode, model) &&
        trimmedStyleInstructions
          ? { styleInstructions: trimmedStyleInstructions }
          : {}),
      };
    }
    return {
      width: null,
      height: null,
      durationSeconds: duration?.seconds ?? boundedDurationSeconds,
      fps: null,
      aspectRatio: null,
      resolution: null,
      generateAudio: null,
      ...(model.provider === "openai" && model.id === openAiTtsModelId
        ? {
            category: "tts",
            voice,
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "elevenlabs" && model.id === elevenLabsTtsModelId
        ? {
            category: "tts",
            voice,
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "elevenlabs" && model.id === elevenLabsMusicModelId
        ? {
            category: "music",
            instrumental,
            ...(trimmedLyrics ? { lyrics: trimmedLyrics } : {}),
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "fal.ai" && model.id === falSoniloTextToMusicModelId
        ? {
            category: "music",
            instrumental,
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "minimax" && model.id === minimaxMusicModelId
        ? {
            category: "music",
            instrumental,
            ...(trimmedLyrics ? { lyrics: trimmedLyrics } : {}),
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "google" && model.id === googleGeminiTtsModelId
        ? {
            category: "tts",
            voice,
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
      ...(model.provider === "google" && model.id === googleLyriaModelId
        ? {
            category: "music",
            instrumental,
            ...(trimmedLyrics ? { lyrics: trimmedLyrics } : {}),
            ...(trimmedStyleInstructions
              ? { styleInstructions: trimmedStyleInstructions }
              : {}),
          }
        : {}),
    };
  }

  if (mode === "image") {
    const quality = selectedGenerationQualityValue(
      qualityValue,
      generationQualityOptionsForModel(mode, model),
    );
    return {
      width: selectedResolution.width,
      height: selectedResolution.height,
      durationSeconds: null,
      fps: null,
      aspectRatio: selectedAspectRatio,
      resolution: imageGenerationProviderResolution(model, selectedResolution),
      numImages: selectedImageCount(mode, model, imageCountValue),
      ...(quality ? { quality } : {}),
      generateAudio: null,
    };
  }

  const videoDimensions = dimensionsForAspectResolution(
    selectedAspectRatio ?? undefined,
    resolution?.providerResolution ?? fallbackVideoResolutionOption.providerResolution,
  ) ?? {
    width: resolution?.width ?? fallbackVideoResolutionOption.width,
    height: resolution?.height ?? fallbackVideoResolutionOption.height,
  };

  return {
    width: videoDimensions.width,
    height: videoDimensions.height,
    durationSeconds:
      model.provider === "google" && model.id === googleVeo31FastModelId
        ? 8
        : duration?.seconds ?? boundedDurationSeconds ?? 5,
    fps: 24,
    aspectRatio: selectedAspectRatio,
    resolution: resolution?.providerResolution ?? null,
    generateAudio: generationModelSupportsAudioToggle(mode, model)
      ? generateAudio
      : true,
  };
}

export function generationModelMaxImages(mode: MediaGenerationMode, model: GenerationModelOption) {
  if (mode !== "image") {
    return 1;
  }
  if (
    Number.isFinite(model.maxImages) &&
    model.maxImages !== undefined &&
    model.maxImages > 0
  ) {
    return Math.floor(model.maxImages);
  }
  if (model.provider === "fal.ai" && model.id === falFluxSchnellModelId) {
    return 4;
  }
  if (model.provider === "replicate" && model.id === "black-forest-labs/flux-schnell") {
    return 4;
  }
  if (model.provider === "openai" && model.id === openAiGptImage2ModelId) {
    return 4;
  }
  return 1;
}

export function selectedImageCount(
  mode: MediaGenerationMode,
  model: GenerationModel,
  imageCountValue: string,
) {
  const maxImages = generationModelMaxImages(mode, model);
  const parsed = Number.parseInt(imageCountValue, 10);
  if (!Number.isFinite(parsed)) {
    return 1;
  }
  return Math.min(maxImages, Math.max(1, parsed));
}

export function imageGenerationProviderResolution(
  model: GenerationModel,
  resolution: GenerationResolutionOption,
) {
  if (model.provider === "fal.ai" && model.id === falNanoBananaProEditModelId) {
    const longestEdge = Math.max(resolution.width, resolution.height);
    if (longestEdge <= 1024) {
      return "1K";
    }
    if (longestEdge <= 2048) {
      return "2K";
    }
    return "4K";
  }

  if (model.provider === "openai" && model.id === openAiGptImage2ModelId) {
    return resolution.value;
  }
  if (model.provider === "openai" && model.id === openAiGptImageEditModelId) {
    return ["1024x1024", "1536x1024", "1024x1536"].includes(resolution.value)
      ? resolution.value
      : "auto";
  }

  return resolution.providerResolution;
}

export function generationVoiceOptionsForModel(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
) {
  if (mode !== "audio") {
    return [];
  }
  if (model.voices?.length) {
    return model.voices;
  }
  if (model.voicesSample?.length) {
    return model.voicesSample;
  }
  if (model.defaultVoice?.trim()) {
    return [model.defaultVoice.trim()];
  }
  if (model.provider === "openai" && model.id === openAiTtsModelId) {
    return openAiTtsVoiceOptions;
  }
  if (model.provider === "elevenlabs" && model.id === elevenLabsTtsModelId) {
    return ["rachel"] as const;
  }
  if (model.provider === "google" && model.id === googleGeminiTtsModelId) {
    return googleGeminiTtsVoiceOptions;
  }
  return [];
}

export function selectedGenerationVoice(
  mode: MediaGenerationMode,
  model: GenerationModelOption,
  voice: string,
) {
  const options = generationVoiceOptionsForModel(mode, model);
  if (options.some((option) => option === voice)) {
    return voice;
  }
  if (mode === "audio" && model.defaultVoice?.trim()) {
    return model.defaultVoice.trim();
  }
  if (model.provider === "openai" && model.id === openAiTtsModelId) {
    return "alloy";
  }
  if (model.provider === "google" && model.id === googleGeminiTtsModelId) {
    return "Kore";
  }
  return options[0] ?? "";
}
