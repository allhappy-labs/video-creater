import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  getSettingsHealthSnapshot,
  type SettingsCategoryHealth,
} from "@/lib/settings/health";
import {
  listSettingsOperations,
  type SettingsOperation,
} from "@/lib/settings/operations";
import {
  getStorageHealth,
  previewStorageCleanup,
  type StorageCleanupPreview,
} from "@/lib/settings/storage";
import {
  getActiveTranscriptionModel,
  getProductionSpeechModelStatus,
  getTranscriptionRuntimeStatus,
  listTranscriptionModels,
  type ProductionSpeechModelStatus,
  type TranscriptionModelStatus,
} from "@/lib/transcription-models";
import { ModelsSettings } from "./models-settings";
import { SettingsDiagnostics } from "./settings-diagnostics";
import { SettingsOperationStatus } from "./settings-operation-status";
import { SettingsShell } from "./settings-shell";
import { Settings } from "./settings";
import { StorageSettings } from "./storage-settings";

const { backendRequestMock, listenMock, openMock } = vi.hoisted(() => ({
  backendRequestMock: vi.fn(),
  listenMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock("@/lib/runtime/backend-client", () => ({
  backendListen: listenMock,
  backendRequest: backendRequestMock,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));
vi.mock("@/lib/settings/health", () => ({
  getSettingsHealthSnapshot: vi.fn(),
}));
vi.mock("@/lib/settings/operations", async () => {
  const actual = await vi.importActual<typeof import("@/lib/settings/operations")>(
    "@/lib/settings/operations",
  );
  return {
    ...actual,
    cancelSettingsOperation: vi.fn(),
    listSettingsOperations: vi.fn(),
  };
});
vi.mock("@/lib/settings/storage", () => ({
  getStorageHealth: vi.fn(),
  previewStorageCleanup: vi.fn(),
  refreshStorageInventory: vi.fn(),
  revealStorageInventoryItem: vi.fn(),
  runStorageCleanup: vi.fn(),
}));
vi.mock("@/lib/project", async () => {
  const actual = await vi.importActual<typeof import("@/lib/project")>("@/lib/project");
  return {
    ...actual,
    listGenerationModelCatalog: vi.fn().mockResolvedValue({ loaded: false }),
  };
});
vi.mock("@/lib/transcription-models", () => ({
  downloadProductionSpeechModels: vi.fn(),
  downloadTranscriptionModel: vi.fn(),
  getActiveTranscriptionModel: vi.fn(),
  getProductionSpeechModelStatus: vi.fn(),
  getTranscriptionRuntimeStatus: vi.fn(),
  importTranscriptionModel: vi.fn(),
  listTranscriptionModels: vi.fn(),
  removeProductionSpeechModels: vi.fn(),
  removeTranscriptionModel: vi.fn(),
  setActiveTranscriptionModel: vi.fn(),
  verifyProductionSpeechModels: vi.fn(),
  verifyTranscriptionModel: vi.fn(),
}));

const mockGetSnapshot = vi.mocked(getSettingsHealthSnapshot);
const mockListOperations = vi.mocked(listSettingsOperations);
const mockGetStorageHealth = vi.mocked(getStorageHealth);
const mockPreviewStorageCleanup = vi.mocked(previewStorageCleanup);
const mockListModels = vi.mocked(listTranscriptionModels);
const mockGetActiveModel = vi.mocked(getActiveTranscriptionModel);
const mockGetRuntime = vi.mocked(getTranscriptionRuntimeStatus);
const mockGetSpeech = vi.mocked(getProductionSpeechModelStatus);

const model: TranscriptionModelStatus = {
  modelId: "nvidia__parakeet-tdt-0.6b-v3",
  displayName: "Parakeet TDT 0.6B v3",
  isActive: false,
  installStatus: "missing",
  localPath: "/Users/test/Library/Application Support/Video Creater/models/nvidia__parakeet-tdt-0.6b-v3",
  approximateSizeBytes: 485_000_000,
  installedBytes: 125_000_000,
  downloadedFiles: 5,
  totalFiles: 18,
  runtimeId: "parakeet-coreml",
  sourceRepoId: "nvidia/parakeet-tdt-0.6b-v3",
  sourceRevision: "pinned-revision",
  sourceLicense: "CC-BY-4.0",
  artifactFormat: "Core ML",
};

const speechModels: ProductionSpeechModelStatus = {
  modelSetId: "silero-vad+wspk-vbx-v1",
  runtimeId: "fluid_audio_speech_analysis",
  ready: false,
  installedFiles: 4,
  totalFiles: 26,
  installedBytes: 4_000_000,
  totalBytes: 22_662_842,
  rootPath: "/models/speech-analysis/production-v1",
  vadRepo: "FluidInference/silero-vad-coreml",
  vadRevision: "vad-revision",
  diarizationRepo: "FluidInference/speaker-diarization-coreml",
  diarizationRevision: "diarization-revision",
  artifactFormat: "compiled_core_ml_bundles",
  licenses: ["MIT", "CC-BY-4.0"],
};

const runningDownload: SettingsOperation = {
  id: "model-download-1",
  kind: "modelDownload",
  targetId: model.modelId,
  phase: "Downloading model files",
  state: "running",
  completedUnits: 125,
  totalUnits: 485,
  unit: "MB",
  cancellable: false,
  message: "Downloading pinned model files from Hugging Face.",
  error: null,
  startedAt: "2026-07-17T12:00:00Z",
  updatedAt: "2026-07-17T12:00:01Z",
};

const snapshot = {
  generatedAt: "2026-07-17T12:00:00Z",
  overall: "actionRequired" as const,
  categories: {
    general: { id: "general", state: "ready" as const, items: [] },
    models: { id: "models", state: "actionRequired" as const, items: [] },
    agent: { id: "agent", state: "failed" as const, items: [] },
    skills: { id: "skills", state: "checking" as const, items: [] },
    storage: { id: "storage", state: "unavailable" as const, items: [] },
    providers: { id: "providers", state: "actionRequired" as const, items: [] },
  },
};

const inventoryHealth: SettingsCategoryHealth = {
  id: "storage",
  state: "ready",
  items: [
    {
      id: "storage.disposableAppCache",
      label: "Disposable application cache",
      state: "ready",
      summary: "125000000 bytes used.",
      actionId: "storage.refreshInventory",
      actionLabel: "Refresh usage",
      lastCheckedAt: "2026-07-17T12:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {
        scope: "disposableAppCache",
        path: "/Users/test/Library/Caches/Video Creater",
        bytes: "125000000",
        freeBytes: "8000000000",
        removable: "true",
      },
    },
  ],
};

const cleanupPreview: StorageCleanupPreview = {
  target: { kind: "disposableAppCache" },
  items: [
    {
      path: "/Users/test/Library/Caches/Video Creater/frames",
      bytes: 125_000_000,
    },
  ],
  totalBytes: 125_000_000,
  previewNonce: "preview-a11y",
  confirmationToken: "storage-cleanup-v1:preview-a11y:hash",
  projectGeneration: null,
};

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

function installMatchMedia(matches: boolean) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn().mockImplementation((media: string) => ({
      matches,
      media,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

function renderModels(operation: SettingsOperation | null = runningDownload) {
  return render(
    <ModelsSettings
      models={[model]}
      runtimeSelection="native"
      operations={operation ? [operation] : []}
      operationsError={null}
      speechModels={null}
      onDownload={vi.fn()}
      onCancelOperation={vi.fn()}
      onVerify={vi.fn()}
      onRemove={vi.fn()}
      onSetActive={vi.fn()}
      onImport={vi.fn()}
    />,
  );
}

describe("Settings accessibility", () => {
  beforeEach(() => {
    vi.useRealTimers();
    window.localStorage.clear();
    delete (window as Window & { __EDITOR_FIXTURE_RUNTIME__?: unknown })
      .__EDITOR_FIXTURE_RUNTIME__;
    backendRequestMock.mockReset().mockResolvedValue([]);
    listenMock.mockReset().mockResolvedValue(vi.fn());
    openMock.mockReset();
    mockListOperations.mockReset().mockResolvedValue([runningDownload]);
    mockGetSnapshot.mockReset().mockResolvedValue(snapshot);
    mockGetStorageHealth.mockReset().mockResolvedValue(snapshot.categories.storage);
    mockPreviewStorageCleanup.mockReset();
    mockListModels.mockReset().mockResolvedValue([model]);
    mockGetActiveModel.mockReset().mockResolvedValue(null as never);
    mockGetRuntime.mockReset().mockResolvedValue("native");
    mockGetSpeech.mockReset().mockResolvedValue(null as never);
  });

  afterEach(() => {
    vi.useRealTimers();
    Reflect.deleteProperty(window, "matchMedia");
  });

  it("exposes one labeled category tablist and one active settings panel", async () => {
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    const tablist = await screen.findByRole("tablist", { name: "Settings categories" });
    expect(screen.getAllByRole("tablist")).toEqual([tablist]);
    expect(within(tablist).getAllByRole("tab")).toHaveLength(7);
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    const panel = screen.getByRole("tabpanel");
    expect(panel).toHaveAttribute("aria-label", "AI & Models settings");
    expect(panel).toBeVisible();
  });

  it("announces the current model action once and exposes named numeric progress", async () => {
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    const status = await screen.findByRole("status", {
      name: "Parakeet TDT 0.6B v3 download status",
    });
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveAttribute("aria-atomic", "true");
    expect(status).toHaveTextContent("Downloading model files");
    expect(status).toHaveTextContent("Downloading pinned model files from Hugging Face.");
    expect(
      screen.getAllByRole("status", {
        name: "Parakeet TDT 0.6B v3 download status",
      }),
    ).toHaveLength(1);

    const progress = screen.getByRole("progressbar", {
      name: "Parakeet TDT 0.6B v3 download progress",
    });
    expect(progress).toHaveAttribute("aria-valuenow", "125");
    expect(progress).toHaveAttribute("aria-valuemin", "0");
    expect(progress).toHaveAttribute("aria-valuemax", "485");
    expect(progress).toHaveAttribute("aria-valuetext", "125 / 485 MB");
  });

  it("moves roving tab focus with horizontal arrow keys", () => {
    installMatchMedia(false);
    render(
      <SettingsShell initialCategory="general" onBack={vi.fn()}>
        {(category) => <div>{category}</div>}
      </SettingsShell>,
    );

    const tablist = screen.getByRole("tablist", { name: "Settings categories" });
    expect(tablist).toHaveAttribute("aria-orientation", "horizontal");
    const general = within(tablist).getByRole("tab", { name: "General" });
    const projects = within(tablist).getByRole("tab", { name: "Projects" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowRight" });
    expect(projects).toHaveFocus();
    expect(projects).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(projects, { key: "ArrowLeft" });
    expect(general).toHaveFocus();
  });

  it("moves roving tab focus with vertical arrow keys", () => {
    installMatchMedia(true);
    render(
      <SettingsShell initialCategory="general" onBack={vi.fn()}>
        {(category) => <div>{category}</div>}
      </SettingsShell>,
    );

    const tablist = screen.getByRole("tablist", { name: "Settings categories" });
    expect(tablist).toHaveAttribute("aria-orientation", "vertical");
    const general = within(tablist).getByRole("tab", { name: "General" });
    const projects = within(tablist).getByRole("tab", { name: "Projects" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowDown" });
    expect(projects).toHaveFocus();
    fireEvent.keyDown(projects, { key: "ArrowUp" });
    expect(general).toHaveFocus();
  });

  it("renders preference categories without health badges or system checks", () => {
    installMatchMedia(true);
    render(
      <SettingsShell initialCategory="general" onBack={vi.fn()}>
        {() => null}
      </SettingsShell>,
    );

    const tablist = screen.getByRole("tablist", { name: "Settings categories" });
    expect(within(tablist).getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "General",
      "Projects",
      "AI & Models",
      "Integrations",
      "Remote access",
      "Storage",
      "Advanced",
    ]);
    expect(screen.queryByText(/Ready|Failed|Unavailable|Action required/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check all systems" })).not.toBeInTheDocument();
  });

  it("keeps every category discoverable in the narrow horizontal scroller", () => {
    installMatchMedia(false);
    render(
      <SettingsShell initialCategory="general" onBack={vi.fn()}>
        {() => null}
      </SettingsShell>,
    );

    const tablist = screen.getByRole("tablist", { name: "Settings categories" });
    expect(tablist).toHaveClass("overflow-x-auto");
    expect(within(tablist).getByRole("tab", { name: "Advanced" })).toBeVisible();
  });

  it("scrolls an externally selected narrow tab into view before focusing its target", async () => {
    installMatchMedia(false);
    const scrolled: HTMLElement[] = [];
    HTMLElement.prototype.scrollIntoView = vi.fn(function (this: HTMLElement) {
      scrolled.push(this);
    });

    render(
      <SettingsShell
        initialCategory="general"
        navigationRequestId={12}
        navigationTarget={{ category: "advanced", item: "execution" }}
        onBack={vi.fn()}
      >
        {(category) =>
          category === "advanced" ? (
            <section data-settings-target="advanced:execution" tabIndex={-1}>
              Execution target
            </section>
          ) : null
        }
      </SettingsShell>,
    );

    const advanced = screen.getByRole("tab", { name: "Advanced" });
    const target = screen.getByText("Execution target");
    await waitFor(() => expect(target).toHaveFocus());
    expect(advanced).toHaveAttribute("aria-selected", "true");
    expect(scrolled[0]).toBe(advanced);
    expect(HTMLElement.prototype.scrollIntoView).toHaveBeenNthCalledWith(1, {
      block: "nearest",
      inline: "nearest",
    });
  });

  it("communicates diagnostics disclosure state during keyboard operation", () => {
    render(
      <SettingsDiagnostics label="Render diagnostics">
        <code>render.runtime.missing</code>
      </SettingsDiagnostics>,
    );

    const disclosure = screen.getByText("Render diagnostics");
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    disclosure.focus();
    fireEvent.keyDown(disclosure, { key: "Enter" });
    expect(disclosure).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("render.runtime.missing")).toBeVisible();
  });

  it("associates a disabled model action with its user-facing reason", () => {
    renderModels();

    const cancel = screen.getByRole("button", {
      name: "Cancel Parakeet TDT 0.6B v3 download",
    });
    expect(cancel).toBeDisabled();
    expect(cancel).toHaveAccessibleDescription(
      "This download is in a phase that cannot be safely cancelled.",
    );
  });

  it("announces shared asynchronous operation status and progress atomically", () => {
    const operation: SettingsOperation = {
      ...runningDownload,
      id: "storage-refresh-1",
      kind: "storageRefresh",
      targetId: "storage",
      completedUnits: 3,
      totalUnits: 6,
      unit: "scopes",
      message: "Inspecting project render artifacts.",
    };
    render(
      <SettingsOperationStatus
        operation={operation}
        ariaLabel="Storage refresh operation"
      />,
    );

    const status = screen.getByRole("status", {
      name: "Storage refresh operation",
    });
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveAttribute("aria-atomic", "true");
    expect(status).toHaveTextContent("Inspecting project render artifacts.");
    const progress = screen.getByRole("progressbar", {
      name: "Storage refresh operation progress",
    });
    expect(progress).toHaveAttribute("aria-valuenow", "3");
    expect(progress).toHaveAttribute("aria-valuemax", "6");
    expect(progress).toHaveAttribute("aria-valuetext", "3 / 6 scopes");
  });

  it.each([null, 0, -4])(
    "treats a shared operation with total %s as indeterminate",
    (totalUnits) => {
      render(
        <SettingsOperationStatus
          operation={{
            ...runningDownload,
            kind: "storageRefresh",
            targetId: "storage",
            completedUnits: 3,
            totalUnits,
            unit: "scopes",
          }}
          ariaLabel="Storage refresh operation"
        />,
      );

      const progress = screen.getByRole("progressbar", {
        name: "Storage refresh operation progress",
      });
      expect(progress).not.toHaveAttribute("aria-valuenow");
      expect(progress).not.toHaveAttribute("aria-valuemax");
      expect(progress).toHaveAttribute("aria-valuetext", "3 scopes completed");
    },
  );

  it.each([
    { completedUnits: -3, expected: "0" },
    { completedUnits: 9, expected: "6" },
  ])(
    "clamps shared determinate progress $completedUnits to $expected",
    ({ completedUnits, expected }) => {
      render(
        <SettingsOperationStatus
          operation={{
            ...runningDownload,
            kind: "storageRefresh",
            targetId: "storage",
            completedUnits,
            totalUnits: 6,
            unit: "scopes",
          }}
          ariaLabel="Storage refresh operation"
        />,
      );

      const progress = screen.getByRole("progressbar", {
        name: "Storage refresh operation progress",
      });
      expect(progress).toHaveAttribute("aria-valuenow", expected);
      expect(progress).toHaveAttribute("aria-valuemax", "6");
      expect(progress).toHaveAttribute(
        "aria-valuetext",
        `${expected} / 6 scopes`,
      );
    },
  );

  it("keeps model progress indeterminate until a positive total is known", () => {
    renderModels({ ...runningDownload, totalUnits: null });

    const progress = screen.getByRole("progressbar", {
      name: "Parakeet TDT 0.6B v3 download progress",
    });
    expect(progress).not.toHaveAttribute("aria-valuenow");
    expect(progress).not.toHaveAttribute("aria-valuemax");
    expect(progress).toHaveAttribute("aria-valuetext", "125 MB completed");
  });

  it("clamps model progress to its positive total", () => {
    renderModels({
      ...runningDownload,
      completedUnits: 600,
      totalUnits: 485,
    });

    const progress = screen.getByRole("progressbar", {
      name: "Parakeet TDT 0.6B v3 download progress",
    });
    expect(progress).toHaveAttribute("aria-valuenow", "485");
    expect(progress).toHaveAttribute("aria-valuemax", "485");
    expect(progress).toHaveAttribute("aria-valuetext", "485 / 485 MB");
  });

  it("announces speech download status and progress without nested live regions", () => {
    const speechOperation: SettingsOperation = {
      ...runningDownload,
      id: "speech-download-1",
      kind: "speechModelsDownload",
      targetId: speechModels.modelSetId,
      phase: "Downloading speech model files",
      completedUnits: 4_000_000,
      totalUnits: speechModels.totalBytes,
      unit: "bytes",
      cancellable: false,
      message: "Downloaded 4 of 26 speech model files.",
    };
    render(
      <ModelsSettings
        models={[model]}
        runtimeSelection="native"
        operations={[speechOperation]}
        operationsError={null}
        speechModels={speechModels}
        onDownload={vi.fn()}
        onCancelOperation={vi.fn()}
        onVerify={vi.fn()}
        onRemove={vi.fn()}
        onSetActive={vi.fn()}
        onImport={vi.fn()}
      />,
    );

    const row = screen.getByRole("article", { name: "Speech analysis models" });
    const status = within(row).getByRole("status", {
      name: "Speech analysis download status",
    });
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveAttribute("aria-atomic", "true");
    expect(status).toHaveTextContent("Downloading speech model files");
    expect(status).toHaveTextContent("Downloaded 4 of 26 speech model files.");
    expect(row.querySelectorAll('[aria-live="polite"]')).toHaveLength(1);

    const progress = within(row).getByRole("progressbar", {
      name: "Speech analysis download progress",
    });
    expect(progress).toHaveAttribute("aria-valuenow", "4000000");
    expect(progress).toHaveAttribute(
      "aria-valuemax",
      String(speechModels.totalBytes),
    );
    expect(progress).toHaveAttribute("aria-valuetext", "4 MB / 22.7 MB");
    expect(row).toHaveTextContent(
      "This verified multi-file install cannot be safely cancelled.",
    );
  });

  it("names model removal with the exact model target", () => {
    render(
      <ModelsSettings
        models={[{ ...model, installStatus: "ready", isActive: true }]}
        runtimeSelection="native"
        operations={[]}
        operationsError={null}
        speechModels={null}
        onDownload={vi.fn()}
        onCancelOperation={vi.fn()}
        onVerify={vi.fn()}
        onRemove={vi.fn()}
        onSetActive={vi.fn()}
        onImport={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", {
        name: "Remove Parakeet TDT 0.6B v3",
      }),
    ).toBeInTheDocument();
  });

  it("names storage cleanup with the exact disposable scope", async () => {
    mockGetStorageHealth.mockResolvedValue(inventoryHealth);
    mockPreviewStorageCleanup.mockResolvedValue(cleanupPreview);
    render(
      <StorageSettings
        projectLocation={{ mode: "ask" }}
        onProjectLocationChange={vi.fn()}
        activeProjectDir={null}
        operations={[]}
      />,
    );

    const cache = await screen.findByRole("group", {
      name: "Disposable application cache storage",
    });
    const trigger = within(cache).getByRole("button", {
      name: "Review cleanup",
    });
    trigger.focus();
    fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog", {
      name: "Confirm storage cleanup",
    });
    expect(
      within(dialog).getByRole("button", {
        name: "Remove 1 disposable application cache item",
      }),
    ).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("immediately removes a listener that resolves after Settings unmounts", async () => {
    vi.useFakeTimers();
    const poll = deferred<SettingsOperation[]>();
    const listener = deferred<() => void>();
    const unlisten = vi.fn();
    let emit: ((payload: SettingsOperation) => void) | null = null;
    mockListOperations.mockReturnValue(poll.promise);
    listenMock.mockImplementation(
      (
        _eventName: string,
        callback: (payload: SettingsOperation) => void,
      ) => {
        emit = callback;
        return listener.promise;
      },
    );
    const view = render(
      <Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />,
    );
    await act(async () => {
      poll.resolve([]);
      await poll.promise;
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(listenMock).toHaveBeenCalledTimes(1);
    view.unmount();

    await act(async () => {
      listener.resolve(unlisten);
      await listener.promise;
      emit?.(runningDownload);
      await vi.runOnlyPendingTimersAsync();
    });

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(mockListOperations).toHaveBeenCalledTimes(1);
  });

  it("does not start listener recovery when the initial poll resolves after unmount", async () => {
    vi.useFakeTimers();
    const poll = deferred<SettingsOperation[]>();
    mockListOperations.mockReturnValue(poll.promise);

    const view = render(
      <Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockListOperations).toHaveBeenCalledTimes(1);
    expect(listenMock).not.toHaveBeenCalled();

    view.unmount();
    await act(async () => {
      poll.resolve([runningDownload]);
      await poll.promise;
      await Promise.resolve();
      await vi.runOnlyPendingTimersAsync();
    });

    expect(mockListOperations).toHaveBeenCalledTimes(1);
    expect(listenMock).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("cancels the recurring operation recovery timer when Settings unmounts", async () => {
    vi.useFakeTimers();
    mockListOperations.mockRejectedValue(new Error("operation journal unavailable"));
    listenMock.mockRejectedValue(new Error("event bridge unavailable"));

    const view = render(
      <Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />,
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mockListOperations).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledTimes(1);
    view.unmount();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(mockListOperations).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
