import { pluralize } from "@/lib/format";
import { flattenMediaFolderNodes, mediaFolderPathLabel, mediaFolderTree } from "@/lib/media/folder-tree";
import { filterVisualMedia, sortMedia, type MediaFilter, type MediaSort } from "@/lib/media/media-filters";
import { mediaMatchesSearch, normalizeSearchText, searchScopeUsesLocalMedia } from "@/lib/media/search";
import type { GeneratedAsset, MediaAsset, MediaFolder, ProjectMediaSearchScope, VideoProject } from "@/lib/project";

/** Legacy internal drag type for moving media between folders (the asset MIME covers timeline drops). */
export const mediaIdDragMimeType = "application/x-video-creater-media-id";

export const thumbnailSizeOptions = [
  { value: 80, label: "Small" },
  { value: 110, label: "Medium" },
  { value: 150, label: "Large" },
  { value: 200, label: "Extra Large" },
] as const;

export type ThumbnailSize = (typeof thumbnailSizeOptions)[number]["value"];

export const searchScopeOptions: readonly { readonly value: ProjectMediaSearchScope; readonly label: string }[] = [
  { value: "both", label: "All" },
  { value: "visual", label: "Visual" },
  { value: "spoken", label: "Transcript" },
  { value: "metadata", label: "Metadata" },
  { value: "generated", label: "Generated" },
];

function folders(project: VideoProject): MediaFolder[] {
  return project.mediaFolders ?? [];
}

/** The open folder, or null when it no longer exists (the legacy auto-reset to the project root). */
export function existingFolderId(project: VideoProject, folderId: string | null): string | null {
  return folderId !== null && folders(project).some((folder) => folder.id === folderId) ? folderId : null;
}

/** Folders from the root to `folderId`, following safe (acyclic, existing) parents. */
export function folderBreadcrumb(project: VideoProject, folderId: string | null): MediaFolder[] {
  if (folderId === null) return [];
  const byId = new Map(folders(project).map((folder) => [folder.id, folder]));
  const path: MediaFolder[] = [];
  const seen = new Set<string>();
  let current = byId.get(folderId);
  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    path.unshift(current);
    current = current.parentId ? byId.get(current.parentId) : undefined;
  }
  return path;
}

/** Direct child folders of `parentId` (null: top-level folders, including orphans). */
export function childFolders(project: VideoProject, parentId: string | null): MediaFolder[] {
  const tree = mediaFolderTree(folders(project));
  if (parentId === null) return tree.map((node) => node.folder);
  const node = flattenMediaFolderNodes(tree).find((candidate) => candidate.folder.id === parentId);
  return node ? node.children.map((child) => child.folder) : [];
}

/** `folderId` and every folder nested under it. */
function folderSubtreeIds(project: VideoProject, folderId: string): Set<string> {
  const tree = flattenMediaFolderNodes(mediaFolderTree(folders(project)));
  const node = tree.find((candidate) => candidate.folder.id === folderId);
  const ids = new Set([folderId]);
  if (node) for (const child of flattenMediaFolderNodes(node.children)) ids.add(child.folder.id);
  return ids;
}

/** Media in the folder or its descendants (the whole library at the root). */
function mediaInScope(project: VideoProject, media: readonly MediaAsset[], folderId: string | null): MediaAsset[] {
  if (folderId === null) return [...media];
  const ids = folderSubtreeIds(project, folderId);
  return media.filter((asset) => asset.folderId != null && ids.has(asset.folderId));
}

/** Recursive item count shown on folder tiles and in the delete confirmation. */
export function folderItemCount(project: VideoProject, folderId: string | null): number {
  return mediaInScope(project, project.media, folderId).length;
}

export interface MediaBrowserQuery {
  readonly filter: MediaFilter;
  readonly folderId: string | null;
  readonly search: string;
  readonly sort: MediaSort;
  readonly scope: ProjectMediaSearchScope;
  /** Media ids returned by indexed search; null searches locally only. */
  readonly indexedMediaIds: ReadonlySet<string> | null;
}

/**
 * The grid's media: the chip filter, then the open folder (recursive), then the search. Local
 * matching applies without indexed results or when the scope covers metadata; indexed hits are
 * always kept. Sorted by the sort menu.
 */
export function visibleMedia(project: VideoProject, query: MediaBrowserQuery): MediaAsset[] {
  const scoped = mediaInScope(project, filterVisualMedia(project, query.filter), query.folderId);
  const text = normalizeSearchText(query.search);
  const matches = text
    ? scoped.filter((asset) => {
        if (query.indexedMediaIds?.has(asset.id)) return true;
        if (query.indexedMediaIds !== null && !searchScopeUsesLocalMedia(query.scope)) return false;
        return mediaMatchesSearch(asset, mediaFolderPathLabel(folders(project), asset.folderId), text);
      })
    : scoped;
  return sortMedia(matches, query.sort);
}

/** "3 items", or `Showing 1 of 3 for "query"` while searching. */
export function mediaCountLabel(project: VideoProject, folderId: string | null, search: string, visibleCount: number): string {
  const total = folderItemCount(project, folderId);
  const query = search.trim();
  return query ? `Showing ${visibleCount} of ${total} for "${query}"` : pluralize(total, "item");
}

/**
 * Generations without usable output, shown as placeholder tiles in their target folder: queued or
 * running, or failed with no output file in the project (a failed download included). A failure that
 * was retried is superseded by its retry; cancelled generations leave the grid.
 */
export function pendingGenerations(project: VideoProject, filter: MediaFilter, search: string, folderId: string | null): GeneratedAsset[] {
  if (filter !== "all" && filter !== "generated") return [];
  const text = normalizeSearchText(search);
  const mediaIds = new Set(project.media.map((asset) => asset.id));
  const retried = new Set(project.generatedAssets.flatMap((asset) => (asset.retryOfAssetId ? [asset.retryOfAssetId] : [])));
  return project.generatedAssets.filter((asset) => {
    const active = asset.status === "queued" || asset.status === "running";
    const unfinished = asset.status === "failed" || (asset.status === "completed" && asset.outputs.length > 0);
    const pending = active ? asset.outputs.length === 0 : unfinished && !retried.has(asset.id) && !asset.outputs.some((output) => mediaIds.has(output.mediaId));
    return (
      pending &&
      (asset.targetFolderId ?? null) === folderId &&
      (!text || `${asset.name ?? ""} ${asset.prompt}`.toLocaleLowerCase().includes(text))
    );
  });
}

/** Clips on the active timeline that use `mediaId`, in timeline order. */
export function mediaClips(project: VideoProject, mediaId: string) {
  return project.timeline.tracks
    .flatMap((track) => track.items)
    .filter((item) => item.source.type === "media" && item.source.mediaId === mediaId)
    .sort((left, right) => left.startSeconds - right.startSeconds);
}

/** The media id in a folder-move drag, accepted only when it is still in the project. */
export function droppedMediaId(project: VideoProject, dataTransfer: DataTransfer): string | null {
  const id = (dataTransfer.getData(mediaIdDragMimeType) || dataTransfer.getData("text/plain")).trim();
  return id && project.media.some((asset) => asset.id === id) ? id : null;
}

export function isMediaMoveDrag(dataTransfer: DataTransfer): boolean {
  return Array.from(dataTransfer.types).includes(mediaIdDragMimeType);
}
