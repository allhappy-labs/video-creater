import { useRef, useState, type DragEvent, type RefObject } from "react";
import { cutNearTime } from "@/lib/timeline-ops/transition-commands";
import type { TransitionCut } from "@/lib/timeline-ops/transitions";
import { useEditorStoreApi } from "../store/editor-store-context";
import { readTransitionDragData } from "./transition-drag-data";
import { useTransitionCommands } from "./transition-commands";
import type { TimelineGeometry } from "./use-timeline-geometry";

interface UseTransitionDropOptions {
  readonly geometry: TimelineGeometry;
  /** The element wrapping the track rows (header column included). */
  readonly rowsRef: RefObject<HTMLElement | null>;
}

/**
 * Drops of Effects tab transition tiles (`application/x-video-creater-transition`): the cut on the
 * hovered row within 12 px of the pointer is highlighted while dragging and takes the transition on
 * drop. Anywhere else the drop is refused with a reason.
 */
export function useTransitionDrop({ geometry, rowsRef }: UseTransitionDropOptions) {
  const store = useEditorStoreApi();
  const commands = useTransitionCommands();
  const [targetCut, setTargetCut] = useState<TransitionCut | null>(null);
  const targetRef = useRef<TransitionCut | null>(null);

  function update(next: TransitionCut | null) {
    const current = targetRef.current;
    if (current === next || (current && next && current.leftItemId === next.leftItemId && current.rightItemId === next.rightItemId)) return;
    targetRef.current = next;
    setTargetCut(next);
  }

  function cutAt(event: DragEvent<HTMLElement>): TransitionCut | null {
    const rows = rowsRef.current;
    if (!rows) return null;
    const rect = rows.getBoundingClientRect();
    const y = event.clientY - rect.top;
    const seconds = (event.clientX - rect.left - geometry.headerWidth) / geometry.pixelsPerSecond;
    const row = geometry.rows.find((candidate) => y >= candidate.top && y < candidate.top + candidate.height);
    return row ? cutNearTime(store.getState().project, row.track, seconds, geometry.pixelsPerSecond) : null;
  }

  function onDragOver(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    const cut = cutAt(event);
    event.dataTransfer.dropEffect = cut ? "copy" : "none";
    update(cut);
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    update(null);
    const kind = readTransitionDragData(event.dataTransfer);
    const cut = cutAt(event);
    if (!kind) {
      store.getState().setLastError("That transition can't be added.");
      return;
    }
    if (!cut) {
      store.getState().setLastError("Drop the transition on a cut between two clips.");
      return;
    }
    void commands.addAtCut(cut, kind);
  }

  return { transitionDropCut: targetCut, clearTransitionDrop: () => update(null), onTransitionDragOver: onDragOver, onTransitionDrop: onDrop };
}
