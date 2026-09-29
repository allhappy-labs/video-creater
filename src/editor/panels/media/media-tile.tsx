import * as ContextMenuPrimitive from "@radix-ui/react-context-menu";
import { Check, ChevronRight, Plus } from "lucide-react";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from "@/components/ui/context-menu";
import { formatDurationBadge } from "@/lib/format";
import type { MediaFolderNode } from "@/lib/media/folder-tree";
import { mediaContentKind } from "@/lib/media/media-filters";
import { mediaDisplayName } from "@/lib/media/names";
import type { MediaAsset } from "@/lib/project";
import { cn } from "@/lib/utils";
import { openContextMenuAt, useLongPress } from "../../shell/use-long-press";
import { writeAssetDragData } from "../../timeline/drag-data";
import { mediaIdDragMimeType } from "./media-browser";
import { MediaThumbnail } from "./media-thumbnail";

export interface MediaTileActions {
  /** Click: preview the asset, replace the target clip in replace mode, or mention it in attach mode. */
  activate(media: MediaAsset): void;
  insert(media: MediaAsset): void;
  rename(media: MediaAsset): void;
  move(media: MediaAsset, folderId: string | null): void;
  revealOnTimeline(media: MediaAsset): void;
  remove(media: MediaAsset): void;
}

interface MediaTileProps {
  readonly media: MediaAsset;
  readonly projectDir: string;
  readonly previewing: boolean;
  readonly flashing: boolean;
  /** The clip label while replace mode is on. */
  readonly replaceLabel: string | null;
  /** Attach mode (from the AI composer): a click mentions the media in the AI draft. */
  readonly attachMode: boolean;
  readonly folders: readonly MediaFolderNode[];
  readonly onTimeline: boolean;
  readonly actions: MediaTileActions;
}

const itemClass = "min-h-8";

/**
 * A media grid tile: thumbnail with duration badge and name, `+` to insert at the playhead
 * (always visible on touch), draggable onto the timeline or a folder, with a context menu.
 */
export function MediaTile({ media, projectDir, previewing, flashing, replaceLabel, attachMode, folders, onTimeline, actions }: MediaTileProps) {
  const name = mediaDisplayName(media);
  const kind = mediaContentKind(media);
  const duration = kind === "image" ? null : formatDurationBadge(media.durationSeconds);
  // Touch: a 500 ms press opens the menu (Radix alone waits 700 ms and gives up on any finger jitter).
  const longPress = useLongPress({ onLongPress: openContextMenuAt });

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild {...longPress.handlers}>
        <li
          draggable
          data-media-id={media.id}
          title={name}
          onDragStart={(event) => {
            writeAssetDragData(event.dataTransfer, { kind: "media", id: media.id });
            event.dataTransfer.setData(mediaIdDragMimeType, media.id);
            event.dataTransfer.setData("text/plain", media.id);
            event.dataTransfer.effectAllowed = "copyMove";
          }}
          className="group relative min-w-0 cursor-grab select-none active:cursor-grabbing [-webkit-touch-callout:none]"
        >
          <button
            type="button"
            aria-label={attachMode ? `Attach ${name}` : replaceLabel ? `Replace ${replaceLabel} with ${name}` : `Preview ${name}`}
            aria-pressed={attachMode || replaceLabel ? undefined : previewing}
            onClick={() => actions.activate(media)}
            onDoubleClick={() => actions.insert(media)}
            className="flex w-full min-w-0 flex-col gap-1 rounded-control text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <span
              className={cn(
                "relative block aspect-square w-full overflow-hidden rounded-control bg-raised ring-inset transition-shadow motion-reduce:transition-none",
                previewing && "ring-2 ring-primary",
                flashing && "ring-2 ring-accent motion-safe:animate-pulse",
              )}
            >
              <MediaThumbnail media={media} projectDir={projectDir} />
              {media.kind === "generated" && (
                <span className="absolute left-1.5 top-1.5 rounded bg-background/80 px-1 text-[10px] font-medium text-foreground">AI</span>
              )}
              {duration && (
                <span className="tabular-time absolute bottom-1.5 right-1.5 rounded bg-background/80 px-1 text-[10px] text-foreground">{duration}</span>
              )}
            </span>
            <span className="truncate px-0.5 text-[12px] leading-snug text-foreground">
              {name}
            </span>
          </button>
          <button
            type="button"
            aria-label={`Add ${name} at the playhead`}
            onClick={() => actions.insert(media)}
            className="absolute right-1.5 top-1.5 grid h-7 w-7 place-items-center rounded-full bg-primary text-primary-foreground opacity-0 shadow-md transition-opacity hover:bg-primary/90 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100 motion-reduce:transition-none [@media(hover:none)]:opacity-100"
          >
            <Plus className="h-4 w-4" aria-hidden />
          </button>
        </li>
      </ContextMenuTrigger>
      <ContextMenuContent aria-label={`${name} actions`}>
        <ContextMenuItem className={itemClass} onSelect={() => actions.insert(media)}>
          Add at playhead
        </ContextMenuItem>
        <ContextMenuItem className={itemClass} onSelect={() => actions.rename(media)}>
          Rename…
        </ContextMenuItem>
        <ContextMenuPrimitive.Sub>
          <ContextMenuPrimitive.SubTrigger className="flex min-h-8 cursor-default select-none items-center gap-2 rounded-md px-2 py-1 text-[13px] outline-none data-[highlighted]:bg-raised data-[state=open]:bg-raised">
            Move to folder
            <ChevronRight className="ml-auto h-3.5 w-3.5 text-dim" aria-hidden />
          </ContextMenuPrimitive.SubTrigger>
          <ContextMenuPrimitive.Portal>
            <ContextMenuPrimitive.SubContent
              collisionPadding={8}
              className="z-50 max-h-72 min-w-44 overflow-y-auto rounded-control border border-line bg-popover p-1 text-popover-foreground shadow-xl"
            >
              <MoveItem label="Project (no folder)" current={!media.folderId} onSelect={() => actions.move(media, null)} />
              {folders.map((node) => (
                <MoveItem key={node.folder.id} label={node.label} current={media.folderId === node.folder.id} onSelect={() => actions.move(media, node.folder.id)} />
              ))}
            </ContextMenuPrimitive.SubContent>
          </ContextMenuPrimitive.Portal>
        </ContextMenuPrimitive.Sub>
        <ContextMenuItem className={itemClass} disabled={!onTimeline} onSelect={() => actions.revealOnTimeline(media)}>
          {onTimeline ? "Reveal on timeline" : "Not on the timeline"}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem className={cn(itemClass, "text-destructive")} onSelect={() => actions.remove(media)}>
          Delete…
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function MoveItem({ label, current, onSelect }: { readonly label: string; readonly current: boolean; readonly onSelect: () => void }) {
  return (
    <ContextMenuItem className={cn(itemClass, "relative pl-7")} disabled={current} onSelect={onSelect}>
      {current && <Check className="absolute left-2 h-3.5 w-3.5" aria-hidden />}
      <span className="truncate">{label}</span>
    </ContextMenuItem>
  );
}
