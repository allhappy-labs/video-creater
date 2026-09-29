import type { GeneratedAsset, MediaAsset, MediaFolder, ProjectJobSummary } from "@/lib/project";
import type { MediaFolderNode } from "@/lib/media/folder-tree";
import { FolderTile } from "./folder-tile";
import { GenerationTile, type GenerationTileActions } from "./generation-tile";
import { MediaTile, type MediaTileActions } from "./media-tile";

export interface FolderTileActions {
  open(folder: MediaFolder): void;
  rename(folder: MediaFolder): void;
  remove(folder: MediaFolder): void;
  dropMedia(folder: MediaFolder, dataTransfer: DataTransfer): void;
}

interface MediaGridProps {
  readonly folders: readonly { readonly folder: MediaFolder; readonly itemCount: number }[];
  readonly media: readonly MediaAsset[];
  readonly pending: readonly GeneratedAsset[];
  readonly jobs: readonly ProjectJobSummary[];
  /** Fixture runtime: pending tiles offer the mock completion controls. */
  readonly fixtureControls: boolean;
  readonly projectDir: string;
  readonly thumbnailSize: number;
  readonly previewingMediaId: string | null;
  readonly flashMediaId: string | null;
  readonly replaceLabel: string | null;
  readonly attachMode: boolean;
  readonly folderNodes: readonly MediaFolderNode[];
  readonly timelineMediaIds: ReadonlySet<string>;
  readonly emptyMessage: string;
  readonly mediaActions: MediaTileActions;
  readonly folderActions: FolderTileActions;
  readonly generationActions: GenerationTileActions;
}

/** Folder tiles, generations without output, and media tiles in one auto-filling grid. */
export function MediaGrid(props: MediaGridProps) {
  const { folders, media, pending, folderActions, mediaActions } = props;
  const empty = folders.length === 0 && media.length === 0 && pending.length === 0;
  if (empty) return <p className="px-1 py-6 text-center text-[12px] text-dim">{props.emptyMessage}</p>;
  return (
    <ul
      aria-label="Media library"
      className="grid gap-x-2 gap-y-3"
      style={{ gridTemplateColumns: `repeat(auto-fill, minmax(min(${props.thumbnailSize}px, 100%), 1fr))` }}
    >
      {folders.map(({ folder, itemCount }) => (
        <FolderTile
          key={folder.id}
          folder={folder}
          itemCount={itemCount}
          onOpen={folderActions.open}
          onRename={folderActions.rename}
          onDelete={folderActions.remove}
          onDropMedia={folderActions.dropMedia}
        />
      ))}
      {pending.map((asset) => (
        <GenerationTile
          key={asset.id}
          asset={asset}
          job={props.jobs.find((job) => job.id === asset.id) ?? null}
          fixtureControls={props.fixtureControls}
          actions={props.generationActions}
        />
      ))}
      {media.length === 0 && folders.length > 0 && (
        <li className="col-span-full px-1 py-3 text-center text-[12px] text-dim">{props.emptyMessage}</li>
      )}
      {media.map((asset) => (
        <MediaTile
          key={asset.id}
          media={asset}
          projectDir={props.projectDir}
          previewing={props.previewingMediaId === asset.id}
          flashing={props.flashMediaId === asset.id}
          replaceLabel={props.replaceLabel}
          attachMode={props.attachMode}
          folders={props.folderNodes}
          onTimeline={props.timelineMediaIds.has(asset.id)}
          actions={mediaActions}
        />
      ))}
    </ul>
  );
}
