import { useEffect } from "react";
import { prepareProjectPreview, projectNeedsCanonicalPreview } from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { noProjectFolderMessage } from "./compositor-model";

/** Edits in quick succession prepare once, after the project has been still this long. */
const preparationDebounceMs = 400;

/**
 * Prepares canonical preview frames when the project needs them (Lottie, richer blend modes,
 * effects, colour grades, or reversed video and audio). Each project snapshot is prepared after a 400 ms debounce, and a
 * newer snapshot or Retry cancels the pending one. Without a backend the preview stays DOM-only.
 */
export function useCanonicalPreparation(): void {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const retryToken = useEditorStore((state) => state.canonicalRetryToken);

  useEffect(() => {
    const { setCanonicalPreparation } = store.getState();
    if (!projectNeedsCanonicalPreview(project)) {
      setCanonicalPreparation(null);
      return;
    }
    if (projectDir.trim().length === 0) {
      setCanonicalPreparation({ sourceProject: project, status: "failed", message: noProjectFolderMessage });
      return;
    }
    let cancelled = false;
    setCanonicalPreparation({ sourceProject: project, status: "preparing" });
    const timer = window.setTimeout(() => {
      prepareProjectPreview({ projectDir, project }).then(
        (result) => {
          if (!cancelled) setCanonicalPreparation({ sourceProject: project, status: "ready", result });
        },
        (error: unknown) => {
          if (cancelled) return;
          setCanonicalPreparation(
            isBackendUnavailableError(error)
              ? { sourceProject: project, status: "unavailable" }
              : { sourceProject: project, status: "failed", message: error instanceof Error ? error.message : String(error) },
          );
        },
      );
    }, preparationDebounceMs);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [project, projectDir, retryToken, store]);
}
