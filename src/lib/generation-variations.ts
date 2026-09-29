export interface GenerationVariationDraft {
  name: string;
  prompt: string;
}

const variationDirections = [
  {
    name: "Storm Clouds",
    suffix: "with dramatic storm clouds, stronger contrast, and a more turbulent atmosphere",
  },
  {
    name: "Radiant Backlight",
    suffix: "with serene backlight, soft cloud wisps, and a more transcendent mood",
  },
  {
    name: "Noir Drama",
    suffix: "with deep shadows, high contrast noir lighting, and a surreal cinematic feel",
  },
  {
    name: "Soft Melancholy",
    suffix: "with gentle diffused light, vintage film grain, and a quieter introspective mood",
  },
] as const;

export function defaultVariationDrafts(basePrompt: string): GenerationVariationDraft[] {
  const trimmed = basePrompt.trim();
  const promptBase = trimmed.length > 0 ? trimmed.replace(/[,\s.]+$/, "") : "Generate a new variation";

  return variationDirections.map((direction) => ({
    name: direction.name,
    prompt: `${promptBase}, ${direction.suffix}.`,
  }));
}

export function validVariationDrafts(
  drafts: readonly GenerationVariationDraft[],
): GenerationVariationDraft[] {
  return drafts
    .map((draft, index) => ({
      name: draft.name.trim() || `Variation ${index + 1}`,
      prompt: draft.prompt.trim(),
    }))
    .filter((draft) => draft.prompt.length > 0);
}
