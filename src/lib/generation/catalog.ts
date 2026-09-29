import { isGenerationModelDisabled, type AppSettingsPreferences } from "@/lib/app-settings";
import type { GenerationModelCatalog, GenerationModelOption } from "@/lib/generation/types";
import type { GenerationModelCatalogPayload } from "@/lib/project";

export function isMediaGenerationMode(value: unknown): value is "image" | "video" | "audio" {
  return value === "image" || value === "video" || value === "audio";
}

export function isGenerationCatalogKind(
  value: unknown,
): value is "image" | "video" | "audio" | "upscale" {
  return isMediaGenerationMode(value) || value === "upscale";
}

export function generationModelCatalogFromPayload(
  payload: GenerationModelCatalogPayload | null | undefined,
  preferences: AppSettingsPreferences,
): GenerationModelCatalog | null {
  if (!payload?.loaded || !Array.isArray(payload.generationModels)) {
    return null;
  }

  const catalog: Record<
    "image" | "video" | "audio" | "upscale",
    GenerationModelOption[]
  > = {
    image: [],
    video: [],
    audio: [],
    upscale: [],
  };
  for (const model of payload.generationModels) {
    if (
      typeof model.provider !== "string" ||
      typeof model.id !== "string" ||
      !isGenerationCatalogKind(model.kind)
    ) {
      continue;
    }
    if (isGenerationModelDisabled(model, preferences)) {
      continue;
    }
    const uiCapabilities =
      model.uiCapabilities &&
      typeof model.uiCapabilities === "object" &&
      !Array.isArray(model.uiCapabilities)
        ? model.uiCapabilities
        : {};
    catalog[model.kind].push({
      ...model,
      ...uiCapabilities,
      provider: model.provider,
      id: model.id,
      kind: model.kind,
      cancellationCapability:
        model.cancellationCapability ??
        (model.provider === "fal.ai" || model.provider === "replicate"
          ? "provider"
          : "local"),
    } as GenerationModelOption);
  }

  return {
    ...(catalog.image.length > 0 ? { image: catalog.image } : {}),
    ...(catalog.video.length > 0 ? { video: catalog.video } : {}),
    ...(catalog.audio.length > 0 ? { audio: catalog.audio } : {}),
    ...(catalog.upscale.length > 0 ? { upscale: catalog.upscale } : {}),
  };
}
