import { useMemo } from "react";
import { revealExportArtifactInSplitProjectFolder } from "@/lib/project";
import { backendArtifactUrl } from "@/lib/runtime/backend-client";
import { getRuntimeMode } from "@/lib/runtime/runtime-mode";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";

/**
 * "Show in folder" for the editor's open project. The backend reveals only files the project records
 * as export output (export artifacts, render outputs and render logs); there is no generic reveal.
 */
export interface RevealService {
  /** Resolves false when the file couldn't be shown, after a toast saying why in plain words. */
  revealExportArtifact(artifactPath: string): Promise<boolean>;
}

const noFolderMessage = "Save this project to a folder before showing its files.";

function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.trim() || "This file couldn't be shown in its folder.";
}

export function createRevealService(store: EditorStore): RevealService {
  const state = () => store.getState();
  return {
    async revealExportArtifact(artifactPath) {
      const { project, projectDir } = state();
      if (!projectDir.trim()) {
        state().pushToast({ title: noFolderMessage });
        return false;
      }
      try {
        if (getRuntimeMode() === "browser") {
          const artifact = project.exportArtifacts?.find((candidate) => candidate.path === artifactPath);
          if (!artifact) throw new Error("This file is not available as a browser download.");
          const url = await backendArtifactUrl(projectDir, artifact.id);
          const link = document.createElement("a");
          link.href = url;
          link.download = artifactPath.split(/[\\/]/).at(-1) || "download";
          link.hidden = true;
          document.body.append(link);
          link.click();
          link.remove();
          return true;
        }
        await revealExportArtifactInSplitProjectFolder({ projectDir, artifactPath });
        return true;
      } catch (error) {
        state().pushToast({ title: errorMessage(error) });
        return false;
      }
    },
  };
}

/** The reveal service bound to the editor store; stable for the lifetime of the store. */
export function useRevealService(): RevealService {
  const store = useEditorStoreApi();
  return useMemo(() => createRevealService(store), [store]);
}
