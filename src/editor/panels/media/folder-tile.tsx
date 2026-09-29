import { Folder } from "lucide-react";
import { useState, type DragEvent } from "react";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from "@/components/ui/context-menu";
import { pluralize } from "@/lib/format";
import type { MediaFolder } from "@/lib/project";
import { cn } from "@/lib/utils";
import { openContextMenuAt, useLongPress } from "../../shell/use-long-press";
import { isMediaMoveDrag } from "./media-browser";

/** Drop-target handlers for moving media into a folder (or the project root). */
export function useMediaMoveTarget(onDropMedia: (dataTransfer: DataTransfer) => void) {
  const [over, setOver] = useState(false);
  return {
    over,
    handlers: {
      onDragOver(event: DragEvent<HTMLElement>) {
        if (!isMediaMoveDrag(event.dataTransfer)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
        setOver(true);
      },
      onDragLeave(event: DragEvent<HTMLElement>) {
        if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return;
        setOver(false);
      },
      onDrop(event: DragEvent<HTMLElement>) {
        setOver(false);
        if (!isMediaMoveDrag(event.dataTransfer)) return;
        event.preventDefault();
        onDropMedia(event.dataTransfer);
      },
    },
  };
}

interface FolderTileProps {
  readonly folder: MediaFolder;
  readonly itemCount: number;
  onOpen(folder: MediaFolder): void;
  onRename(folder: MediaFolder): void;
  onDelete(folder: MediaFolder): void;
  onDropMedia(folder: MediaFolder, dataTransfer: DataTransfer): void;
}

/** A folder in the grid: opens on click, accepts dragged media, and has Rename and Delete. */
export function FolderTile({ folder, itemCount, onOpen, onRename, onDelete, onDropMedia }: FolderTileProps) {
  const target = useMediaMoveTarget((dataTransfer) => onDropMedia(folder, dataTransfer));
  const count = pluralize(itemCount, "item");
  // Touch: a 500 ms press opens the menu (Radix alone waits 700 ms and gives up on any finger jitter).
  const longPress = useLongPress({ onLongPress: openContextMenuAt });
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild {...longPress.handlers}>
        <li className="min-w-0 select-none [-webkit-touch-callout:none]" {...target.handlers}>
          <button
            type="button"
            aria-label={`Open folder ${folder.name}, ${count}`}
            onClick={() => onOpen(folder)}
            className="flex w-full min-w-0 flex-col gap-1 rounded-control text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <span
              className={cn(
                "grid aspect-square w-full place-items-center rounded-control bg-raised text-muted-foreground transition-colors hover:bg-hover motion-reduce:transition-none",
                target.over && "bg-accent-soft text-foreground ring-2 ring-inset ring-primary",
              )}
            >
              <Folder className="h-8 w-8" aria-hidden />
            </span>
            <span className="truncate px-0.5 text-[12px] text-foreground" title={folder.name}>
              {folder.name}
            </span>
            <span className="-mt-1 px-0.5 text-[11px] text-dim">{count}</span>
          </button>
        </li>
      </ContextMenuTrigger>
      <ContextMenuContent aria-label={`${folder.name} folder actions`}>
        <ContextMenuItem className="min-h-8" onSelect={() => onOpen(folder)}>
          Open
        </ContextMenuItem>
        <ContextMenuItem className="min-h-8" onSelect={() => onRename(folder)}>
          Rename…
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem className="min-h-8 text-destructive" onSelect={() => onDelete(folder)}>
          Delete…
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
