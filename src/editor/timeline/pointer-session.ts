import { createTimelineFrameCoalescer } from "@/lib/timeline-interaction-frame";

export interface PointerPoint {
  readonly clientX: number;
  readonly clientY: number;
}

interface PointerSessionHandlers {
  move(point: PointerPoint): void;
  release(point: PointerPoint): void;
  cancel(): void;
}

function pointOf(event: PointerEvent): PointerPoint {
  return { clientX: event.clientX, clientY: event.clientY };
}

/**
 * Follows one pointer on the window until it is released, cancelled, or Escape is pressed.
 * Window listeners (rather than pointer capture) keep the session alive when the pressed
 * element re-renders elsewhere. Returns a function that stops following without callbacks.
 */
export function followPointer(pointerId: number, handlers: PointerSessionHandlers): () => void {
  const onMove = (event: PointerEvent) => {
    if (event.pointerId === pointerId) handlers.move(pointOf(event));
  };
  const onUp = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    stop();
    handlers.release(pointOf(event));
  };
  const onCancel = (event: PointerEvent) => {
    if (event.pointerId !== pointerId) return;
    stop();
    handlers.cancel();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    stop();
    handlers.cancel();
  };
  function stop() {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onCancel);
    window.removeEventListener("keydown", onKeyDown, true);
  }
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onCancel);
  window.addEventListener("keydown", onKeyDown, true);
  return stop;
}

/** At most one evaluation per animation frame; `flush` evaluates the latest point immediately. */
export function animationFrameCoalescer<TInput, TResult>(evaluate: (value: TInput) => TResult, deliver: (result: TResult) => void) {
  return createTimelineFrameCoalescer({
    evaluate,
    deliver,
    requestFrame: (callback) => window.requestAnimationFrame(callback),
    cancelFrame: (handle) => window.cancelAnimationFrame(handle),
  });
}
