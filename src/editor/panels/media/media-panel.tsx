import { Download, Sparkles, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { flattenMediaFolderNodes, mediaFolderTree } from "@/lib/media/folder-tree";
import type { MediaSort } from "@/lib/media/media-filters";
import type { MediaAsset, ProjectMediaSearchScope } from "@/lib/project";
import { useGenerationService } from "../../services/generation-service";
import { useMediaService } from "../../services/media-service";
import { useRuntimeMode } from "../../services/use-runtime-mode";
import { uploadBrowserFile } from "@/lib/runtime/adapters/browser-file-upload";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { useTimelineCommands } from "../../timeline/timeline-commands";
import { GenerateView } from "../generate/generate-view";
import { DropOverlay, ImportDropZone, useMediaFileDrops } from "./import-drop-zone";
import { MediaBrowseBar } from "./media-browse-bar";
import {
  childFolders,
  droppedMediaId,
  existingFolderId,
  folderBreadcrumb,
  folderItemCount,
  mediaClips,
  mediaCountLabel,
  pendingGenerations,
  visibleMedia,
  type ThumbnailSize,
} from "./media-browser";
import type { GenerationTileActions } from "./generation-tile";
import { MediaGrid, type FolderTileActions } from "./media-grid";
import { MediaPanelDialogs, type MediaPanelDialog } from "./media-panel-dialogs";
import type { MediaTileActions } from "./media-tile";
import { MediaToolbar } from "./media-toolbar";
import { AttachBanner, ReplaceBanner } from "./replace-banner";
import { useIndexedSearch } from "./use-indexed-search";
import { useRevealMedia } from "./use-reveal-media";

const actionButtonClass =
  "flex h-9 items-center justify-center gap-2 rounded-control text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50";

/** The Media tab: import and generate, filters, folders, search, and the media grid. */
export function MediaPanel() {
  const store = useEditorStoreApi();
  const service = useMediaService();
  const generation = useGenerationService();
  const runtimeMode = useRuntimeMode();
  const fixtureControls = runtimeMode === "fixture";
  const commands = useTimelineCommands();
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const filter = useEditorStore((state) => state.mediaFilter);
  const search = useEditorStore((state) => state.mediaSearch);
  const storedFolderId = useEditorStore((state) => state.mediaFolderId);
  const replaceTargetItemId = useEditorStore((state) => state.replaceTargetItemId);
  const attachMode = useEditorStore((state) => state.mediaAttachMode);
  const previewSource = useEditorStore((state) => state.previewSource);
  const generateView = useEditorStore((state) => state.generateView);

  const [sort, setSort] = useState<MediaSort>("dateAdded");
  const [thumbnailSize, setThumbnailSize] = useState<ThumbnailSize>(80);
  const [smartEnabled, setSmartEnabled] = useState(false);
  const [scope, setScope] = useState<ProjectMediaSearchScope>("both");
  const [importing, setImporting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [rebuilding, setRebuilding] = useState(false);
  const [rebuildMessage, setRebuildMessage] = useState<string | null>(null);
  const [dialog, setDialog] = useState<MediaPanelDialog | null>(null);
  const [replaceError, setReplaceError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const moreButtonRef = useRef<HTMLButtonElement>(null);
  const browserFileInputRef = useRef<HTMLInputElement>(null);
  const uploadAbortRef = useRef<AbortController | null>(null);
  const retryFilesRef = useRef<readonly File[] | null>(null);
  const [uploadProgress, setUploadProgress] = useState<number | null>(null);

  useEffect(() => () => uploadAbortRef.current?.abort(), []);

  const folderId = existingFolderId(project, storedFolderId);
  const indexed = useIndexedSearch(service, smartEnabled, search, scope);
  const media = useMemo(
    () => visibleMedia(project, { filter, folderId, search, sort, scope, indexedMediaIds: indexed.mediaIds }),
    [project, filter, folderId, search, sort, scope, indexed.mediaIds],
  );
  const flashMediaId = useRevealMedia(listRef, media);
  const folderNodes = useMemo(() => flattenMediaFolderNodes(mediaFolderTree(project.mediaFolders ?? [])), [project.mediaFolders]);
  const timelineMediaIds = useMemo(
    () => new Set(project.timeline.tracks.flatMap((track) => track.items.flatMap((item) => (item.source.type === "media" ? [item.source.mediaId] : [])))),
    [project.timeline],
  );
  const replaceTarget = replaceTargetItemId
    ? project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === replaceTargetItemId) ?? null
    : null;

  async function importFiles(paths?: readonly string[], files?: readonly File[]) {
    setImporting(true);
    setNotice(null);
    if (runtimeMode === "browser" && files?.length) {
      const controller = new AbortController();
      uploadAbortRef.current = controller;
      retryFilesRef.current = files;
      try {
        for (const file of files) {
          const current = store.getState();
          const result = await uploadBrowserFile(file, {
            projectId: current.projectDir,
            expectedRevision: current.project.contentRevision ?? 0,
            onProgress: setUploadProgress,
            signal: controller.signal,
          });
          current.replaceProject(result.project);
        }
        retryFilesRef.current = null;
        setNotice(null);
      } catch (error) {
        setNotice(error instanceof DOMException && error.name === "AbortError"
          ? "Upload cancelled."
          : error instanceof Error ? error.message : "Upload failed.");
      } finally {
        if (uploadAbortRef.current === controller) uploadAbortRef.current = null;
        setUploadProgress(null);
        setImporting(false);
      }
      return;
    }
    const outcome = await service.importMediaFiles(paths);
    setImporting(false);
    if (outcome.status === "imported") setNotice(outcome.notice);
    else if (outcome.status === "failed") setNotice(outcome.message);
  }

  const dropActive = useMediaFileDrops(rootRef, (paths, files) => void importFiles(paths, files));

  function chooseFiles() {
    if (runtimeMode === "browser" && importing) uploadAbortRef.current?.abort();
    else if (runtimeMode === "browser") browserFileInputRef.current?.click();
    else void importFiles();
  }

  async function rebuildIndex() {
    if (rebuilding) return;
    setRebuilding(true);
    setRebuildMessage(null);
    const outcome = await service.rebuildIndex();
    setRebuildMessage(outcome.message);
    if (outcome.status === "rebuilt") indexed.refresh();
    setRebuilding(false);
  }

  function moveDropped(targetFolderId: string | null, dataTransfer: DataTransfer) {
    const mediaId = droppedMediaId(store.getState().project, dataTransfer);
    const current = project.media.find((asset) => asset.id === mediaId);
    if (mediaId && current && (current.folderId ?? null) !== targetFolderId) void service.assignFolder(mediaId, targetFolderId);
  }

  const mediaActions: MediaTileActions = {
    activate(asset: MediaAsset) {
      if (attachMode) {
        store.getState().attachAgentMedia(asset.id);
        return;
      }
      if (!replaceTarget) {
        store.getState().previewAsset(asset.id);
        return;
      }
      setReplaceError(null);
      void service.replaceItemWithMedia(replaceTarget.id, asset.id).then((replaced) => {
        if (!replaced) setReplaceError(store.getState().lastError);
      });
    },
    insert(asset) {
      setNotice(null);
      void commands.insertAssetAtPlayhead({ kind: "media", id: asset.id }).then((inserted) => {
        if (!inserted) setNotice(store.getState().lastError);
      });
    },
    rename: (asset) => setDialog({ kind: "renameMedia", media: asset }),
    move: (asset, targetFolderId) => void service.assignFolder(asset.id, targetFolderId),
    revealOnTimeline(asset) {
      const [clip] = mediaClips(store.getState().project, asset.id);
      if (!clip) return;
      const state = store.getState();
      state.previewTimeline();
      state.selectItems([clip.id]);
      state.seek(clip.startSeconds);
    },
    remove: (asset) => setDialog({ kind: "deleteMedia", media: asset }),
  };

  const folderActions: FolderTileActions = {
    open: (folder) => store.getState().setMediaFolderId(folder.id),
    rename: (folder) => setDialog({ kind: "renameFolder", folder }),
    remove: (folder) => setDialog({ kind: "deleteFolder", folder }),
    dropMedia: (folder, dataTransfer) => moveDropped(folder.id, dataTransfer),
  };

  const generationActions: GenerationTileActions = {
    rerun: (asset) => void generation.rerun(asset.id),
    retryDownload: (asset) => void generation.retryDownload(asset.id),
    cancel: (asset) => void generation.cancelGeneration(asset.id),
    completeMock: (asset) => void generation.completeMock(asset.id),
    failMock: (asset) => void generation.cancelGeneration(asset.id),
  };

  if (generateView?.open && generateView.mode !== "audio") {
    return (
      <GenerateView
        tab="media"
        initialMode={generateView.mode}
        targetFolderId={folderId}
        placementIntent={generateView.placementIntent}
        onClose={() => store.getState().setGenerateView(null)}
      />
    );
  }

  const libraryEmpty = project.media.length === 0 && (project.mediaFolders ?? []).length === 0;
  const emptyMessage = search.trim()
    ? folderId
      ? "No media matches this folder"
      : "No media matches this search"
    : folderId && folderItemCount(project, folderId) === 0
      ? "Folder is empty"
      : "No media matches the active filters";

  return (
    <div ref={rootRef} className="relative flex min-h-full flex-col gap-3 p-3">
      {runtimeMode === "browser" ? (
        <input
          ref={browserFileInputRef}
          aria-label="Choose media files"
          className="sr-only"
          type="file"
          multiple
          accept="video/*,audio/*,image/*"
          onChange={(event) => {
            const files = Array.from(event.currentTarget.files ?? []);
            event.currentTarget.value = "";
            if (files.length) void importFiles([], files);
          }}
        />
      ) : null}
      {dropActive && <DropOverlay />}
      <div className="grid grid-cols-2 gap-2">
        <button
          type="button"
          aria-label={runtimeMode === "browser" && importing
            ? `Cancel upload${uploadProgress === null ? "" : `, ${Math.round(uploadProgress * 100)}%`}`
            : "Import media"}
          disabled={importing && runtimeMode !== "browser"}
          onClick={chooseFiles}
          className={`${actionButtonClass} bg-primary text-primary-foreground hover:bg-primary/90`}
        >
          {runtimeMode === "browser" && importing
            ? <X className="h-4 w-4" aria-hidden />
            : <Download className="h-4 w-4" aria-hidden />}
          {runtimeMode === "browser" && importing && uploadProgress !== null
            ? `Cancel ${Math.round(uploadProgress * 100)}%`
            : runtimeMode === "browser" && importing ? "Cancel upload"
            : importing && uploadProgress !== null
            ? `Uploading ${Math.round(uploadProgress * 100)}%`
            : importing ? "Importing" : "Import"}
        </button>
        <button
          type="button"
          onClick={() => store.getState().setGenerateView({ open: true, mode: "video" })}
          className={`${actionButtonClass} bg-raised text-foreground hover:bg-hover`}
        >
          <Sparkles className="h-4 w-4" aria-hidden />
          Generate
        </button>
      </div>
      {attachMode && <AttachBanner onCancel={() => store.getState().cancelAgentMediaAttach()} />}
      {replaceTarget && !attachMode && (
        <ReplaceBanner
          label={replaceTarget.label}
          error={replaceError}
          onCancel={() => {
            setReplaceError(null);
            store.getState().setReplaceTargetItemId(null);
          }}
        />
      )}
      {notice && (
        <div role="status" className="flex items-start gap-2 rounded-control bg-raised px-2.5 py-2 text-[12px] text-warning">
          <p className="min-w-0 flex-1 break-words">{notice}</p>
          {runtimeMode === "browser" && retryFilesRef.current ? (
            <button
              type="button"
              disabled={importing}
              onClick={() => {
                const files = retryFilesRef.current;
                if (files) void importFiles([], files);
              }}
              className="h-6 shrink-0 rounded-control px-2 font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
            >
              Retry upload
            </button>
          ) : null}
          <button type="button" aria-label="Dismiss notice" onClick={() => setNotice(null)} className="grid h-5 w-5 shrink-0 place-items-center rounded text-muted-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
            <X className="h-3.5 w-3.5" aria-hidden />
          </button>
        </div>
      )}
      {libraryEmpty ? (
        <ImportDropZone importing={importing} onImport={chooseFiles} />
      ) : (
        <>
          <MediaToolbar
            filter={filter}
            search={search}
            smart={{ enabled: smartEnabled, scope, statusLabels: indexed.statusLabels, errorMessage: indexed.errorMessage, needsRebuild: indexed.needsRebuild, rebuilding, rebuildMessage }}
            onFilterChange={(value) => store.getState().setMediaFilter(value)}
            onSearchChange={(value) => store.getState().setMediaSearch(value)}
            onSmartChange={(enabled) => {
              setSmartEnabled(enabled);
              setScope("both");
            }}
            onScopeChange={setScope}
            onRebuild={() => void rebuildIndex()}
          />
          <MediaBrowseBar
            breadcrumb={folderBreadcrumb(project, folderId)}
            countLabel={mediaCountLabel(project, folderId, search, media.length)}
            sort={sort}
            thumbnailSize={thumbnailSize}
            rebuilding={rebuilding}
            moreButtonRef={moreButtonRef}
            onNavigate={(id) => store.getState().setMediaFolderId(id)}
            onDropMedia={moveDropped}
            onSortChange={setSort}
            onThumbnailSizeChange={setThumbnailSize}
            onNewFolder={() => setDialog({ kind: "newFolder" })}
            onCreateMatte={() => setDialog({ kind: "matte" })}
            onOrganize={() => service.organizeWithAi()}
            onRebuildIndex={() => void rebuildIndex()}
          />
          <div ref={listRef}>
            <MediaGrid
              // Search results already include nested folders' media, so folder tiles step aside.
              folders={search.trim() ? [] : childFolders(project, folderId).map((folder) => ({ folder, itemCount: folderItemCount(project, folder.id) }))}
              media={media}
              pending={pendingGenerations(project, filter, search, folderId)}
              jobs={project.jobs}
              fixtureControls={fixtureControls}
              projectDir={projectDir}
              thumbnailSize={thumbnailSize}
              previewingMediaId={previewSource.kind === "asset" ? previewSource.mediaId : null}
              flashMediaId={flashMediaId}
              replaceLabel={replaceTarget?.label ?? null}
              attachMode={attachMode}
              folderNodes={folderNodes}
              timelineMediaIds={timelineMediaIds}
              emptyMessage={emptyMessage}
              mediaActions={mediaActions}
              folderActions={folderActions}
              generationActions={generationActions}
            />
          </div>
        </>
      )}
      <MediaPanelDialogs dialog={dialog} service={service} folderId={folderId} moreButtonRef={moreButtonRef} onClose={() => setDialog(null)} />
    </div>
  );
}
