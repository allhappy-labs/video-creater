import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import type { ClipSnapOptions } from "@/lib/timeline-ops/clip-drag";
import { evaluateClipRippleTrim, evaluateClipTrim, type ClipTrimEdge } from "@/lib/timeline-ops/clip-trim";
import type { StickyTimelineSnap } from "@/lib/timeline-snap";
import { useEditorStoreApi } from "../store/editor-store-context";
import { animationFrameCoalescer, followPointer, type PointerPoint } from "./pointer-session";
import { useTimelineCommands } from "./timeline-commands";
import type { TimelineGeometry } from "./use-timeline-geometry";

/** Keyboard trim step, as in the legacy resize handles. */
const keyboardTrimStepSeconds = 0.25;

export interface LivePlacement {
  readonly startSeconds: number;
  readonly durationSeconds: number;
}

/** A trim in progress: live clip boxes plus the outcome for the tooltip and guide. */
export interface ClipTrimState {
  readonly itemId: string;
  readonly edge: ClipTrimEdge;
  readonly ripple: boolean;
  readonly placements: ReadonlyMap<string, LivePlacement>;
  readonly state: "accepted" | "clamped" | "rejected";
  readonly reason: string | null;
  readonly guideSeconds: number | null;
}

interface TrimOutcome {
  readonly view: ClipTrimState;
  readonly result: CommandResult;
  readonly sticky: StickyTimelineSnap | null;
}

/**
 * Trim handle pointer and keyboard handling. Dragging evaluates once per frame through the
 * resize evaluator (with snapping), or through the ripple-trim dry run when Shift is held at
 * the press; release commits one `trimItems`/`resizeItems` or `rippleTrimItem` action.
 */
export function useClipTrim({ geometry }: { readonly geometry: TimelineGeometry }) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const [trim, setTrim] = useState<ClipTrimState | null>(null);
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;
  const stopRef = useRef<(() => void) | null>(null);

  useEffect(() => () => stopRef.current?.(), []);

  function outcome(item: TimelineItem, edge: ClipTrimEdge, ripple: boolean, deltaSeconds: number, sticky: StickyTimelineSnap | null): TrimOutcome {
    const state = store.getState();
    const { project } = state;
    const base = { itemId: item.id, edge, ripple };
    if (ripple) {
      const planned = evaluateClipRippleTrim(project, { itemId: item.id, edge, deltaSeconds });
      return {
        view: { ...base, placements: planned.placements, state: planned.blocked ? "rejected" : "accepted", reason: planned.blocked, guideSeconds: null },
        result: planned.blocked ? { blocked: planned.blocked } : { actions: planned.action ? [planned.action] : [] },
        sticky: null,
      };
    }
    const snap: ClipSnapOptions | null = state.snapEnabled
      ? { playheadSeconds: state.playheadSeconds, pixelsPerSecond: geometryRef.current.pixelsPerSecond, sticky }
      : null;
    const preview = evaluateClipTrim({ project, itemId: item.id, edge, deltaSeconds, snap });
    return {
      view: {
        ...base,
        placements: new Map([[item.id, { startSeconds: preview.placement.startSeconds, durationSeconds: preview.placement.durationSeconds }]]),
        state: preview.state,
        reason: preview.state === "accepted" ? null : preview.reason,
        guideSeconds: preview.guideSeconds,
      },
      result: preview.state === "rejected" ? { blocked: preview.reason ?? "Invalid resize" } : { actions: preview.action ? [preview.action] : [] },
      sticky: preview.sticky,
    };
  }

  function onTrimPointerDown(event: ReactPointerEvent<HTMLElement>, item: TimelineItem, edge: ClipTrimEdge) {
    if (event.button !== 0 || stopRef.current) return;
    event.stopPropagation();
    event.preventDefault();
    const startX = event.clientX;
    const ripple = event.shiftKey;
    let sticky: StickyTimelineSnap | null = null;
    const coalescer = animationFrameCoalescer(
      (point: PointerPoint) => outcome(item, edge, ripple, (point.clientX - startX) / geometryRef.current.pixelsPerSecond, sticky),
      (next: TrimOutcome) => {
        sticky = next.sticky;
        setTrim(next.view);
      },
    );
    const end = () => {
      stopRef.current = null;
      coalescer.cancel();
      setTrim(null);
    };
    stopRef.current = followPointer(event.pointerId, {
      move: (point) => coalescer.schedule(point),
      release(point) {
        const finished = point.clientX === startX ? null : coalescer.flush(point);
        end();
        if (finished) void commands.applyPlan(finished.result);
      },
      cancel: end,
    });
  }

  function onTrimKeyDown(event: KeyboardEvent<HTMLElement>, item: TimelineItem, edge: ClipTrimEdge) {
    if ((event.key !== "ArrowLeft" && event.key !== "ArrowRight") || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    const delta = event.key === "ArrowLeft" ? -keyboardTrimStepSeconds : keyboardTrimStepSeconds;
    void commands.applyPlan(outcome(item, edge, event.shiftKey, delta, null).result);
  }

  return { trim, onTrimPointerDown, onTrimKeyDown };
}
