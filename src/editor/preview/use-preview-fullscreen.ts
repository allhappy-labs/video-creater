import { useCallback, useEffect, type RefObject } from "react";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";

/**
 * Preview fullscreen through the Fullscreen API on the preview panel, mirrored in the store
 * `fullscreen` flag. When the webview refuses element fullscreen, the flag alone fills the window
 * with the panel (Esc leaves it through the editor shortcuts).
 */
export function usePreviewFullscreen(panelRef: RefObject<HTMLElement | null>) {
  const store = useEditorStoreApi();
  const fullscreen = useEditorStore((state) => state.fullscreen);

  useEffect(() => {
    const syncFromDocument = () => {
      const panel = panelRef.current;
      if (panel && document.fullscreenElement === panel) store.getState().setFullscreen(true);
      else if (store.getState().fullscreen) store.getState().setFullscreen(false);
    };
    document.addEventListener("fullscreenchange", syncFromDocument);
    return () => {
      document.removeEventListener("fullscreenchange", syncFromDocument);
      if (panelRef.current && document.fullscreenElement === panelRef.current) void document.exitFullscreen();
      store.getState().setFullscreen(false);
    };
  }, [panelRef, store]);

  const toggleFullscreen = useCallback(async () => {
    const panel = panelRef.current;
    if (!panel) return;
    const { fullscreen: active, setFullscreen } = store.getState();
    if (active) {
      if (document.fullscreenElement === panel) await document.exitFullscreen().catch(() => undefined);
      setFullscreen(false);
      return;
    }
    if (typeof panel.requestFullscreen === "function") {
      try {
        await panel.requestFullscreen();
      } catch {
        // Fall back to filling the window.
      }
    }
    setFullscreen(true);
  }, [panelRef, store]);

  return { fullscreen, toggleFullscreen };
}
