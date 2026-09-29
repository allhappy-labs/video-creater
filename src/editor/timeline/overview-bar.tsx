import { useRef, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "@/lib/utils";
import {
  basePixelsPerSecond,
  interactionWindow,
  type TimelineOverviewInteraction,
  type TimelineOverviewInteractionMode,
  type TimelineOverviewWindow,
} from "@/lib/timeline-ops/navigation";
import { trackBand, type TrackBand } from "@/lib/timeline-ops/track-bands";
import { useEditorStore } from "../store/editor-store-context";
import { clamp, zoomBounds } from "../store/persisted-layout";
import { clipFillClass, clipTone } from "./clip-kind";
import type { TimelineGeometry } from "./use-timeline-geometry";

const overviewHeight = 18;
const keyboardPanFraction = 0.1;

/** Vertical placement of density marks per band: thin above and below, thick for the main band. */
const bandMarkClass: Readonly<Record<TrackBand, string>> = {
  above: "top-0.5 h-[3px]",
  main: "top-[5px] h-2",
  below: "bottom-0.5 h-[3px]",
};

interface OverviewDrag {
  readonly interaction: TimelineOverviewInteraction;
  readonly durationSeconds: number;
  readonly viewportWidth: number;
}

interface OverviewBarProps {
  readonly geometry: TimelineGeometry;
  /** Id of the scroll container this bar scrolls. */
  readonly controlsId?: string;
}

/** Clip density of the whole timeline with a draggable viewport window. */
export function OverviewBar({ geometry, controlsId }: OverviewBarProps) {
  const setScrollLeft = useEditorStore((state) => state.setScrollLeft);
  const setZoomPercent = useEditorStore((state) => state.setZoomPercent);
  const barRef = useRef<HTMLDivElement>(null);
  const drag = useRef<OverviewDrag | null>(null);
  const { contentWidth, viewportWidth, pixelsPerSecond, renderedDurationSeconds: duration, scrollLeft } = geometry;
  const visibleSeconds = viewportWidth / pixelsPerSecond;
  const currentWindow: TimelineOverviewWindow = {
    startSeconds: scrollLeft / pixelsPerSecond,
    endSeconds: (scrollLeft + viewportWidth) / pixelsPerSecond,
  };

  function barWidth(): number {
    return barRef.current?.getBoundingClientRect().width || viewportWidth;
  }

  function begin(event: PointerEvent<HTMLElement>, mode: TimelineOverviewInteractionMode, window = currentWindow) {
    drag.current = {
      interaction: { mode, pointerId: event.pointerId, pointerStartX: event.clientX, overviewWidth: barWidth(), window, moved: false },
      durationSeconds: duration,
      viewportWidth,
    };
    barRef.current?.setPointerCapture?.(event.pointerId);
  }

  function onBarPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const seconds = ((event.clientX - rect.left) / barWidth()) * duration;
    const startSeconds = clamp(seconds - visibleSeconds / 2, 0, Math.max(0, duration - visibleSeconds));
    setScrollLeft(startSeconds * pixelsPerSecond);
    begin(event, "pan", { startSeconds, endSeconds: startSeconds + visibleSeconds });
  }

  function onHandlePointerDown(event: PointerEvent<HTMLElement>, mode: TimelineOverviewInteractionMode) {
    if (event.button !== 0) return;
    event.stopPropagation();
    begin(event, mode);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    const active = drag.current;
    if (!active) return;
    const next = interactionWindow(active.interaction, event.clientX, active.durationSeconds);
    if (!next) return;
    if (active.interaction.mode === "pan") {
      setScrollLeft(next.startSeconds * pixelsPerSecond);
      return;
    }
    const requestedPixelsPerSecond = active.viewportWidth / Math.max(0.001, next.endSeconds - next.startSeconds);
    const zoomPercent = clamp((requestedPixelsPerSecond / basePixelsPerSecond) * 100, zoomBounds.min, zoomBounds.max);
    setZoomPercent(zoomPercent);
    setScrollLeft(next.startSeconds * ((basePixelsPerSecond * zoomPercent) / 100));
  }

  function onPointerEnd(event: PointerEvent<HTMLDivElement>) {
    drag.current = null;
    barRef.current?.releasePointerCapture?.(event.pointerId);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const step = viewportWidth * keyboardPanFraction;
    if (event.key === "ArrowLeft") setScrollLeft(Math.max(0, scrollLeft - step));
    else if (event.key === "ArrowRight") setScrollLeft(Math.min(contentWidth - viewportWidth, scrollLeft + step));
    else return;
    event.preventDefault();
  }

  const maxScroll = Math.max(0, contentWidth - viewportWidth);
  return (
    <div
      ref={barRef}
      role="scrollbar"
      aria-label="Timeline overview"
      aria-controls={controlsId}
      aria-orientation="horizontal"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={maxScroll > 0 ? Math.round((scrollLeft / maxScroll) * 100) : 0}
      tabIndex={0}
      onPointerDown={onBarPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
      onKeyDown={onKeyDown}
      style={{ height: overviewHeight }}
      className="relative shrink-0 cursor-pointer touch-none select-none overflow-hidden rounded bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      {geometry.rows.flatMap((row) =>
        row.track.items.map((item) => (
          <span
            key={item.id}
            data-overview-mark={item.id}
            aria-hidden
            className={cn("absolute rounded-sm opacity-80", bandMarkClass[trackBand(row.track.kind)], clipFillClass[clipTone(item)])}
            style={{
              left: `${(item.startSeconds / duration) * 100}%`,
              width: `max(1px, ${(item.durationSeconds / duration) * 100}%)`,
            }}
          />
        )),
      )}
      <div
        data-testid="overview-window"
        aria-hidden
        onPointerDown={(event) => onHandlePointerDown(event, "pan")}
        className="absolute inset-y-0 cursor-grab rounded border-[1.5px] border-foreground/60 bg-foreground/5 active:cursor-grabbing"
        style={{ left: `${(scrollLeft / contentWidth) * 100}%`, width: `${(viewportWidth / contentWidth) * 100}%` }}
      >
        <span
          data-testid="overview-window-start"
          onPointerDown={(event) => onHandlePointerDown(event, "resizeStart")}
          className="absolute inset-y-0 left-0 w-1.5 cursor-ew-resize"
        />
        <span
          data-testid="overview-window-end"
          onPointerDown={(event) => onHandlePointerDown(event, "resizeEnd")}
          className="absolute inset-y-0 right-0 w-1.5 cursor-ew-resize"
        />
      </div>
    </div>
  );
}
