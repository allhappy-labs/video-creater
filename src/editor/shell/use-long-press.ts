import { useEffect, useRef, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from "react";

/** How long a finger must rest before the press counts as a long press. */
export const longPressMilliseconds = 500;
/** Finger travel that turns a long press into a scroll or drag. */
export const longPressTolerancePixels = 8;
/** A click this soon after a long press fired belongs to the same gesture and is swallowed. */
const clickSuppressMilliseconds = 800;

export interface LongPressPoint {
  readonly target: Element;
  readonly pointerId: number;
  readonly clientX: number;
  readonly clientY: number;
}

interface UseLongPressOptions {
  /** Called once the press has rested for `longPressMilliseconds` without moving more than `longPressTolerancePixels`. */
  readonly onLongPress: (press: LongPressPoint) => void;
  /** Off turns every handler into a no-op. Defaults to on. */
  readonly enabled?: boolean;
}

interface PendingPress {
  readonly pointerId: number;
  readonly clientX: number;
  readonly clientY: number;
  readonly timer: number;
}

type PressEvent = Pick<ReactPointerEvent<Element>, "pointerId" | "pointerType" | "clientX" | "clientY" | "target">;

interface LongPressHandlers {
  onPointerDown(event: ReactPointerEvent<HTMLElement>): void;
  onPointerMove(event: ReactPointerEvent<HTMLElement>): void;
  onPointerUp(event: ReactPointerEvent<HTMLElement>): void;
  onPointerCancel(event: ReactPointerEvent<HTMLElement>): void;
  /** Swallows the click that ends a long press, so the pressed control does not also activate. */
  onClickCapture(event: ReactMouseEvent<HTMLElement>): void;
}

export interface LongPress {
  /** Spread onto the pressed element. */
  readonly handlers: LongPressHandlers;
  /** Starts timing a touch or pen press (mouse presses are ignored). */
  start(event: PressEvent): void;
  /** Cancels the press when its pointer travels past the tolerance. */
  move(event: Pick<PressEvent, "pointerId" | "clientX" | "clientY">): void;
  /** Ends the press when its pointer lifts or is cancelled. */
  end(event: Pick<PressEvent, "pointerId">): void;
  cancel(): void;
}

/** True when a pointer has travelled past the long-press tolerance from where it started. */
export function exceedsLongPressTolerance(start: { readonly clientX: number; readonly clientY: number }, point: { readonly clientX: number; readonly clientY: number }): boolean {
  return Math.hypot(point.clientX - start.clientX, point.clientY - start.clientY) > longPressTolerancePixels;
}

/**
 * Opens the context menu under `press.target` as a right-click would. Radix context menu triggers
 * (and the timeline's menu, which resolves the clip, cut or lane under the target) listen for it.
 */
export function openContextMenuAt(press: LongPressPoint): void {
  if (!press.target.isConnected) return;
  press.target.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: press.clientX, clientY: press.clientY }));
}

/**
 * Touch long press: a touch or pen press that rests for 500 ms without moving more than 8 px calls
 * `onLongPress`. Lifting, cancelling or moving further cancels it. Mouse input is ignored, since a
 * mouse has a right button.
 */
export function useLongPress({ onLongPress, enabled = true }: UseLongPressOptions): LongPress {
  const pending = useRef<PendingPress | null>(null);
  const firedAt = useRef(Number.NEGATIVE_INFINITY);
  const callback = useRef(onLongPress);
  callback.current = onLongPress;

  useEffect(() => () => window.clearTimeout(pending.current?.timer), []);

  function cancel() {
    window.clearTimeout(pending.current?.timer);
    pending.current = null;
  }

  function start(event: PressEvent) {
    cancel();
    if (!enabled || event.pointerType === "mouse") return;
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    const { pointerId, clientX, clientY } = event;
    const timer = window.setTimeout(() => {
      pending.current = null;
      if (!target.isConnected) return;
      firedAt.current = Date.now();
      callback.current({ target, pointerId, clientX, clientY });
    }, longPressMilliseconds);
    pending.current = { pointerId, clientX, clientY, timer };
  }

  function move(event: Pick<PressEvent, "pointerId" | "clientX" | "clientY">) {
    const press = pending.current;
    if (press?.pointerId === event.pointerId && exceedsLongPressTolerance(press, event)) cancel();
  }

  function end(event: Pick<PressEvent, "pointerId">) {
    if (pending.current?.pointerId === event.pointerId) cancel();
  }

  return {
    start,
    move,
    end,
    cancel,
    handlers: {
      onPointerDown: start,
      onPointerMove: move,
      onPointerUp: end,
      onPointerCancel: end,
      onClickCapture: (event) => {
        if (Date.now() - firedAt.current > clickSuppressMilliseconds) return;
        firedAt.current = Number.NEGATIVE_INFINITY;
        event.preventDefault();
        event.stopPropagation();
      },
    },
  };
}
