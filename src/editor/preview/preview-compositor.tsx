import { Film } from "lucide-react";
import { useState } from "react";
import { refreshRemoteMediaUrl } from "@/lib/runtime/adapters/remote-resource-cache";
import type { MediaAsset } from "@/lib/project";
import { findTimelineItem, type PreviewOutputSize } from "@/lib/preview/canvas-geometry";
import type { Timeline } from "@/lib/timeline";
import type { TimelinePreviewFrame } from "@/lib/timeline-preview";
import type { CompositorCanonicalState } from "./canonical-frames";
import { buildCompositorModel, type CompositorModelInput } from "./compositor-model";
import { PreviewAudioLayer, PreviewMediaLayer, PreviewTransitionSolid } from "./media-layer";
import { PreviewOverlayLayer } from "./overlay-layer";
import type { OverlayTextInteraction } from "./inline-text-editor";
import { PreviewFailure } from "./preview-failure";

const noFailedLayers: ReadonlySet<string> = new Set();

export interface PreviewCompositorProps {
  readonly frame: TimelinePreviewFrame;
  readonly timeline: Timeline;
  readonly media: readonly MediaAsset[];
  readonly playheadSeconds: number;
  readonly playing: boolean;
  /** Render size for position offsets. */
  readonly outputSize: PreviewOutputSize;
  readonly mediaPreviewUrls: Readonly<Record<string, string | null | undefined>>;
  readonly canonical: CompositorCanonicalState;
  readonly coverageItemIds: ReadonlySet<string>;
  readonly canonicalFrameCount: number;
  /** The ready prepared project's audio layers; reversed audio plays these. */
  readonly preparedAudioLayers: CompositorModelInput["preparedAudioLayers"];
  readonly onRetryCanonical: () => void;
  readonly onOpenSource: (itemId: string, mediaId: string) => void;
  /** Canvas selection and inline editing for caption and text layers. */
  readonly textInteraction?: OverlayTextInteraction | undefined;
  readonly layerStackingOrder?: Readonly<Record<string, number>> | undefined;
  readonly mediaLoading?: boolean | undefined;
}

/**
 * The DOM composition of one timeline frame inside the preview canvas: visual media layers in track
 * order, hidden audio elements, captions, text and templates, the empty state and the issue message.
 * Canonical prepared frames and canvas selection render as siblings in the same canvas.
 */
export function PreviewCompositor(props: PreviewCompositorProps) {
  const { frame, timeline, playing, outputSize, onRetryCanonical, onOpenSource, textInteraction } = props;
  // Failed layers reset whenever the set of active media layers changes.
  const activeLayerKey = [...frame.layers, ...frame.audioLayers].map((layer) => `${layer.itemId}:${layer.relativePath}:${props.mediaPreviewUrls[layer.mediaId] ?? ""}`).join("|");
  const [failed, setFailed] = useState<{ key: string; ids: ReadonlySet<string> }>({ key: activeLayerKey, ids: noFailedLayers });
  const failedLayerIds = failed.key === activeLayerKey ? failed.ids : noFailedLayers;
  const model = buildCompositorModel({ ...props, failedLayerIds });
  const { problemLayer } = model;
  const solidsBefore = (itemId: string | null) =>
    model.transitionSolids
      .filter((solid) => solid.beforeItemId === itemId)
      .map((solid) => <PreviewTransitionSolid key={`transition-solid:${solid.transitionId}`} transitionId={solid.transitionId} color={solid.color} stackingIndex={solid.beforeItemId === null ? -1 : (props.layerStackingOrder?.[solid.beforeItemId] ?? 0) - 1} />);

  function markFailed(itemId: string) {
    setFailed((current) => {
      const ids = new Set(current.key === activeLayerKey ? current.ids : []);
      ids.add(itemId);
      return { key: activeLayerKey, ids };
    });
  }

  function retry() {
    if (model.retryPreparesCanonical) onRetryCanonical();
    if (model.retryReloadsLayers) {
      for (const layer of [...frame.layers, ...frame.audioLayers]) {
        const url = props.mediaPreviewUrls[layer.mediaId];
        if (url && failedLayerIds.has(layer.itemId)) refreshRemoteMediaUrl(url, true);
      }
      setFailed({ key: activeLayerKey, ids: noFailedLayers });
    }
  }

  return (
    <>
      {[
        // Dip solids sit just beneath their transition's clips, in one keyed list so media elements keep their identity.
        ...model.visibleMediaLayers.flatMap(({ layer, sourceUrl }) => [
          ...solidsBefore(layer.itemId),
          <PreviewMediaLayer key={layer.itemId} layer={layer} sourceUrl={sourceUrl} playing={playing} outputSize={outputSize} stackingIndex={props.layerStackingOrder?.[layer.itemId]} onLoadError={() => { markFailed(layer.itemId); refreshRemoteMediaUrl(sourceUrl); }} />,
        ]),
        ...solidsBefore(null),
      ]}
      {model.visibleAudioLayers.map(({ layer, sourceUrl }) => (
        <PreviewAudioLayer key={layer.itemId} layer={layer} sourceUrl={sourceUrl} playing={playing} onLoadError={() => { markFailed(layer.itemId); refreshRemoteMediaUrl(sourceUrl); }} />
      ))}
      {model.emptyStateCopy !== null && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 px-5 text-center text-[12px] text-muted-foreground">
          <Film className="h-5 w-5" aria-hidden />
          <span>{model.emptyStateCopy}</span>
        </div>
      )}
      {model.visibleOverlayLayers.map((layer) => (
        <PreviewOverlayLayer
          key={layer.itemId}
          layer={layer}
          item={layer.overlayKind === "template" ? findTimelineItem(timeline, layer.itemId) : null}
          outputSize={outputSize}
          interaction={textInteraction}
        />
      ))}
      {model.issues[0] !== undefined && (
        <PreviewFailure
          title={model.issueTitle}
          message={model.issues[0]}
          details={model.issues}
          tone={model.issueState === "retry" ? "failure" : "notice"}
          onRetry={model.issueState === "retry" ? retry : undefined}
          onOpenSource={problemLayer ? () => onOpenSource(problemLayer.itemId, problemLayer.mediaId) : undefined}
        />
      )}
      {model.failedLayerCount > 0 && (
        <div className="sr-only" aria-live="polite">
          {`Preview skipped ${model.failedLayerCount} failed timeline layer${model.failedLayerCount === 1 ? "" : "s"}.`}
        </div>
      )}
    </>
  );
}
