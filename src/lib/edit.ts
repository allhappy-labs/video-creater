type EditPreset = "trailer_cut" | "highlight_reel" | "story_cut";
type LanguageMode = "en" | "uk" | "auto";
type CaptionStyle = "bold";

export interface EditPresetOption {
  value: EditPreset;
  label: string;
  durationLabel: string;
  description: string;
  defaultPrompt: string;
}

export interface EditLanguageOption {
  value: LanguageMode;
  label: string;
  description: string;
}

export interface BuildEditJobRequestInput {
  mediaId: string;
  preset: EditPreset;
  prompt: string;
  targetDurationSeconds?: number;
  languageMode?: LanguageMode;
}

export interface EditJobRequest {
  mediaId: string;
  preset: EditPreset;
  prompt: string;
  targetDurationSeconds?: number;
  languageMode: LanguageMode;
  captionStyle: CaptionStyle;
  createdAt: string;
}

export const editPresetOptions: EditPresetOption[] = [
  {
    value: "trailer_cut",
    label: "Trailer Cut",
    durationLabel: "30-60s",
    description: "Cinematic pacing with a strong hook, bold captions, and title-card moments.",
    defaultPrompt:
      "Make it feel like a cinematic trailer with a strong hook, dramatic pacing, bold captions, and punchy title cards.",
  },
  {
    value: "highlight_reel",
    label: "Highlight Reel",
    durationLabel: "45-90s",
    description: "Creator-native recap that keeps the best spoken, visual, or action moments.",
    defaultPrompt:
      "Make a tight highlight reel that keeps the best moments, removes dead time, and uses readable captions.",
  },
  {
    value: "story_cut",
    label: "Story Cut",
    durationLabel: "90-180s",
    description: "Longer narrative edit with context, beat labels, and less aggressive cutting.",
    defaultPrompt:
      "Make a longer story edit that preserves context, keeps the pacing clean, and uses captions for sound-off viewing.",
  },
];

export const editLanguageOptions: EditLanguageOption[] = [
  {
    value: "en",
    label: "English",
    description: "Use English ASR hints for creator voiceovers and English source audio.",
  },
  {
    value: "uk",
    label: "Ukrainian",
    description: "Use Ukrainian ASR hints to avoid Russian-looking captions.",
  },
  {
    value: "auto",
    label: "Auto",
    description: "Let the selected transcription model detect the source language.",
  },
];

export function buildEditJobRequest(input: BuildEditJobRequestInput): EditJobRequest {
  return {
    mediaId: input.mediaId,
    preset: input.preset,
    prompt: input.prompt,
    ...(input.targetDurationSeconds === undefined
      ? {}
      : { targetDurationSeconds: input.targetDurationSeconds }),
    languageMode: input.languageMode ?? "en",
    captionStyle: "bold",
    createdAt: new Date().toISOString(),
  };
}
