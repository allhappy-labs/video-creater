import { filenameFromPath } from "../../media/names";
import { mediaMatchesSearch } from "../../media/search";
import type {
  ImportMediaResult,
  MediaAsset,
  MediaKind,
  PreparedProjectPreview,
  ProjectAction,
  ProjectActionWriteResult,
  ProjectMediaSearchResult,
  ProjectMediaSearchScope,
  ProjectSearchIndexRebuildReport,
  TimelineFilmstripReport,
  VideoProject,
} from "../../project";
import { shaderBackgroundTemplateCatalog } from "../../shader-background-templates";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import { fixtureMediaChooserOperation } from "../adapters/tauri-dialog";
import { fixtureWriteReport, type FixtureProjectStore } from "./fixture-project-store";

/**
 * DEV-only project folder handlers over the shared fixture project store: open, save, reload and
 * edit the folder, import media, and answer the preview, filmstrip, background and search commands
 * the editor makes while the acceptance flows run.
 *
 * The media chooser (`fixtureMediaChooserOperation`) picks the fixture's `preview.webm`, and import
 * resolves any path by file name: `preview.webm` and `preview-frame.png` carry their real metadata and
 * other supported names a plausible default. The e2e media route serves those files for every
 * `/__editor-fixture-media/*` URL, so imported tiles and clips play.
 */

/** The file the fixture media chooser selects. */
export const fixtureImportFile = "/fixtures/preview.webm";

type ImportMetadata = Pick<MediaAsset, "kind" | "durationSeconds" | "width" | "height" | "fps">;

/** Metadata of the files under e2e/fixtures, as the native probe reads them. */
const knownFixtureFiles: Readonly<Record<string, ImportMetadata>> = {
  "preview.webm": { kind: "video", durationSeconds: 0.999, width: 1920, height: 1080, fps: 30 },
  "preview-frame.png": { kind: "image", durationSeconds: 0, width: 320, height: 180, fps: null },
};

/** Rust `importable_media_kind` (Lottie JSON aside, which needs the file's contents). */
const extensionsByKind: readonly (readonly [MediaKind, readonly string[]])[] = [
  ["video", ["mp4", "mov", "m4v", "webm", "mkv", "avi", "flv", "ogv"]],
  ["audio", ["wav", "mp3", "m4a", "aac", "aiff", "aifc", "flac", "ogg"]],
  ["image", ["png", "jpg", "jpeg", "webp", "tiff", "tif", "heic", "heif"]],
];

function importableKind(path: string): MediaKind | null {
  const extension = extensionOf(path);
  return extensionsByKind.find(([, extensions]) => extensions.includes(extension))?.[0] ?? null;
}

const defaultMetadata: Readonly<Record<MediaKind, ImportMetadata>> = {
  video: { kind: "video", durationSeconds: 4, width: 1920, height: 1080, fps: 30 },
  audio: { kind: "audio", durationSeconds: 4, width: null, height: null, fps: null },
  image: { kind: "image", durationSeconds: 0, width: 1920, height: 1080, fps: null },
  lottie: { kind: "lottie", durationSeconds: 2, width: 1920, height: 1080, fps: 30 },
  generated: { kind: "generated", durationSeconds: 4, width: 1920, height: 1080, fps: 30 },
};

function extensionOf(path: string): string {
  const name = filenameFromPath(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** Rust `sanitize_file_stem`. */
function fileStem(path: string): string {
  const name = filenameFromPath(path);
  const dot = name.lastIndexOf(".");
  const stem = (dot > 0 ? name.slice(0, dot) : name)
    .replace(/[^A-Za-z0-9_-]/g, "-")
    .toLowerCase()
    .split("-")
    .filter(Boolean)
    .join("-");
  return stem || "media";
}

/** The next deterministic import id: `media-fixture-import-<n>`, counting the folder's earlier imports. */
function nextImportId(project: VideoProject, taken: readonly MediaAsset[]): string {
  const prefix = "media-fixture-import-";
  const used = [...project.media, ...taken].map((media) => media.id).filter((id) => id.startsWith(prefix)).length;
  return `${prefix}${(used + 1).toString()}`;
}

/** The native import's file stem for a display name: lowercase letters, digits, `-` and `_`. */
function sanitizedStem(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .split("-")
    .filter(Boolean)
    .join("-");
}

function importMedia(store: FixtureProjectStore, input: Record<string, unknown>): ImportMediaResult {
  const { sourcePaths, names = {} } = input as { sourcePaths?: string[]; names?: Record<string, string> };
  if (!sourcePaths || sourcePaths.length === 0) throw "no source files were selected";
  const base = store.require();
  const imported: MediaAsset[] = [];
  const skipped: ImportMediaResult["skipped"] = [];
  for (const sourcePath of sourcePaths) {
    const kind = importableKind(sourcePath);
    if (!kind) {
      skipped.push({ sourcePath, reason: "unsupported media extension" });
      continue;
    }
    const metadata = knownFixtureFiles[filenameFromPath(sourcePath)] ?? defaultMetadata[kind];
    const id = nextImportId(base, imported);
    // Like the native import, a named source (a saved range) takes its display name and a stem from it.
    const displayName = names[sourcePath]?.trim().slice(0, 120);
    const stem = (displayName && sanitizedStem(displayName)) || fileStem(sourcePath);
    imported.push({ id, name: displayName || fileStem(sourcePath), relativePath: `media/${id}-${stem}.${extensionOf(sourcePath)}`, ...metadata, folderId: null });
  }
  if (imported.length === 0) throw "no selected files can be imported";
  const project = store.write({ ...base, media: [...base.media, ...imported] });
  return { project, imported, skipped };
}

/** The chooser picks the fixture video when the filters accept it, and reads as cancelled otherwise. */
function chooseMediaFiles(input: Record<string, unknown>): string[] | null {
  const { filters } = input as { filters?: { extensions: string[] }[] };
  const extension = extensionOf(fixtureImportFile);
  const accepted = !filters || filters.some((filter) => filter.extensions.includes(extension));
  return accepted ? [fixtureImportFile] : null;
}

function folderLabel(project: VideoProject, folderId: string | null | undefined): string | null {
  return project.mediaFolders?.find((folder) => folder.id === folderId)?.name ?? null;
}

/** Local matching in the native result shape: metadata, spoken words and generations, no visual index. */
function searchMedia(store: FixtureProjectStore, input: Record<string, unknown>): ProjectMediaSearchResult {
  const { query = "", limit = 20, scope = "both" } = input as { query?: string; limit?: number; scope?: ProjectMediaSearchScope };
  const project = store.require();
  const needle = query.trim().toLocaleLowerCase();
  const metadata =
    scope === "both" || scope === "metadata"
      ? project.media
          .filter((media) => mediaMatchesSearch(media, folderLabel(project, media.folderId), needle))
          .map((media) => ({ mediaId: media.id, name: media.name ?? null, relativePath: media.relativePath, kind: media.kind }))
      : [];
  const spoken =
    scope === "both" || scope === "spoken"
      ? project.transcripts.flatMap((transcript) =>
          transcript.words
            .filter((word) => needle.length > 0 && word.text.toLocaleLowerCase().includes(needle))
            .map((word) => ({ mediaId: transcript.mediaId, transcriptId: transcript.id, text: word.text, startSeconds: word.startSeconds, endSeconds: word.endSeconds })),
        )
      : [];
  const generated =
    scope === "both" || scope === "generated"
      ? project.generatedAssets
          .filter((asset) => needle.length > 0 && [asset.id, asset.name, asset.prompt, asset.model.id].some((value) => value?.toLocaleLowerCase().includes(needle)))
          .map((asset) => ({ assetId: asset.id, name: asset.name ?? null, mediaIds: asset.outputs.map((output) => output.mediaId) }))
      : [];
  const results = [
    ...spoken.map((result) => ({ kind: "spoken" as const, result })),
    ...metadata.map((result) => ({ kind: "metadata" as const, result })),
    ...generated.map((result) => ({ kind: "generated" as const, result })),
  ].slice(0, limit);
  return {
    query,
    limit,
    visualStatus: "notInstalled",
    spokenStatus: project.transcripts.length > 0 ? "ready" : "noTranscripts",
    groups: { spoken, visual: [], metadata, generated },
    results,
    returned: results.length,
    indexStatus: { stored: true, source: "stored", schemaVersion: 1, projectUpdatedAt: project.updatedAt },
  };
}

function writeResult(project: VideoProject): ProjectActionWriteResult {
  return { project, report: fixtureWriteReport() };
}

export function projectFixtureOperations(store: FixtureProjectStore): ReadonlyMap<string, FixtureOperationHandler> {
  return new Map<string, FixtureOperationHandler>([
    ["materialize_sample_project_media", () => undefined],
    [
      "save_split_project_to_folder",
      (input) => {
        const { project, projectDir = "", expectedRevision } = input as { project?: VideoProject; projectDir?: string; expectedRevision?: number };
        if (!project) throw "missing field `project`";
        return writeResult(store.save(project, projectDir, expectedRevision));
      },
    ],
    ["load_split_project_from_folder", () => store.reload()],
    ["apply_project_actions_to_split_project_folder", (input) => writeResult(store.apply((input as { actions: ProjectAction[] }).actions))],
    ["apply_project_action_to_split_project_folder", (input) => writeResult(store.apply([(input as { action: ProjectAction }).action]))],
    [
      "update_project_settings_in_split_project_folder",
      (input) => {
        const { name, renderSettings } = input as Pick<VideoProject, "name" | "renderSettings">;
        return writeResult(store.write({ ...store.require(), name, renderSettings }));
      },
    ],
    [fixtureMediaChooserOperation, chooseMediaFiles],
    ["import_media_to_project", (input) => importMedia(store, input)],
    [
      "prepare_project_preview",
      (input) => {
        const result: PreparedProjectPreview = { project: (input as { project: VideoProject }).project, reports: [], frameSequences: [] };
        return result;
      },
    ],
    [
      "cache_timeline_filmstrip_in_split_project_folder",
      (input) => {
        const request = input as Omit<TimelineFilmstripReport, "sourceFingerprint" | "cacheKey" | "cacheHit" | "samplingPolicy" | "frames">;
        const cacheKey = `fixture-${request.mediaId}-${request.zoomBucket.toString()}-${request.heightBucket.toString()}`;
        const report: TimelineFilmstripReport = {
          mediaId: request.mediaId,
          sourceFingerprint: "fixture",
          cacheKey,
          cacheHit: true,
          sourceIn: request.sourceIn,
          sourceOut: request.sourceOut,
          speed: request.speed,
          zoomBucket: request.zoomBucket,
          heightBucket: request.heightBucket,
          samplingPolicy: "fixture",
          frames: [],
        };
        return report;
      },
    ],
    ["list_shader_background_templates", () => structuredClone(shaderBackgroundTemplateCatalog)],
    ["search_project_media", (input) => searchMedia(store, input)],
    [
      "rebuild_project_search_index",
      () => {
        const project = store.require();
        const report: ProjectSearchIndexRebuildReport = {
          projectId: project.id,
          indexPath: ".video-creater/search-index.json",
          mediaCount: project.media.length,
          transcriptCount: project.transcripts.length,
          generatedAssetCount: project.generatedAssets.length,
        };
        return report;
      },
    ],
  ]);
}
