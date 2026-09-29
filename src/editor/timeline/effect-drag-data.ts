import type { ApplicableEffect } from "../panels/effects/effect-apply";

/** Drag-and-drop MIME type for Effects tab tiles dropped onto a timeline clip. */
export const effectDragMimeType = "application/x-video-creater-effect";

/** Starts an effect drag; the payload carries what applying needs, so drops never reload the catalog. */
export function writeEffectDragData(dataTransfer: DataTransfer, effect: ApplicableEffect): void {
  const payload: ApplicableEffect = {
    id: effect.id,
    displayName: effect.displayName,
    params: effect.params.map((param) => ({ key: param.key, defaultValue: param.defaultValue })),
    resourceKey: effect.resourceKey ?? null,
  };
  dataTransfer.setData(effectDragMimeType, JSON.stringify(payload));
  dataTransfer.effectAllowed = "copy";
}

/** True while dragging an effect; data is unreadable before `drop`, but the types are listed. */
export function hasEffectDragData(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(effectDragMimeType);
}

function nonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function validParam(value: unknown): value is ApplicableEffect["params"][number] {
  if (!value || typeof value !== "object") return false;
  const { key, defaultValue } = value as Record<string, unknown>;
  return nonEmptyString(key) && typeof defaultValue === "number" && Number.isFinite(defaultValue);
}

/** The dropped effect, or null when the payload is missing or malformed. */
export function readEffectDragData(dataTransfer: DataTransfer): ApplicableEffect | null {
  try {
    const parsed: unknown = JSON.parse(dataTransfer.getData(effectDragMimeType));
    if (!parsed || typeof parsed !== "object") return null;
    const { id, displayName, params, resourceKey } = parsed as Record<string, unknown>;
    if (!nonEmptyString(id) || !nonEmptyString(displayName) || !Array.isArray(params) || !params.every(validParam)) return null;
    return { id, displayName, params, resourceKey: nonEmptyString(resourceKey) ? resourceKey : null };
  } catch {
    return null;
  }
}
