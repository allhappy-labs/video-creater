import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectLocationPreference } from "@/lib/app-settings";
import type { SettingsCategoryHealth } from "@/lib/settings/health";
import type { SettingsOperation } from "@/lib/settings/operations";
import { requiredAt } from "@/test-utils/required";
import {
  getStorageHealth,
  previewStorageCleanup,
  refreshStorageInventory,
  revealStorageInventoryItem,
  runStorageCleanup,
} from "@/lib/settings/storage";
import { defaultHostPlatform, installHostPlatform } from "@/lib/runtime/platform";
import { StorageSettings } from "./storage-settings";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@/lib/settings/storage", () => ({
  getStorageHealth: vi.fn(),
  previewStorageCleanup: vi.fn(),
  refreshStorageInventory: vi.fn(),
  revealStorageInventoryItem: vi.fn(),
  runStorageCleanup: vi.fn(),
}));

const mockGetStorageHealth = vi.mocked(getStorageHealth);
const mockPreviewStorageCleanup = vi.mocked(previewStorageCleanup);
const mockRefreshStorageInventory = vi.mocked(refreshStorageInventory);
const mockRevealStorageInventoryItem = vi.mocked(revealStorageInventoryItem);
const mockRunStorageCleanup = vi.mocked(runStorageCleanup);

const inventoryHealth: SettingsCategoryHealth = {
  id: "storage",
  state: "unavailable",
  items: [
    {
      id: "storage.globalModels",
      label: "Global models",
      state: "ready",
      summary: "1500000000 bytes used; 8000000000 bytes available on this volume.",
      actionId: "storage.refreshInventory",
      actionLabel: "Refresh usage",
      lastCheckedAt: "2026-07-17T12:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {
        scope: "globalModels",
        path: "/Users/test/Library/Application Support/Video Creater/models",
        bytes: "1500000000",
        freeBytes: "8000000000",
        removable: "false",
      },
    },
    {
      id: "storage.disposableAppCache",
      label: "Disposable application cache",
      state: "ready",
      summary: "250000000 bytes used; 8000000000 bytes available on this volume.",
      actionId: "storage.refreshInventory",
      actionLabel: "Refresh usage",
      lastCheckedAt: "2026-07-17T12:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {
        scope: "disposableAppCache",
        path: "/Users/test/Library/Caches/Video Creater",
        bytes: "250000000",
        freeBytes: "8000000000",
        removable: "true",
      },
    },
    ...([
      ["storage.projectMedia", "Project media", "projectMedia"],
      ["storage.projectTranscripts", "Project transcripts", "projectTranscripts"],
      ["storage.projectRenderArtifacts", "Project render artifacts", "projectRenderArtifacts"],
      ["storage.projectWorkflowArtifacts", "Project workflow and log artifacts", "projectWorkflowArtifacts"],
    ] satisfies Array<readonly [string, string, string]>).map(([id, label, scope]) => ({
      id,
      label,
      state: "unavailable" as const,
      summary: "Open a project to inspect",
      actionId: "storage.refreshInventory",
      actionLabel: "Refresh usage",
      lastCheckedAt: "2026-07-17T12:00:00Z",
      diagnosticCode: "storage.projectUnavailable",
      diagnosticDetail: null,
      provenance: { scope, bytes: "0", removable: scope === "projectRenderArtifacts" ? "true" : "false" },
    })),
  ],
};

function operation(
  id: string,
  state: SettingsOperation["state"],
  kind: SettingsOperation["kind"] = "storageRefresh",
): SettingsOperation {
  return {
    id,
    kind,
    targetId: "storage",
    phase: state,
    state,
    completedUnits: state === "succeeded" ? 6 : 0,
    totalUnits: 6,
    unit: "scopes",
    cancellable: false,
    message: `Storage ${state}.`,
    error: null,
    startedAt: "2026-07-17T12:00:00Z",
    updatedAt: "2026-07-17T12:00:01Z",
  };
}

function renderStorage(
  projectLocation: ProjectLocationPreference = { mode: "ask" },
  operations: SettingsOperation[] = [],
  activeProjectDir: string | null = null,
  onOpenSettingsTarget = vi.fn(),
) {
  const onProjectLocationChange = vi.fn();
  const result = render(
    <StorageSettings
      projectLocation={projectLocation}
      onProjectLocationChange={onProjectLocationChange}
      activeProjectDir={activeProjectDir}
      operations={operations}
      onOpenSettingsTarget={onOpenSettingsTarget}
    />,
  );
  return { ...result, onProjectLocationChange, onOpenSettingsTarget };
}

describe("StorageSettings", () => {
  beforeEach(() => {
    openMock.mockReset();
    mockGetStorageHealth.mockReset();
    mockGetStorageHealth.mockResolvedValue(inventoryHealth);
    mockPreviewStorageCleanup.mockReset();
    mockRefreshStorageInventory.mockReset();
    mockRevealStorageInventoryItem.mockReset();
    mockRevealStorageInventoryItem.mockResolvedValue(undefined);
    mockRunStorageCleanup.mockReset();
  });

  it("renders only global storage and links model removal to AI & Models", async () => {
    const view = renderStorage();

    expect(await screen.findByText("Application cache")).toBeInTheDocument();
    expect(screen.getByText("Global models")).toBeInTheDocument();
    expect(screen.queryByText("Project media")).not.toBeInTheDocument();
    expect(screen.queryByText("Project transcripts")).not.toBeInTheDocument();
    expect(screen.queryByText("Project render artifacts")).not.toBeInTheDocument();
    expect(mockGetStorageHealth).toHaveBeenCalledWith(null);

    fireEvent.click(
      screen.getByRole("button", { name: "Manage installed models" }),
    );
    expect(view.onOpenSettingsTarget).toHaveBeenCalledWith({
      category: "aiModels",
      item: "transcription",
    });
  });

  it("chooses an absolute suggested parent with the native directory dialog and can clear it", async () => {
    openMock.mockResolvedValue("/Volumes/Projects/Video Creater");
    const view = renderStorage();

    expect(screen.getByText("Ask for a folder")).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));

    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: "Choose suggested project parent folder",
      }),
    );
    expect(view.onProjectLocationChange).toHaveBeenCalledWith({
      mode: "suggestedParent",
      parentPath: "/Volumes/Projects/Video Creater",
    });

    view.rerender(
      <StorageSettings
        projectLocation={{ mode: "suggestedParent", parentPath: "/Volumes/Projects/Video Creater" }}
        onProjectLocationChange={view.onProjectLocationChange}
        activeProjectDir={null}
        operations={[]}
      />,
    );
    expect(screen.getByText("/Volumes/Projects/Video Creater")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(view.onProjectLocationChange).toHaveBeenLastCalledWith({ mode: "ask" });
  });

  it("renders human-readable global usage and validated reveal actions", async () => {
    renderStorage();

    const models = await screen.findByRole("group", { name: "Global models storage" });
    expect(models).toHaveTextContent("1.5 GB used");
    expect(models).toHaveTextContent("8 GB free");
    expect(screen.queryByText("Open a project to inspect")).not.toBeInTheDocument();

    fireEvent.click(
      within(models).getByRole("button", { name: "Reveal Global models in Finder" }),
    );
    await waitFor(() =>
      expect(mockRevealStorageInventoryItem).toHaveBeenCalledWith(
        "/Users/test/Library/Application Support/Video Creater/models",
        null,
      ),
    );
  });

  it("names the Linux file manager for reveal actions and failures", async () => {
    installHostPlatform("linux");
    try {
      mockRevealStorageInventoryItem.mockRejectedValueOnce(new Error("no file manager"));
      renderStorage();

      const models = await screen.findByRole("group", { name: "Global models storage" });
      expect(
        within(models).queryByRole("button", { name: "Reveal Global models in Finder" }),
      ).not.toBeInTheDocument();
      fireEvent.click(
        within(models).getByRole("button", { name: "Show Global models in file manager" }),
      );
      expect(
        await screen.findByText(/The file manager could not show Global models: .*no file manager/),
      ).toBeInTheDocument();
      expect(document.body).not.toHaveTextContent("Finder");
    } finally {
      act(() => installHostPlatform(defaultHostPlatform));
    }
  });

  it("refetches inventory only after its background refresh becomes terminal", async () => {
    const queued = operation("storage-refresh-1", "queued");
    mockRefreshStorageInventory.mockResolvedValue(queued);
    const view = renderStorage();
    await screen.findByRole("group", { name: "Global models storage" });
    expect(mockGetStorageHealth).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Refresh storage usage" }));
    await waitFor(() => expect(mockRefreshStorageInventory).toHaveBeenCalledWith(null));
    expect(mockGetStorageHealth).toHaveBeenCalledTimes(1);

    view.rerender(
      <StorageSettings
        projectLocation={{ mode: "ask" }}
        onProjectLocationChange={view.onProjectLocationChange}
        activeProjectDir={null}
        operations={[operation("storage-refresh-1", "succeeded")]}
      />,
    );
    await waitFor(() => expect(mockGetStorageHealth).toHaveBeenCalledTimes(2));
  });

  it("observes a terminal refresh event that arrived before the command response", async () => {
    let resolveRefresh!: (operation: SettingsOperation) => void;
    mockRefreshStorageInventory.mockImplementation(
      () => new Promise((resolve) => { resolveRefresh = resolve; }),
    );
    renderStorage({ mode: "ask" }, [operation("storage-refresh-race", "succeeded")]);
    await screen.findByRole("group", { name: "Global models storage" });

    fireEvent.click(screen.getByRole("button", { name: "Refresh storage usage" }));
    resolveRefresh(operation("storage-refresh-race", "queued"));

    await waitFor(() => expect(mockGetStorageHealth).toHaveBeenCalledTimes(2));
  });

  it("locks refresh while its command response is pending", async () => {
    mockRefreshStorageInventory.mockImplementation(() => new Promise(() => {}));
    renderStorage();
    await screen.findByRole("group", { name: "Global models storage" });
    const refresh = screen.getByRole("button", { name: "Refresh storage usage" });

    fireEvent.click(refresh);

    expect(refresh).toBeDisabled();
    fireEvent.click(refresh);
    expect(mockRefreshStorageInventory).toHaveBeenCalledOnce();
  });

  it.each(["storageRefresh", "storageCleanup"] as const)(
    "observes an external %s across a Storage remount and refetches once at terminal state",
    async (kind) => {
      const id = `external-${kind}`;
      const firstMount = renderStorage();
      await screen.findByRole("group", { name: "Global models storage" });
      firstMount.unmount();

      const remount = renderStorage(
        { mode: "ask" },
        [operation(id, "running", kind)],
        "/projects/current",
      );
      await waitFor(() =>
        expect(mockGetStorageHealth).toHaveBeenLastCalledWith(null),
      );
      const callsBeforeTerminal = mockGetStorageHealth.mock.calls.length;

      remount.rerender(
        <StorageSettings
          projectLocation={{ mode: "ask" }}
          onProjectLocationChange={remount.onProjectLocationChange}
          activeProjectDir="/projects/current"
          operations={[operation(id, "succeeded", kind)]}
        />,
      );
      await waitFor(() =>
        expect(mockGetStorageHealth).toHaveBeenCalledTimes(callsBeforeTerminal + 1),
      );

      remount.rerender(
        <StorageSettings
          projectLocation={{ mode: "ask" }}
          onProjectLocationChange={remount.onProjectLocationChange}
          activeProjectDir="/projects/current"
          operations={[operation(id, "succeeded", kind)]}
        />,
      );
      await Promise.resolve();
      expect(mockGetStorageHealth).toHaveBeenCalledTimes(callsBeforeTerminal + 1);
    },
  );

  it.each(["storageRefresh", "storageCleanup"] as const)(
    "keeps an external %s global when the active project prop changes",
    async (kind) => {
      const id = `old-root-${kind}`;
      const view = renderStorage(
        { mode: "ask" },
        [operation(id, "running", kind)],
        "/projects/old",
      );
      await waitFor(() =>
        expect(mockGetStorageHealth).toHaveBeenLastCalledWith(null),
      );

      view.rerender(
        <StorageSettings
          projectLocation={{ mode: "ask" }}
          onProjectLocationChange={view.onProjectLocationChange}
          activeProjectDir="/projects/new"
          operations={[operation(id, "running", kind)]}
        />,
      );
      expect(mockGetStorageHealth).toHaveBeenLastCalledWith(null);
      const callsAfterRootChange = mockGetStorageHealth.mock.calls.length;

      view.rerender(
        <StorageSettings
          projectLocation={{ mode: "ask" }}
          onProjectLocationChange={view.onProjectLocationChange}
          activeProjectDir="/projects/new"
          operations={[operation(id, "succeeded", kind)]}
        />,
      );
      await waitFor(() =>
        expect(mockGetStorageHealth).toHaveBeenCalledTimes(callsAfterRootChange + 1),
      );
      expect(mockGetStorageHealth).toHaveBeenLastCalledWith(null);
    },
  );

  it("confirms cleanup from an exact path-and-byte preview without free-text or terminal commands", async () => {
    mockPreviewStorageCleanup.mockResolvedValue({
      target: { kind: "disposableAppCache" },
      items: [
        { path: "/Users/test/Library/Caches/Video Creater/frames", bytes: 125000000 },
        { path: "/Users/test/Library/Caches/Video Creater/probes", bytes: 50000000 },
      ],
      totalBytes: 175000000,
      previewNonce: "preview-1",
      confirmationToken: "storage-cleanup-v1:preview-1:hash",
      projectGeneration: null,
    });
    mockRunStorageCleanup.mockResolvedValue(
      operation("storage-cleanup-1", "queued", "storageCleanup"),
    );
    renderStorage();
    const cache = await screen.findByRole("group", {
      name: "Disposable application cache storage",
    });

    fireEvent.click(within(cache).getByRole("button", { name: "Review cleanup" }));
    const dialog = await screen.findByRole("dialog", { name: "Confirm storage cleanup" });
    expect(dialog).toHaveTextContent("/Users/test/Library/Caches/Video Creater/frames");
    expect(dialog).toHaveTextContent("125 MB");
    expect(dialog).toHaveTextContent("/Users/test/Library/Caches/Video Creater/probes");
    expect(dialog).toHaveTextContent("50 MB");
    expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByText(/copy .*command|run diagnostic|terminal/i)).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", {
      name: "Remove 2 disposable application cache items",
    }));
    await waitFor(() =>
      expect(mockRunStorageCleanup).toHaveBeenCalledWith(
        { kind: "disposableAppCache" },
        "storage-cleanup-v1:preview-1:hash",
        null,
      ),
    );
  });

  it("never exposes project render cleanup from global Settings", async () => {
    const renderHealth: SettingsCategoryHealth = {
      ...inventoryHealth,
      state: "ready",
      items: inventoryHealth.items.map((item) =>
        item.id === "storage.projectRenderArtifacts"
          ? {
              ...item,
              state: "ready",
              summary: "300000000 bytes used; 8000000000 bytes available on this volume.",
              diagnosticCode: null,
              provenance: {
                ...item.provenance,
                path: "/projects/current/renders",
                bytes: "300000000",
                freeBytes: "8000000000",
                artifactIds: JSON.stringify(["draft-001", "final-002"]),
              },
            }
          : item,
      ),
    };
    mockGetStorageHealth.mockResolvedValue(renderHealth);

    renderStorage({ mode: "ask" }, [], "/projects/current");
    await screen.findByRole("group", { name: "Global models storage" });
    expect(
      screen.queryByRole("group", { name: "Project render artifacts storage" }),
    ).not.toBeInTheDocument();
    expect(mockPreviewStorageCleanup).not.toHaveBeenCalledWith(
      expect.objectContaining({ kind: "projectRenderArtifacts" }),
      expect.anything(),
    );
  });

  it("traps cleanup focus, closes with Escape, and restores the review trigger", async () => {
    mockPreviewStorageCleanup.mockResolvedValue({
      target: { kind: "disposableAppCache" },
      items: [{ path: "/Users/test/Library/Caches/Video Creater/frames", bytes: 125000000 }],
      totalBytes: 125000000,
      previewNonce: "preview-focus",
      confirmationToken: "storage-cleanup-v1:preview-focus:hash",
      projectGeneration: null,
    });
    renderStorage();
    const cache = await screen.findByRole("group", {
      name: "Disposable application cache storage",
    });
    const review = within(cache).getByRole("button", { name: "Review cleanup" });
    review.focus();
    fireEvent.click(review);

    const dialog = await screen.findByRole("dialog", { name: "Confirm storage cleanup" });
    const close = within(dialog).getByRole("button", { name: "Close cleanup preview" });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    const confirm = within(dialog).getByRole("button", {
      name: "Remove 1 disposable application cache item",
    });
    await waitFor(() => expect(cancel).toHaveFocus());

    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(close).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Escape" });

    await waitFor(() => expect(review).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("runs cleanup once and keeps every dismissal action locked while confirmation starts", async () => {
    let resolveCleanup!: (operation: SettingsOperation) => void;
    mockPreviewStorageCleanup.mockResolvedValue({
      target: { kind: "disposableAppCache" },
      items: [{ path: "/Users/test/Library/Caches/Video Creater/frames", bytes: 125000000 }],
      totalBytes: 125000000,
      previewNonce: "preview-lock",
      confirmationToken: "storage-cleanup-v1:preview-lock:hash",
      projectGeneration: null,
    });
    mockRunStorageCleanup.mockImplementation(
      () => new Promise((resolve) => { resolveCleanup = resolve; }),
    );
    renderStorage();
    const cache = await screen.findByRole("group", {
      name: "Disposable application cache storage",
    });
    const review = within(cache).getByRole("button", { name: "Review cleanup" });
    fireEvent.click(review);
    const dialog = await screen.findByRole("dialog", { name: "Confirm storage cleanup" });
    const confirm = within(dialog).getByRole("button", {
      name: "Remove 1 disposable application cache item",
    });

    fireEvent.click(confirm);
    fireEvent.click(confirm);
    fireEvent.keyDown(dialog, { key: "Escape" });

    expect(mockRunStorageCleanup).toHaveBeenCalledOnce();
    expect(within(dialog).getByRole("button", {
      name: "Starting disposable application cache cleanup",
    })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Close cleanup preview" })).toBeDisabled();
    expect(dialog).toBeInTheDocument();
    expect(screen.queryAllByRole("alert")).toHaveLength(0);

    resolveCleanup(operation("storage-cleanup-lock", "queued", "storageCleanup"));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Refresh storage usage" })).toHaveFocus(),
    );
  });

  it("names the volume and blocked operation when storage is too low", async () => {
    mockGetStorageHealth.mockResolvedValue({
      id: "storage",
      state: "actionRequired",
      items: [
        {
          ...requiredAt(inventoryHealth.items, 0, "global model storage fixture"),
          state: "actionRequired",
          diagnosticCode: "storage.lowSpace",
          summary: "The render is blocked by low storage.",
          provenance: {
            ...requiredAt(inventoryHealth.items, 0, "global model storage fixture").provenance,
            volume: "Macintosh HD",
            blockedOperation: "render this project",
            freeBytes: "900000000",
          },
        },
      ],
    });
    renderStorage();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Macintosh HD does not have enough free space to render this project",
    );
    expect(screen.getByRole("alert")).toHaveTextContent("900 MB free");
  });
});
