import type { KeyboardEvent, PointerEvent } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import type { TimelineItem } from "@/lib/timeline";
import type { ClipTrimEdge } from "@/lib/timeline-ops/clip-trim";
import { cn } from "@/lib/utils";
import type { LivePlacement } from "./use-clip-trim";
import { timelineClipInset } from "./use-timeline-geometry";

/** Trim handle hit widths: the 7 px bar itself with a mouse, a 24 px touch target on touch. */
export const trimHandleHitWidth = { pointer: 7, touch: 24 } as const;
/** On touch the hit area reaches this far past the clip edge; the bar stays inside the clip. */
const touchOutsetPixels = 8;

/** The horizontal hit box of a trim handle whose clip edge sits at `edgeX`. */
export function trimHandleHitBox(edge: ClipTrimEdge, edgeX: number, touch: boolean): { readonly left: number; readonly width: number } {
  const width = touch ? trimHandleHitWidth.touch : trimHandleHitWidth.pointer;
  const outside = touch ? touchOutsetPixels : 0;
  return { left: edge === "left" ? edgeX - outside : edgeX + outside - width, width };
}

interface ClipTrimHandleProps {
  readonly item: TimelineItem;
  readonly edge: ClipTrimEdge;
  /** The clip box, live while trimming. */
  readonly placement: LivePlacement;
  readonly pixelsPerSecond: number;
  readonly rowHeight: number;
  /** Latest time the edge can reach, for `aria-valuemax`. */
  readonly maxSeconds: number;
  readonly touch: boolean;
  readonly onPointerDown: (event: PointerEvent<HTMLDivElement>, item: TimelineItem, edge: ClipTrimEdge) => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLDivElement>, item: TimelineItem, edge: ClipTrimEdge) => void;
}

/**
 * A trim handle for a selected clip: a slider over the clip edge (7 px bar, 24 px hit area on
 * touch). Drag to trim; Arrow Left/Right trim by 0.25 s; Shift ripple trims.
 */
export function ClipTrimHandle({ item, edge, placement, pixelsPerSecond, rowHeight, maxSeconds, touch, onPointerDown, onKeyDown }: ClipTrimHandleProps) {
  const edgeSeconds = edge === "left" ? placement.startSeconds : placement.startSeconds + placement.durationSeconds;
  const { left, width } = trimHandleHitBox(edge, edgeSeconds * pixelsPerSecond, touch);
  const side = edge === "left" ? "Left" : "Right";

  return (
    <div
      role="slider"
      tabIndex={0}
      data-trim-handle={edge}
      aria-label={`Resize ${item.label} ${edge} edge`}
      aria-orientation="horizontal"
      aria-valuemin={0}
      aria-valuemax={roundTimelineSeconds(Math.max(maxSeconds, edgeSeconds))}
      aria-valuenow={roundTimelineSeconds(edgeSeconds)}
      aria-valuetext={`${side} edge at ${edgeSeconds.toFixed(2)} seconds; duration ${placement.durationSeconds.toFixed(2)} seconds`}
      onPointerDown={(event) => onPointerDown(event, item, edge)}
      onKeyDown={(event) => onKeyDown(event, item, edge)}
      style={{ left, width, top: timelineClipInset, height: Math.max(1, rowHeight - timelineClipInset * 2) }}
      className="group absolute z-20 cursor-ew-resize touch-none rounded-clip focus-visible:outline-none"
    >
      <span
        aria-hidden
        className={cn(
          "absolute inset-y-0 flex w-[7px] items-center justify-center bg-foreground group-focus-visible:ring-2 group-focus-visible:ring-ring",
          edge === "left" ? "rounded-l-clip" : "rounded-r-clip",
          touch ? (edge === "left" ? "left-2" : "right-2") : edge === "left" ? "left-0" : "right-0",
        )}
      >
        <span className="h-3 w-px rounded-full bg-background/60" />
      </span>
    </div>
  );
}
