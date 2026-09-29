import { ArrowDownUp, ChevronRight, FolderPlus, MoreHorizontal, Palette, RefreshCw, WandSparkles } from "lucide-react";
import type { RefObject } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { mediaSortOptions, type MediaSort } from "@/lib/media/media-filters";
import type { MediaFolder } from "@/lib/project";
import { cn } from "@/lib/utils";
import { useMediaMoveTarget } from "./folder-tile";
import { thumbnailSizeOptions, type ThumbnailSize } from "./media-browser";

interface MediaBrowseBarProps {
  readonly breadcrumb: readonly MediaFolder[];
  readonly countLabel: string;
  readonly sort: MediaSort;
  readonly thumbnailSize: ThumbnailSize;
  readonly rebuilding: boolean;
  /** Receives focus after an overflow action's dialog closes. */
  readonly moreButtonRef: RefObject<HTMLButtonElement | null>;
  onNavigate(folderId: string | null): void;
  onDropMedia(folderId: string | null, dataTransfer: DataTransfer): void;
  onSortChange(sort: MediaSort): void;
  onThumbnailSizeChange(size: ThumbnailSize): void;
  onNewFolder(): void;
  onCreateMatte(): void;
  onOrganize(): void;
  onRebuildIndex(): void;
}

function Crumb({ label, current, onClick, onDropMedia }: { readonly label: string; readonly current: boolean; onClick(): void; onDropMedia(dataTransfer: DataTransfer): void }) {
  const target = useMediaMoveTarget(onDropMedia);
  return (
    <button
      type="button"
      aria-current={current ? "location" : undefined}
      onClick={onClick}
      {...target.handlers}
      className={cn(
        "min-w-0 truncate rounded-md px-1 py-0.5 text-[12px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        current ? "font-medium text-foreground" : "text-muted-foreground hover:bg-hover hover:text-foreground",
        target.over && "bg-accent-soft text-foreground ring-1 ring-primary",
      )}
    >
      {label}
    </button>
  );
}

/** Folder breadcrumb with the item count, the sort and size menu, and the overflow menu. */
export function MediaBrowseBar(props: MediaBrowseBarProps) {
  const { breadcrumb } = props;
  const sortLabel = mediaSortOptions.find((option) => option.value === props.sort)?.label ?? "Date added";
  return (
    <div className="flex items-center gap-1">
      <nav aria-label="Folder path" className="flex min-w-0 flex-1 items-center">
        <Crumb label="Project" current={breadcrumb.length === 0} onClick={() => props.onNavigate(null)} onDropMedia={(data) => props.onDropMedia(null, data)} />
        {breadcrumb.map((folder, index) => (
          <span key={folder.id} className="flex min-w-0 items-center">
            <ChevronRight className="h-3 w-3 shrink-0 text-dim" aria-hidden />
            <Crumb
              label={folder.name}
              current={index === breadcrumb.length - 1}
              onClick={() => props.onNavigate(folder.id)}
              onDropMedia={(data) => props.onDropMedia(folder.id, data)}
            />
          </span>
        ))}
        <span className="ml-1 shrink-0 text-[12px] text-dim">· {props.countLabel}</span>
      </nav>
      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label={`Sort media, ${sortLabel}`}
          className="flex h-7 shrink-0 items-center gap-1 rounded-md px-1.5 text-[12px] text-muted-foreground hover:bg-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ArrowDownUp className="h-3.5 w-3.5" aria-hidden />
          Sort
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>Sort by</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={props.sort} onValueChange={(value) => props.onSortChange(value as MediaSort)}>
            {mediaSortOptions.map((option) => (
              <DropdownMenuRadioItem key={option.value} value={option.value}>
                {option.label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>Thumbnail size</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={String(props.thumbnailSize)} onValueChange={(value) => props.onThumbnailSizeChange(Number(value) as ThumbnailSize)}>
            {thumbnailSizeOptions.map((option) => (
              <DropdownMenuRadioItem key={option.value} value={String(option.value)}>
                {option.label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger
          ref={props.moreButtonRef}
          aria-label="More media actions"
          className="grid h-7 w-7 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <MoreHorizontal className="h-4 w-4" aria-hidden />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={props.onNewFolder}>
            <FolderPlus className="h-4 w-4 text-muted-foreground" aria-hidden />
            New folder
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={props.onCreateMatte}>
            <Palette className="h-4 w-4 text-muted-foreground" aria-hidden />
            Create matte
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={props.onOrganize}>
            <WandSparkles className="h-4 w-4 text-muted-foreground" aria-hidden />
            Organize with AI
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={props.rebuilding} onSelect={props.onRebuildIndex}>
            <RefreshCw className="h-4 w-4 text-muted-foreground" aria-hidden />
            {props.rebuilding ? "Rebuilding index" : "Rebuild search index"}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
