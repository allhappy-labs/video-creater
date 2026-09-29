import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { openContextMenuAt, useLongPress } from "../shell/use-long-press";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import type { PointerPoint } from "./pointer-session";
import type { TimelineGeometry } from "./use-timeline-geometry";

/** Scroll and playhead differences within this many pixels are echoes, not changes. */
const echoPixels = 0.5;

type CenterGeometry = Pick<TimelineGeometry, "pixelsPerSecond" | "viewportWidth" | "centerPadding">;

/** Timeline seconds under the fixed centre playhead at `scrollLeft`. */
export function centeredPlayheadSeconds(geometry: CenterGeometry, scrollLeft: number): number {
  return Math.max(0, (scrollLeft + geometry.viewportWidth / 2 - geometry.centerPadding) / geometry.pixelsPerSecond);
}

/** The scroll offset that puts `seconds` under the fixed centre playhead. */
export function scrollLeftForCenteredPlayhead(geometry: CenterGeometry, seconds: number): number {
  return Math.max(0, seconds * geometry.pixelsPerSecond + geometry.centerPadding - geometry.viewportWidth / 2);
}

function pointOf(event: ReactPointerEvent<HTMLElement>): PointerPoint {
  return { clientX: event.clientX, clientY: event.clientY };
}

function distance(points: readonly PointerPoint[]): number {
  const [first, second] = points;
  if (!first || !second) return 0;
  return Math.hypot(second.clientX - first.clientX, second.clientY - first.clientY);
}

/** Ends any clip drag, trim or marquee session following `pointerId` (they listen on window). */
function cancelPointerSessions(pointerId: number) {
  window.dispatchEvent(new PointerEvent("pointercancel", { pointerId }));
}

interface UseTouchTimelineOptions {
  /** Touch behaviour is on in the mobile layout only. */
  readonly enabled: boolean;
  readonly geometry: TimelineGeometry;
  readonly scrollerRef: RefObject<HTMLElement | null>;
}

/**
 * Mobile timeline touch handling:
 * - the playhead is fixed at the viewport centre: scrolling seeks, and seeks (playback,
 *   zoom, commands) scroll the content so the playhead time stays under the centre;
 * - during playback the content follows the playhead; a manual scroll pauses that follow (the
 *   playhead then moves across the view) until playback stops or restarts;
 * - two touch pointers pinch the zoom around the centre;
 * - a long press (`useLongPress`) opens the context menu of the clip, cut or lane under the finger.
 */
export function useTouchTimeline({ enabled, geometry, scrollerRef }: UseTouchTimelineOptions) {
  const store = useEditorStoreApi();
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const playing = useEditorStore((state) => state.playing && state.previewSource.kind === "timeline");
  const [followPaused, setFollowPaused] = useState(false);
  const [followPlaying, setFollowPlaying] = useState(playing);
  // Playback starting or stopping resumes the follow, adjusting state during render.
  if (followPlaying !== playing) {
    setFollowPlaying(playing);
    setFollowPaused(false);
  }
  const following = enabled && !(followPaused && playing);
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;
  const touches = useRef(new Map<number, PointerPoint>());
  const pinch = useRef<{ readonly distance: number; readonly zoomPercent: number } | null>(null);
  const longPress = useLongPress({
    enabled,
    onLongPress: (press) => {
      cancelPointerSessions(press.pointerId);
      // The context menu trigger resolves the clip, cut or lane under the event target.
      openContextMenuAt(press);
    },
  });
  const { pixelsPerSecond, viewportWidth, centerPadding } = geometry;

  useLayoutEffect(() => {
    if (!following) return;
    const target = scrollLeftForCenteredPlayhead({ pixelsPerSecond, viewportWidth, centerPadding }, playheadSeconds);
    const state = store.getState();
    if (Math.abs(state.scrollLeft - target) > echoPixels) state.setScrollLeft(target);
  }, [following, store, playheadSeconds, pixelsPerSecond, viewportWidth, centerPadding]);

  // Two-finger moves must not pan or zoom the page, or the browser cancels the pointers.
  useEffect(() => {
    const element = scrollerRef.current;
    if (!enabled || !element) return undefined;
    const onTouchMove = (event: TouchEvent) => {
      if (event.touches.length > 1 && event.cancelable) event.preventDefault();
    };
    element.addEventListener("touchmove", onTouchMove, { passive: false });
    return () => element.removeEventListener("touchmove", onTouchMove);
  }, [enabled, scrollerRef]);

  /**
   * Keeps the playhead under the centre while the user scrolls. `manual` scrolls moved the content
   * away from the stored scroll offset (a finger, wheel or scrollbar, not a follow echo): during
   * playback they pause the follow instead of seeking.
   */
  function onScroll(scrollLeft: number, manual: boolean) {
    if (!enabled) return;
    if (playing) {
      if (manual && !followPaused) setFollowPaused(true);
      return;
    }
    const current = geometryRef.current;
    const seconds = centeredPlayheadSeconds(current, scrollLeft);
    const state = store.getState();
    if (Math.abs(seconds - state.playheadSeconds) * current.pixelsPerSecond > echoPixels) state.seek(seconds);
  }

  function onPointerDownCapture(event: ReactPointerEvent<HTMLElement>) {
    if (event.pointerType !== "touch") return;
    touches.current.set(event.pointerId, pointOf(event));
    if (touches.current.size === 1) {
      longPress.start(event);
      return;
    }
    // A second finger starts a pinch: it replaces the first finger's press, drag or marquee.
    longPress.cancel();
    for (const pointerId of touches.current.keys()) if (pointerId !== event.pointerId) cancelPointerSessions(pointerId);
    pinch.current = { distance: Math.max(1, distance([...touches.current.values()])), zoomPercent: store.getState().zoomPercent };
    event.stopPropagation();
  }

  function onPointerMoveCapture(event: ReactPointerEvent<HTMLElement>) {
    if (!touches.current.has(event.pointerId)) return;
    const point = pointOf(event);
    touches.current.set(event.pointerId, point);
    longPress.move(event);
    const active = pinch.current;
    if (!active || touches.current.size < 2) return;
    store.getState().setZoomPercent((active.zoomPercent * distance([...touches.current.values()])) / active.distance);
    event.stopPropagation();
  }

  function onPointerEndCapture(event: ReactPointerEvent<HTMLElement>) {
    if (!touches.current.delete(event.pointerId)) return;
    longPress.end(event);
    if (touches.current.size < 2) pinch.current = null;
  }

  return {
    onScroll,
    /** The playhead is off the centre: playback runs while the user looks elsewhere. */
    followPaused: enabled && followPaused && playing,
    touchHandlers: enabled
      ? {
          onPointerDownCapture,
          onPointerMoveCapture,
          onPointerUpCapture: onPointerEndCapture,
          onPointerCancelCapture: onPointerEndCapture,
          onClickCapture: longPress.handlers.onClickCapture,
        }
      : {},
  };
}
