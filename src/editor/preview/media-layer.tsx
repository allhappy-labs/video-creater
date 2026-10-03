import { useRef, type ReactNode } from "react";
import { previewMotionStyle, type PreviewOutputSize } from "@/lib/preview/canvas-geometry";
import { transitionClipPath } from "@/lib/preview/transition-frame";
import type { TimelinePreviewLayer } from "@/lib/timeline-preview";
import { cn } from "@/lib/utils";
import type { TimelinePreviewAudioLayer } from "./compositor-model";
import { useMediaSynchronization } from "./use-media-synchronization";

const layerClassName = "absolute inset-0 h-full w-full object-contain";

interface MediaLayerProps {
  readonly layer: TimelinePreviewLayer;
  readonly sourceUrl: string;
  readonly playing: boolean;
  /** Render size for position offsets. */
  readonly outputSize: PreviewOutputSize;
  readonly onLoadError: () => void;
  readonly stackingIndex?: number | undefined;
}

/**
 * The canvas-sized frame around one visual layer. It is always present so a layer entering or
 * leaving a transition keeps its media element. A wipe clips the frame, so the reveal runs across
 * the output canvas whatever the layer's own position, scale, rotation or flip. The data attributes
 * expose the layer's transition role for tests and QA.
 */
export function PreviewLayerFrame({
  layer,
  className,
  children,
  stackingIndex,
}: {
  readonly layer: Pick<TimelinePreviewLayer, "itemId" | "transition">;
  readonly className?: string;
  readonly children: ReactNode;
  readonly stackingIndex?: number | undefined;
}) {
  const { transition } = layer;
  return (
    <div
      data-testid="preview-layer"
      data-item-id={layer.itemId}
      data-transition-kind={transition?.kind}
      data-transition-role={transition?.role}
      data-transition-progress={transition ? String(Math.round(transition.progress * 1000) / 1000) : undefined}
      className={cn("absolute inset-0", className)}
      style={{ clipPath: transitionClipPath(transition), zIndex: stackingIndex }}
    >
      {children}
    </div>
  );
}

/** One visual layer: an image, a Lottie placeholder, or a muted video that follows the playback clock. */
export function PreviewMediaLayer(props: MediaLayerProps) {
  return (
    <PreviewLayerFrame layer={props.layer} stackingIndex={props.stackingIndex}>
      <MediaLayerContent {...props} />
    </PreviewLayerFrame>
  );
}

/** A dip transition's opaque solid; render parity needs pure black or white, not theme colors. */
export function PreviewTransitionSolid({ transitionId, color, stackingIndex }: { readonly transitionId: string; readonly color: "black" | "white"; readonly stackingIndex?: number | undefined }) {
  return (
    <div
      aria-hidden
      data-testid="preview-transition-solid"
      data-transition-id={transitionId}
      data-color={color}
      className={cn("absolute inset-0", color === "black" ? "bg-black" : "bg-white")}
      style={{ zIndex: stackingIndex }}
    />
  );
}

function MediaLayerContent({ layer, sourceUrl, playing, outputSize, onLoadError }: MediaLayerProps) {
  if (layer.mediaKind === "image") {
    return <img alt={`Timeline image ${layer.label}`} src={sourceUrl} className={layerClassName} style={previewMotionStyle(layer, outputSize)} onError={onLoadError} />;
  }
  if (layer.mediaKind === "lottie") {
    return (
      <div role="img" aria-label={`Timeline Lottie ${layer.label}`} className="absolute inset-0 overflow-hidden bg-clip-graphics/15" style={previewMotionStyle(layer, outputSize)}>
        <div className="absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-warning/45" />
        <div className="absolute left-1/2 top-1/2 h-9 w-9 -translate-x-1/2 -translate-y-1/2 rounded-full bg-warning/30" />
      </div>
    );
  }
  return <PreviewVideoLayer layer={layer} sourceUrl={sourceUrl} playing={playing} outputSize={outputSize} onLoadError={onLoadError} />;
}

function PreviewVideoLayer({ layer, sourceUrl, playing, outputSize, onLoadError }: MediaLayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const synchronize = useMediaSynchronization(videoRef, {
    playing,
    itemId: layer.itemId,
    sourceUrl,
    sourceTimeSeconds: layer.sourceTimeSeconds,
    playbackRate: layer.playbackRate,
  });
  return (
    // Timeline video is always muted; sound comes from the audio layers.
    <video
      ref={videoRef}
      aria-label={`Timeline video ${layer.label}`}
      className={layerClassName}
      muted
      playsInline
      preload="metadata"
      src={sourceUrl}
      style={previewMotionStyle(layer, outputSize)}
      onError={onLoadError}
      onLoadedMetadata={synchronize}
    />
  );
}

/** A hidden audio element for one timeline audio layer; its volume follows the layer gain. */
export function PreviewAudioLayer({
  layer,
  sourceUrl,
  playing,
  onLoadError,
}: Omit<MediaLayerProps, "layer" | "outputSize"> & { readonly layer: TimelinePreviewAudioLayer }) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const synchronize = useMediaSynchronization(audioRef, {
    playing,
    itemId: layer.itemId,
    sourceUrl,
    sourceTimeSeconds: layer.sourceTimeSeconds,
    gain: layer.gain,
    playbackRate: layer.playbackRate,
  });
  return (
    <audio
      ref={audioRef}
      aria-label={`Timeline audio ${layer.label}`}
      className="hidden"
      preload="metadata"
      src={sourceUrl}
      onError={onLoadError}
      onLoadedMetadata={synchronize}
    />
  );
}
