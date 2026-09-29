import { useRef, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "@/lib/utils";

interface SplitResizerProps {
  readonly orientation: "vertical" | "horizontal";
  readonly label: string;
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly invert?: boolean;
  readonly step?: number;
  readonly onResize: (value: number) => void;
}

export function SplitResizer({ orientation, label, value, min, max, invert = false, step = 10, onResize }: SplitResizerProps) {
  const drag = useRef<{ origin: number; start: number } | null>(null);
  const coordinate = (event: PointerEvent) => (orientation === "vertical" ? event.clientX : event.clientY);
  const clamp = (next: number) => Math.min(max, Math.max(min, next));

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    drag.current = { origin: coordinate(event), start: value };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!drag.current) return;
    const delta = coordinate(event) - drag.current.origin;
    onResize(clamp(drag.current.start + (invert ? -delta : delta)));
  }

  function onPointerUp(event: PointerEvent<HTMLDivElement>) {
    drag.current = null;
    event.currentTarget.releasePointerCapture?.(event.pointerId);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const decrease = orientation === "vertical" ? "ArrowLeft" : invert ? "ArrowDown" : "ArrowUp";
    const increase = orientation === "vertical" ? "ArrowRight" : invert ? "ArrowUp" : "ArrowDown";
    if (event.key === decrease) onResize(clamp(value - step));
    else if (event.key === increase) onResize(clamp(value + step));
    else return;
    event.preventDefault();
  }

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
      className={cn(
        "shrink-0 touch-none rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        orientation === "vertical" ? "w-1.5 cursor-col-resize" : "h-1.5 cursor-row-resize",
      )}
    />
  );
}
