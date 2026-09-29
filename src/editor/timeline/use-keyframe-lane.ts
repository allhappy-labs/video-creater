import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent as ReactPointerEvent } from "react";
import type { ProjectAction, ProjectActionKeyframe } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { moveThresholdPixels } from "@/lib/timeline-ops/clip-drag";
import {
  addKeyframeAction,
  draggedKeyframe,
  keyframeEditActions,
  keyframeKeyboardEdit,
  type KeyframeLaneModel,
} from "@/lib/timeline-ops/keyframe-lane";
import { animationFrameCoalescer, followPointer, type PointerPoint } from "./pointer-session";
import { useTimelineCommands } from "./timeline-commands";

/** Vertical padding inside the lane; diamonds use the remaining height for the value range. */
export const keyframeLanePadding = 8;

export interface KeyframeDragState {
  readonly fromSeconds: number;
  readonly atSeconds: number;
  readonly value: number;
}

interface UseKeyframeLaneOptions {
  readonly item: TimelineItem;
  readonly model: KeyframeLaneModel;
  readonly pixelsPerSecond: number;
  readonly laneHeight: number;
  /** Locked tracks show keyframes read-only. */
  readonly locked: boolean;
}

/** Pointer travel under the threshold on one axis leaves that axis unchanged. */
function axisTravel(delta: number): number {
  return Math.abs(delta) >= moveThresholdPixels ? delta : 0;
}

/**
 * Keyframe lane editing: drag a diamond in time (and value, vertically) with one commit on
 * release, edit a focused diamond from the keyboard, and double-click the lane to add a
 * keyframe at the pointer. Each edit is one batch; focus follows the edited keyframe.
 */
export function useKeyframeLane({ item, model, pixelsPerSecond, laneHeight, locked }: UseKeyframeLaneOptions) {
  const commands = useTimelineCommands();
  const [drag, setDrag] = useState<KeyframeDragState | null>(null);
  const stopRef = useRef<(() => void) | null>(null);
  /** Clip-local time of the keyframe to focus once the edit lands. */
  const pendingFocusSeconds = useRef<number | null>(null);

  useEffect(() => () => stopRef.current?.(), []);

  function commit(actions: ProjectAction[], focusSeconds: number | null) {
    if (actions.length === 0) return;
    pendingFocusSeconds.current = focusSeconds;
    void commands.applyPlan({ actions });
  }

  function onDiamondPointerDown(event: ReactPointerEvent<HTMLElement>, keyframe: ProjectActionKeyframe) {
    if (event.button !== 0 || locked || stopRef.current) return;
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.focus({ preventScroll: true });
    const start = { clientX: event.clientX, clientY: event.clientY };
    const valueRange = model.config.maximum - model.config.minimum;
    const innerHeight = Math.max(1, laneHeight - keyframeLanePadding * 2);
    const evaluate = (point: PointerPoint): KeyframeDragState => ({
      fromSeconds: keyframe.atSeconds,
      ...draggedKeyframe(
        item,
        model.config,
        keyframe,
        axisTravel(point.clientX - start.clientX) / pixelsPerSecond,
        (-axisTravel(point.clientY - start.clientY) / innerHeight) * valueRange,
      ),
    });
    const coalescer = animationFrameCoalescer(evaluate, setDrag);
    const end = () => {
      stopRef.current = null;
      coalescer.cancel();
      setDrag(null);
    };
    stopRef.current = followPointer(event.pointerId, {
      move: (point) => coalescer.schedule(point),
      release(point) {
        const moved = evaluate(point);
        end();
        commit(keyframeEditActions(item.id, model.config.property, keyframe, moved), moved.atSeconds);
      },
      cancel: end,
    });
  }

  function onDiamondKeyDown(event: KeyboardEvent<HTMLElement>, keyframe: ProjectActionKeyframe) {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const edit = keyframeKeyboardEdit(item, model.config, keyframe, event);
    if (edit === null) return;
    event.preventDefault();
    event.stopPropagation();
    if (locked) return;
    const { property } = model.config;
    if (edit === "delete") {
      commit([{ type: "deleteItemKeyframe", itemId: item.id, property, atSeconds: keyframe.atSeconds }], null);
      return;
    }
    commit(keyframeEditActions(item.id, property, keyframe, edit), edit.atSeconds);
  }

  function onLaneDoubleClick(event: MouseEvent<HTMLElement>) {
    if (locked || (event.target instanceof Element && event.target.closest("[data-keyframe-at]"))) return;
    const left = event.currentTarget.getBoundingClientRect().left;
    const localSeconds = (event.clientX - left) / pixelsPerSecond - item.startSeconds;
    if (localSeconds < 0 || localSeconds > item.durationSeconds) return;
    const action = addKeyframeAction(item, model.config, model.keyframes, localSeconds);
    commit([action], action.type === "upsertItemKeyframe" ? action.keyframe.atSeconds : null);
  }

  return { drag, pendingFocusSeconds, onDiamondPointerDown, onDiamondKeyDown, onLaneDoubleClick };
}
