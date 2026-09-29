import type {
  GeneratedAssetSettings,
  GenerationModelUiCapabilities,
  GenerationPlacementIntent,
  MediaKind,
} from "@/lib/project";

export type MediaGenerationMode = "image" | "video" | "audio";
interface MediaGenerationTimelineTarget {
  trackName: string;
  startSeconds: number;
}
export interface MediaGenerationTimelineSourceRange {
  label: string;
  startSeconds: number;
  endSeconds: number;
}
export type MediaGenerationTimelineTargets = Partial<
  Record<MediaGenerationMode, MediaGenerationTimelineTarget>
>;

export interface MediaGenerationRequest {
  kind: MediaKind;
  name: string | null;
  targetFolderId: string | null;
  placementIntent: GenerationPlacementIntent;
  prompt: string;
  model: {
    provider: string;
    id: string;
  };
  references: {
    mediaIds: string[];
    firstFrameMediaId: string | null;
    lastFrameMediaId: string | null;
    sourceVideoMediaRef?: string;
    referenceImageMediaRefs?: string[];
    referenceVideoMediaRefs?: string[];
    referenceAudioMediaRefs?: string[];
  };
  settings: GeneratedAssetSettings;
}

export type GenerationModel = MediaGenerationRequest["model"];
export type GenerationModelOption = GenerationModel & {
  kind?: MediaGenerationMode | "upscale";
  displayName?: string;
  durations?: readonly number[];
  aspectRatios?: readonly string[];
  resolutions?: readonly string[];
  qualities?: readonly string[];
  supportsFirstFrame?: boolean;
  supportsLastFrame?: boolean;
  supportsReferences?: boolean;
  supportsImageReference?: boolean;
  requiresReferenceImage?: boolean;
  requiresSourceVideo?: boolean;
  supportsSourceVideo?: boolean;
  category?: string;
  inputs?: readonly string[];
  minPromptLength?: number;
  defaultVoice?: string;
  voices?: readonly string[];
  voicesSample?: readonly string[];
  supportsLyrics?: boolean;
  supportsInstrumental?: boolean;
  supportsStyleInstructions?: boolean;
  minSeconds?: number;
  maxSeconds?: number;
  maxReferenceImages?: number;
  maxReferenceVideos?: number;
  maxReferenceAudios?: number;
  maxTotalReferences?: number | null;
  maxCombinedVideoRefSeconds?: number | null;
  maxCombinedAudioRefSeconds?: number | null;
  maxImages?: number;
  framesAndReferencesExclusive?: boolean;
  creditsPerSecond?: GenerationCreditRateTable;
  audioDiscountRate?: GenerationCreditRateTable;
  creditsPerImage?: GenerationCreditRateTable;
  audioPricing?: AudioGenerationPricing;
  uiCapabilities?: GenerationModelUiCapabilities;
  cancellationCapability?: "provider" | "local" | "none";
  paidOnly?: boolean;
};
export type GenerationModelCatalog = Partial<
  Record<MediaGenerationMode | "upscale", readonly GenerationModelOption[]>
>;
export type GenerationReferencePromptTag = {
  mediaId: string;
  tag: string;
  kindLabel: "Image" | "Video" | "Audio";
};

export type GenerationCreditRateTable = Record<string, number>;

export type AudioGenerationPricing =
  | { mode: "perThousandChars"; rate: number }
  | { mode: "perSecond"; rate: number }
  | { mode: "flat"; price: number };

export interface GenerationResolutionOption {
  value: string;
  label?: string;
  width: number;
  height: number;
  providerResolution: string;
}

export type GenerationDurationOption = {
  value: string;
  seconds: number;
};

export type GenerationDurationBounds = {
  minSeconds: number;
  maxSeconds: number;
  defaultSeconds: number;
};

export type GenerationReferenceLimits = {
  requiresReferenceImage: boolean;
  maxReferenceImages: number;
  maxReferenceVideos: number;
  maxReferenceAudios: number;
  maxTotalReferences: number | null;
  maxCombinedVideoRefSeconds: number | null;
  maxCombinedAudioRefSeconds: number | null;
};

// Structurally identical to ReturnType<typeof typedGenerationReferenceMediaRefs>
// in media-bin.tsx; spelled out so lib does not depend on the UI module.
export type TypedGenerationReferenceMediaRefs = {
  referenceImageMediaRefs: string[];
  referenceVideoMediaRefs: string[];
  referenceAudioMediaRefs: string[];
};

export interface SourceClipUpscaleContext {
  itemId: string;
  sourceIn: number;
  sourceOut: number;
}

export type SourceClipVideoAudioKind = "music" | "sfx";

export interface SourceClipGenerationContext {
  itemId: string;
  timelineStartSeconds: number;
  durationSeconds: number;
  sourceIn?: number;
  sourceOut?: number;
}

export type SourceClipVideoAudioContext = SourceClipGenerationContext;
