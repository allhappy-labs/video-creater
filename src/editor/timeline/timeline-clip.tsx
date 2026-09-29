import { Rewind } from "lucide-react";
import { useId, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { Tooltip } from "@/components/ui/tooltip";
import { formatTimecode } from "@/lib/format";
import { generatedAssetForTimelineItem, generatedAssetTitleWithPrompt } from "@/lib/generation/assets";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { timelineItemHasAudio } from "@/lib/timeline-ops/automation";
import { deadAirRangesForItem } from "@/lib/timeline-ops/dead-air";
import { numberProperty, visualClipOpacity } from "@/lib/timeline-ops/item-properties";
import { isReversedItem } from "@/lib/timeline-ops/reverse";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { ClipFilmstrip } from "./clip-filmstrip";
import { clipFillClass, clipTone, clipToneIcon } from "./clip-kind";
import { ClipWaveform } from "./clip-waveform";
import { laneLayerClass } from "./timeline-layers";
import type { LivePlacement } from "./use-clip-trim";
import { timelineClipInset as clipInset } from "./use-timeline-geometry";

const minimumClipWidth = 2;
/** Clips narrower than this drop the label chip; the option name still carries the label. */
const minimumLabelWidth = 24;

function timelineClipName(item: TimelineItem): string {
  return `${item.label}, ${formatTimecode(item.startSeconds)}, ${formatTimecode(item.durationSeconds)}`;
}

function fadeSeconds(item: TimelineItem, key: "fadeInSeconds" | "fadeOutSeconds"): number {
  const value = numberProperty(item, key);
  return value !== null && value > 0 && item.durationSeconds > 0 ? Math.min(value, item.durationSeconds) : 0;
}

type GenerationState = { readonly kind: "running" } | { readonly kind: "failed"; readonly message: string } | null;

function generationState(project: VideoProject, item: TimelineItem): GenerationState {
  const asset = generatedAssetForTimelineItem(project, item);
  if (!asset) return null;
  if (asset.status === "queued" || asset.status === "running") return { kind: "running" };
  if (asset.status === "failed") return { kind: "failed", message: `Generation failed: ${generatedAssetTitleWithPrompt(asset)}` };
  return null;
}

function ClipOverlays({ item, project, audio }: { item: TimelineItem; project: VideoProject; audio: boolean }) {
  const opacity = visualClipOpacity(item);
  const fadeIn = fadeSeconds(item, "fadeInSeconds");
  const fadeOut = fadeSeconds(item, "fadeOutSeconds");
  const deadAir = audio ? deadAirRangesForItem(project, item) : [];
  return (
    <>
      {opacity !== null && (
        <span data-testid="clip-opacity-overlay" aria-hidden className="pointer-events-none absolute inset-0 bg-background" style={{ opacity: 1 - opacity }} />
      )}
      {deadAir.map((range) => (
        <span
          key={range.startSeconds}
          data-testid="dead-air-hatch"
          aria-hidden
          className="pointer-events-none absolute inset-y-0 bg-[repeating-linear-gradient(135deg,hsl(var(--background)_/_0.6)_0_2px,transparent_2px_6px)]"
          style={{ left: `${range.startRatio * 100}%`, width: `${(range.endRatio - range.startRatio) * 100}%` }}
        />
      ))}
      {fadeIn > 0 && (
        <span
          data-testid="clip-fade-in"
          aria-hidden
          className="pointer-events-none absolute inset-y-0 left-0 bg-background/45 [clip-path:polygon(0_0,100%_0,0_100%)]"
          style={{ width: `${(fadeIn / item.durationSeconds) * 100}%` }}
        />
      )}
      {fadeOut > 0 && (
        <span
          data-testid="clip-fade-out"
          aria-hidden
          className="pointer-events-none absolute inset-y-0 right-0 bg-background/45 [clip-path:polygon(0_0,100%_0,100%_100%)]"
          style={{ width: `${(fadeOut / item.durationSeconds) * 100}%` }}
        />
      )}
    </>
  );
}

interface TimelineClipProps {
  readonly item: TimelineItem;
  readonly pixelsPerSecond: number;
  readonly rowHeight: number;
  readonly selected: boolean;
  /** Changed by the AI edit whose Show changes is active: an accent outline and a description. */
  readonly highlighted?: boolean;
  /** Live start and duration while the clip is trimmed or ripple-previewed. */
  readonly placement?: LivePlacement | undefined;
  /** The clip is being dragged: its ghost shows the target, so the original dims. */
  readonly dragging?: boolean;
  /** The track is hidden or muted. */
  readonly dimmed: boolean;
  readonly tabIndex: 0 | -1;
  /** Pointer selection, drag and blade; clicks without a pointer (keyboard) use `onSelect`. */
  readonly onPointerDown?: ((event: PointerEvent<HTMLDivElement>, item: TimelineItem) => void) | undefined;
  readonly onSelect: (item: TimelineItem, additive: boolean) => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLDivElement>, item: TimelineItem) => void;
  readonly onFocus: (item: TimelineItem) => void;
}

/**
 * One clip as a listbox option: kind fill, visuals and label chip, outlined when selected. A clip
 * highlighted by Show changes gets an accent outline (inside the selection outline when both apply).
 */
export function TimelineClip({ item, pixelsPerSecond, rowHeight, selected, highlighted = false, placement, dragging = false, dimmed, tabIndex, onPointerDown, onSelect, onKeyDown, onFocus }: TimelineClipProps) {
  const project = useEditorStore((state) => state.project);
  const blade = useEditorStore((state) => state.tool === "blade");
  const failureId = useId();
  const highlightId = useId();
  const reversedId = useId();
  const tone = clipTone(item);
  const ToneIcon = clipToneIcon[tone];
  const startSeconds = placement?.startSeconds ?? item.startSeconds;
  const width = Math.max(minimumClipWidth, (placement?.durationSeconds ?? item.durationSeconds) * pixelsPerSecond);
  const height = Math.max(1, rowHeight - clipInset * 2);
  const audio = timelineItemHasAudio(project, item);
  const generation = generationState(project, item);
  const reversed = isReversedItem(item);
  const describedBy = [generation?.kind === "failed" ? failureId : null, highlighted ? highlightId : null, reversed ? reversedId : null]
    .filter(Boolean)
    .join(" ");
  const media = tone === "video";
  const text = item.source.type === "text" && item.source.text.trim() ? item.source.text : item.label;
  const showLabel = width >= minimumLabelWidth;

  // Pointer clicks were handled on pointer down (select, drag or blade); keyboard and
  // assistive-technology clicks report no click count and select here.
  function onClick(event: MouseEvent<HTMLDivElement>) {
    if (event.detail > 0 && onPointerDown) return;
    onSelect(item, event.shiftKey || event.metaKey || event.ctrlKey);
  }

  // Clicking a partly visible clip must not jump the view: take focus without scrolling.
  // Keyboard focus still scrolls natively so arrowed-to clips come into view.
  function onMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
  }

  return (
    <div
      role="option"
      aria-selected={selected}
      aria-label={timelineClipName(item)}
      aria-describedby={describedBy || undefined}
      data-item-id={item.id}
      tabIndex={tabIndex}
      onClick={onClick}
      onMouseDown={onMouseDown}
      onPointerDown={onPointerDown ? (event) => onPointerDown(event, item) : undefined}
      onKeyDown={(event) => onKeyDown(event, item)}
      onFocus={() => onFocus(item)}
      data-dragging={dragging || undefined}
      style={{ left: startSeconds * pixelsPerSecond, width, top: clipInset, height }}
      className={cn(
        "absolute select-none rounded-clip text-[11px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-panel",
        // Focus lifts the ring over neighbours; selection opens no stacking context, so the label keeps
        // painting over a transition window (see laneLayerClass).
        "focus-visible:z-10",
        blade && "cursor-crosshair",
        dragging && "opacity-40",
      )}
    >
      <div
        data-testid="clip-body"
        className={cn(
          "absolute inset-0 overflow-hidden rounded-clip",
          clipFillClass[tone],
          dimmed && "opacity-45 grayscale",
          selected && "outline outline-2 -outline-offset-1 outline-foreground",
        )}
      >
        {(item.kind === "video_clip" || item.kind === "image_clip") && <ClipFilmstrip item={item} width={width} height={height} />}
        {media && audio && (
          <ClipWaveform item={item} width={width} className="absolute inset-x-0 bottom-0 h-3.5 w-full text-foreground/35" />
        )}
        {tone === "audio" && <ClipWaveform item={item} width={width} className="absolute inset-x-0 bottom-0 top-3.5 h-auto w-full text-foreground/45" />}
        <ClipOverlays item={item} project={project} audio={audio} />
        {generation?.kind === "running" && (
          <span data-testid="clip-generation-progress" aria-hidden className="pointer-events-none absolute inset-0 bg-foreground/10 motion-safe:animate-pulse" />
        )}
        {showLabel &&
          (media ? (
            <span className={cn("absolute left-1 top-1 max-w-[calc(100%-8px)] truncate rounded-[3px] bg-background/85 px-1.5 leading-4 text-foreground", laneLayerClass.clipLabel)}>
              {item.label}
            </span>
          ) : (
            <span
              className={cn(
                "absolute inset-x-0 flex min-w-0 items-center gap-1 px-1.5",
                laneLayerClass.clipLabel,
                tone === "audio" ? "top-0 h-3.5 text-[10.5px]" : "inset-y-0",
                tone === "caption" ? "text-background" : "text-foreground",
              )}
            >
              <ToneIcon className="h-3 w-3 shrink-0" aria-hidden />
              <span className="truncate">{tone === "audio" || tone === "graphics" ? item.label : text}</span>
            </span>
          ))}
      </div>
      {reversed && (
        <>
          {showLabel && (
            <span
              data-testid="clip-reversed-mark"
              aria-hidden
              className={cn("pointer-events-none absolute right-1 top-0.5 grid h-3.5 w-3.5 place-items-center rounded-[3px] bg-background/85 text-foreground", laneLayerClass.clipLabel)}
            >
              <Rewind className="h-2.5 w-2.5" />
            </span>
          )}
          <span id={reversedId} className="sr-only">
            Plays in reverse
          </span>
        </>
      )}
      {highlighted && (
        <>
          <span
            data-testid="clip-highlight"
            aria-hidden
            className={cn(
              "pointer-events-none absolute shadow-[inset_0_0_0_2px_hsl(var(--accent))]",
              selected ? "inset-[2px] rounded-[4px]" : "inset-0 rounded-clip",
            )}
          />
          <span id={highlightId} className="sr-only">
            Changed by the AI edit
          </span>
        </>
      )}
      {generation?.kind === "failed" && (
        <>
          <Tooltip content={generation.message}>
            <span
              data-testid="clip-failure-mark"
              className="absolute right-0 top-0 z-20 h-2.5 w-2.5 rounded-tr-clip bg-destructive [clip-path:polygon(0_0,100%_0,100%_100%)]"
            />
          </Tooltip>
          <span id={failureId} className="sr-only">
            {generation.message}
          </span>
        </>
      )}
    </div>
  );
}
