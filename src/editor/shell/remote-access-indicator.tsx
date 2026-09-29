import { useState, useSyncExternalStore } from "react";

import { Button } from "@/components/ui/button";
import {
  remoteProjectAccess,
  subscribeRemoteProjectAccess,
  takeOverRemoteProject,
} from "@/lib/runtime/adapters/remote-project-access";
import { OverlayDialog } from "../overlays/overlay-dialog";
import { useEditorStore } from "../store/editor-store-context";
import { useRuntimeMode } from "../services/use-runtime-mode";

export function RemoteAccessIndicator({ compact }: { readonly compact: boolean }) {
  const runtimeMode = useRuntimeMode();
  const projectId = useEditorStore((state) => state.projectDir);
  const access = useSyncExternalStore(
    subscribeRemoteProjectAccess,
    () => remoteProjectAccess(projectId),
    () => remoteProjectAccess(projectId),
  );
  const [confirming, setConfirming] = useState(false);
  const [takingOver, setTakingOver] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (runtimeMode !== "browser") return null;
  if (access.mode !== "readOnly") {
    return (
      <span
        role="status"
        aria-label={access.mode === "editing" ? "Connected, editing" : "Connected"}
        title={access.mode === "editing" ? "Connected to the host with editing access" : "Connected to the host"}
        className="flex h-7 shrink-0 items-center gap-1.5 rounded-full bg-raised px-2 text-[11px] text-muted-foreground"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-success" aria-hidden />
        {!compact ? (access.mode === "editing" ? "Editing" : "Connected") : null}
      </span>
    );
  }

  return (
    <>
      <button
        type="button"
        onClick={() => setConfirming(true)}
        className="h-7 shrink-0 rounded-full bg-warning/15 px-2 text-[11px] font-medium text-warning focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        View only{compact ? "" : " · Take over"}
      </button>
      <OverlayDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Take over editing?"
        description={`${access.editorDisplayName} will become view only. Unsaved work on that device may need to be reloaded.`}
        footer={(
          <>
            <Button variant="ghost" size="sm" disabled={takingOver} onClick={() => setConfirming(false)}>Cancel</Button>
            <Button
              size="sm"
              disabled={takingOver}
              onClick={() => {
                setTakingOver(true);
                setError(null);
                void takeOverRemoteProject(projectId)
                  .then(() => setConfirming(false))
                  .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : "Editing access could not be moved."))
                  .finally(() => setTakingOver(false));
              }}
            >
              {takingOver ? "Taking over…" : "Take over"}
            </Button>
          </>
        )}
      >
        {error ? <p role="alert" className="px-4 py-3 text-[12px] text-destructive">{error}</p> : <div />}
      </OverlayDialog>
    </>
  );
}
