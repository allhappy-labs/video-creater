import { useState, useSyncExternalStore } from "react";
import type { VideoProject } from "@/lib/project";
import { reconcileRemoteProject, remoteUnknownOutcome, subscribeRemoteUnknownOutcome } from "@/lib/runtime/adapters/remote-outcome-state";
import { useEditorStore } from "../store/editor-store-context";

export function RemoteOutcomeNotice() {
  const projectDir = useEditorStore((state) => state.projectDir);
  const projectId = useEditorStore((state) => state.project.id);
  const install = useEditorStore((state) => state.installReconciledProject);
  const outcome = useSyncExternalStore(subscribeRemoteUnknownOutcome, () => remoteUnknownOutcome(projectDir), () => remoteUnknownOutcome(projectDir));
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!outcome) return null;

  function refresh() {
    setRefreshing(true);
    setError(null);
    void reconcileRemoteProject<VideoProject>(projectDir, projectId)
      .then((project) => install(project))
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : "The project could not be refreshed."))
      .finally(() => setRefreshing(false));
  }

  return (
    <div role="alert" aria-label="Unconfirmed edit" className="flex items-center gap-2 px-3 py-2 text-[12px] text-warning">
      <span className="min-w-0 flex-1">{error ?? "The last operation may have completed. Refresh the project to continue editing."}</span>
      <button type="button" disabled={refreshing} onClick={refresh} className="shrink-0 rounded-control px-2 py-1 text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50">{refreshing ? "Refreshing…" : "Refresh project"}</button>
    </div>
  );
}
