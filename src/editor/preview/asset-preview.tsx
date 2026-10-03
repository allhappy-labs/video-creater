import { AudioLines, Image as ImageIcon, Sparkles, Undo2, Video } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useMediaReadiness } from "@/lib/media/use-media-readiness";
import { refreshRemoteMediaUrl } from "@/lib/runtime/adapters/remote-resource-cache";
import {
  mediaElementDurationSeconds,
  parsePreviewDurationLabel,
  selectedPreviewSource,
  sourcePreviewStepSeconds,
  type PreviewSource,
} from "@/lib/media/preview-source";
import { useEditorStore } from "../store/editor-store-context";
import { PreviewTransport, type PreviewFullscreenControls } from "./preview-transport";
import { PreviewViewport } from "./preview-viewport";
import { usePreviewKeys } from "./use-preview-keys";
import { AnimatedAssetPreview } from "./animated-asset-preview";

const playableKinds: ReadonlySet<string> = new Set(["video", "generated", "audio"]);
const waveformBars = Array.from({ length: 28 }, (_, index) => 18 + ((index * 13) % 42));

/** The "Previewing: name · Back to timeline" chip. */
export function AssetPreviewChip({ label, onBack }: { readonly label: string; readonly onBack: () => void }) {
  return (
    <div className="absolute left-3 top-3 z-40 flex max-w-[calc(100%-1.5rem)] items-center gap-1.5 rounded-full bg-popover/95 py-1 pl-3 pr-1 text-[12px] text-popover-foreground shadow-lg">
      <span className="min-w-0 truncate">
        Previewing: <span className="font-medium">{label}</span>
      </span>
      <span aria-hidden className="text-dim">
        ·
      </span>
      <button
        type="button"
        onClick={onBack}
        className="flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 font-medium text-accent hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Undo2 className="h-3.5 w-3.5" aria-hidden />
        Back to timeline
      </button>
    </div>
  );
}

function SourcePlaceholder({ label, kind }: { readonly label: string; readonly kind: string }) {
  const Icon = kind === "image" ? ImageIcon : kind === "lottie" ? Sparkles : Video;
  return (
    <div role="img" aria-label={label} className="absolute inset-0 flex items-center justify-center bg-raised text-dim">
      <Icon className="h-8 w-8" aria-hidden />
    </div>
  );
}

/**
 * Asset preview mode: one media asset alone in the viewer with its own transport. Playback follows
 * the store `playing` flag (Space and the play button toggle it); the media element reports pauses,
 * the end and load failures back.
 */
export function AssetPreview(props: { readonly mediaId: string } & PreviewFullscreenControls) {
  const kind = useEditorStore((state) => state.project.media.find((media) => media.id === props.mediaId)?.kind);
  return kind === "lottie" ? <AnimatedAssetPreview {...props} /> : <PlayableAssetPreview {...props} />;
}

function PlayableAssetPreview({ mediaId, fullscreen, onToggleFullscreen }: { readonly mediaId: string } & PreviewFullscreenControls) {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const mediaReadiness = useMediaReadiness(projectDir);
  const playing = useEditorStore((state) => state.playing);
  const setPlaying = useEditorStore((state) => state.setPlaying);
  const togglePlaying = useEditorStore((state) => state.togglePlaying);
  const previewTimeline = useEditorStore((state) => state.previewTimeline);
  const source = selectedPreviewSource(project, mediaId, null, projectDir);
  const asset = project.media.find((media) => media.id === mediaId);
  const elementRef = useRef<HTMLMediaElement | null>(null);
  const lastElementRef = useRef<HTMLMediaElement | null>(null);
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = Boolean(source?.previewUrl) && failedUrl === source?.previewUrl;
  const [position, setPosition] = useState(() => ({ currentSeconds: 0, durationSeconds: parsePreviewDurationLabel(source?.durationLabel) }));
  const canPlay = Boolean(source?.previewUrl) && !failed && playableKinds.has(source?.kind ?? "");
  const canSeek = canPlay && position.durationSeconds > 0;
  const stepSeconds = sourcePreviewStepSeconds(source);

  const desiredPlayingRef = useRef(playing);
  desiredPlayingRef.current = playing;
  const positionRef = useRef(position.currentSeconds);
  const sourceUrlRef = useRef(source?.previewUrl);
  const restoringRef = useRef(false);
  if (sourceUrlRef.current !== source?.previewUrl) {
    sourceUrlRef.current = source?.previewUrl;
    restoringRef.current = true;
  }

  function syncPosition() {
    if (restoringRef.current) return;
    const element = elementRef.current;
    positionRef.current = element && Number.isFinite(element.currentTime) ? Math.max(0, element.currentTime) : 0;
    setPosition({
      currentSeconds: element && Number.isFinite(element.currentTime) ? Math.max(0, element.currentTime) : 0,
      durationSeconds: mediaElementDurationSeconds(element, source),
    });
  }

  function synchronizePlayback() {
    const element = elementRef.current;
    if (!playing) {
      if (element && !element.paused) element.pause();
      return;
    }
    if (!canPlay || !element) {
      if (mediaReadiness.status !== "loading") setPlaying(false);
      return;
    }
    const url = source?.previewUrl;
    if (element.paused) void element.play().then(() => {
      if (!desiredPlayingRef.current || !element.isConnected) element.pause();
    }).catch(() => {
      if (sourceUrlRef.current === url && desiredPlayingRef.current) setPlaying(false);
    });
  }

  useEffect(synchronizePlayback, [canPlay, playing, setPlaying, source?.previewUrl, mediaReadiness.status]);

  function loadedMetadata() {
    const element = elementRef.current;
    if (element && restoringRef.current) {
      element.currentTime = positionRef.current;
      restoringRef.current = false;
    }
    syncPosition();
    synchronizePlayback();
  }

  if (elementRef.current) lastElementRef.current = elementRef.current;
  useEffect(() => () => { desiredPlayingRef.current = false; const element = elementRef.current ?? lastElementRef.current; if (element && !element.paused) element.pause(); }, []);

  function step(direction: -1 | 1) {
    const element = elementRef.current;
    if (!canPlay || !element || stepSeconds <= 0) return;
    const duration = mediaElementDurationSeconds(element, source);
    element.currentTime = Math.max(0, Math.min(duration > 0 ? duration : Infinity, element.currentTime + stepSeconds * direction));
    syncPosition();
  }

  function seek(seconds: number) {
    const element = elementRef.current;
    if (!canPlay || !element || !Number.isFinite(seconds)) return;
    const duration = mediaElementDurationSeconds(element, source);
    if (duration <= 0) return;
    element.currentTime = Math.max(0, Math.min(duration, seconds));
    syncPosition();
  }

  const onKeyDown = usePreviewKeys({ canPlay, canStep: canPlay && stepSeconds > 0, onTogglePlay: togglePlaying, onStep: step });
  if (!source || !asset) return null;

  return (
    <div className="flex min-h-0 flex-1 flex-col" onKeyDown={onKeyDown}>
      <PreviewViewport
        width={asset.width ?? project.renderSettings.width}
        height={asset.height ?? project.renderSettings.height}
        chrome={<AssetPreviewChip label={source.label} onBack={previewTimeline} />}
      >
        <SourceViewer
          source={source}
          failed={failed}
          elementRef={elementRef}
          onTimeChange={syncPosition}
          onLoadedMetadata={loadedMetadata}
          onPlay={() => setPlaying(true)}
          onStop={() => { if (!restoringRef.current) setPlaying(false); }}
          onError={() => {
            setFailedUrl(source.previewUrl ?? null);
            if (source.previewUrl) refreshRemoteMediaUrl(source.previewUrl);
            setPlaying(false);
          }}
          onRetry={() => {
            if (source.previewUrl) refreshRemoteMediaUrl(source.previewUrl, true);
            setFailedUrl(null);
            setPlaying(false);
          }}
        />
      </PreviewViewport>
      <PreviewTransport
        currentSeconds={position.currentSeconds}
        durationSeconds={position.durationSeconds}
        playing={playing}
        canPlay={canPlay}
        canSeek={canSeek}
        canStep={canPlay && stepSeconds > 0}
        scrubStepSeconds={stepSeconds > 0 ? stepSeconds : 0.01}
        aspectLabel={source.aspectRatioLabel ?? null}
        fullscreen={fullscreen}
        onTogglePlay={togglePlaying}
        onSeek={seek}
        onStep={step}
        onToggleFullscreen={onToggleFullscreen}
      />
    </div>
  );
}

interface SourceViewerProps {
  readonly source: PreviewSource;
  readonly failed: boolean;
  readonly elementRef: { current: HTMLMediaElement | null };
  readonly onTimeChange: () => void;
  readonly onLoadedMetadata: () => void;
  readonly onPlay: () => void;
  readonly onStop: () => void;
  readonly onError: () => void;
  readonly onRetry: () => void;
}

function SourceViewer({ source, failed, elementRef, onTimeChange, onLoadedMetadata, onPlay, onStop, onError, onRetry }: SourceViewerProps) {
  if (failed) {
    return (
      <div role="alert" className="absolute inset-0 flex flex-col items-center justify-center gap-3 px-6 text-center text-[13px] text-foreground">
        <p className="font-medium">Preview failed</p>
        <button
          type="button"
          onClick={onRetry}
          className="h-7 rounded-md bg-accent px-2.5 text-[12px] font-medium text-accent-foreground hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Retry preview
        </button>
      </div>
    );
  }
  const mediaEvents = {
    onDurationChange: onTimeChange,
    onLoadedMetadata,
    onTimeUpdate: onTimeChange,
    onPlay,
    onPause: onStop,
    onEnded: onStop,
    onError,
  };
  const url = source.previewUrl ?? undefined;

  if (source.kind === "audio") {
    return (
      <div className="absolute inset-0 flex items-center justify-center px-[6%]">
        {url && (
          <audio ref={(element) => void (elementRef.current = element)} aria-label={`Audio preview ${source.label}`} className="hidden" preload="metadata" src={url} {...mediaEvents} />
        )}
        <div aria-label="Audio source waveform" className="flex h-[40%] w-full items-center gap-[0.6cqw] rounded-md bg-clip-audio/25 px-[3cqw]">
          <AudioLines className="mr-2 h-5 w-5 shrink-0 text-foreground" aria-hidden />
          {waveformBars.map((height, index) => (
            <span key={index} className="flex-1 rounded-full bg-clip-audio" style={{ height: `${height}%` }} />
          ))}
        </div>
      </div>
    );
  }
  if (url && (source.kind === "video" || source.kind === "generated")) {
    return (
      // Source playback includes its own sound; timeline sound uses separate audio layers.
      <video
        ref={(element) => void (elementRef.current = element)}
        aria-label={`Video preview ${source.label}`}
        className="absolute inset-0 h-full w-full object-contain"
        playsInline
        preload="metadata"
        src={url}
        {...mediaEvents}
      />
    );
  }
  if (url && source.kind === "image") {
    return <img alt={`Image preview ${source.label}`} className="absolute inset-0 h-full w-full object-contain" src={url} onError={onError} />;
  }
  return (
    <SourcePlaceholder
      kind={source.kind}
      label={source.kind === "image" ? "Image source frame" : source.kind === "lottie" ? `Lottie preview ${source.label}` : "Video source frame"}
    />
  );
}
