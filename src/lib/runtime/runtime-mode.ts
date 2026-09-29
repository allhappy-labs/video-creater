import type { RuntimeMode } from "./runtime-descriptor";

let installedRuntimeMode: RuntimeMode = "browser";
const listeners = new Set<() => void>();

/** The fixture runtime exists only in development builds that opt in through the window marker. */
export function fixtureRuntimeMarkerEnabled(): boolean {
  return (
    import.meta.env.DEV === true &&
    typeof window !== "undefined" &&
    window.__EDITOR_FIXTURE_RUNTIME__?.enabled === true
  );
}

/**
 * Returns the runtime selected at bootstrap. A fixture mode is re-checked against the build and
 * marker, so fixture-only controls can never surface in a production or desktop runtime.
 */
export function getRuntimeMode(): RuntimeMode {
  if (installedRuntimeMode === "fixture" && !fixtureRuntimeMarkerEnabled()) return "browser";
  return installedRuntimeMode;
}

export function installRuntimeMode(mode: RuntimeMode): void {
  if (mode === installedRuntimeMode) return;
  installedRuntimeMode = mode;
  for (const listener of listeners) listener();
}

export function subscribeRuntimeMode(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
