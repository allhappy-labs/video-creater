import { AlertTriangle, Loader2, MoreHorizontal, RotateCcw } from "lucide-react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { generatedOutputHasProviderSourceUrl } from "@/lib/generation/assets";
import type { GeneratedAsset, ProjectJobSummary } from "@/lib/project";

export interface GenerationTileActions {
  /** Queues the generation again with its prompt and settings. */
  rerun(asset: GeneratedAsset): void;
  retryDownload(asset: GeneratedAsset): void;
  cancel(asset: GeneratedAsset): void;
  /** Fixture runtime only. */
  completeMock(asset: GeneratedAsset): void;
  /** Fixture runtime only. */
  failMock(asset: GeneratedAsset): void;
}

interface GenerationTileProps {
  readonly asset: GeneratedAsset;
  /** The asset's workflow job (same id), when recorded. */
  readonly job: ProjectJobSummary | null;
  /** Fixture runtime: adds the mock completion controls. Never true in the desktop app. */
  readonly fixtureControls: boolean;
  readonly actions: GenerationTileActions;
}

const menuItemClass = "min-h-8";

/**
 * A generation without usable output: a progress overlay while queued or running, and a failure
 * mark with Retry after. Its menu adds Retry download (when the provider still has the file),
 * Cancel for real runs, and the fixture-only mock controls.
 */
export function GenerationTile({ asset, job, fixtureControls, actions }: GenerationTileProps) {
  const active = asset.status === "queued" || asset.status === "running";
  const name = asset.name?.trim() || "Generated media";
  const canRerun = !active && asset.prompt.trim().length > 0;
  const canRetryDownload = asset.outputs.some(generatedOutputHasProviderSourceUrl);
  const cancelable = active && job?.kind === "generate_media" && job.startRequest?.input.mockMode === false;
  const status = active ? (asset.status === "queued" ? "Queued" : "Generating") : canRetryDownload && asset.status === "completed" ? "Download failed" : "Failed";
  const hasMenu = canRerun || canRetryDownload || cancelable || (fixtureControls && active);

  return (
    <li className="group relative min-w-0" aria-label={`${name}, ${status.toLowerCase()}`}>
      <span className="relative block aspect-square w-full overflow-hidden rounded-control bg-raised">
        <span className="absolute inset-0 grid place-items-center bg-background/60 text-foreground">
          {active ? (
            <Loader2 className="h-6 w-6 motion-safe:animate-spin" aria-hidden />
          ) : canRerun ? (
            <button
              type="button"
              aria-label={`Retry ${name}`}
              onClick={() => actions.rerun(asset)}
              className="grid h-9 w-9 place-items-center rounded-full bg-raised text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <RotateCcw className="h-4 w-4" aria-hidden />
            </button>
          ) : (
            <AlertTriangle className="h-6 w-6 text-destructive" aria-hidden />
          )}
        </span>
        <span className="absolute inset-x-1.5 bottom-1.5 flex items-center justify-center gap-1 truncate text-[10px] text-muted-foreground">
          {!active && <AlertTriangle className="h-3 w-3 shrink-0 text-destructive" aria-hidden />}
          {status}
        </span>
      </span>
      <span className="mt-1 block truncate px-0.5 text-[12px] text-foreground" title={name}>
        {name}
      </span>
      {hasMenu && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label={`${name} generation actions`}
              className="absolute right-1.5 top-1.5 grid h-7 w-7 place-items-center rounded-full bg-background/85 text-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <MoreHorizontal className="h-4 w-4" aria-hidden />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" aria-label={`${name} generation actions`}>
            {canRerun && (
              <DropdownMenuItem className={menuItemClass} onSelect={() => actions.rerun(asset)}>
                Retry
              </DropdownMenuItem>
            )}
            {canRetryDownload && (
              <DropdownMenuItem className={menuItemClass} onSelect={() => actions.retryDownload(asset)}>
                Retry download
              </DropdownMenuItem>
            )}
            {cancelable && (
              <DropdownMenuItem className={menuItemClass} onSelect={() => actions.cancel(asset)}>
                Cancel generation
              </DropdownMenuItem>
            )}
            {fixtureControls && active && (
              <>
                {cancelable && <DropdownMenuSeparator />}
                <DropdownMenuItem className={menuItemClass} onSelect={() => actions.completeMock(asset)}>
                  Complete (fixture)
                </DropdownMenuItem>
                <DropdownMenuItem className={menuItemClass} onSelect={() => actions.failMock(asset)}>
                  Fail (fixture)
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </li>
  );
}
