import type { AssetRef } from "@/lib/timeline-ops/asset-insert";

/** Drag-and-drop MIME type for panel assets dropped on the timeline. */
export const assetDragMimeType = "application/x-video-creater-asset";
/** Optional explicit drop time in seconds, kept from the legacy timeline. */
const startSecondsMimeType = "application/x-video-creater-start-seconds";

export type AssetDragPayload = AssetRef;

const payloadKinds: ReadonlySet<string> = new Set<AssetDragPayload["kind"]>(["media", "template", "background"]);

/** Starts an asset drag; panels call this from `dragstart`. */
export function writeAssetDragData(dataTransfer: DataTransfer, payload: AssetDragPayload): void {
  dataTransfer.setData(assetDragMimeType, JSON.stringify({ kind: payload.kind, id: payload.id }));
  dataTransfer.effectAllowed = "copy";
}

/** True while dragging an asset; data is unreadable before `drop`, but the types are listed. */
export function hasAssetDragData(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(assetDragMimeType);
}

/** The dropped asset, or null when the payload is missing or malformed. */
export function readAssetDragData(dataTransfer: DataTransfer): AssetDragPayload | null {
  try {
    const parsed: unknown = JSON.parse(dataTransfer.getData(assetDragMimeType));
    if (!parsed || typeof parsed !== "object") return null;
    const { kind, id } = parsed as Record<string, unknown>;
    if (typeof kind !== "string" || !payloadKinds.has(kind) || typeof id !== "string" || id.trim().length === 0) return null;
    return { kind: kind as AssetDragPayload["kind"], id };
  } catch {
    return null;
  }
}

/**
 * The explicit drop time, or null to use the pointer. Unlike the legacy reader, an empty or
 * missing value is null rather than `Number("") === 0`.
 */
export function readDropStartSeconds(dataTransfer: DataTransfer): number | null {
  const raw = dataTransfer.getData(startSecondsMimeType).trim();
  if (raw.length === 0) return null;
  const seconds = Number(raw);
  return Number.isFinite(seconds) && seconds >= 0 ? seconds : null;
}
