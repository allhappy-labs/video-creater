import { useEffect, useRef, useState } from "react";
import { refreshRemoteMediaUrl } from "@/lib/runtime/adapters/remote-resource-cache";
import { previewMotionStyle, type PreviewOutputSize } from "@/lib/preview/canvas-geometry";
import { CanonicalFramePreloader, canonicalPreloadUrls, type CanonicalFrameLayer, type CanonicalFrameSequence } from "./canonical-frames";
import { PreviewLayerFrame } from "./media-layer";
import { PreviewFailure } from "./preview-failure";

interface CanonicalFrameLayersProps {
  readonly frameLayers: readonly CanonicalFrameLayer[];
  readonly sequences: readonly CanonicalFrameSequence[];
  /** Timeline time clamped to the duration. */
  readonly seconds: number;
  /** Render size for position offsets. */
  readonly outputSize: PreviewOutputSize;
  readonly onRetry: () => void;
  readonly onOpenSource: (itemId: string, mediaId: string) => void;
}

/**
 * Canonical prepared frames drawn over the DOM media layers (below captions), with frame
 * preloading around the playhead, the "Canonical ready" badge and the frame load failure message.
 */
export function CanonicalFrameLayers({ frameLayers, sequences, seconds, outputSize, onRetry, onOpenSource }: CanonicalFrameLayersProps) {
  const preloaderRef = useRef<CanonicalFramePreloader | null>(null);
  const [failedFrameUrl, setFailedFrameUrl] = useState<string | null>(null);
  // A failure only counts while an active layer still shows that frame.
  const failedLayer = failedFrameUrl ? frameLayers.find(({ frameUrl }) => frameUrl === failedFrameUrl)?.layer : undefined;

  useEffect(() => {
    if (failedFrameUrl && !failedLayer) setFailedFrameUrl(null);
  }, [failedFrameUrl, failedLayer]);

  useEffect(() => {
    preloaderRef.current ??= new CanonicalFramePreloader();
    const preloader = preloaderRef.current;
    return () => preloader.clear();
  }, [sequences]);

  useEffect(() => {
    preloaderRef.current?.preload(canonicalPreloadUrls(sequences, seconds));
  }, [sequences, seconds]);

  return (
    <>
      {frameLayers.map(({ layer, frameUrl }) =>
        frameUrl === failedFrameUrl ? null : (
          <PreviewLayerFrame key={layer.itemId} layer={layer} className="pointer-events-none z-10">
            <img
              src={frameUrl}
              alt=""
              aria-hidden
              data-testid="canonical-prepared-preview-frame"
              data-prepared-item-id={layer.itemId}
              data-prepared-frame-url={frameUrl}
              className="pointer-events-none absolute object-contain"
              style={{
                ...(layer.centerX === undefined && layer.centerY === undefined ? { inset: 0, width: "100%", height: "100%" } : {}),
                ...previewMotionStyle(layer, outputSize),
              }}
              onError={() => { setFailedFrameUrl(frameUrl); refreshRemoteMediaUrl(frameUrl); }}
            />
          </PreviewLayerFrame>
        ),
      )}
      {frameLayers.length > 0 && !failedLayer && (
        <span className="pointer-events-none absolute bottom-2 left-2 z-30 rounded-[4px] bg-background/80 px-1.5 py-0.5 text-[10px] font-medium text-success">
          Canonical ready
        </span>
      )}
      {failedLayer && (
        <PreviewFailure
          title="Preview failed"
          message="Canonical preview frame failed to load."
          details={["Rebuild the prepared frame, or open its source to inspect the clip."]}
          tone="failure"
          onRetry={() => {
            setFailedFrameUrl(null);
            if (!failedFrameUrl || !refreshRemoteMediaUrl(failedFrameUrl, true)) onRetry();
          }}
          onOpenSource={() => onOpenSource(failedLayer.itemId, failedLayer.mediaId)}
        />
      )}
    </>
  );
}
