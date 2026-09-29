import { formatGenerationCreditEstimate, selectedGenerationCost } from "@/lib/generation/pricing";
import {
  generationAudioPromptCategory,
  generationModelAcceptsSelectedSourceVideo,
  generationModelOptionsFromCatalog,
  generationModelPreservesSelectedSourceVideoSettings,
  generationModelReferenceLimits,
  generationModelRequiresFirstFrame,
  generationModelRequiresSelectedSourceVideo,
  generationModelSupportsAudioToggle,
  generationModelSupportsFrameReferences,
  generationModelSupportsInstrumental,
  generationModelSupportsLyrics,
  generationModelSupportsReferenceMedia,
  generationModelSupportsStyleInstructions,
  generationModelUsesExclusiveFrameReferenceModes,
  generationModelUsesTypedImageReferences,
  generationModelValue,
  generationPromptReadinessMessage,
  generationProviderDisplayName,
  generationRequestModel,
} from "@/lib/generation/provider-rules";
import {
  generationReferenceLimitMessage,
  generationReferenceMediaForModel,
  isFrameReferenceMediaAsset,
  isSourceVideoMediaAsset,
  typedGenerationReferenceMediaRefs,
} from "@/lib/generation/references";
import {
  generationAspectRatioOptionsForModel,
  generationDurationBoundsForModel,
  generationDurationOptionsForModel,
  generationModelMaxImages,
  generationQualityOptionsForModel,
  generationResolutionOptionsForModel,
  generationVoiceOptionsForModel,
  selectedBoundedGenerationDurationValue,
  selectedGenerationQualityValue,
  selectedGenerationResolutionValue,
  selectedGenerationSettings,
  selectedGenerationVoice,
  selectedImageCount,
} from "@/lib/generation/settings-options";
import type {
  GenerationDurationBounds,
  GenerationDurationOption,
  GenerationModelCatalog,
  GenerationModelOption,
  GenerationResolutionOption,
  MediaGenerationMode,
  MediaGenerationRequest,
} from "@/lib/generation/types";
import type { GenerationPlacementIntent, MediaAsset } from "@/lib/project";
import type { AppSettingsTarget } from "@/lib/settings/target";
import type { GenerationConfiguration } from "../../services/editor-environment";

/** Composer chips: Video/Image in the Media tab, Music/SFX/Voice (audio categories) in the Audio tab. */
export type ComposerMode = "video" | "image" | "music" | "sfx" | "voice";
export type ReferencePane = "frames" | "references";

export const mediaComposerModes: readonly { readonly value: ComposerMode; readonly label: string }[] = [
  { value: "video", label: "Video" },
  { value: "image", label: "Image" },
];
export const audioComposerModes: readonly { readonly value: ComposerMode; readonly label: string }[] = [
  { value: "music", label: "Music" },
  { value: "sfx", label: "SFX" },
  { value: "voice", label: "Voice" },
];

/** The provider audio category (`generationAudioPromptCategory`) each audio chip lists. */
const audioCategories: Readonly<Record<"music" | "sfx" | "voice", string>> = { music: "music", sfx: "sfx", voice: "tts" };

export interface ComposerState {
  readonly mode: ComposerMode;
  readonly prompt: string;
  /** Chosen model per chip (`provider:id`); a missing or stale value falls back to the first model. */
  readonly modelValues: Readonly<Partial<Record<ComposerMode, string>>>;
  readonly duration: string;
  readonly aspectRatio: string;
  readonly resolution: string;
  readonly imageCount: string;
  readonly quality: string;
  readonly generateAudio: boolean;
  readonly instrumental: boolean;
  readonly voice: string;
  readonly lyrics: string;
  readonly styleInstructions: string;
  readonly firstFrameId: string | null;
  readonly lastFrameId: string | null;
  readonly referenceIds: readonly string[];
  readonly sourceVideoId: string | null;
  /** For models that take frames or references but not both. */
  readonly referencePane: ReferencePane;
}

/** Pre-cut composer defaults. */
export function initialComposerState(mode: ComposerMode): ComposerState {
  return {
    mode,
    prompt: "",
    modelValues: {},
    duration: "5s",
    aspectRatio: "16:9",
    resolution: "1280x720",
    imageCount: "1",
    quality: "high",
    generateAudio: true,
    instrumental: false,
    voice: "",
    lyrics: "",
    styleInstructions: "",
    firstFrameId: null,
    lastFrameId: null,
    referenceIds: [],
    sourceVideoId: null,
    referencePane: "frames",
  };
}

export function generationModeFor(mode: ComposerMode): MediaGenerationMode {
  return mode === "video" || mode === "image" ? mode : "audio";
}

/**
 * Models for a chip. A loaded catalog is authoritative (it only lists enabled models); without one
 * the built-in model list is offered, as in the pre-cut composer.
 */
export function composerModels(mode: ComposerMode, catalog: GenerationModelCatalog | null): readonly GenerationModelOption[] {
  const generationMode = generationModeFor(mode);
  const options = catalog ? (catalog[generationMode] ?? []) : generationModelOptionsFromCatalog(null)[generationMode];
  if (mode === "video" || mode === "image") return options;
  return options.filter((model) => generationAudioPromptCategory(model) === audioCategories[mode]);
}

export function selectedComposerModel(state: ComposerState, models: readonly GenerationModelOption[]): GenerationModelOption | null {
  const value = state.modelValues[state.mode];
  return models.find((model) => generationModelValue(model) === value) ?? models[0] ?? null;
}

export interface ComposerFields {
  readonly voices: readonly string[];
  readonly voice: string;
  readonly durationChoices: readonly GenerationDurationOption[];
  readonly durationBounds: GenerationDurationBounds | null;
  /** The selected duration: an option value, or whole seconds within `durationBounds`. */
  readonly duration: string;
  readonly aspectChoices: readonly string[];
  readonly aspectRatio: string | null;
  readonly resolutionChoices: readonly GenerationResolutionOption[];
  readonly resolution: string;
  readonly qualityChoices: readonly string[];
  readonly quality: string | null;
  /** Image count choices (1…max); empty unless the model makes several images. */
  readonly imageCountChoices: readonly string[];
  readonly imageCount: string;
  readonly showAudioToggle: boolean;
  readonly showInstrumental: boolean;
  readonly showLyrics: boolean;
  readonly showStyleInstructions: boolean;
}

/** Pre-cut field visibility: each option shows only when the model has choices for it in this mode. */
export function composerFields(state: ComposerState, model: GenerationModelOption): ComposerFields {
  const mode = generationModeFor(state.mode);
  const durationChoices = mode === "image" ? [] : generationDurationOptionsForModel(mode, model);
  const durationBounds = generationDurationBoundsForModel(mode, model);
  const visual = mode !== "audio";
  const aspectChoices = visual ? generationAspectRatioOptionsForModel(mode, model) : [];
  const resolutionChoices = visual ? generationResolutionOptionsForModel(mode, model) : [];
  const qualityChoices = mode === "image" ? generationQualityOptionsForModel(mode, model) : [];
  const maxImages = mode === "image" ? generationModelMaxImages(mode, model) : 1;
  return {
    voices: generationVoiceOptionsForModel(mode, model),
    voice: selectedGenerationVoice(mode, model, state.voice),
    durationChoices,
    durationBounds,
    duration: durationBounds
      ? selectedBoundedGenerationDurationValue(state.duration, durationBounds)
      : (durationChoices.find((option) => option.value === state.duration) ?? durationChoices[0])?.value ?? "",
    aspectChoices,
    aspectRatio: aspectChoices.includes(state.aspectRatio) ? state.aspectRatio : (aspectChoices[0] ?? null),
    resolutionChoices,
    resolution: selectedGenerationResolutionValue(state.resolution, resolutionChoices),
    qualityChoices,
    quality: selectedGenerationQualityValue(state.quality, qualityChoices) ?? null,
    imageCountChoices: maxImages > 1 ? Array.from({ length: maxImages }, (_, index) => String(index + 1)) : [],
    imageCount: String(selectedImageCount(mode, model, state.imageCount)),
    showAudioToggle: generationModelSupportsAudioToggle(mode, model),
    showInstrumental: generationModelSupportsInstrumental(mode, model),
    showLyrics: generationModelSupportsLyrics(mode, model),
    showStyleInstructions: generationModelSupportsStyleInstructions(mode, model),
  };
}

export function composerCostLabel(state: ComposerState, model: GenerationModelOption): string {
  const fields = composerFields(state, model);
  const mode = generationModeFor(state.mode);
  return formatGenerationCreditEstimate(
    selectedGenerationCost(mode, model, fields.duration, fields.resolution, fields.imageCount, state.generateAudio, state.prompt, fields.quality ?? state.quality),
  );
}

/** Which input slots the model takes, and the media each slot accepts. */
export interface ReferenceSupport {
  readonly frames: boolean;
  readonly references: boolean;
  /** Frames and references are mutually exclusive, so a pane switch picks one. */
  readonly exclusive: boolean;
  readonly sourceVideo: boolean;
  readonly frameMedia: readonly MediaAsset[];
  readonly referenceMedia: readonly MediaAsset[];
  readonly sourceVideoMedia: readonly MediaAsset[];
}

export function referenceSupport(state: ComposerState, model: GenerationModelOption, media: readonly MediaAsset[]): ReferenceSupport {
  const mode = generationModeFor(state.mode);
  const limits = generationModelReferenceLimits(mode, model);
  // Some video models "support" references but accept none of any kind; they get no slot.
  const acceptsAny = !limits || (limits.maxTotalReferences !== 0 && limits.maxReferenceImages + limits.maxReferenceVideos + limits.maxReferenceAudios > 0);
  const references = mode !== "audio" && generationModelSupportsReferenceMedia(mode, model) && acceptsAny;
  return {
    frames: generationModelSupportsFrameReferences(mode, model),
    references,
    exclusive: generationModelUsesExclusiveFrameReferenceModes(mode, model),
    sourceVideo: generationModelAcceptsSelectedSourceVideo(mode, model),
    frameMedia: media.filter(isFrameReferenceMediaAsset),
    referenceMedia: references ? generationReferenceMediaForModel(mode, model, media) : [],
    sourceVideoMedia: media.filter(isSourceVideoMediaAsset),
  };
}

type ConfigurationBlocker =
  | { readonly kind: "checking"; readonly message: string }
  | { readonly kind: "action"; readonly message: string; readonly target: AppSettingsTarget; readonly label: string };

/** The provider's name as Settings shows it (its credential status), else the built-in display name. */
export function providerDisplayName(configuration: GenerationConfiguration, model: GenerationModelOption): string {
  const status = configuration.providerStatuses?.find((candidate) => candidate.provider.trim().toLowerCase() === model.provider.trim().toLowerCase());
  return status?.displayName ?? generationProviderDisplayName(model.provider);
}

const checkingCopy = "Checking generation configuration…";
const generationModelsTarget: AppSettingsTarget = { category: "aiModels", item: "generationModels" };
const configureModelsLabel = "Configure generation models in Settings";

/** Pre-cut configuration blocker precedence: models, then the model's provider key, then Temporal. */
export function configurationBlocker(
  configuration: GenerationConfiguration,
  mode: ComposerMode,
  model: GenerationModelOption | null,
  modeLabel: string,
): ConfigurationBlocker | null {
  const { readiness, catalog, providerStatuses, preferences } = configuration;
  if (readiness?.models === "checking") return { kind: "checking", message: checkingCopy };
  if (readiness?.models === "failed") {
    return { kind: "action", message: "Generation model configuration could not be checked.", target: generationModelsTarget, label: configureModelsLabel };
  }
  const anyModels = catalog === null || Object.values(catalog).some((models) => Boolean(models?.length));
  if (!anyModels) {
    return { kind: "action", message: "Media generation needs at least one enabled generation model.", target: generationModelsTarget, label: configureModelsLabel };
  }
  if (!model) {
    return { kind: "action", message: `Enable a ${modeLabel.toLowerCase()} generation model to generate here.`, target: generationModelsTarget, label: configureModelsLabel };
  }
  if (readiness?.providers === "checking") return { kind: "checking", message: checkingCopy };
  const status = providerStatuses?.find((candidate) => candidate.provider.trim().toLowerCase() === model.provider.trim().toLowerCase());
  const providerName = providerDisplayName(configuration, model);
  if (readiness?.providers === "failed") {
    return { kind: "action", message: `${providerName} provider configuration could not be checked.`, target: { category: "integrations", provider: model.provider }, label: `Configure ${providerName} in Settings` };
  }
  if (status && !status.configured) {
    const article = /^[aeiou]/i.test(status.displayName.trim()) ? "an" : "a";
    const generationMode = generationModeFor(mode);
    return {
      kind: "action",
      message:
        status.source === "missing"
          ? `${status.displayName} ${generationMode} generation needs ${article} ${status.displayName} provider key.`
          : `${status.displayName} ${generationMode} generation cannot access its provider key.`,
      target: { category: "integrations", provider: status.provider },
      label: `Configure ${status.displayName} in Settings`,
    };
  }
  if (preferences.generationExecutionBackend !== "temporal") return null;
  if (readiness?.temporal === "checking") return { kind: "checking", message: checkingCopy };
  const temporalTarget: AppSettingsTarget = { category: "advanced", item: "execution" };
  if (readiness?.temporal === "failed") {
    return { kind: "action", message: "Temporal configuration could not be checked.", target: temporalTarget, label: "Configure Temporal in Settings" };
  }
  if (configuration.temporalBackendReady === false) {
    return { kind: "action", message: "Temporal generation is selected, but the Temporal backend is not configured.", target: temporalTarget, label: "Configure Temporal in Settings" };
  }
  return null;
}

/** Frame and reference ids that will actually be sent, after the exclusive-pane and media filters. */
function submittedInputs(state: ComposerState, model: GenerationModelOption, media: readonly MediaAsset[]) {
  const support = referenceSupport(state, model, media);
  const framesCleared = support.exclusive && state.referencePane === "references";
  const frameId = (id: string | null) => (!framesCleared && id && support.frameMedia.some((asset) => asset.id === id) ? id : null);
  const referenceIds =
    support.exclusive && state.referencePane === "frames"
      ? []
      : state.referenceIds.filter((id) => support.referenceMedia.some((asset) => asset.id === id));
  const sourceVideoId = support.sourceVideo && state.sourceVideoId && support.sourceVideoMedia.some((asset) => asset.id === state.sourceVideoId) ? state.sourceVideoId : null;
  return { support, firstFrameId: frameId(state.firstFrameId), lastFrameId: frameId(state.lastFrameId), referenceIds, sourceVideoId };
}

/**
 * Pre-cut queue readiness: the reason Generate is disabled, or null when ready. Precedence:
 * configuration, prompt, source video, first frame, reference limits.
 */
export function readinessReason(
  state: ComposerState,
  model: GenerationModelOption | null,
  media: readonly MediaAsset[],
  blocker: ConfigurationBlocker | null,
): string | null {
  if (blocker) return blocker.kind === "checking" ? "Checking configuration" : "Configuration required";
  if (!model) return "Configuration required";
  const mode = generationModeFor(state.mode);
  const inputs = submittedInputs(state, model, media);
  const requiresSourceVideo = generationModelRequiresSelectedSourceVideo(mode, model);
  const promptMessage = requiresSourceVideo && inputs.sourceVideoId ? null : generationPromptReadinessMessage(mode, model, state.prompt);
  if (promptMessage) return promptMessage;
  if (requiresSourceVideo && !inputs.sourceVideoId) return "Select video";
  if (generationModelRequiresFirstFrame(mode, model) && !inputs.firstFrameId) return "Select first frame";
  return generationReferenceLimitMessage(mode, model, typedGenerationReferenceMediaRefs(mode, model, inputs.referenceIds, media), media);
}

/** Pre-cut `submitGeneration`; null when the composer is not ready. */
export function buildGenerationRequest(
  state: ComposerState,
  model: GenerationModelOption,
  media: readonly MediaAsset[],
  targetFolderId: string | null,
  placementIntent: GenerationPlacementIntent = "library",
): MediaGenerationRequest | null {
  if (readinessReason(state, model, media, null) !== null) return null;
  const mode = generationModeFor(state.mode);
  const inputs = submittedInputs(state, model, media);
  const supportsReferences = inputs.support.references;
  const typed = typedGenerationReferenceMediaRefs(mode, model, inputs.referenceIds, media);
  const usesTypedImages = generationModelUsesTypedImageReferences(mode, model);
  const imageRefs = usesTypedImages ? inputs.referenceIds : typed.referenceImageMediaRefs;
  const fields = composerFields(state, model);
  const base = selectedGenerationSettings(
    mode,
    model,
    fields.duration,
    state.aspectRatio,
    state.resolution,
    state.imageCount,
    state.generateAudio,
    state.instrumental,
    fields.voice,
    state.lyrics,
    state.styleInstructions,
    state.quality,
  );
  const videoEdit = mode === "video" && inputs.sourceVideoId !== null && !generationModelPreservesSelectedSourceVideoSettings(mode, model);
  const settings = videoEdit ? { ...base, durationSeconds: null, aspectRatio: null, resolution: null } : base;
  const framesSent = inputs.support.frames && supportsReferences;
  return {
    kind: mode === "audio" ? "audio" : "generated",
    name: null,
    targetFolderId,
    placementIntent,
    prompt: state.prompt.trim(),
    model: generationRequestModel(model),
    references: {
      mediaIds: inputs.sourceVideoId ? [inputs.sourceVideoId] : supportsReferences ? inputs.referenceIds : [],
      ...(inputs.sourceVideoId ? { sourceVideoMediaRef: inputs.sourceVideoId } : {}),
      firstFrameMediaId: framesSent ? inputs.firstFrameId : null,
      lastFrameMediaId: framesSent ? inputs.lastFrameId : null,
      ...((usesTypedImages && supportsReferences) || (!usesTypedImages && imageRefs.length > 0) ? { referenceImageMediaRefs: imageRefs } : {}),
      ...(typed.referenceVideoMediaRefs.length > 0 ? { referenceVideoMediaRefs: typed.referenceVideoMediaRefs } : {}),
      ...(typed.referenceAudioMediaRefs.length > 0 ? { referenceAudioMediaRefs: typed.referenceAudioMediaRefs } : {}),
    },
    settings,
  };
}
