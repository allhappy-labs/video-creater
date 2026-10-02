import { useMemo, useState } from "react";
import { aspectRatioLabel } from "@/lib/media/names";
import { previewUrlsForTimelineSources } from "@/lib/media/preview-source";
import { useMediaReadiness } from "@/lib/media/use-media-readiness";
import { projectNeedsCanonicalPreview } from "@/lib/project";
import { previewOutputSize } from "@/lib/preview/canvas-geometry";
import { frameStepSeconds } from "@/lib/preview/playback-clock";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { useEditorStore } from "../store/editor-store-context";
import { CanonicalFrameLayers } from "./canonical-frame-layers";
import { canvasEditableItemIds, frameWithCanvasOverride, interactiveCanvasLayers, type CanvasLayerOverride } from "./canvas-editing";
import { CanvasSelection } from "./canvas-selection";
import { useInlineTextEditing } from "./inline-text-editor";
import {
  canonicalCoverageItemIds,
  canonicalFrameLayers,
  canonicalFrameSequences,
  canonicalStateForProject,
  preparedAudioLayers,
  preparedResultForProject,
} from "./canonical-frames";
import { PreviewCompositor } from "./preview-compositor";
import { PreviewTransport, type PreviewFullscreenControls } from "./preview-transport";
import { timelineWithPropertyPreview } from "./property-preview";
import { PreviewViewport } from "./preview-viewport";
import { usePreviewKeys } from "./use-preview-keys";

const fallbackFps = 24;

/** Timeline mode: the composition at the store playhead, canonical prepared frames and the timeline transport. */
export function TimelinePreview({
  onOpenSource,
  fullscreen,
  onToggleFullscreen,
}: { readonly onOpenSource: (itemId: string, mediaId: string) => void } & PreviewFullscreenControls) {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const mediaReadiness = useMediaReadiness(projectDir);
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const playing = useEditorStore((state) => state.playing);
  const preparation = useEditorStore((state) => state.canonicalPreparation);
  const retryCanonicalPreparation = useEditorStore((state) => state.retryCanonicalPreparation);
  const seek = useEditorStore((state) => state.seek);
  const togglePlaying = useEditorStore((state) => state.togglePlaying);
  const propertyPreview = useEditorStore((state) => state.propertyPreview);
  const [canvasOverride, setCanvasOverride] = useState<CanvasLayerOverride | null>(null);
  const { timeline, timelines, media, generatedAssets, renderSettings } = project;
  const durationSeconds = Math.max(0, timeline.durationSeconds);
  const clampedSeconds = Math.min(durationSeconds, Math.max(0, playheadSeconds));
  const outputSize = previewOutputSize(renderSettings);
  const fps = renderSettings.fps > 0 ? renderSettings.fps : fallbackFps;
  const transportReady = Number.isFinite(timeline.durationSeconds) && timeline.durationSeconds > 0;
  const stepFrame = (direction: -1 | 1) => seek(frameStepSeconds(clampedSeconds, fps, direction));
  const onKeyDown = usePreviewKeys({ canPlay: transportReady, canStep: transportReady, onTogglePlay: togglePlaying, onStep: stepFrame });

  const mediaPreviewUrls = useMemo(() => previewUrlsForTimelineSources(projectDir, media, generatedAssets), [generatedAssets, media, projectDir, mediaReadiness.version]);
  const needsCanonical = useMemo(() => projectNeedsCanonicalPreview(project), [project]);
  const canonical = canonicalStateForProject(preparation, project, needsCanonical);
  const prepared = preparedResultForProject(preparation, project);
  const sequences = useMemo(() => (prepared ? canonicalFrameSequences(prepared, projectDir) : []), [prepared, projectDir, mediaReadiness.version]);

  // A dragged Properties value shows live without touching the project.
  const previewTimeline = useMemo(() => timelineWithPropertyPreview(timeline, propertyPreview), [propertyPreview, timeline]);
  const frame = frameWithCanvasOverride(
    buildTimelinePreviewFrame({ timeline: previewTimeline, ...(timelines ? { timelines } : {}), media, generatedAssets, playheadSeconds, fps }),
    canvasOverride,
  );
  const editableItemIds = useMemo(() => canvasEditableItemIds(timeline), [timeline]);
  const textInteraction = useInlineTextEditing(timeline, frame.overlayLayers.map((layer) => layer.itemId));
  const preparedFrame = prepared
    ? buildTimelinePreviewFrame({
        timeline: prepared.project.timeline,
        ...(prepared.project.timelines ? { timelines: prepared.project.timelines } : {}),
        media: prepared.project.media,
        generatedAssets: prepared.project.generatedAssets,
        playheadSeconds: clampedSeconds,
        fps,
      })
    : null;
  const frameLayers = canonicalFrameLayers(preparedFrame, sequences, clampedSeconds);
  const coverageItemIds = canonicalCoverageItemIds(frameLayers, preparedFrame, frame);
  const preparedAudio = preparedAudioLayers(preparedFrame, projectDir);
  const interactiveLayers = interactiveCanvasLayers({ frame, canonical, coverageItemIds, mediaPreviewUrls });

  return (
    <div className="flex min-h-0 flex-1 flex-col" onKeyDown={onKeyDown}>
      <PreviewViewport width={renderSettings.width} height={renderSettings.height}>
        <PreviewCompositor
          frame={frame}
          timeline={previewTimeline}
          media={media}
          playheadSeconds={playheadSeconds}
          playing={playing}
          outputSize={outputSize}
          mediaPreviewUrls={mediaPreviewUrls}
          canonical={canonical}
          coverageItemIds={coverageItemIds}
          canonicalFrameCount={frameLayers.length}
          preparedAudioLayers={preparedAudio}
          onRetryCanonical={retryCanonicalPreparation}
          onOpenSource={onOpenSource}
          textInteraction={textInteraction}
        />
        <CanonicalFrameLayers
          frameLayers={frameLayers}
          sequences={sequences}
          seconds={clampedSeconds}
          outputSize={outputSize}
          onRetry={retryCanonicalPreparation}
          onOpenSource={onOpenSource}
        />
        <CanvasSelection
          layers={interactiveLayers}
          editableItemIds={editableItemIds}
          outputSize={outputSize}
          override={canvasOverride}
          onOverride={setCanvasOverride}
        />
      </PreviewViewport>
      <PreviewTransport
        currentSeconds={clampedSeconds}
        durationSeconds={durationSeconds}
        playing={playing}
        canPlay={transportReady}
        canSeek={transportReady}
        canStep={transportReady}
        scrubStepSeconds={1 / fps}
        aspectLabel={renderSettings.width > 0 && renderSettings.height > 0 ? aspectRatioLabel(renderSettings.width, renderSettings.height) : null}
        fullscreen={fullscreen}
        onTogglePlay={togglePlaying}
        onSeek={seek}
        onStep={stepFrame}
        onToggleFullscreen={onToggleFullscreen}
      />
    </div>
  );
}
