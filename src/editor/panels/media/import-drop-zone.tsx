import { Upload } from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";
import { listenForBrowserFileDrops } from "@/lib/runtime/adapters/browser-file-drop";
import { listenForFileDrops } from "@/lib/runtime/adapters/tauri-drag-drop";
import type { FileDropEvent } from "@/lib/runtime/file-drop";
import { useRuntimeMode } from "../../services/use-runtime-mode";

/**
 * External file drops while the Media tab is mounted: native webview drops in the desktop app,
 * HTML5 drops on `target` elsewhere (the fixture runtime falls back to file names). Returns
 * whether files are over the drop target.
 */
export function useMediaFileDrops(
  target: RefObject<HTMLElement | null>,
  onDrop: (paths: readonly string[], files?: readonly File[]) => void,
): boolean {
  const mode = useRuntimeMode();
  const [active, setActive] = useState(false);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    const handle = (event: FileDropEvent) => {
      setActive(event.type === "over");
      if (event.type === "drop" && (event.paths.length > 0 || (event.files?.length ?? 0) > 0)) {
        onDropRef.current(event.paths, event.files);
      }
    };
    if (mode === "desktop") return listenForFileDrops(handle);
    const element = target.current;
    if (!element) return undefined;
    return listenForBrowserFileDrops(element, handle, { fileNameFallback: mode === "fixture" });
  }, [mode, target]);

  return active;
}

/** Covers the panel while external files are dragged over it. */
export function DropOverlay() {
  return (
    <div
      role="status"
      className="pointer-events-none absolute inset-2 z-20 grid place-items-center rounded-panel border-2 border-dashed border-primary bg-background/80 text-[13px] font-medium text-foreground"
    >
      Drop media to import
    </div>
  );
}

/** The empty library: a large import target that also accepts dropped files. */
export function ImportDropZone({ importing, onImport }: { readonly importing: boolean; onImport(): void }) {
  return (
    <div className="flex flex-col items-center gap-3 rounded-panel border-2 border-dashed border-line px-4 py-10 text-center">
      <Upload className="h-7 w-7 text-dim" aria-hidden />
      <div>
        <p className="text-[13px] font-medium text-foreground">No media imported</p>
        <p className="mt-1 text-[12px] text-muted-foreground">Drop video, image or audio files here, or import them.</p>
      </div>
      <button
        type="button"
        disabled={importing}
        onClick={onImport}
        className="h-8 rounded-control bg-primary px-3 text-[13px] font-medium text-primary-foreground hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
      >
        Choose files
      </button>
    </div>
  );
}
