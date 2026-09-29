import { useEffect, useRef, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { isMarqueeClick, itemsInMarquee, type MarqueeRect } from "@/lib/timeline-ops/marquee";
import { timelineGapAtSeconds, type TimelineGapSelection } from "@/lib/timeline-ops/navigation";
import { useEditorStoreApi } from "../store/editor-store-context";
import { followPointer } from "./pointer-session";
import { marqueeGeometry, type TimelineGeometry } from "./use-timeline-geometry";

interface UseMarqueeOptions {
  readonly geometry: TimelineGeometry;
  /** The element wrapping the track rows (header column included); its top is the first row top. */
  readonly rowsRef: RefObject<HTMLElement | null>;
}

/** Empty lane space inside a track listbox, not a clip or a control. */
function isEmptyLaneTarget(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[role="listbox"]') !== null && target.closest('[role="option"]') === null;
}

/** The empty gap between clips under a tracks-area point, if any. */
function gapAtPoint(geometry: TimelineGeometry, x: number, y: number): TimelineGapSelection | null {
  const row = geometry.rows.find((candidate) => y >= candidate.top && y < candidate.top + candidate.height);
  return row ? timelineGapAtSeconds(row.track, x / geometry.pixelsPerSecond) : null;
}

/**
 * Rubber-band selection from empty lane space with the select tool. The marquee rect lives
 * in the store (tracks-area pixels); release selects the clips it touches, adding to the
 * selection with Shift, Ctrl or Cmd. A press without a 3 px drag clears the selection and
 * selects the empty gap between clips under the pointer, which Delete closes.
 */
export function useMarquee({ geometry, rowsRef }: UseMarqueeOptions) {
  const store = useEditorStoreApi();
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;
  const stopRef = useRef<(() => void) | null>(null);

  useEffect(() => () => stopRef.current?.(), []);

  function onRowsPointerDown(event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0 || stopRef.current || !isEmptyLaneTarget(event.target)) return;
    const state = store.getState();
    if (state.tool !== "select") return;
    const rows = rowsRef.current;
    if (!rows) return;
    const additive = event.shiftKey || event.metaKey || event.ctrlKey;
    const toContent = (clientX: number, clientY: number) => {
      const rect = rows.getBoundingClientRect();
      return { x: clientX - rect.left - geometryRef.current.headerWidth, y: clientY - rect.top };
    };
    const origin = toContent(event.clientX, event.clientY);
    let rect: MarqueeRect = { startX: origin.x, startY: origin.y, endX: origin.x, endY: origin.y };

    const end = () => {
      stopRef.current = null;
      store.getState().setMarquee(null);
    };
    stopRef.current = followPointer(event.pointerId, {
      move(point) {
        const current = toContent(point.clientX, point.clientY);
        rect = { ...rect, endX: current.x, endY: current.y };
        if (!isMarqueeClick(rect)) store.getState().setMarquee(rect);
      },
      release(point) {
        const current = toContent(point.clientX, point.clientY);
        rect = { ...rect, endX: current.x, endY: current.y };
        end();
        const latest = store.getState();
        if (isMarqueeClick(rect)) {
          if (additive) return;
          latest.clearSelection();
          const gap = gapAtPoint(geometryRef.current, rect.endX, rect.endY);
          if (gap) latest.selectGap(gap);
          return;
        }
        const itemIds = itemsInMarquee(marqueeGeometry(geometryRef.current), rect);
        latest.selectItems(additive ? [...latest.selectedItemIds, ...itemIds] : itemIds);
      },
      cancel: end,
    });
  }

  return { onRowsPointerDown };
}
