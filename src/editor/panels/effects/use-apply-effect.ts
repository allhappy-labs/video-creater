import { useMemo } from "react";
import { propertySheetId } from "../../properties/mobile-property-sheets";
import type { EditorStore } from "../../store/editor-store";
import { useEditorStoreApi } from "../../store/editor-store-context";
import { planEffectApply, type ApplicableEffect } from "./effect-apply";

/** Resolves `true` when the effect was applied; a blocked or failed apply leaves the reason in `project.lastError`. */
export type ApplyEffect = (itemId: string, effect: ApplicableEffect) => Promise<boolean>;

/**
 * Shows the clip's Properties Video tab, whose Effects section holds the resource field. On
 * phones the Effects tab sheet is swapped for the clip's Effects property sheet.
 */
function revealEffectInProperties(store: EditorStore, itemId: string) {
  const state = store.getState();
  state.selectItems([itemId]);
  state.setPropertiesTab("visual", "video");
  if (state.openSheetId !== null) state.openSheet(propertySheetId("effects"));
}

/**
 * Applies a catalog effect to one clip as a single undo step. Resource-backed effects then open
 * Properties so the resource can be entered.
 */
export function useApplyEffect(): ApplyEffect {
  const store = useEditorStoreApi();
  return useMemo(
    () => async (itemId, effect) => {
      const result = planEffectApply(store.getState().project, itemId, effect);
      if ("blocked" in result) {
        store.getState().setLastError(result.blocked);
        return false;
      }
      const applied = await store.getState().applyActions(result.actions);
      if (!applied) return false;
      if (effect.resourceKey) revealEffectInProperties(store, itemId);
      return true;
    },
    [store],
  );
}
