import { useEffect, useState, type RefObject } from "react";
import type { MediaAsset } from "@/lib/project";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";

const flashMilliseconds = 1600;

/**
 * Consumes `ui.revealMediaId` ("Reveal in Media", new imports and mattes): clears the filter,
 * search and folder when the tile is hidden, scrolls the tile into view and flashes it.
 * Returns the flashing media id.
 */
export function useRevealMedia(listRef: RefObject<HTMLElement | null>, visible: readonly MediaAsset[]): string | null {
  const store = useEditorStoreApi();
  const revealMediaId = useEditorStore((state) => state.revealMediaId);
  const [flashMediaId, setFlashMediaId] = useState<string | null>(null);

  useEffect(() => {
    if (revealMediaId === null) return;
    const state = store.getState();
    if (!state.project.media.some((media) => media.id === revealMediaId)) {
      state.setRevealMediaId(null);
      return;
    }
    if (!visible.some((media) => media.id === revealMediaId)) {
      // The next render shows the whole library, and this effect runs again.
      state.setMediaFilter("all");
      state.setMediaSearch("");
      state.setMediaFolderId(null);
      if (state.mediaFilter === "all" && state.mediaSearch === "" && state.mediaFolderId === null) state.setRevealMediaId(null);
      return;
    }
    const tile = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-media-id]") ?? []).find(
      (element) => element.dataset.mediaId === revealMediaId,
    );
    const reduceMotion = typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    tile?.scrollIntoView?.({ block: "nearest", behavior: reduceMotion ? "auto" : "smooth" });
    setFlashMediaId(revealMediaId);
    state.setRevealMediaId(null);
  }, [store, revealMediaId, visible, listRef]);

  useEffect(() => {
    if (flashMediaId === null) return undefined;
    const timer = window.setTimeout(() => setFlashMediaId(null), flashMilliseconds);
    return () => window.clearTimeout(timer);
  }, [flashMediaId]);

  return flashMediaId;
}
