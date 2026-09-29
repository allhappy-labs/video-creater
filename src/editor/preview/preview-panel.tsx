import { useRef } from "react";
import { cn } from "@/lib/utils";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { AssetPreview } from "./asset-preview";
import { TimelinePreview } from "./timeline-preview";
import { useCanonicalPreparation } from "./use-canonical-preparation";
import { usePlaybackClock } from "./use-playback-clock";
import { usePreviewFullscreen } from "./use-preview-fullscreen";

/**
 * The preview panel: timeline playback or a single previewed asset, the playback clock, canonical
 * preparation and fullscreen. `insetRight` keeps the canvas and transport clear of a panel laid
 * over the preview's right edge. The panel isolates its stacking so canvas handles never paint
 * over that panel.
 */
export function PreviewPanel({ insetRight = 0 }: { readonly insetRight?: number }) {
  const store = useEditorStoreApi();
  const panelRef = useRef<HTMLElement>(null);
  const { fullscreen, toggleFullscreen } = usePreviewFullscreen(panelRef);
  // An asset that left the project falls back to the timeline.
  const assetMediaId = useEditorStore(({ previewSource, project }) =>
    previewSource.kind === "asset" && project.media.some((media) => media.id === previewSource.mediaId) ? previewSource.mediaId : null,
  );
  usePlaybackClock();
  useCanonicalPreparation();

  function openSource(itemId: string, mediaId: string) {
    const state = store.getState();
    state.selectItems([itemId]);
    state.previewAsset(mediaId);
  }

  const fullscreenControls = { fullscreen, onToggleFullscreen: () => void toggleFullscreen() };
  return (
    <section
      ref={panelRef}
      aria-label="Preview panel"
      className={cn("isolate flex h-full min-h-0 flex-col overflow-hidden bg-panel", fullscreen ? "fixed inset-0 z-50" : "rounded-panel")}
      style={fullscreen || insetRight === 0 ? undefined : { paddingRight: insetRight }}
    >
      {assetMediaId ? (
        <AssetPreview key={assetMediaId} mediaId={assetMediaId} {...fullscreenControls} />
      ) : (
        <TimelinePreview onOpenSource={openSource} {...fullscreenControls} />
      )}
    </section>
  );
}
