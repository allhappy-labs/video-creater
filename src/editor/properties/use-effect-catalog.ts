import { useEffect, useState } from "react";
import { listVisualEffectCatalog, type VisualEffectDescriptor } from "@/lib/project";

interface EffectCatalogState {
  readonly status: "loading" | "ready";
  /** Empty when the backend is unavailable or the catalog failed to load. */
  readonly effects: readonly VisualEffectDescriptor[];
}

let loaded: readonly VisualEffectDescriptor[] | null = null;
let pending: Promise<readonly VisualEffectDescriptor[]> | null = null;

/**
 * Loads the catalog once per editor session. Without a backend (browser fixtures) or on a
 * failure the catalog is empty; a failure is retried the next time a section mounts.
 */
function loadEffectCatalog(): Promise<readonly VisualEffectDescriptor[]> {
  pending ??= Promise.resolve()
    .then(() => listVisualEffectCatalog())
    .then(
      (catalog: unknown) => {
        const effects = (catalog as { effects?: unknown } | undefined)?.effects;
        loaded = Array.isArray(effects) ? (effects as VisualEffectDescriptor[]) : [];
        return loaded;
      },
      () => {
        pending = null;
        return [];
      },
    );
  return pending;
}

/** Test seam: forget the session's catalog. */
export function resetEffectCatalogForTests(): void {
  loaded = null;
  pending = null;
}

export function useEffectCatalog(): EffectCatalogState {
  const [effects, setEffects] = useState<readonly VisualEffectDescriptor[] | null>(loaded);
  useEffect(() => {
    if (effects !== null) return;
    let cancelled = false;
    void loadEffectCatalog().then((next) => {
      if (!cancelled) setEffects(next);
    });
    return () => {
      cancelled = true;
    };
  }, [effects]);
  return effects === null ? { status: "loading", effects: [] } : { status: "ready", effects };
}
