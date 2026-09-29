import { RefreshCw, Search, Sparkles } from "lucide-react";
import { mediaFilterOptions, type MediaFilter } from "@/lib/media/media-filters";
import type { ProjectMediaSearchScope } from "@/lib/project";
import { cn } from "@/lib/utils";
import { searchScopeOptions } from "./media-browser";

interface SmartSearchState {
  readonly enabled: boolean;
  readonly scope: ProjectMediaSearchScope;
  readonly statusLabels: readonly string[];
  /** "Search index unavailable" after a failed indexed search; local matches still show. */
  readonly errorMessage: string | null;
  readonly needsRebuild: boolean;
  readonly rebuilding: boolean;
  readonly rebuildMessage: string | null;
}

interface MediaToolbarProps {
  readonly filter: MediaFilter;
  readonly search: string;
  readonly smart: SmartSearchState;
  onFilterChange(filter: MediaFilter): void;
  onSearchChange(query: string): void;
  onSmartChange(enabled: boolean): void;
  onScopeChange(scope: ProjectMediaSearchScope): void;
  onRebuild(): void;
}

const chipClass =
  "h-7 rounded-full px-3 text-[12px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none";

/** Filter chips, the search field with its Smart search toggle, and the indexed search status. */
export function MediaToolbar({ filter, search, smart, onFilterChange, onSearchChange, onSmartChange, onScopeChange, onRebuild }: MediaToolbarProps) {
  const querying = search.trim().length > 0;
  return (
    <div className="flex flex-col gap-2">
      <div role="group" aria-label="Media type" className="flex flex-wrap gap-1.5">
        {mediaFilterOptions.map((option) => (
          <button
            key={option.value}
            type="button"
            aria-pressed={option.value === filter}
            onClick={() => onFilterChange(option.value)}
            className={cn(
              chipClass,
              option.value === filter ? "bg-accent-soft text-foreground" : "text-muted-foreground hover:bg-hover hover:text-foreground",
            )}
          >
            {option.label}
          </button>
        ))}
      </div>
      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-dim" aria-hidden />
        <input
          type="search"
          aria-label="Search project media"
          placeholder={smart.enabled ? "Describe a shot, word or file" : "Search"}
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          className="h-8 w-full rounded-control bg-raised pl-8 pr-10 text-[12px] text-foreground outline-none placeholder:text-dim focus-visible:ring-2 focus-visible:ring-ring"
        />
        <button
          type="button"
          aria-label="Smart search"
          aria-pressed={smart.enabled}
          title={smart.enabled ? "Smart search is on: visual, transcript and metadata index" : "Smart search is off: names and folders only"}
          onClick={() => onSmartChange(!smart.enabled)}
          className={cn(
            "absolute right-1 top-1/2 grid h-6 w-7 -translate-y-1/2 place-items-center rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
            smart.enabled ? "bg-accent-soft text-primary" : "text-dim hover:bg-hover hover:text-foreground",
          )}
        >
          <Sparkles className="h-3.5 w-3.5" aria-hidden />
        </button>
      </div>
      {smart.enabled && (
        <div className="flex flex-col gap-1.5">
          <label className="flex items-center gap-2 text-[11px] text-muted-foreground">
            Search in
            <select
              aria-label="Indexed search scope"
              value={smart.scope}
              onChange={(event) => onScopeChange(event.target.value as ProjectMediaSearchScope)}
              className="h-6 rounded-md bg-raised px-1.5 text-[11px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {searchScopeOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          {querying && (
            <div role="status" aria-label="Search index status" className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-dim">
              {smart.errorMessage ? <span className="text-warning">{smart.errorMessage}</span> : smart.statusLabels.map((label) => <span key={label}>{label}</span>)}
              {smart.needsRebuild && (
                <button
                  type="button"
                  disabled={smart.rebuilding}
                  onClick={onRebuild}
                  className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium text-primary hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
                >
                  <RefreshCw className="h-3 w-3" aria-hidden />
                  {smart.rebuilding ? "Rebuilding index" : "Rebuild search index"}
                </button>
              )}
            </div>
          )}
        </div>
      )}
      {smart.rebuildMessage && <p role="status" className="text-[11px] text-dim">{smart.rebuildMessage}</p>}
    </div>
  );
}
