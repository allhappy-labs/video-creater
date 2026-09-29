import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { assetDragMimeType } from "../../timeline/drag-data";
import { backendRequest } from "@/lib/runtime/backend-client";
import { uploadBrowserFile } from "@/lib/runtime/adapters/browser-file-upload";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { longPressMilliseconds } from "../../shell/use-long-press";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { mediaIdDragMimeType } from "./media-browser";
import { MediaPanel } from "./media-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (path: string) => path }));
vi.mock("@/lib/runtime/adapters/browser-file-upload", () => ({ uploadBrowserFile: vi.fn() }));

beforeEach(() => {
  delete window.__EDITOR_FIXTURE_RUNTIME__;
  installRuntimeMode("desktop");
  vi.mocked(uploadBrowserFile).mockReset();
});

/** The sample project with every media item at the library root plus one still image. */
function project(): VideoProject {
  const base = fixtureProject();
  return {
    ...base,
    media: [
      ...base.media,
      { id: "media-still", name: "Poster", relativePath: "media/poster.png", kind: "image", durationSeconds: 0, width: 10, height: 10, fps: null, folderId: null },
    ],
  };
}

function renderPanel(base: VideoProject = project(), projectDir = "") {
  const rendered = renderWithEditorStore(<MediaPanel />, { project: base, projectDir });
  const batches: ProjectAction[][] = [];
  const realApply = rendered.store.getState().applyActions;
  act(() =>
    rendered.store.setState({
      applyActions: (actions, options) => {
        batches.push([...actions]);
        return realApply(actions, options);
      },
    }),
  );
  return { ...rendered, batches };
}

function mediaTiles(): string[] {
  return within(screen.getByRole("list", { name: "Media library" }))
    .queryAllByRole("button", { name: /^Preview / })
    .map((button) => button.getAttribute("aria-label")?.replace(/^Preview /, "") ?? "");
}

function tile(name: string): HTMLElement {
  const element = screen.getByRole("button", { name: `Preview ${name}` }).closest("li");
  if (!element) throw new Error(`no tile for ${name}`);
  return element;
}

async function openMoreMenu() {
  fireEvent.keyDown(screen.getByRole("button", { name: "More media actions" }), { key: "Enter" });
  return within(await screen.findByRole("menu"));
}

async function click(element: HTMLElement) {
  await act(async () => {
    fireEvent.click(element);
    await Promise.resolve();
  });
}

function dataTransfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    get types() {
      return [...data.keys()];
    },
    setData: (type: string, value: string) => data.set(type, value),
    getData: (type: string) => data.get(type) ?? "",
    effectAllowed: "none",
    dropEffect: "none",
    files: [] as unknown as FileList,
  } as unknown as DataTransfer;
}

describe("MediaPanel browsing", () => {
  beforeEach(() => window.localStorage.clear());

  it("shows folders and media, and filters with the chips", () => {
    renderPanel();
    expect(screen.getByRole("button", { name: "Open folder Source footage, 1 item" })).toBeInTheDocument();
    expect(mediaTiles()).toEqual(["input.mp4", "voiceover.m4a", "product-reveal.mp4", "Poster"]);
    fireEvent.click(screen.getByRole("button", { name: "Images" }));
    expect(mediaTiles()).toEqual(["Poster"]);
    fireEvent.click(screen.getByRole("button", { name: "Generated" }));
    expect(mediaTiles()).toEqual(["product-reveal.mp4"]);
  });

  it("keeps tile names on one ellipsized line with the full name as the tile tooltip", () => {
    renderPanel();
    const voiceover = tile("voiceover.m4a");
    expect(voiceover).toHaveAttribute("title", "voiceover.m4a");
    const name = within(voiceover).getByText("voiceover.m4a");
    // Wrapping broke long names mid-word on narrow phone grids.
    expect(name).toHaveClass("truncate");
    expect(name.className).not.toMatch(/line-clamp|break-words|break-all/);
  });

  it("filters by search and shows the count", () => {
    renderPanel();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search project media" }), { target: { value: "voice" } });
    expect(mediaTiles()).toEqual(["voiceover.m4a"]);
    expect(screen.queryByRole("button", { name: /^Open folder / })).not.toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Folder path" })).toHaveTextContent('Showing 1 of 4 for "voice"');
  });

  it("opens folders from tiles and returns with the breadcrumb", () => {
    const { store } = renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Open folder Audio, 1 item" }));
    expect(store.getState().mediaFolderId).toBe("folder-audio");
    expect(mediaTiles()).toEqual(["voiceover.m4a"]);
    fireEvent.click(screen.getByRole("button", { name: "Project" }));
    expect(mediaTiles()).toHaveLength(4);
  });

  it("previews on click and inserts at the playhead with + or double-click", async () => {
    const { store, batches } = renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Preview Poster" }));
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-still" });
    await click(screen.getByRole("button", { name: "Add Poster at the playhead" }));
    await waitFor(() => expect(batches).toHaveLength(1));
    expect(batches[0]?.some((action) => action.type === "addItems" && action.items[0]?.source.type === "media")).toBe(true);
    // Placing the asset ends asset preview: the viewer is back on the timeline.
    await waitFor(() => expect(store.getState().previewSource).toEqual({ kind: "timeline" }));
    await act(async () => {
      fireEvent.doubleClick(screen.getByRole("button", { name: "Preview Poster" }));
      await Promise.resolve();
    });
    await waitFor(() => expect(batches).toHaveLength(2));
  });

  it("drags with the timeline asset payload and moves media into a folder on drop", async () => {
    const { store, batches } = renderPanel();
    const transfer = dataTransfer();
    fireEvent.dragStart(tile("Poster"), { dataTransfer: transfer });
    expect(JSON.parse(transfer.getData(assetDragMimeType))).toEqual({ kind: "media", id: "media-still" });
    expect(transfer.getData(mediaIdDragMimeType)).toBe("media-still");
    expect(transfer.effectAllowed).toBe("copyMove");

    const folder = screen.getByRole("button", { name: "Open folder Audio, 1 item" }).closest("li");
    if (!folder) throw new Error("folder tile");
    fireEvent.dragOver(folder, { dataTransfer: transfer });
    await act(async () => {
      fireEvent.drop(folder, { dataTransfer: transfer });
      await Promise.resolve();
    });
    await waitFor(() => expect(batches).toEqual([[{ type: "assignMediaFolder", mediaId: "media-still", folderId: "folder-audio" }]]));
    expect(store.getState().project.media.find((media) => media.id === "media-still")?.folderId).toBe("folder-audio");
  });

  it("uses smart search results and status, and falls back to local matches when the index fails", async () => {
    vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
      if (command !== "search_project_media") return undefined;
      if (input?.query === "broken") throw new Error("index locked");
      return {
        query: input?.query,
        limit: 20,
        visualStatus: "ready",
        spokenStatus: "noTranscripts",
        groups: { spoken: [], visual: [{ mediaId: "media-still" }], metadata: [], generated: [] },
        results: [],
        returned: 1,
        indexStatus: { stored: false, reason: "stale", schemaVersion: 1, projectUpdatedAt: "" },
      };
    });
    renderPanel(project(), "/projects/demo");
    fireEvent.click(screen.getByRole("button", { name: "Smart search" }));
    const search = screen.getByRole("searchbox", { name: "Search project media" });
    fireEvent.change(search, { target: { value: "sunset" } });
    await waitFor(() => expect(mediaTiles()).toEqual(["Poster"]));
    expect(vi.mocked(backendRequest)).toHaveBeenCalledWith("search_project_media", { projectDir: "/projects/demo", query: "sunset", limit: 20, scope: "both" });
    const status = screen.getByRole("status", { name: "Search index status" });
    expect(status).toHaveTextContent("Visual ready");
    expect(within(status).getByRole("button", { name: "Rebuild search index" })).toBeInTheDocument();

    fireEvent.change(search, { target: { value: "broken" } });
    expect(await screen.findByText("Search index unavailable")).toBeInTheDocument();
    expect(screen.getByText("No media matches this search")).toBeInTheDocument();
    vi.mocked(backendRequest).mockReset();
  });

  it("shows the empty import drop zone for an empty library", () => {
    renderPanel({ ...fixtureProject(), media: [], mediaFolders: [], generatedAssets: [] });
    expect(screen.getByText("No media imported")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose files" })).toBeEnabled();
  });

  it("imports files dropped on the panel and reveals the first one", async () => {
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");
    const base = project();
    const imported = { id: "media-dropped", relativePath: "media/dropped.mp4", kind: "video" as const, durationSeconds: 2, width: 1, height: 1, fps: 30 };
    vi.mocked(backendRequest).mockImplementation(async (command: string) =>
      command === "import_media_to_project" ? { project: { ...base, media: [...base.media, imported] }, imported: [imported], skipped: [] } : undefined,
    );
    const { container, store } = renderPanel(base);
    const files = [{ name: "dropped.mp4", path: "/in/dropped.mp4" }];
    await act(async () => {
      fireEvent.drop(container.firstElementChild as HTMLElement, { dataTransfer: { types: ["Files"], files, getData: () => "" } });
      await Promise.resolve();
    });
    await waitFor(() => expect(mediaTiles()).toContain("dropped.mp4"));
    expect(vi.mocked(backendRequest)).toHaveBeenCalledWith("import_media_to_project", expect.objectContaining({ sourcePaths: ["/in/dropped.mp4"] }));
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-dropped" });
    vi.mocked(backendRequest).mockReset();
    delete window.__EDITOR_FIXTURE_RUNTIME__;
  });

  it("explains that import needs the desktop app", async () => {
    renderPanel();
    await click(screen.getByRole("button", { name: "Import media" }));
    expect(await screen.findByText("Import needs the desktop app")).toBeInTheDocument();
  });
});

describe("MediaPanel folders, media dialogs and handoffs", () => {
  beforeEach(() => window.localStorage.clear());

  it("creates, renames and deletes folders through dialogs", async () => {
    const { store, batches } = renderPanel();
    await click((await openMoreMenu()).getByRole("menuitem", { name: "New folder" }));
    const create = within(await screen.findByRole("dialog", { name: "New folder" }));
    expect(create.getByRole("button", { name: "Create folder" })).toBeDisabled();
    fireEvent.change(create.getByLabelText("Folder name"), { target: { value: "  B-roll " } });
    await click(create.getByRole("button", { name: "Create folder" }));
    await waitFor(() => expect(batches.at(-1)).toEqual([{ type: "createMediaFolder", folder: { id: "folder-b-roll", name: "B-roll", parentId: null } }]));

    fireEvent.contextMenu(screen.getByRole("button", { name: "Open folder Source footage, 1 item" }));
    await click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Rename…" }));
    const rename = within(await screen.findByRole("dialog", { name: 'Rename folder "Source footage"' }));
    fireEvent.change(rename.getByLabelText("Folder name"), { target: { value: "Footage" } });
    await click(rename.getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(batches.at(-1)).toEqual([{ type: "renameMediaFolder", folderId: "folder-source", name: "Footage" }]));

    fireEvent.contextMenu(await screen.findByRole("button", { name: "Open folder Footage, 1 item" }));
    await click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Delete…" }));
    const remove = within(await screen.findByRole("dialog", { name: 'Delete folder "Footage"?' }));
    expect(remove.getByText("It holds 1 item. They stay in the project; only the folder is removed.")).toBeInTheDocument();
    await click(remove.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(batches.at(-1)).toEqual([{ type: "deleteMediaFolder", folderId: "folder-source" }]));
    expect(store.getState().project.mediaFolders?.some((folder) => folder.id === "folder-source")).toBe(false);
  });

  it("opens a media tile's context menu on a touch long press without previewing it", () => {
    installPointerEventPolyfill();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const { store } = renderPanel();
      const preview = screen.getByRole("button", { name: "Preview voiceover.m4a" });
      fireEvent.pointerDown(preview, { pointerId: 3, pointerType: "touch", clientX: 40, clientY: 40 });
      act(() => vi.advanceTimersByTime(longPressMilliseconds));
      expect(screen.getByRole("menu", { name: "voiceover.m4a actions" })).toBeInTheDocument();
      fireEvent.pointerUp(preview, { pointerId: 3, pointerType: "touch", clientX: 40, clientY: 40 });
      fireEvent.click(preview);
      expect(store.getState().previewSource).toEqual({ kind: "timeline" });
    } finally {
      vi.useRealTimers();
    }
  });

  it("confirms deleting media with its clip count", async () => {
    const { batches } = renderPanel();
    fireEvent.contextMenu(tile("voiceover.m4a"));
    await click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Delete…" }));
    const dialog = within(await screen.findByRole("dialog", { name: 'Delete "voiceover.m4a"?' }));
    expect(dialog.getByText("This removes it from the project and deletes the clip that uses it on the timeline.")).toBeInTheDocument();
    await click(dialog.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(batches.at(-1)).toEqual([{ type: "deleteMedia", mediaIds: ["media-voiceover"] }]));
  });

  it("commits replace mode as one batch and leaves replace mode", async () => {
    const base = project();
    const target = fixtureItem(base, "video");
    const { store, batches } = renderPanel(base);
    act(() => store.getState().setReplaceTargetItemId(target.id));
    expect(screen.getByRole("region", { name: "Replace mode" })).toHaveTextContent(`Choose media to replace ${target.label}`);
    await click(screen.getByRole("button", { name: `Replace ${target.label} with Poster` }));
    await waitFor(() => expect(batches).toHaveLength(1));
    expect(batches[0]?.map((action) => action.type)).toEqual(["removeItems", "addItems"]);
    expect(store.getState().replaceTargetItemId).toBeNull();
    expect(screen.queryByRole("region", { name: "Replace mode" })).not.toBeInTheDocument();
  });

  it("validates the matte hex and explains that the project must be saved", async () => {
    renderPanel();
    await click((await openMoreMenu()).getByRole("menuitem", { name: "Create matte" }));
    const dialog = within(await screen.findByRole("dialog", { name: "Create matte" }));
    fireEvent.change(dialog.getByLabelText("Matte color"), { target: { value: "#12" } });
    expect(dialog.getByRole("alert")).toHaveTextContent("Enter a six-digit hex color, such as #112233.");
    expect(dialog.getByRole("button", { name: "Create matte" })).toBeDisabled();
    fireEvent.change(dialog.getByLabelText("Matte color"), { target: { value: "#112233" } });
    expect(dialog.getByText("1920 × 1080")).toBeInTheDocument();
    await click(dialog.getByRole("button", { name: "Create matte" }));
    expect(await dialog.findByText("Save this project before creating a matte.")).toBeInTheDocument();
  });

  it("hands Organize with AI to the AI tab, to send after confirmation", async () => {
    const { store } = renderPanel();
    await click((await openMoreMenu()).getByRole("menuitem", { name: "Organize with AI" }));
    expect(store.getState().pendingAgentRequest).toMatchObject({ itemIds: [], confirmSend: true });
    expect(store.getState().pendingAgentRequest?.prompt).toMatch(/^Organize the current project media/);
    expect(store.getState().activeTab).toBe("ai");
  });

  it("reveals hidden media by clearing the filters and flashing its tile", async () => {
    const { store } = renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Images" }));
    act(() => store.getState().setRevealMediaId("media-voiceover"));
    await waitFor(() => expect(store.getState().revealMediaId).toBeNull());
    expect(store.getState().mediaFilter).toBe("all");
    expect(tile("voiceover.m4a").querySelector(".ring-accent")).not.toBeNull();
  });

  it("opens the generate view state from Generate", () => {
    const { store } = renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    expect(store.getState().generateView).toEqual({ open: true, mode: "video" });
    fireEvent.click(screen.getByRole("button", { name: "Back to media" }));
    expect(store.getState().generateView).toBeNull();
  });
});

describe("MediaPanel browser uploads", () => {
  beforeEach(() => installRuntimeMode("browser"));

  it("opens the browser picker, reports progress, and imports selected bytes", async () => {
    const base = project();
    const imported = { id: "media-uploaded", name: "uploaded.mp4", relativePath: "media/uploaded.mp4", kind: "video" as const, durationSeconds: 2, width: 1, height: 1, fps: 30, folderId: null };
    let finish: ((value: Awaited<ReturnType<typeof uploadBrowserFile>>) => void) | undefined;
    vi.mocked(uploadBrowserFile).mockImplementation(async (_file, options) => {
      options.onProgress?.(0.5);
      return await new Promise((resolve) => { finish = resolve; });
    });
    const { store } = renderPanel(base, "opaque-project");
    const file = new File([new Uint8Array(16)], "uploaded.mp4", { type: "video/mp4" });

    fireEvent.change(screen.getByLabelText("Choose media files"), { target: { files: [file] } });
    expect(await screen.findByRole("button", { name: "Cancel upload, 50%" })).toBeInTheDocument();
    await act(async () => finish?.({ project: { ...base, media: [...base.media, imported] }, imported: [imported], skipped: [] }));

    await waitFor(() => expect(store.getState().project.media).toContainEqual(imported));
    expect(uploadBrowserFile).toHaveBeenCalledWith(file, expect.objectContaining({ projectId: "opaque-project", expectedRevision: 0 }));
  });

  it("cancels an active browser upload", async () => {
    vi.mocked(uploadBrowserFile).mockImplementation(async (_file, options) => await new Promise((_resolve, reject) => {
      options.onProgress?.(0.25);
      options.signal?.addEventListener("abort", () => reject(new DOMException("Upload cancelled", "AbortError")), { once: true });
    }));
    renderPanel(project(), "opaque-project");
    const file = new File([new Uint8Array(16)], "cancel.mp4", { type: "video/mp4" });
    fireEvent.change(screen.getByLabelText("Choose media files"), { target: { files: [file] } });

    await click(await screen.findByRole("button", { name: "Cancel upload, 25%" }));

    expect(await screen.findByText("Upload cancelled.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry upload" })).toBeInTheDocument();
  });

  it("uploads dropped browser files and retries a failed attempt", async () => {
    const base = project();
    const imported = { id: "media-drop-upload", name: "drop.mp4", relativePath: "media/drop.mp4", kind: "video" as const, durationSeconds: 2, width: 1, height: 1, fps: 30, folderId: null };
    vi.mocked(uploadBrowserFile)
      .mockRejectedValueOnce(new Error("Host storage is full."))
      .mockResolvedValueOnce({ project: { ...base, media: [...base.media, imported] }, imported: [imported], skipped: [] });
    const { container, store } = renderPanel(base, "opaque-project");
    const file = new File([new Uint8Array(16)], "drop.mp4", { type: "video/mp4" });

    fireEvent.drop(container.firstElementChild as HTMLElement, { dataTransfer: { types: ["Files"], files: [file], getData: () => "" } });
    expect(await screen.findByText("Host storage is full.")).toBeInTheDocument();
    await click(screen.getByRole("button", { name: "Retry upload" }));

    await waitFor(() => expect(store.getState().project.media).toContainEqual(imported));
    expect(uploadBrowserFile).toHaveBeenCalledTimes(2);
  });
});
