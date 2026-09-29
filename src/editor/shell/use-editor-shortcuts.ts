import { useEffect } from "react";
import { matchShortcut, type ShortcutPlatform } from "@/lib/keymap";
import { isEditableKeyboardTarget } from "@/lib/timeline-ops/navigation";
import { useEditorStoreApi } from "../store/editor-store-context";
import type { EditorTabId } from "../store/persisted-layout";
import { editorTabs } from "./editor-tabs";

/** Keys with nothing focused land on the body (or the window itself). */
function isUnfocusedTarget(target: EventTarget | null): boolean {
  return target === null || target === window || target === document.body || target === document.documentElement;
}

const tabByShortcut: ReadonlyMap<string, EditorTabId> = new Map(editorTabs.map((tab) => [tab.shortcutId, tab.id]));

export function useEditorShortcuts(platform: ShortcutPlatform): void {
  const store = useEditorStoreApi();

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || isEditableKeyboardTarget(event.target)) return;
      const state = store.getState();
      const shortcut = matchShortcut(event, "global", platform);
      if (!shortcut) {
        // With nothing focused, Space still plays or pauses the preview.
        if (isUnfocusedTarget(event.target) && matchShortcut(event, "preview", platform)?.id === "preview.toggle") {
          state.togglePlaying();
          event.preventDefault();
        }
        return;
      }
      const tab = tabByShortcut.get(shortcut.id);
      if (tab) {
        state.setActiveTab(tab);
      } else if (shortcut.id === "editor.undo") {
        void state.undo();
      } else if (shortcut.id === "editor.redo") {
        void state.redo();
      } else if (shortcut.id === "editor.export") {
        state.openExportPopover();
      } else if (shortcut.id === "editor.shortcuts") {
        state.openOverlay("shortcuts");
      } else if (shortcut.id === "editor.clearSelection") {
        // Esc backs out one level: a sheet, the window-filling preview, asset preview, then the
        // selection together with the Show changes highlights.
        if (state.openSheetId) state.closeSheet();
        else if (state.fullscreen && !document.fullscreenElement) state.setFullscreen(false);
        else if (state.previewSource.kind === "asset") state.previewTimeline();
        else {
          state.clearSelection();
          state.clearHighlights();
        }
      } else {
        return;
      }
      event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [platform, store]);
}
