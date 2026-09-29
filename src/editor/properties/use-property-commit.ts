import { useCallback, useMemo } from "react";
import type { ProjectAction } from "@/lib/project";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { useEditorStoreApi } from "../store/editor-store-context";

/** Merges per-item builder results into one batch; the first blocked message wins. */
export function combineResults(results: readonly CommandResult[]): CommandResult {
  const actions: ProjectAction[] = [];
  for (const result of results) {
    if ("blocked" in result) return result;
    actions.push(...result.actions);
  }
  return { actions };
}

interface PropertyCommit {
  /**
   * Commits one control change as one `applyActions` call (one undo step) and clears the live
   * preview. A blocked result shows its message instead; an empty batch does nothing.
   */
  commit(result: CommandResult | readonly ProjectAction[]): Promise<unknown> | undefined;
  /** Live value while dragging; the preview applies it without touching the project. */
  preview(itemId: string, patch: Readonly<Record<string, unknown>>): void;
}

export function usePropertyCommit(): PropertyCommit {
  const store = useEditorStoreApi();
  const commit = useCallback(
    (result: CommandResult | readonly ProjectAction[]) => {
      const state = store.getState();
      state.clearPropertyPreview();
      if ("blocked" in result) {
        state.setLastError(result.blocked);
        return undefined;
      }
      const actions = "actions" in result ? result.actions : result;
      return actions.length > 0 ? state.applyActions(actions) : undefined;
    },
    [store],
  );
  const preview = useCallback(
    (itemId: string, patch: Readonly<Record<string, unknown>>) => store.getState().setPropertyPreview({ itemId, patch }),
    [store],
  );
  return useMemo(() => ({ commit, preview }), [commit, preview]);
}
