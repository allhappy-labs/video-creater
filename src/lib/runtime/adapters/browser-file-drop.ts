import { dataTransferHasFiles, externalFilePathsFromDataTransfer } from "../../media-import";
import type { FileDropHandler, StopFileDrops } from "../file-drop";

export interface BrowserFileDropOptions {
  /** Use `File.name` when the browser exposes no paths (fixture runtime only). */
  readonly fileNameFallback?: boolean;
}

function isDragEvent(event: Event): event is DragEvent {
  return "dataTransfer" in event;
}

/**
 * HTML5 fallback for runtimes without native webview drops. Attaches to a drop-zone element and
 * emits the same events as the Tauri adapter. Only drags that carry external files are handled,
 * so internal media and clip drags keep their own handlers.
 */
export function listenForBrowserFileDrops(
  target: HTMLElement,
  handler: FileDropHandler,
  options: BrowserFileDropOptions = {},
): StopFileDrops {
  const onDragOver = (event: Event) => {
    if (!isDragEvent(event) || !dataTransferHasFiles(event.dataTransfer)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    handler({ type: "over" });
  };
  const onDragLeave = (event: Event) => {
    const related = isDragEvent(event) ? event.relatedTarget : null;
    if (related instanceof Node && target.contains(related)) return;
    handler({ type: "leave" });
  };
  const onDrop = (event: Event) => {
    if (!isDragEvent(event) || !event.dataTransfer || !dataTransferHasFiles(event.dataTransfer)) {
      return;
    }
    const paths = externalFilePathsFromDataTransfer(event.dataTransfer, options);
    const files = Array.from(event.dataTransfer.files);
    if (paths.length === 0 && files.length === 0) {
      handler({ type: "leave" });
      return;
    }
    event.preventDefault();
    handler({ type: "drop", paths, files });
  };

  target.addEventListener("dragenter", onDragOver);
  target.addEventListener("dragover", onDragOver);
  target.addEventListener("dragleave", onDragLeave);
  target.addEventListener("drop", onDrop);
  return () => {
    target.removeEventListener("dragenter", onDragOver);
    target.removeEventListener("dragover", onDragOver);
    target.removeEventListener("dragleave", onDragLeave);
    target.removeEventListener("drop", onDrop);
  };
}
