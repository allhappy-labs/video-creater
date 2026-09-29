import { getCurrentWebview } from "@tauri-apps/api/webview";

import type { FileDropHandler, StopFileDrops } from "../file-drop";

/**
 * Listens for native file drops on the current Tauri webview. Returns a synchronous stop function
 * that also detaches a listener still being installed. Outside the desktop app, or when the native
 * listener cannot be installed, this is a no-op; browser runtimes use `listenForBrowserFileDrops`.
 */
export function listenForFileDrops(handler: FileDropHandler): StopFileDrops {
  if (typeof window === "undefined" || window.__TAURI_INTERNALS__ === undefined) {
    return () => undefined;
  }
  let stopped = false;
  let unlisten: (() => void) | null = null;

  void Promise.resolve()
    .then(() =>
      getCurrentWebview().onDragDropEvent(({ payload }) => {
        if (stopped) return;
        if (payload.type === "enter" || payload.type === "over") {
          handler({ type: "over" });
        } else if (payload.type === "leave") {
          handler({ type: "leave" });
        } else {
          handler({ type: "drop", paths: [...payload.paths] });
        }
      }),
    )
    .then((stopListening) => {
      if (stopped) stopListening();
      else unlisten = stopListening;
    })
    .catch(() => {
      // Without a native drag-drop listener the DOM drop zone remains the import path.
    });

  return () => {
    stopped = true;
    unlisten?.();
    unlisten = null;
  };
}
