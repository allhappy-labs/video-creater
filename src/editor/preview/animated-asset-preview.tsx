import { useEffect, useMemo, useRef, useState } from "react";
import { aspectRatioLabel, mediaDisplayName } from "@/lib/media/names";
import { useMediaReadiness } from "@/lib/media/use-media-readiness";
import { prepareProjectPreview, type PreparedProjectPreview } from "@/lib/project";
import { advancePlayhead, frameStepSeconds } from "@/lib/preview/playback-clock";
import { previewOutputSize } from "@/lib/preview/canvas-geometry";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { useEditorStore } from "../store/editor-store-context";
import { AssetPreviewChip } from "./asset-preview";
import { canonicalFrameLayers, canonicalFrameSequences } from "./canonical-frames";
import { CanonicalFrameLayers } from "./canonical-frame-layers";
import { PreviewFailure } from "./preview-failure";
import { PreviewTransport, type PreviewFullscreenControls } from "./preview-transport";
import { PreviewViewport } from "./preview-viewport";
import { usePreviewKeys } from "./use-preview-keys";
import { useCanonicalFrameResources } from "./use-canonical-frame-resources";
import { previewPlaybackReady } from "./playback-readiness";

type Preparation = { status: "loading" } | { status: "ready"; result: PreparedProjectPreview } | { status: "failed"; message: string };

/** Source animation uses the same sampled renderer as the timeline and export. */
export function AnimatedAssetPreview({ mediaId, fullscreen, onToggleFullscreen }: { readonly mediaId: string } & PreviewFullscreenControls) {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const playing = useEditorStore((state) => state.playing);
  const setPlaying = useEditorStore((state) => state.setPlaying);
  const previewTimeline = useEditorStore((state) => state.previewTimeline);
  const mediaReadiness = useMediaReadiness(projectDir);
  const asset = project.media.find((media) => media.id === mediaId)!;
  const [preparation, setPreparation] = useState<Preparation>({ status: "loading" });
  const [retry, setRetry] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const secondsRef = useRef(seconds);
  const lastFrameRef = useRef<number | null>(null);
  const previewRef = useRef<HTMLDivElement | null>(null);
  const ready = preparation.status === "ready";
  const duration = ready ? preparation.result.project.timeline.durationSeconds : asset.durationSeconds ?? 0;
  const settings = ready ? preparation.result.project.renderSettings : { ...project.renderSettings, width: asset.width ?? project.renderSettings.width, height: asset.height ?? project.renderSettings.height, fps: asset.fps && asset.fps > 0 ? asset.fps : project.renderSettings.fps };
  const fps = settings.fps;

  useEffect(() => {
    let cancelled = false;
    setPreparation({ status: "loading" });
    setPlaying(false);
    void prepareProjectPreview({ projectDir, project, mediaId }).then(
      (result) => { if (!cancelled) setPreparation({ status: "ready", result }); },
      (error: unknown) => { if (!cancelled) setPreparation({ status: "failed", message: error instanceof Error ? error.message : String(error) }); },
    );
    return () => { cancelled = true; };
  }, [mediaId, project, projectDir, retry, setPlaying]);

  useEffect(() => {
    if (!playing || !ready) return;
    lastFrameRef.current = null;
    let frameId = 0;
    const reanchor = () => { lastFrameRef.current = null; };
    const tick = (now: number) => {
      if (!previewPlaybackReady(previewRef.current)) {
        lastFrameRef.current = null;
        frameId = requestAnimationFrame(tick);
        return;
      }
      const next = advancePlayhead({ playheadSeconds: secondsRef.current, durationSeconds: duration, lastFrameMs: lastFrameRef.current }, now);
      secondsRef.current = next.playheadSeconds;
      lastFrameRef.current = next.lastFrameMs;
      setSeconds(next.playheadSeconds);
      if (next.ended) setPlaying(false);
      else frameId = requestAnimationFrame(tick);
    };
    frameId = requestAnimationFrame(tick);
    document.addEventListener("visibilitychange", reanchor);
    return () => { cancelAnimationFrame(frameId); document.removeEventListener("visibilitychange", reanchor); };
    // A scrub reanchors the source clock at the newly selected time.
  }, [playing, ready, duration, setPlaying]);

  useCanonicalFrameResources(ready ? preparation.result : null, projectDir, seconds);
  const sequences = useMemo(() => ready ? canonicalFrameSequences(preparation.result, projectDir) : [], [preparation, ready, projectDir, mediaReadiness.version]);
  const displaySeconds = Math.min(seconds, Math.max(0, duration - 1 / fps));
  const frame = ready ? buildTimelinePreviewFrame({ ...preparation.result.project, playheadSeconds: displaySeconds, fps }) : null;
  const layers = canonicalFrameLayers(frame, sequences, displaySeconds);
  const outputSize = previewOutputSize(settings);
  const seek = (next: number) => { secondsRef.current = Math.max(0, Math.min(duration, next)); lastFrameRef.current = null; setSeconds(secondsRef.current); };
  const toggle = () => {
    if (!ready) return;
    if (!playing && seconds >= duration) seek(0);
    setPlaying(!playing);
  };
  const step = (direction: -1 | 1) => seek(frameStepSeconds(seconds, fps, direction));
  const onKeyDown = usePreviewKeys({ canPlay: ready, canStep: ready, onTogglePlay: toggle, onStep: step });
  return (
    <div ref={previewRef} data-preview-buffering={ready && layers.length === 0 ? "true" : undefined} className="flex min-h-0 flex-1 flex-col" onKeyDown={onKeyDown}>
      <PreviewViewport width={outputSize.width} height={outputSize.height} chrome={<AssetPreviewChip label={mediaDisplayName(asset)} onBack={previewTimeline} />}>
        {preparation.status === "loading" && <p role="status" className="absolute inset-0 flex items-center justify-center text-[13px] text-muted-foreground">Preparing animated source…</p>}
        {preparation.status === "failed" && <PreviewFailure title="Preview failed" message={preparation.message} details={[]} tone="failure" onRetry={() => setRetry((value) => value + 1)} />}
        <CanonicalFrameLayers frameLayers={layers} sequences={sequences} seconds={displaySeconds} outputSize={outputSize} onRetry={() => setRetry((value) => value + 1)} />
      </PreviewViewport>
      <PreviewTransport currentSeconds={seconds} durationSeconds={duration} playing={playing} canPlay={ready && duration > 0} canSeek={ready && duration > 0} canStep={ready && duration > 0} scrubStepSeconds={1 / fps} aspectLabel={aspectRatioLabel(outputSize.width, outputSize.height)} fullscreen={fullscreen} onTogglePlay={toggle} onSeek={seek} onStep={step} onToggleFullscreen={onToggleFullscreen} />
    </div>
  );
}
