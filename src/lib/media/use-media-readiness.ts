import { useCallback, useSyncExternalStore } from "react";
import { remoteMediaReadiness, subscribeRemoteMediaReadiness } from "@/lib/runtime/adapters/remote-resource-cache";

/** Ticket changes must refresh URL consumers without changing canonical project state. */
export function useMediaReadiness(projectDir: string) {
  const snapshot = useCallback(() => remoteMediaReadiness(projectDir), [projectDir]);
  const subscribe = useCallback((listener: () => void) => subscribeRemoteMediaReadiness(listener, projectDir), [projectDir]);
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}
