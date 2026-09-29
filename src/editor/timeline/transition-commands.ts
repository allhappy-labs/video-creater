import { useMemo } from "react";
import type { TransitionKind } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import {
  locateTransition,
  noCutReason,
  planAddTransition,
  planRemoveTransition,
  planTransitionDuration,
  planTransitionKind,
  transitionAddTarget,
} from "@/lib/timeline-ops/transition-commands";
import type { TransitionCut } from "@/lib/timeline-ops/transitions";
import type { EditorStore } from "../store/editor-store";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";

/**
 * Store-bound transition edits. Each applied command is one `applyActions` call (one undo step)
 * and resolves `true`; a refused one resolves `false` with its reason in `project.lastError`.
 */
export interface TransitionCommands {
  /** Adds `kind` on `cut` (or changes the type already there) and selects the transition. */
  addAtCut(cut: TransitionCut, kind: TransitionKind): Promise<boolean>;
  /** The Effects tab `+`: adds on the cut `transitionAddTarget` resolves. */
  addAtTarget(kind: TransitionKind): Promise<boolean>;
  setKind(transitionId: string, kind: TransitionKind): Promise<boolean>;
  setDuration(transitionId: string, seconds: number): Promise<boolean>;
  remove(transitionId: string): Promise<boolean>;
}

function createTransitionCommands(store: EditorStore): TransitionCommands {
  const state = () => store.getState();

  async function run(result: CommandResult): Promise<boolean> {
    if ("blocked" in result) {
      state().setLastError(result.blocked);
      return false;
    }
    if (result.actions.length === 0) return true;
    return (await state().applyActions(result.actions)) !== null;
  }

  async function addAtCut(cut: TransitionCut, kind: TransitionKind) {
    const plan = planAddTransition(state().project, cut, kind);
    if (!(await run(plan)) || "blocked" in plan) return false;
    state().selectTransition(plan.transitionId);
    if (plan.shortenedMessage) state().pushToast({ title: plan.shortenedMessage });
    return true;
  }

  return {
    addAtCut,
    addAtTarget(kind) {
      const cut = transitionAddTarget(state().project, state());
      if (!cut) {
        state().setLastError(noCutReason);
        return Promise.resolve(false);
      }
      return addAtCut(cut, kind);
    },
    setKind: (transitionId, kind) => run(planTransitionKind(state().project, transitionId, kind)),
    setDuration: (transitionId, seconds) => run(planTransitionDuration(state().project, transitionId, seconds)),
    async remove(transitionId) {
      if (!(await run(planRemoveTransition(state().project, transitionId)))) return false;
      if (state().selectedTransitionId === transitionId) state().selectTransition(null);
      return true;
    },
  };
}

export function useTransitionCommands(): TransitionCommands {
  const store = useEditorStoreApi();
  return useMemo(() => createTransitionCommands(store), [store]);
}

/** The selected transition on the active timeline and its track, or null. */
export function useSelectedTransition() {
  const timeline = useEditorStore((state) => state.project.timeline);
  const transitionId = useEditorStore((state) => state.selectedTransitionId);
  return useMemo(() => locateTransition(timeline, transitionId), [timeline, transitionId]);
}
