import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import type { TimelineItem } from "@/lib/timeline";
import {
  bladeSplitSeconds,
  clipMoveGroup,
  evaluateClipMove,
  moveThresholdPixels,
  planClipMoveCommit,
  type ClipMovePreview,
  type ClipSnapOptions,
} from "@/lib/timeline-ops/clip-drag";
import type { StickyTimelineSnap } from "@/lib/timeline-snap";
import { useEditorStoreApi } from "../store/editor-store-context";
import { animationFrameCoalescer, followPointer, type PointerPoint } from "./pointer-session";
import { useTimelineCommands } from "./timeline-commands";
import { trackTargetAtY, type TimelineGeometry } from "./use-timeline-geometry";

/** A clip drag past the move threshold, rendered as ghosts and target feedback. */
export interface ClipDragState {
  readonly leadItemId: string;
  readonly itemIds: ReadonlySet<string>;
  readonly duplicate: boolean;
  readonly preview: ClipMovePreview;
}

interface DragSession {
  readonly pointerId: number;
  readonly start: PointerPoint;
  readonly leadItemId: string;
  readonly leadTrackId: string;
  readonly itemIds: readonly string[];
  readonly duplicate: boolean;
  /** A plain press on a clip of a multi-selection selects only that clip if it never moves. */
  readonly collapseTo: string | null;
  /** Timeline time under the press, where a click (never a drag) moves the playhead. */
  readonly pressSeconds: number;
  passed: boolean;
  sticky: StickyTimelineSnap | null;
}

interface UseClipDragOptions {
  readonly geometry: TimelineGeometry;
}

/**
 * Clip pointer handling for the select and blade tools. A press selects (Shift, Ctrl or Cmd
 * toggle); moving 3 px starts a drag evaluated once per frame with snapping; release commits
 * one batch. A click that never drags also seeks to the pressed time while playback is stopped,
 * so the playhead lands over the clip for split and trim-to-playhead. Alt at press duplicates. The blade splits the pressed clip at the pointer.
 */
export function useClipDrag({ geometry }: UseClipDragOptions) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const [drag, setDrag] = useState<ClipDragState | null>(null);
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;
  const stopRef = useRef<(() => void) | null>(null);

  useEffect(() => () => stopRef.current?.(), []);

  function snapOptions(sticky: StickyTimelineSnap | null): ClipSnapOptions | null {
    const state = store.getState();
    return state.snapEnabled
      ? { playheadSeconds: state.playheadSeconds, pixelsPerSecond: geometryRef.current.pixelsPerSecond, sticky }
      : null;
  }

  function evaluate(session: DragSession, point: PointerPoint): ClipMovePreview {
    const currentGeometry = geometryRef.current;
    // Follow the pointer's vertical travel from the lead row's current centre, so a layout
    // shift after the press (the keyframe lane moving with the selection) does not jump rows.
    const row = currentGeometry.rows.find((candidate) => candidate.track.id === session.leadTrackId);
    const stay = { trackId: session.leadTrackId, insertIndex: null };
    const target = row ? (trackTargetAtY(currentGeometry.rows, row.top + row.height / 2 + point.clientY - session.start.clientY) ?? stay) : stay;
    return evaluateClipMove({
      timeline: store.getState().project.timeline,
      leadItemId: session.leadItemId,
      itemIds: session.itemIds,
      deltaSeconds: (point.clientX - session.start.clientX) / currentGeometry.pixelsPerSecond,
      target,
      duplicate: session.duplicate,
      snap: snapOptions(session.sticky),
    });
  }

  /** Timeline seconds under the pointer, measured from the pressed clip's box. */
  function pointerSecondsOnClip(event: ReactPointerEvent<HTMLElement>, item: TimelineItem): number {
    const left = event.currentTarget.getBoundingClientRect().left;
    return item.startSeconds + (event.clientX - left) / geometryRef.current.pixelsPerSecond;
  }

  function splitWithBlade(event: ReactPointerEvent<HTMLElement>, item: TimelineItem, locked: boolean) {
    const state = store.getState();
    state.selectItems([item.id]);
    if (locked) return;
    const pointerSeconds = pointerSecondsOnClip(event, item);
    const seconds = bladeSplitSeconds(state.project.timeline, item.id, pointerSeconds, snapOptions(null));
    if (seconds !== null) void commands.splitItemAt(item.id, seconds);
  }

  function onClipPointerDown(event: ReactPointerEvent<HTMLElement>, item: TimelineItem) {
    if (event.button !== 0 || stopRef.current) return;
    const state = store.getState();
    const track = state.project.timeline.tracks.find((candidate) => candidate.items.some((entry) => entry.id === item.id));
    if (!track) return;
    if (state.tool === "blade") {
      splitWithBlade(event, item, track.locked);
      return;
    }

    let collapseTo: string | null = null;
    if (event.shiftKey || event.metaKey || event.ctrlKey) state.toggleItemSelection(item.id);
    else if (!state.selectedItemIds.includes(item.id)) state.selectItems([item.id]);
    else if (state.selectedItemIds.length > 1) collapseTo = item.id;
    const selection = store.getState().selectedItemIds;
    if (!selection.includes(item.id)) return;
    const itemIds = clipMoveGroup(state.project.timeline, selection, item.id);
    if (!itemIds) return;

    const session: DragSession = {
      pointerId: event.pointerId,
      start: { clientX: event.clientX, clientY: event.clientY },
      leadItemId: item.id,
      leadTrackId: track.id,
      itemIds,
      duplicate: event.altKey,
      collapseTo,
      pressSeconds: pointerSecondsOnClip(event, item),
      passed: false,
      sticky: null,
    };
    const dragIds = new Set(itemIds);
    const coalescer = animationFrameCoalescer(
      (point: PointerPoint) => evaluate(session, point),
      (preview: ClipMovePreview) => {
        session.sticky = preview.sticky;
        setDrag({ leadItemId: item.id, itemIds: dragIds, duplicate: session.duplicate, preview });
      },
    );
    const end = () => {
      stopRef.current = null;
      coalescer.cancel();
      setDrag(null);
    };

    stopRef.current = followPointer(event.pointerId, {
      move(point) {
        if (!session.passed) {
          const travel = Math.max(Math.abs(point.clientX - session.start.clientX), Math.abs(point.clientY - session.start.clientY));
          if (travel < moveThresholdPixels) return;
          session.passed = true;
        }
        coalescer.schedule(point);
      },
      release(point) {
        const preview = session.passed ? coalescer.flush(point) : null;
        end();
        if (!preview) {
          const current = store.getState();
          if (session.collapseTo) current.selectItems([session.collapseTo]);
          // `seek` clamps to the timeline duration.
          if (!current.playing) current.seek(roundTimelineSeconds(session.pressSeconds));
          return;
        }
        void commands.applyPlan(
          planClipMoveCommit(store.getState().project, preview, session.duplicate),
          session.duplicate ? "added" : "keep",
        );
      },
      cancel: end,
    });
  }

  return { drag, onClipPointerDown };
}
