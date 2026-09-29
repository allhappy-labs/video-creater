import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, ProjectMediaSearchResult, VideoProject } from "@/lib/project";
import { openMediaFiles } from "@/lib/runtime/adapters/tauri-dialog";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore, type EditorStore } from "../store/editor-store";
import { createMediaService } from "./media-service";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (path: string) => path }));
vi.mock("@/lib/runtime/adapters/tauri-dialog", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/runtime/adapters/tauri-dialog")>()),
  openMediaFiles: vi.fn(),
}));

const request = vi.mocked(backendRequest);

/** Rejects only `command`, so unrelated backend calls keep their default (undefined) result. */
function rejectCommand(command: string, error: Error) {
  request.mockImplementation(async (operation: string) => {
    if (operation === command) throw error;
    return undefined;
  });
}

function splitProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2 };
}

function setup(project: VideoProject = fixtureProject(), projectDir = "/projects/demo") {
  const store = createEditorStore({ projectDir, project });
  const applied: ProjectAction[][] = [];
  const realApply = store.getState().applyActions;
  store.setState({
    applyActions: (actions, options) => {
      applied.push([...actions]);
      return realApply(actions, options);
    },
  });
  return { store, applied, service: createMediaService(store) };
}

function searchResult(mediaIds: string[]): ProjectMediaSearchResult {
  return {
    query: "reveal",
    limit: 20,
    visualStatus: "ready",
    spokenStatus: "ready",
    groups: { spoken: [], visual: mediaIds.map((mediaId) => ({ mediaId })), metadata: [], generated: [] },
    results: [],
    returned: mediaIds.length,
  };
}

function itemIds(store: EditorStore) {
  return store.getState().project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id));
}

describe("media service import", () => {
  beforeEach(() => {
    request.mockReset();
    // Outside the desktop app the chooser is unavailable, like the real adapter.
    vi.mocked(openMediaFiles).mockReset().mockRejectedValue(new BackendUnavailableError());
  });

  it("imports paths, replaces the project, and previews and reveals the first new media", async () => {
    const { store, service } = setup();
    const imported = { id: "media-new", relativePath: "media/new.mp4", kind: "video" as const, durationSeconds: 3, width: 1, height: 1, fps: 30 };
    const next = { ...store.getState().project, media: [...store.getState().project.media, imported] };
    request.mockResolvedValue({ project: next, imported: [imported], skipped: [{ sourcePath: "/in/notes.txt", reason: "unsupported file type" }] });

    const outcome = await service.importMediaFiles(["/in/new.mp4", "/in/notes.txt"]);

    expect(request).toHaveBeenCalledWith("import_media_to_project", {
      projectDir: "/projects/demo",
      project: expect.objectContaining({ id: "project-sample" }),
      sourcePaths: ["/in/new.mp4", "/in/notes.txt"],
    });
    expect(outcome).toEqual({ status: "imported", mediaIds: ["media-new"], notice: "notes.txt: unsupported file type" });
    expect(store.getState().project).toBe(next);
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-new" });
    expect(store.getState().revealMediaId).toBe("media-new");
  });

  it("limits the chooser to audio and leaves the preview alone for audio-only imports", async () => {
    const { store, service } = setup();
    const imported = { id: "media-music", name: "Music", relativePath: "media/music.wav", kind: "audio" as const, durationSeconds: 30, width: null, height: null, fps: null };
    vi.mocked(openMediaFiles).mockResolvedValueOnce(["/in/music.wav"]);
    request.mockResolvedValue({ project: { ...store.getState().project, media: [imported] }, imported: [imported], skipped: [] });

    await expect(service.importMediaFiles(undefined, { audioOnly: true })).resolves.toEqual({ status: "imported", mediaIds: ["media-music"], notice: null });

    expect(openMediaFiles).toHaveBeenCalledWith({ title: "Import audio", filters: [{ name: "Audio", extensions: expect.arrayContaining(["wav", "mp3", "flac"]) }] });
    expect(store.getState().previewSource).toEqual({ kind: "timeline" });
    expect(store.getState().revealMediaId).toBeNull();
  });

  it("treats a cancelled audio chooser as cancelled", async () => {
    const { service } = setup();
    vi.mocked(openMediaFiles).mockResolvedValueOnce(null);
    await expect(service.importMediaFiles(undefined, { audioOnly: true })).resolves.toEqual({ status: "cancelled" });
    expect(request).not.toHaveBeenCalled();
  });

  it("does nothing for an empty path list", async () => {
    const { service } = setup();
    await expect(service.importMediaFiles([])).resolves.toEqual({ status: "cancelled" });
    expect(request).not.toHaveBeenCalled();
  });

  it("explains that import needs the desktop app when no backend is available", async () => {
    const { store, service } = setup();
    rejectCommand("import_media_to_project", new BackendUnavailableError());

    await expect(service.importMediaFiles(["/in/a.mp4"])).resolves.toEqual({ status: "failed", message: "Import needs the desktop app" });
    expect(store.getState().lastError).toBe("Import needs the desktop app");
  });

  it("opens the media chooser without paths and reports its unavailability outside the desktop app", async () => {
    const { store, service } = setup();
    await expect(service.importMediaFiles()).resolves.toEqual({ status: "failed", message: "Import needs the desktop app" });
    expect(request).not.toHaveBeenCalled();
    expect(store.getState().lastError).toBe("Import needs the desktop app");
  });

  it("reports other import failures verbatim", async () => {
    const { store, service } = setup();
    rejectCommand("import_media_to_project", new Error("Disk is full"));
    await expect(service.importMediaFiles(["/in/a.mp4"])).resolves.toEqual({ status: "failed", message: "Disk is full" });
    expect(store.getState().lastError).toBe("Disk is full");
  });
});

describe("media service folders and media", () => {
  it("creates a root folder with a trimmed name and a slug id", async () => {
    const { store, applied, service } = setup();
    await expect(service.createFolder("   ")).resolves.toBeNull();
    await expect(service.createFolder("  Source footage ")).resolves.toBe("folder-source-footage");
    expect(applied).toEqual([[{ type: "createMediaFolder", folder: { id: "folder-source-footage", name: "Source footage", parentId: null } }]]);
    expect(store.getState().project.mediaFolders?.map((folder) => folder.name)).toContain("Source footage");
  });

  it("renames, deletes, and assigns folders with project actions", async () => {
    const { store, applied, service } = setup();
    store.getState().setMediaFolderId("folder-audio");
    await expect(service.renameFolder("folder-audio", "  ")).resolves.toBe(false);
    await service.renameFolder("folder-audio", " Voice ");
    await service.assignFolder("media-1", null);
    await service.deleteFolder("folder-audio");
    expect(applied).toEqual([
      [{ type: "renameMediaFolder", folderId: "folder-audio", name: "Voice" }],
      [{ type: "assignMediaFolder", mediaId: "media-1", folderId: null }],
      [{ type: "deleteMediaFolder", folderId: "folder-audio" }],
    ]);
    expect(store.getState().mediaFolderId).toBeNull();
    expect(store.getState().project.media.find((media) => media.id === "media-voiceover")?.folderId).toBeNull();
  });

  it("renames media and deletes it with its clips", async () => {
    const { store, applied, service } = setup();
    store.getState().previewAsset("media-voiceover");
    await service.renameMedia("media-voiceover", " Narration ");
    await expect(service.deleteMedia("media-voiceover")).resolves.toBe(true);
    expect(applied).toEqual([
      [{ type: "renameMedia", mediaId: "media-voiceover", name: "Narration" }],
      [{ type: "deleteMedia", mediaIds: ["media-voiceover"] }],
    ]);
    const { project, previewSource } = store.getState();
    expect(project.media.some((media) => media.id === "media-voiceover")).toBe(false);
    expect(project.timeline.tracks.flatMap((track) => track.items).some((item) => item.source.type === "media" && item.source.mediaId === "media-voiceover")).toBe(false);
    expect(previewSource).toEqual({ kind: "timeline" });
  });

  it("blocks deleting media that a generation uses", async () => {
    const { store, applied, service } = setup();
    await expect(service.deleteMedia("media-1")).resolves.toBe(false);
    expect(applied).toEqual([]);
    expect(store.getState().lastError).toBe("This media is used by a generated asset, so it can't be deleted.");
  });
});

describe("media service search and matte", () => {
  beforeEach(() => request.mockReset());

  it("searches the index with the trimmed query, limit and scope", async () => {
    const { service } = setup();
    request.mockResolvedValue(searchResult(["media-1"]));
    const outcome = await service.searchMedia("  reveal ", "visual");
    expect(request).toHaveBeenCalledWith("search_project_media", { projectDir: "/projects/demo", query: "reveal", limit: 20, scope: "visual" });
    expect(outcome).toEqual({ status: "indexed", result: searchResult(["media-1"]) });
  });

  it("falls back to local matching when the index is unavailable", async () => {
    const { service } = setup();
    rejectCommand("search_project_media", new BackendUnavailableError());
    await expect(service.searchMedia("reveal", "both")).resolves.toEqual({ status: "local", message: "Search index unavailable" });
    await expect(setup(fixtureProject(), "").service.searchMedia("reveal", "both")).resolves.toEqual({ status: "local", message: null });
  });

  it("rebuilds the index and reports the result", async () => {
    const { service } = setup();
    request.mockResolvedValueOnce({ projectId: "p", indexPath: "i", mediaCount: 1, transcriptCount: 0, generatedAssetCount: 0 });
    await expect(service.rebuildIndex()).resolves.toEqual({ status: "rebuilt", message: "Search index rebuilt" });
    expect(request).toHaveBeenCalledWith("rebuild_project_search_index", { projectDir: "/projects/demo" });
    request.mockRejectedValueOnce(new Error("locked"));
    await expect(service.rebuildIndex()).resolves.toEqual({ status: "failed", message: "Search index rebuild failed" });
  });

  it("requires a saved split project before creating a matte", async () => {
    await expect(setup().service.createMatte({ hex: "#112233", aspectRatio: "16:9" }, null)).rejects.toThrow(
      "Save this project before creating a matte.",
    );
    await expect(setup(splitProject(), "").service.createMatte({ hex: "#112233", aspectRatio: "16:9" }, null)).rejects.toThrow(
      "Save this project before creating a matte.",
    );
    expect(request).not.toHaveBeenCalled();
  });

  it("creates a matte in the open folder and reveals it", async () => {
    const { store, service } = setup(splitProject());
    const matte = { id: "media-matte", relativePath: "media/matte.png", kind: "image" as const, durationSeconds: 0, width: 1920, height: 1080, fps: null };
    const next = { ...store.getState().project, media: [...store.getState().project.media, matte] };
    request.mockResolvedValue({ project: next, media: matte });
    await service.createMatte({ hex: "#112233", aspectRatio: "Project" }, "folder-source");
    expect(request).toHaveBeenCalledWith("create_matte_in_split_project_folder", {
      projectDir: "/projects/demo",
      request: { hex: "#112233", aspectRatio: "Project", folderId: "folder-source" },
    });
    expect(store.getState().project).toBe(next);
    expect(store.getState().revealMediaId).toBe("media-matte");
  });
});

describe("media service replace and handoff", () => {
  it("replaces a clip with other media as one remove-and-add batch at the same start", async () => {
    const project = fixtureProject();
    const target = project.timeline.tracks.find((track) => track.kind === "video")?.items[0];
    if (!target) throw new Error("fixture video clip");
    const { store, applied, service } = setup(project);
    store.getState().setReplaceTargetItemId(target.id);

    await expect(service.replaceItemWithMedia(target.id, "sample-generated-output")).resolves.toBe(true);

    expect(applied).toHaveLength(1);
    const [remove, add] = applied[0] ?? [];
    expect(remove).toEqual({ type: "removeItems", itemIds: [target.id] });
    expect(add).toMatchObject({
      type: "addItems",
      items: [{ startSeconds: target.startSeconds, source: { type: "media", mediaId: "sample-generated-output" } }],
    });
    expect(itemIds(store)).not.toContain(target.id);
    expect(store.getState().replaceTargetItemId).toBeNull();
  });

  it("uses the generated output replacement for generated media in a saved project", async () => {
    const project = splitProject();
    const target = project.timeline.tracks.find((track) => track.kind === "video")?.items[0];
    if (!target) throw new Error("fixture video clip");
    const { store, service } = setup(project);
    const spy = vi.fn(async () => store.getState().project);
    store.setState({ applyActions: spy });
    await service.replaceItemWithMedia(target.id, "sample-generated-output");
    expect(spy).toHaveBeenCalledWith([
      { type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: target.id, mediaId: "sample-generated-output" } },
    ]);
  });

  it("blocks replacing an audio clip with visual media", async () => {
    const project = fixtureProject();
    const target = project.timeline.tracks.find((track) => track.kind === "audio")?.items[0];
    if (!target) throw new Error("fixture audio clip");
    const { store, applied, service } = setup(project);
    await expect(service.replaceItemWithMedia(target.id, "media-1")).resolves.toBe(false);
    expect(applied).toEqual([]);
    expect(store.getState().lastError).toBe("Choose audio to replace an audio clip.");
  });

  it("hands Organize with AI to the AI tab", () => {
    const { store, service } = setup();
    store.getState().openSheet("media");
    service.organizeWithAi();
    expect(store.getState().pendingAgentRequest).toMatchObject({ itemIds: [], prompt: expect.stringContaining("Organize the current project media") });
    expect(store.getState()).toMatchObject({ activeTab: "ai", openSheetId: "ai" });
  });
});
