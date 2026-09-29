import { useCallback, type KeyboardEvent } from "react";
import { matchShortcut } from "@/lib/keymap";
import { isEditableKeyboardTarget } from "@/lib/timeline-ops/navigation";
import { useShortcutPlatform } from "../shell/use-shortcut-platform";

interface PreviewKeyActions {
  readonly canPlay: boolean;
  readonly canStep: boolean;
  readonly onTogglePlay: () => void;
  readonly onStep: (direction: -1 | 1) => void;
}

/**
 * Preview-scope keys for the preview panel's `onKeyDown`: Space plays or pauses, ArrowLeft and
 * ArrowRight step one frame. Editable targets, keys already handled (the scrubber), and Space on a
 * focused button (which presses it) are left alone.
 */
export function usePreviewKeys({ canPlay, canStep, onTogglePlay, onStep }: PreviewKeyActions) {
  const platform = useShortcutPlatform();
  return useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.defaultPrevented || isEditableKeyboardTarget(event.target)) return;
      const shortcut = matchShortcut(event.nativeEvent, "preview", platform);
      if (shortcut?.id === "preview.toggle") {
        if (!canPlay || (event.target instanceof Element && event.target.closest("button"))) return;
        event.preventDefault();
        onTogglePlay();
      } else if (shortcut?.id === "preview.stepBack" || shortcut?.id === "preview.stepForward") {
        if (!canStep) return;
        event.preventDefault();
        onStep(shortcut.id === "preview.stepBack" ? -1 : 1);
      }
    },
    [canPlay, canStep, onStep, onTogglePlay, platform],
  );
}
