import { useRef } from "react";
import { cn } from "@/lib/utils";
import { useMediaReadiness } from "@/lib/media/use-media-readiness";
import { retryRemoteMediaTickets } from "@/lib/runtime/adapters/remote-resource-cache";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { AssetPreview } from "./asset-preview";
import { TimelinePreview } from "./timeline-preview";
import { useCanonicalPreparation } from "./use-canonical-preparation";
import { usePlaybackClock } from "./use-playback-clock";
import { usePreviewFullscreen } from "./use-preview-fullscreen";
import { RemoteOutcomeNotice } from "./remote-outcome-notice";

/**
 * The preview panel: timeline playback or a single previewed asset, the playback clock, canonical
 * preparation and fullscreen. `insetRight` keeps the canvas and transport clear of a panel laid
 * over the preview's right edge. The panel isolates its stacking so canvas handles never paint
 * over that panel.
 */
export function PreviewPanel({ insetRight = 0 }: { readonly insetRight?: number }) {
  const store = useEditorStoreApi();
  const projectDir = useEditorStore((state) => state.projectDir);
  const mediaReadiness = useMediaReadiness(projectDir);
  const panelRef = useRef<HTMLElement>(null);
  const { fullscreen, toggleFullscreen } = usePreviewFullscreen(panelRef);
  // An asset that left the project falls back to the timeline.
  const assetMediaId = useEditorStore(({ previewSource, project }) =>
    previewSource.kind === "asset" && project.media.some((media) => media.id === previewSource.mediaId) ? previewSource.mediaId : null,
  );
  usePlaybackClock(panelRef);
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
      <RemoteOutcomeNotice />
      {mediaReadiness.status === "loading" && <p role="status" className="px-3 py-2 text-[12px] text-muted-foreground">Connecting project media…</p>}
      {mediaReadiness.status === "failed" && (
        <div role="alert" aria-label="Project media issue" className="flex items-center gap-2 px-3 py-2 text-[12px] text-warning">
          <span className="min-w-0 flex-1">{mediaReadiness.message}</span>
          <button type="button" onClick={() => void retryRemoteMediaTickets(projectDir)} className="shrink-0 rounded-control px-2 py-1 text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">Retry media</button>
        </div>
      )}
      {assetMediaId ? (
        <AssetPreview key={assetMediaId} mediaId={assetMediaId} {...fullscreenControls} />
      ) : (
        <TimelinePreview onOpenSource={openSource} {...fullscreenControls} />
      )}
    </section>
  );
}
