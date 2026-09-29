import { useEffect, useMemo, useState } from "react";
import { indexedSearchNeedsRebuild, indexedSearchStatusLabels, mediaIdsFromIndexedSearch } from "@/lib/media/search";
import type { ProjectMediaSearchResult, ProjectMediaSearchScope } from "@/lib/project";
import type { MediaService } from "../../services/media-service";

export interface IndexedSearch {
  /** Media ids from the index, or null to match locally. */
  readonly mediaIds: ReadonlySet<string> | null;
  readonly statusLabels: readonly string[];
  readonly errorMessage: string | null;
  readonly needsRebuild: boolean;
  /** Re-runs the current query (after a rebuild). */
  refresh(): void;
}

/**
 * Legacy indexed search: no debounce and no minimum length; each change of the trimmed query or
 * scope searches (limit 20) and a superseded response is dropped. Off or empty clears the result.
 */
export function useIndexedSearch(service: MediaService, enabled: boolean, query: string, scope: ProjectMediaSearchScope): IndexedSearch {
  const [result, setResult] = useState<ProjectMediaSearchResult | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [token, setToken] = useState(0);
  const trimmed = query.trim();

  useEffect(() => {
    setResult(null);
    setErrorMessage(null);
    if (!enabled || !trimmed) return undefined;
    let cancelled = false;
    void service.searchMedia(trimmed, scope).then((outcome) => {
      if (cancelled) return;
      if (outcome.status === "indexed") setResult(outcome.result);
      else setErrorMessage(outcome.message);
    });
    return () => {
      cancelled = true;
    };
  }, [service, enabled, trimmed, scope, token]);

  const mediaIds = useMemo(() => (result ? mediaIdsFromIndexedSearch(result) : null), [result]);
  return {
    mediaIds,
    statusLabels: indexedSearchStatusLabels(result),
    errorMessage,
    needsRebuild: indexedSearchNeedsRebuild(result),
    refresh: () => setToken((value) => value + 1),
  };
}
