import type { BackendConnection } from "./backend-client";
import type { RemoteSessionState } from "./remote-session";

export type RuntimeMode = "desktop" | "browser" | "fixture";

export interface RuntimeDescriptor {
  readonly mode: RuntimeMode;
  readonly connection: BackendConnection;
  readonly remoteSession?: RemoteSessionState;
}

const runtimeModes = new Set<RuntimeMode>(["desktop", "browser", "fixture"]);

export function createRuntimeDescriptor(
  mode: RuntimeMode,
  connection: BackendConnection,
  remoteSession?: RemoteSessionState,
): RuntimeDescriptor {
  if (!runtimeModes.has(mode)) {
    throw new Error(`Unsupported runtime mode: ${String(mode)}`);
  }
  if (mode !== "browser" && connection.status !== "connected") {
    throw new Error(`${mode} runtime requires a connected backend`);
  }
  return Object.freeze({
    mode,
    connection: Object.freeze(connection),
    ...(remoteSession ? { remoteSession: Object.freeze(remoteSession) } : {}),
  });
}
