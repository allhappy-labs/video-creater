import type { TransitionKind } from "@/lib/timeline";
import { transitionKinds } from "@/lib/timeline-ops/transition-commands";

/** Drag-and-drop MIME type for Effects tab transition tiles dropped onto a cut. */
export const transitionDragMimeType = "application/x-video-creater-transition";

/** Starts a transition tile drag. */
export function writeTransitionDragData(dataTransfer: DataTransfer, kind: TransitionKind): void {
  dataTransfer.setData(transitionDragMimeType, JSON.stringify({ kind }));
  dataTransfer.effectAllowed = "copy";
}

/** True while dragging a transition tile; data is unreadable before `drop`, but the types are listed. */
export function hasTransitionDragData(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(transitionDragMimeType);
}

/** The dropped transition kind, or null when the payload is missing or malformed. */
export function readTransitionDragData(dataTransfer: DataTransfer): TransitionKind | null {
  try {
    const parsed: unknown = JSON.parse(dataTransfer.getData(transitionDragMimeType));
    if (!parsed || typeof parsed !== "object") return null;
    const { kind } = parsed as Record<string, unknown>;
    return transitionKinds.find((candidate) => candidate === kind) ?? null;
  } catch {
    return null;
  }
}
