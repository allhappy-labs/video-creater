import { useRef, useState, type DragEvent, type RefObject } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import type { AssetPlacement } from "@/lib/timeline-ops/asset-insert";
import type { TimelineItem } from "@/lib/timeline";
import { snapClipProbes } from "@/lib/timeline-ops/clip-drag";
import { effectTarget } from "../panels/effects/effect-apply";
import { useApplyEffect } from "../panels/effects/use-apply-effect";
import { useEditorStoreApi } from "../store/editor-store-context";
import { hasAssetDragData, readAssetDragData, readDropStartSeconds } from "./drag-data";
import { hasEffectDragData, readEffectDragData } from "./effect-drag-data";
import { hasTransitionDragData } from "./transition-drag-data";
import { useTimelineCommands } from "./timeline-commands";
import { trackTargetAtY, type TimelineGeometry } from "./use-timeline-geometry";
import { useTransitionDrop } from "./use-transition-drop";

/** Where a dragged asset would land, drawn by the interaction layer and the target lane. */
export interface TimelineDropIndicator {
  readonly placement: AssetPlacement;
  /** Tracks-area y of the new-track line between rows, or null when over a row. */
  readonly insertTop: number | null;
  /** The hovered row is locked: the drop is refused. */
  readonly rejected: boolean;
}

interface UseTimelineDropOptions {
  readonly geometry: TimelineGeometry;
  /** The element wrapping the track rows (header column included). */
  readonly rowsRef: RefObject<HTMLElement | null>;
}

function sameIndicator(left: TimelineDropIndicator | null, right: TimelineDropIndicator): boolean {
  return (
    left !== null &&
    left.insertTop === right.insertTop &&
    left.rejected === right.rejected &&
    left.placement.hoveredTrackId === right.placement.hoveredTrackId &&
    left.placement.insertIndex === right.placement.insertIndex &&
    left.placement.startSeconds === right.placement.startSeconds
  );
}

/**
 * Drops of panel assets (`application/x-video-creater-asset`) on the tracks area. Over a row the
 * asset targets that track; near a row edge or below the rows it targets a new track at that
 * index. The start is the explicit start-seconds value or the pointer time, snapped when snapping
 * is on. A drop commits one batch; a refused drop shows the planner's reason and commits nothing.
 * Effects tab tiles (`application/x-video-creater-effect`) drop onto the clip under the pointer
 * instead: a visual clip on an unlocked track accepts them, and the clip is selected afterwards.
 * Transition tiles drop onto a cut (see `useTransitionDrop`).
 */
export function useTimelineDrop({ geometry, rowsRef }: UseTimelineDropOptions) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const [indicator, setIndicator] = useState<TimelineDropIndicator | null>(null);
  const indicatorRef = useRef<TimelineDropIndicator | null>(null);
  const applyEffect = useApplyEffect();
  const transitionDrop = useTransitionDrop({ geometry, rowsRef });

  function update(next: TimelineDropIndicator | null) {
    if (next === null ? indicatorRef.current === null : sameIndicator(indicatorRef.current, next)) return;
    indicatorRef.current = next;
    setIndicator(next);
  }

  function locate(event: DragEvent<HTMLElement>): TimelineDropIndicator | null {
    const rows = rowsRef.current;
    if (!rows) return null;
    const rect = rows.getBoundingClientRect();
    const state = store.getState();
    const x = event.clientX - rect.left - geometry.headerWidth;
    const pointerSeconds = Math.max(0, x / geometry.pixelsPerSecond);
    const explicitSeconds = readDropStartSeconds(event.dataTransfer);
    const snap = state.snapEnabled
      ? snapClipProbes(state.project.timeline, [{ seconds: pointerSeconds, edge: "start" }], new Set(), {
          playheadSeconds: state.playheadSeconds,
          pixelsPerSecond: geometry.pixelsPerSecond,
          sticky: null,
        }).deltaSeconds
      : 0;
    const startSeconds = roundTimelineSeconds(explicitSeconds ?? Math.max(0, pointerSeconds + snap));
    const target = trackTargetAtY(geometry.rows, event.clientY - rect.top);
    if (!target) return { placement: { hoveredTrackId: null, insertIndex: null, startSeconds }, insertTop: geometry.totalHeight, rejected: false };
    if (target.insertIndex !== null) {
      const insertTop = geometry.rows[target.insertIndex]?.top ?? geometry.totalHeight;
      return { placement: { hoveredTrackId: null, insertIndex: target.insertIndex, startSeconds }, insertTop, rejected: false };
    }
    const locked = geometry.rows.find((row) => row.track.id === target.trackId)?.track.locked === true;
    return { placement: { hoveredTrackId: target.trackId, insertIndex: null, startSeconds }, insertTop: null, rejected: locked };
  }

  /** The clip under the pointer, on its row's lane. */
  function clipAt(event: DragEvent<HTMLElement>): TimelineItem | null {
    const rows = rowsRef.current;
    if (!rows) return null;
    const rect = rows.getBoundingClientRect();
    const y = event.clientY - rect.top;
    const seconds = (event.clientX - rect.left - geometry.headerWidth) / geometry.pixelsPerSecond;
    const row = geometry.rows.find((candidate) => y >= candidate.top && y < candidate.top + candidate.height);
    return row?.track.items.find((item) => seconds >= item.startSeconds && seconds < item.startSeconds + item.durationSeconds) ?? null;
  }

  function onEffectDragOver(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    const item = clipAt(event);
    const accepted = item !== null && "item" in effectTarget(store.getState().project, [item.id]);
    event.dataTransfer.dropEffect = accepted ? "copy" : "none";
    update(null);
  }

  function onEffectDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    update(null);
    const effect = readEffectDragData(event.dataTransfer);
    const item = clipAt(event);
    if (!effect) {
      store.getState().setLastError("That effect can't be applied.");
      return;
    }
    if (!item) {
      store.getState().setLastError("Drop the effect onto a video or image clip.");
      return;
    }
    void applyEffect(item.id, effect).then((applied) => {
      if (applied) store.getState().selectItems([item.id]);
    });
  }

  function onDragOver(event: DragEvent<HTMLElement>) {
    if (hasTransitionDragData(event.dataTransfer)) {
      update(null);
      transitionDrop.onTransitionDragOver(event);
      return;
    }
    if (hasEffectDragData(event.dataTransfer)) {
      onEffectDragOver(event);
      return;
    }
    if (!hasAssetDragData(event.dataTransfer)) return;
    event.preventDefault();
    const next = locate(event);
    event.dataTransfer.dropEffect = next?.rejected ? "none" : "copy";
    update(next);
  }

  function onDragLeave(event: DragEvent<HTMLElement>) {
    const related = event.relatedTarget;
    if (related instanceof Node && event.currentTarget.contains(related)) return;
    update(null);
    transitionDrop.clearTransitionDrop();
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    if (hasTransitionDragData(event.dataTransfer)) {
      transitionDrop.onTransitionDrop(event);
      return;
    }
    if (hasEffectDragData(event.dataTransfer)) {
      onEffectDrop(event);
      return;
    }
    if (!hasAssetDragData(event.dataTransfer)) return;
    event.preventDefault();
    const located = locate(event);
    update(null);
    const asset = readAssetDragData(event.dataTransfer);
    if (!asset) {
      store.getState().setLastError("That item can't be placed on the timeline.");
      return;
    }
    if (located) void commands.insertAsset(asset, located.placement);
  }

  return { dropIndicator: indicator, transitionDropCut: transitionDrop.transitionDropCut, dropHandlers: { onDragOver, onDragLeave, onDrop } };
}
