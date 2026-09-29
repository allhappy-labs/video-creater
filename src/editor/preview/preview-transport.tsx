import { ChevronLeft, ChevronRight, Maximize, Minimize, Pause, Play } from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import { Slider } from "@/components/ui/slider";
import { formatPreviewCurrentTime } from "@/lib/media/preview-source";
import { cn } from "@/lib/utils";
import { labelWithShortcut, useShortcutPlatform } from "../shell/use-shortcut-platform";

export interface PreviewFullscreenControls {
  readonly fullscreen: boolean;
  readonly onToggleFullscreen: () => void;
}

interface PreviewTransportProps extends PreviewFullscreenControls {
  readonly currentSeconds: number;
  readonly durationSeconds: number;
  readonly playing: boolean;
  readonly canPlay: boolean;
  readonly canSeek: boolean;
  readonly canStep: boolean;
  /** Scrubber keyboard step in seconds. */
  readonly scrubStepSeconds: number;
  readonly aspectLabel: string | null;
  readonly onTogglePlay: () => void;
  readonly onSeek: (seconds: number) => void;
  readonly onStep: (direction: -1 | 1) => void;
}

/** Narrow transports (the phone layout) keep time, play/pause and fullscreen; frame keys still step. */
const wideOnly = "[@container(max-width:30rem)]:hidden";

/** The transport row: scrubber, time, previous and next frame, play/pause, aspect ratio and fullscreen. */
export function PreviewTransport(props: PreviewTransportProps) {
  const { currentSeconds, durationSeconds, playing, canPlay, canSeek, canStep, fullscreen } = props;
  const platform = useShortcutPlatform();
  const scrubberMax = durationSeconds > 0 ? durationSeconds : 1;
  const scrubberValue = durationSeconds > 0 ? Math.min(durationSeconds, Math.max(0, currentSeconds)) : 0;
  const playLabel = playing ? "Pause preview" : "Play preview";
  const fullscreenLabel = fullscreen ? "Exit fullscreen" : "Enter fullscreen";

  return (
    <div role="group" aria-label="Preview transport" className="shrink-0 px-3 pb-1 [container-type:inline-size]">
      <Slider
        label="Preview scrubber"
        valueText={formatPreviewCurrentTime(scrubberValue)}
        min={0}
        max={scrubberMax}
        step={props.scrubStepSeconds}
        value={[scrubberValue]}
        disabled={!canSeek}
        onValueChange={([seconds]) => {
          if (seconds !== undefined && Number.isFinite(seconds)) props.onSeek(seconds);
        }}
        className="w-full"
      />
      <div className="grid h-9 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-2">
        <p className="tabular-time truncate text-[12px] text-foreground">
          {formatPreviewCurrentTime(currentSeconds)}
          <span className="text-muted-foreground"> / {formatPreviewCurrentTime(durationSeconds)}</span>
        </p>
        <div className="flex items-center gap-1">
          <IconButton label="Previous frame" tooltip={labelWithShortcut("Previous frame", "preview.stepBack", platform)} disabled={!canStep} onClick={() => props.onStep(-1)} className={wideOnly}>
            <ChevronLeft className="h-4 w-4" aria-hidden />
          </IconButton>
          <IconButton label={playLabel} tooltip={labelWithShortcut(playLabel, "preview.toggle", platform)} disabled={!canPlay} onClick={props.onTogglePlay} className="text-foreground">
            {playing ? <Pause className="h-4 w-4" aria-hidden /> : <Play className="h-4 w-4" aria-hidden />}
          </IconButton>
          <IconButton label="Next frame" tooltip={labelWithShortcut("Next frame", "preview.stepForward", platform)} disabled={!canStep} onClick={() => props.onStep(1)} className={wideOnly}>
            <ChevronRight className="h-4 w-4" aria-hidden />
          </IconButton>
        </div>
        <div className="flex min-w-0 items-center justify-end gap-1">
          {props.aspectLabel && (
            <span className={cn("tabular-time shrink-0 rounded-md bg-raised px-2 py-0.5 text-[11px] text-muted-foreground", wideOnly)}>{props.aspectLabel}</span>
          )}
          <IconButton label={fullscreenLabel} onClick={props.onToggleFullscreen}>
            {fullscreen ? <Minimize className="h-4 w-4" aria-hidden /> : <Maximize className="h-4 w-4" aria-hidden />}
          </IconButton>
        </div>
      </div>
    </div>
  );
}
