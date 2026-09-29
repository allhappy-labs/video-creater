import { useMemo, useRef, type PointerEvent } from "react";
import { formatTimecode, roundTimelineSeconds } from "@/lib/format";
import { timelineSnapSeconds } from "@/lib/timeline-ops/navigation";
import { adaptiveTimelineTicks } from "@/lib/timeline-viewport";
import { useEditorStore } from "../store/editor-store-context";
import { timelineRulerHeight, type TimelineGeometry } from "./use-timeline-geometry";

/** Time ruler across the lane content; pointer down and drag seek the playhead. */
export function TimelineRuler({ geometry }: { readonly geometry: TimelineGeometry }) {
  const seek = useEditorStore((state) => state.seek);
  const dragging = useRef(false);
  const { pixelsPerSecond, window: viewportWindow, contentWidth } = geometry;
  const ticks = useMemo(
    () => adaptiveTimelineTicks({ window: viewportWindow, pixelsPerSecond, snapSeconds: timelineSnapSeconds }),
    [viewportWindow, pixelsPerSecond],
  );

  function seekTo(event: PointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    seek(roundTimelineSeconds(Math.max(0, event.clientX - rect.left) / pixelsPerSecond));
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    dragging.current = true;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    seekTo(event);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (dragging.current) seekTo(event);
  }

  function onPointerEnd(event: PointerEvent<HTMLDivElement>) {
    dragging.current = false;
    event.currentTarget.releasePointerCapture?.(event.pointerId);
  }

  return (
    <div
      data-testid="timeline-ruler"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
      style={{ width: contentWidth, height: timelineRulerHeight }}
      className="tabular-time relative shrink-0 cursor-pointer touch-none select-none overflow-hidden border-b border-line text-[10px] text-dim"
    >
      {ticks.minorSeconds.map((seconds) => (
        <span
          key={`minor-${seconds}`}
          data-testid="ruler-minor-tick"
          aria-hidden
          className="absolute bottom-0 h-1 w-px bg-line"
          style={{ left: seconds * pixelsPerSecond }}
        />
      ))}
      {ticks.majorSeconds.map((seconds) => (
        <span key={`major-${seconds}`} aria-hidden>
          <span
            data-testid="ruler-major-tick"
            className="absolute bottom-0 h-2 w-px bg-dim"
            style={{ left: seconds * pixelsPerSecond }}
          />
          <span
            data-testid="ruler-label"
            className="absolute top-1 whitespace-nowrap pl-1"
            style={{ left: seconds * pixelsPerSecond }}
          >
            {formatTimecode(seconds)}
          </span>
        </span>
      ))}
    </div>
  );
}
