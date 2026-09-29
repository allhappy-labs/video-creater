import { useMemo } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import { organizeMediaPrompt } from "@/lib/generation/provider-rules";
import { mediaTimelineItem } from "@/lib/generation/timeline-placement";
import { formatSkippedMediaImports, openMediaFilePaths } from "@/lib/media-import";
import { mediaFolderIdForName } from "@/lib/media/folder-tree";
import type { MatteCreateInput } from "@/lib/media/matte";
import { mediaContentKind } from "@/lib/media/media-filters";
import {
  createMatteInSplitProjectFolder,
  importMediaToProject,
  rebuildProjectSearchIndex,
  searchProjectMedia,
  type MediaAsset,
  type ProjectAction,
  type ProjectMediaSearchResult,
  type ProjectMediaSearchScope,
  type VideoProject,
} from "@/lib/project";
import { openMediaFiles } from "@/lib/runtime/adapters/tauri-dialog";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { itemAllowedOnTrack } from "@/lib/timeline";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";

type MediaImportOutcome =
  | { readonly status: "imported"; readonly mediaIds: readonly string[]; readonly notice: string | null }
  | { readonly status: "cancelled" }
  | { readonly status: "failed"; readonly message: string };

interface MediaImportOptions {
  /** Limits the native chooser to audio files (the Audio tab) and leaves the preview and Media grid alone. */
  readonly audioOnly?: boolean;
}

/** Indexed results, or local matching with the status to show (null when there is no index to use). */
type MediaSearchOutcome =
  | { readonly status: "indexed"; readonly result: ProjectMediaSearchResult }
  | { readonly status: "local"; readonly message: string | null };

type RebuildIndexOutcome =
  | { readonly status: "rebuilt"; readonly message: string }
  | { readonly status: "failed"; readonly message: string };

/**
 * Store-bound Media tab operations. Edits go through `applyActions` (one undo step each); backend
 * commands that return a whole project (import, matte) replace it. Blocked edits resolve `false`
 * with the reason in `lastError`.
 */
export interface MediaService {
  /** Imports `paths`, or asks with the native chooser when absent. */
  importMediaFiles(paths?: readonly string[], options?: MediaImportOptions): Promise<MediaImportOutcome>;
  /** Creates a top-level folder; resolves its id, or null when the name is blank or the edit failed. */
  createFolder(name: string): Promise<string | null>;
  renameFolder(folderId: string, name: string): Promise<boolean>;
  deleteFolder(folderId: string): Promise<boolean>;
  assignFolder(mediaId: string, folderId: string | null): Promise<boolean>;
  renameMedia(mediaId: string, name: string): Promise<boolean>;
  /** Deletes the media with its transcripts and clips on the active timeline. */
  deleteMedia(mediaId: string): Promise<boolean>;
  searchMedia(query: string, scope: ProjectMediaSearchScope): Promise<MediaSearchOutcome>;
  rebuildIndex(): Promise<RebuildIndexOutcome>;
  /** Rejects with user-facing copy when the project isn't saved or the backend fails. */
  createMatte(input: MatteCreateInput, folderId: string | null): Promise<MediaAsset>;
  /** Replaces a timeline clip with `mediaId` at the same start (and duration where the source allows). */
  replaceItemWithMedia(itemId: string, mediaId: string): Promise<boolean>;
  /** Drafts the organize-media request for the AI tab and switches to it. */
  organizeWithAi(): void;
}

const importNeedsDesktopCopy = "Import needs the desktop app";
export const mediaInUseCopy = "This media is used by a generated asset, so it can't be deleted.";
const searchUnavailableCopy = "Search index unavailable";
const matteNeedsSaveCopy = "Save this project before creating a matte.";
const searchLimit = 20;
/** Audio formats the backend importer accepts (the audio subset of the media chooser). */
const audioImportExtensions = ["wav", "mp3", "m4a", "aac", "aiff", "aifc", "flac"] as const;

async function chooseImportPaths(audioOnly: boolean): Promise<string[]> {
  if (!audioOnly) return openMediaFilePaths();
  return (await openMediaFiles({ title: "Import audio", filters: [{ name: "Audio", extensions: audioImportExtensions }] })) ?? [];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function usesSplitFolder(project: VideoProject, projectDir: string): boolean {
  return project.schemaVersion >= 2 && projectDir.trim().length > 0;
}

/** The generation that references `mediaId` (as an input or an output), which blocks deleting it. */
export function mediaUsedByGeneration(project: VideoProject, mediaId: string): boolean {
  return project.generatedAssets.some(
    (asset) =>
      asset.references.mediaIds.includes(mediaId) ||
      asset.references.firstFrameMediaId === mediaId ||
      asset.references.lastFrameMediaId === mediaId ||
      asset.outputs.some((output) => output.mediaId === mediaId),
  );
}

function completedGeneratedOutput(project: VideoProject, media: MediaAsset): boolean {
  return (
    media.kind === "generated" &&
    project.generatedAssets.some(
      (asset) => asset.status === "completed" && asset.outputs.some((output) => output.mediaId === media.id),
    )
  );
}

type ReplacePlan = { readonly actions: ProjectAction[]; readonly itemId: string } | { readonly blocked: string };

function planReplace(project: VideoProject, projectDir: string, itemId: string, mediaId: string): ReplacePlan {
  const track = project.timeline.tracks.find((candidate) => candidate.items.some((item) => item.id === itemId));
  const target = track?.items.find((item) => item.id === itemId);
  if (!track || !target) return { blocked: "That clip is no longer on the timeline." };
  const media = project.media.find((asset) => asset.id === mediaId);
  if (!media) return { blocked: "That media is no longer in the project." };
  if (track.locked) return { blocked: "Unlock the track to replace this clip." };
  const base = mediaTimelineItem(project, mediaId, target.startSeconds);
  if (!base) return { blocked: "That media can't be placed on the timeline." };
  if (!itemAllowedOnTrack(base.kind, track.kind)) {
    return { blocked: track.kind === "audio" ? "Choose audio to replace an audio clip." : "Choose a video or image to replace this clip." };
  }
  if (track.kind === "video" && completedGeneratedOutput(project, media) && usesSplitFolder(project, projectDir)) {
    return { actions: [{ type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId, mediaId } }], itemId };
  }
  // No source-replacement action exists: remove the clip and add the new one in the same batch.
  const timed = mediaContentKind(media) !== "image" && media.durationSeconds > 0;
  const durationSeconds = roundTimelineSeconds(timed ? Math.min(target.durationSeconds, media.durationSeconds) : target.durationSeconds);
  const item = { ...base, durationSeconds, properties: { ...base.properties, sourceIn: 0, sourceOut: durationSeconds } };
  return {
    actions: [
      { type: "removeItems", itemIds: [itemId] },
      { type: "addItems", targetTrackId: track.id, items: [item] },
    ],
    itemId: item.id,
  };
}

export function createMediaService(store: EditorStore): MediaService {
  const state = () => store.getState();

  function block(reason: string): false {
    state().setLastError(reason);
    return false;
  }

  async function apply(actions: ProjectAction[]): Promise<boolean> {
    return (await state().applyActions(actions)) !== null;
  }

  return {
    async importMediaFiles(paths, options) {
      const audioOnly = options?.audioOnly === true;
      try {
        const sourcePaths = paths ? [...paths] : await chooseImportPaths(audioOnly);
        if (sourcePaths.length === 0) return { status: "cancelled" };
        const { project, projectDir } = state();
        const result = await importMediaToProject({ projectDir, project, sourcePaths });
        state().replaceProject(result.project);
        state().setLastError(null);
        const mediaIds = result.imported.map((media) => media.id);
        const [first] = mediaIds;
        if (first !== undefined && !audioOnly) {
          state().previewAsset(first);
          state().setRevealMediaId(first);
        }
        return { status: "imported", mediaIds, notice: formatSkippedMediaImports(result.skipped) };
      } catch (error) {
        const message = isBackendUnavailableError(error) ? importNeedsDesktopCopy : errorMessage(error);
        state().setLastError(message);
        return { status: "failed", message };
      }
    },

    async createFolder(name) {
      const trimmed = name.trim();
      if (!trimmed) return null;
      const id = mediaFolderIdForName(trimmed, state().project.mediaFolders ?? []);
      return (await apply([{ type: "createMediaFolder", folder: { id, name: trimmed, parentId: null } }])) ? id : null;
    },

    async renameFolder(folderId, name) {
      const trimmed = name.trim();
      if (!trimmed) return false;
      return apply([{ type: "renameMediaFolder", folderId, name: trimmed }]);
    },

    async deleteFolder(folderId) {
      const applied = await apply([{ type: "deleteMediaFolder", folderId }]);
      if (applied && state().mediaFolderId === folderId) state().setMediaFolderId(null);
      return applied;
    },

    assignFolder(mediaId, folderId) {
      return apply([{ type: "assignMediaFolder", mediaId, folderId }]);
    },

    async renameMedia(mediaId, name) {
      const trimmed = name.trim();
      if (!trimmed) return false;
      return apply([{ type: "renameMedia", mediaId, name: trimmed }]);
    },

    async deleteMedia(mediaId) {
      if (mediaUsedByGeneration(state().project, mediaId)) {
        return block(mediaInUseCopy);
      }
      const applied = await apply([{ type: "deleteMedia", mediaIds: [mediaId] }]);
      const { previewSource } = state();
      if (applied && previewSource.kind === "asset" && previewSource.mediaId === mediaId) state().previewTimeline();
      return applied;
    },

    async searchMedia(query, scope) {
      const { projectDir } = state();
      if (!projectDir.trim()) return { status: "local", message: null };
      try {
        const result = await searchProjectMedia({ projectDir, query: query.trim(), limit: searchLimit, scope });
        return { status: "indexed", result };
      } catch {
        return { status: "local", message: searchUnavailableCopy };
      }
    },

    async rebuildIndex() {
      try {
        await rebuildProjectSearchIndex({ projectDir: state().projectDir });
        return { status: "rebuilt", message: "Search index rebuilt" };
      } catch {
        return { status: "failed", message: "Search index rebuild failed" };
      }
    },

    async createMatte(input, folderId) {
      const { project, projectDir } = state();
      if (!usesSplitFolder(project, projectDir)) throw new Error(matteNeedsSaveCopy);
      const result = await createMatteInSplitProjectFolder({
        projectDir,
        request: { hex: input.hex, aspectRatio: input.aspectRatio, ...(folderId ? { folderId } : {}) },
      });
      state().replaceProject(result.project);
      state().setRevealMediaId(result.media.id);
      return result.media;
    },

    async replaceItemWithMedia(itemId, mediaId) {
      const { project, projectDir } = state();
      const plan = planReplace(project, projectDir, itemId, mediaId);
      if ("blocked" in plan) return block(plan.blocked);
      if (!(await apply(plan.actions))) return false;
      state().setReplaceTargetItemId(null);
      state().selectItems([plan.itemId]);
      return true;
    },

    organizeWithAi() {
      state().requestAgent({ itemIds: [], prompt: organizeMediaPrompt, confirmSend: true });
    },
  };
}

/** The media service bound to the editor store; stable for the lifetime of the store. */
export function useMediaService(): MediaService {
  const store = useEditorStoreApi();
  return useMemo(() => createMediaService(store), [store]);
}
