import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getSettingsHealthSnapshot } from "@/lib/settings/health";
import { cancelSettingsOperation, type SettingsOperation } from "@/lib/settings/operations";
import { getStorageHealth } from "@/lib/settings/storage";
import { useSettingsOperations } from "@/lib/settings/use-settings-operations";
import {
  downloadTranscriptionModel,
  getActiveTranscriptionModel,
  getProductionSpeechModelStatus,
  getTranscriptionRuntimeStatus,
  listTranscriptionModels,
  verifyProductionSpeechModels,
  type ProductionSpeechModelStatus,
  type TranscriptionModelStatus,
} from "@/lib/transcription-models";
import type { ModelsSettingsProps } from "./models-settings";
import { Settings } from "./settings";

const { modelsProps } = vi.hoisted(() => ({
  modelsProps: { current: null as ModelsSettingsProps | null },
}));

vi.mock("./models-settings", () => ({
  ModelsSettings: (props: ModelsSettingsProps) => {
    modelsProps.current = props;
    return (
      <section aria-label="Owned models settings">
        <button type="button" onClick={() => void props.onDownload("model-1")}>Download owned model</button>
        <button type="button" onClick={() => void props.onVerify("model-1")}>Recheck owned model</button>
        <button type="button" onClick={() => void props.onCancelOperation("operation-1")}>Cancel owned model</button>
        <button type="button" onClick={() => void props.onVerifySpeechModels?.()}>Verify owned speech models</button>
        {props.actionError ? (
          <div role="alert">
            {props.actionError.code} {props.actionError.message} {props.actionError.detail} {props.actionError.recoveryAction}
            {props.onRetryAction ? <button type="button" onClick={() => void props.onRetryAction?.()}>Retry owned status</button> : null}
          </div>
        ) : null}
        <div>Operation A reconciled {props.reconciledOperationIds?.has("operation-a") ? "yes" : "no"}</div>
        <div>Operation B reconciled {props.reconciledOperationIds?.has("operation-b") ? "yes" : "no"}</div>
        <div>Speech state {props.speechModels?.lastErrorCode ? "failed" : props.speechModels?.ready ? "ready" : "missing"}</div>
      </section>
    );
  },
}));
vi.mock("./general-settings", () => ({ GeneralSettings: () => null }));
vi.mock("./agent-mcp-settings", () => ({ AgentMcpSettings: () => null }));
vi.mock("./storage-settings", () => ({ StorageSettings: () => null }));
vi.mock("./providers-settings", () => ({ ProvidersSettings: () => null }));

vi.mock("@/lib/settings/health", () => ({ getSettingsHealthSnapshot: vi.fn() }));
vi.mock("@/lib/settings/use-settings-operations", () => ({ useSettingsOperations: vi.fn() }));
vi.mock("@/lib/settings/operations", () => ({ cancelSettingsOperation: vi.fn() }));
vi.mock("@/lib/settings/storage", () => ({ getStorageHealth: vi.fn() }));
vi.mock("@/lib/project", async () => {
  const actual = await vi.importActual<typeof import("@/lib/project")>("@/lib/project");
  return { ...actual, listGenerationModelCatalog: vi.fn().mockResolvedValue({ loaded: false }) };
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

const mockSnapshot = vi.mocked(getSettingsHealthSnapshot);
const mockOperations = vi.mocked(useSettingsOperations);
const mockCancel = vi.mocked(cancelSettingsOperation);
const mockStorage = vi.mocked(getStorageHealth);
const mockDownload = vi.mocked(downloadTranscriptionModel);
const mockListModels = vi.mocked(listTranscriptionModels);
const mockActiveModel = vi.mocked(getActiveTranscriptionModel);
const mockRuntime = vi.mocked(getTranscriptionRuntimeStatus);
const mockSpeech = vi.mocked(getProductionSpeechModelStatus);
const mockVerifySpeech = vi.mocked(verifyProductionSpeechModels);

const model: TranscriptionModelStatus = {
  modelId: "model-1",
  displayName: "Owned model",
  isActive: true,
  installStatus: "ready",
  localPath: "/models/model-1",
  approximateSizeBytes: 10,
  downloadedFiles: 1,
  totalFiles: 1,
};

const readySpeech: ProductionSpeechModelStatus = {
  modelSetId: "speech-1",
  runtimeId: "speech-runtime",
  ready: true,
  installedFiles: 2,
  totalFiles: 2,
  installedBytes: 20,
  totalBytes: 20,
  rootPath: "/models/speech",
  vadRepo: "vad",
  vadRevision: "vad-revision",
  diarizationRepo: "diarization",
  diarizationRevision: "diarization-revision",
  artifactFormat: "compiled_core_ml_bundles",
  licenses: ["MIT"],
};

const healthSnapshot = {
  generatedAt: "2026-07-17T12:00:00Z",
  overall: "ready" as const,
  categories: {
    general: { id: "general", state: "ready" as const, items: [] },
    models: { id: "models", state: "ready" as const, items: [] },
    agent: { id: "agent", state: "ready" as const, items: [] },
    skills: { id: "skills", state: "ready" as const, items: [] },
    storage: { id: "storage", state: "ready" as const, items: [] },
    providers: { id: "providers", state: "ready" as const, items: [] },
  },
};

function operation(id: string, kind: SettingsOperation["kind"] = "modelDownload"): SettingsOperation {
  return {
    id,
    kind,
    targetId: kind === "speechModelsDownload" ? "speech-1" : "model-1",
    phase: "ready",
    state: "succeeded",
    completedUnits: 1,
    totalUnits: 1,
    unit: "files",
    cancellable: false,
    message: "Completed.",
    error: null,
    startedAt: "2026-07-17T12:00:00Z",
    updatedAt: "2026-07-17T12:00:01Z",
  };
}

function operationsSnapshot(operations: SettingsOperation[] = [], baseline = new Set<string>()) {
  return {
    operations,
    error: null,
    hydrated: true,
    baselineOperationIds: baseline,
    baselineTerminalOperationIds: baseline,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

describe("Settings model operation owner", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    window.localStorage.clear();
    modelsProps.current = null;
    mockSnapshot.mockReset().mockResolvedValue(healthSnapshot);
    mockOperations.mockReset().mockReturnValue(operationsSnapshot());
    mockCancel.mockReset().mockResolvedValue({
      ...operation("operation-1"),
      state: "cancelled",
      phase: "cancelled",
    });
    mockStorage.mockReset().mockResolvedValue({ id: "storage", state: "ready", items: [] });
    mockDownload.mockReset().mockResolvedValue(operation("download-start"));
    mockListModels.mockReset().mockResolvedValue([model]);
    mockActiveModel.mockReset().mockResolvedValue(model);
    mockRuntime.mockReset().mockResolvedValue("native");
    mockSpeech.mockReset().mockResolvedValue(readySpeech);
    mockVerifySpeech.mockReset().mockResolvedValue(readySpeech);
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("owns model download and cancellation errors with structured recovery copy", async () => {
    mockDownload.mockRejectedValueOnce({
      code: "model.download.repositoryUnavailable",
      message: "The model download could not start.",
      detail: "Hugging Face returned 503.",
      recoveryAction: "Check the network connection and retry.",
    });
    render(<Settings projectRoot={null} onBack={vi.fn()} />);
    await screen.findByRole("region", { name: "Owned models settings" });

    fireEvent.click(screen.getByRole("button", { name: "Download owned model" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("model.download.repositoryUnavailable");
    expect(screen.getByRole("alert")).toHaveTextContent("Hugging Face returned 503.");

    mockCancel.mockRejectedValueOnce({
      code: "settings.operation.cancelRejected",
      message: "The model download could not be cancelled.",
      detail: "The operation is already publishing.",
      recoveryAction: "Wait for verification to finish.",
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel owned model" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("settings.operation.cancelRejected"));
    expect(mockCancel).toHaveBeenCalledWith("operation-1");
  });

  it("refreshes durable speech failure after a rejected speech action", async () => {
    const failedSpeech = {
      ...readySpeech,
      ready: false,
      lastErrorCode: "speechModels.verification.failed",
      lastErrorDetail: "Pinned speech model hash mismatch.",
    };
    mockVerifySpeech.mockRejectedValueOnce({
      code: "speechModels.verification.failed",
      message: "Speech verification failed.",
      detail: "Pinned speech model hash mismatch.",
      recoveryAction: "Retry the verified download.",
    });
    mockSpeech.mockResolvedValueOnce(readySpeech).mockResolvedValueOnce(failedSpeech);
    render(<Settings projectRoot={null} onBack={vi.fn()} />);
    expect(await screen.findByText("Speech state ready")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Verify owned speech models" }));

    expect(await screen.findByText("Speech state failed")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Pinned speech model hash mismatch.");
    expect(mockSpeech).toHaveBeenCalledTimes(2);
  });

  it("retries only the durable speech status read after a mutation and status refresh both fail", async () => {
    const failedSpeech = {
      ...readySpeech,
      ready: false,
      lastErrorCode: "speechModels.verification.failed",
      lastErrorDetail: "Pinned speech model hash mismatch.",
    };
    mockVerifySpeech.mockRejectedValueOnce({
      code: "speechModels.verification.failed",
      message: "Speech verification failed.",
      detail: "Pinned speech model hash mismatch.",
      recoveryAction: "Retry the verified download.",
    });
    mockSpeech
      .mockResolvedValueOnce(readySpeech)
      .mockRejectedValueOnce(new Error("speech status temporarily unavailable"))
      .mockResolvedValueOnce(failedSpeech);
    render(<Settings projectRoot={null} onBack={vi.fn()} />);
    expect(await screen.findByText("Speech state ready")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Verify owned speech models" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("speech status temporarily unavailable");

    fireEvent.click(screen.getByRole("button", { name: "Retry owned status" }));

    expect(await screen.findByText("Speech state failed")).toBeInTheDocument();
    expect(mockVerifySpeech).toHaveBeenCalledTimes(1);
    expect(mockSpeech).toHaveBeenCalledTimes(3);
  });

  it("seeds retained terminal history without refreshing, then reconciles one live terminal once", async () => {
    const retained = operation("operation-a");
    mockOperations.mockReturnValue(operationsSnapshot([retained], new Set([retained.id])));
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    expect(await screen.findByText("Operation A reconciled no")).toBeInTheDocument();
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));

    const live = operation("operation-b");
    mockOperations.mockReturnValue(operationsSnapshot([retained, live], new Set([retained.id])));
    view.rerender(<Settings {...props} />);

    expect(await screen.findByText("Operation B reconciled yes")).toBeInTheDocument();
    expect(screen.getByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(mockListModels).toHaveBeenCalledTimes(2);

    mockOperations.mockReturnValue(operationsSnapshot([{ ...live, updatedAt: "2026-07-17T12:00:02Z" }], new Set([retained.id])));
    view.rerender(<Settings {...props} />);
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(mockListModels).toHaveBeenCalledTimes(2);
  });

  it("hydrates retained transcription history without waiting for speech status", async () => {
    const retained = operation("operation-a");
    mockOperations.mockReturnValue(operationsSnapshot([retained], new Set([retained.id])));
    mockSpeech.mockRejectedValueOnce(new Error("speech status unavailable"));

    render(<Settings projectRoot={null} onBack={vi.fn()} />);

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(mockListModels).toHaveBeenCalledTimes(1);
  });

  it("publishes model catalog loading and ready states without a false outage", async () => {
    const pendingModels = deferred<TranscriptionModelStatus[]>();
    mockListModels.mockReturnValueOnce(pendingModels.promise);

    render(<Settings projectRoot={null} onBack={vi.fn()} />);
    await screen.findByRole("region", { name: "Owned models settings" });
    expect(
      (modelsProps.current as ModelsSettingsProps & { catalogState?: string } | null)
        ?.catalogState,
    ).toBe("loading");

    pendingModels.resolve([]);
    await waitFor(() =>
      expect(
        (modelsProps.current as ModelsSettingsProps & { catalogState?: string } | null)
          ?.catalogState,
      ).toBe("ready"),
    );
  });

  it("hydrates retained speech history without waiting for transcription status", async () => {
    const retained = operation("operation-a", "speechModelsDownload");
    mockOperations.mockReturnValue(operationsSnapshot([retained], new Set([retained.id])));
    mockListModels.mockRejectedValueOnce(new Error("catalog unavailable"));
    mockRuntime.mockRejectedValueOnce(new Error("runtime unavailable"));

    render(<Settings projectRoot={null} onBack={vi.fn()} />);

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(mockSpeech).toHaveBeenCalledTimes(1);
  });

  it("reconciles a live terminal observed while model status hydration is still pending", async () => {
    const initialModels = deferred<TranscriptionModelStatus[]>();
    const initialRuntime = deferred<"native">();
    mockListModels.mockReturnValueOnce(initialModels.promise);
    mockRuntime.mockReturnValueOnce(initialRuntime.promise);
    mockOperations.mockReturnValue({
      ...operationsSnapshot(),
      hydrated: false,
    });
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));

    mockOperations.mockReturnValue(operationsSnapshot([operation("operation-a")]));
    view.rerender(<Settings {...props} />);
    initialModels.resolve([model]);
    initialRuntime.resolve("native");

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(mockListModels).toHaveBeenCalledTimes(2);
  });

  it("uses an authoritative model action result to hydrate a terminal while the initial model request is stale", async () => {
    const initialModels = deferred<TranscriptionModelStatus[]>();
    const initialRuntime = deferred<"native">();
    mockListModels
      .mockReturnValueOnce(initialModels.promise)
      .mockResolvedValue([model]);
    mockRuntime
      .mockReturnValueOnce(initialRuntime.promise)
      .mockResolvedValue("native");
    mockOperations.mockReturnValue(operationsSnapshot([operation("operation-a")]));
    render(<Settings projectRoot={null} onBack={vi.fn()} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "Recheck owned model" }));
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(2));
    initialModels.resolve([model]);
    initialRuntime.resolve("native");

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
  });

  it("recovers after initial operation polling failure without consuming a live terminal as retained", async () => {
    const retained = operation("operation-a");
    const live = operation("operation-b");
    mockOperations.mockReturnValue({
      ...operationsSnapshot(),
      hydrated: false,
      error: {
        code: "settings.operations.pollFailed",
        message: "Settings operation recovery is temporarily unavailable.",
        detail: "registry starting",
      },
    });
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));

    mockOperations.mockReturnValue(operationsSnapshot(
      [retained, live],
      new Set([retained.id]),
    ));
    view.rerender(<Settings {...props} />);

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(await screen.findByText("Operation B reconciled yes")).toBeInTheDocument();
    expect(mockListModels).toHaveBeenCalledTimes(2);
  });

  it("keeps a failed terminal refresh retryable until model and runtime status reconcile", async () => {
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));
    mockListModels.mockRejectedValueOnce(new Error("catalog temporarily unavailable"));
    mockRuntime.mockRejectedValueOnce(new Error("runtime temporarily unavailable"));
    mockOperations.mockReturnValue(operationsSnapshot([operation("operation-a")]));
    view.rerender(<Settings {...props} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("settings.models.refreshFailed");
    expect(screen.getByRole("alert")).toHaveTextContent("catalog temporarily unavailable");
    expect(screen.getByText("Operation A reconciled no")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry owned status" }));
    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(mockListModels).toHaveBeenCalledTimes(3);
    expect(mockRuntime).toHaveBeenCalledTimes(3);
  });

  it("adopts a newer authoritative model action result when terminal reconciliation completes stale", async () => {
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));
    const staleModels = deferred<TranscriptionModelStatus[]>();
    const staleRuntime = deferred<"native">();
    mockListModels.mockReturnValueOnce(staleModels.promise);
    mockRuntime.mockReturnValueOnce(staleRuntime.promise);
    mockOperations.mockReturnValue(operationsSnapshot([operation("operation-a")]));
    view.rerender(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(2));

    fireEvent.click(screen.getByRole("button", { name: "Recheck owned model" }));
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(3));
    staleModels.resolve([model]);
    staleRuntime.resolve("native");

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("drains a second terminal operation that arrives during an in-flight reconciliation", async () => {
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));
    const firstModels = deferred<TranscriptionModelStatus[]>();
    const firstRuntime = deferred<"native">();
    mockListModels.mockReturnValueOnce(firstModels.promise);
    mockRuntime.mockReturnValueOnce(firstRuntime.promise);
    const operationA = operation("operation-a");
    mockOperations.mockReturnValue(operationsSnapshot([operationA]));
    view.rerender(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(2));

    const operationB = operation("operation-b", "speechModelsDownload");
    mockOperations.mockReturnValue(operationsSnapshot([operationA, operationB]));
    view.rerender(<Settings {...props} />);
    firstModels.resolve([model]);
    firstRuntime.resolve("native");

    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(3));
    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(screen.getByText("Operation B reconciled yes")).toBeInTheDocument();
  });

  it("keeps a speech terminal in its captured lane when it arrives during transcription reconciliation", async () => {
    const props = { projectRoot: null, onBack: vi.fn() };
    const view = render(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(1));
    const firstModels = deferred<TranscriptionModelStatus[]>();
    const firstRuntime = deferred<"native">();
    mockListModels.mockReturnValueOnce(firstModels.promise);
    mockRuntime.mockReturnValueOnce(firstRuntime.promise);
    const operationA = operation("operation-a");
    mockOperations.mockReturnValue(operationsSnapshot([operationA]));
    view.rerender(<Settings {...props} />);
    await waitFor(() => expect(mockListModels).toHaveBeenCalledTimes(2));

    mockSpeech.mockRejectedValueOnce(new Error("speech lane refresh failed"));
    const operationB = operation("operation-b", "speechModelsDownload");
    mockOperations.mockReturnValue(operationsSnapshot([operationA, operationB]));
    view.rerender(<Settings {...props} />);
    firstModels.resolve([model]);
    firstRuntime.resolve("native");

    expect(await screen.findByText("Operation A reconciled yes")).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent("speech lane refresh failed");
    expect(screen.getByText("Operation B reconciled no")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry owned status" }));
    expect(await screen.findByText("Operation B reconciled yes")).toBeInTheDocument();
  });
});
