import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SettingsOperation } from "@/lib/settings/operations";
import type {
  ProductionSpeechModelStatus,
  TranscriptionModelStatus,
} from "@/lib/transcription-models";
import type { GenerationSettingsModel } from "./generation-model-multiselect";
import { defaultHostPlatform, installHostPlatform } from "@/lib/runtime/platform";
import { ModelsSettings } from "./models-settings";

const { openMock } = vi.hoisted(() => ({
  openMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

const model: TranscriptionModelStatus = {
  modelId: "nvidia/parakeet-tdt-0.6b-v3",
  displayName: "Parakeet TDT 0.6B v3",
  isActive: true,
  installStatus: "missing",
  localPath: "/models/nvidia__parakeet-tdt-0.6b-v3",
  approximateSizeBytes: 485_000_000,
  installedBytes: 0,
  downloadedFiles: 0,
  totalFiles: 18,
  sourceRepoId: "nvidia/parakeet-tdt-0.6b-v3",
  sourceRevision: "3f19d18",
  sourceLicense: "CC-BY-4.0",
  artifactFormat: "core_ml_bundle",
  runtimeId: "fluid_audio_coreml",
  lastErrorCode: null,
  lastErrorDetail: null,
};

const speechModels: ProductionSpeechModelStatus = {
  modelSetId: "silero-vad+wspk-vbx-v1",
  runtimeId: "fluid_audio_speech_analysis",
  ready: false,
  installedFiles: 0,
  totalFiles: 26,
  installedBytes: 0,
  totalBytes: 22_662_842,
  rootPath: "/models/speech-analysis/production-v1",
  vadRepo: "FluidInference/silero-vad-coreml",
  vadRevision: "b419383c55c110e2c9271fa6ee0ea83d03c70d96",
  diarizationRepo: "FluidInference/speaker-diarization-coreml",
  diarizationRevision: "1ed7a662fdc7109e36d822db793ee6eebdaf8594",
  artifactFormat: "compiled_core_ml_bundles",
  licenses: ["MIT", "CC-BY-4.0"],
};

const generationModels: GenerationSettingsModel[] = [
  {
    provider: "openai",
    id: "gpt-image-2",
    kind: "image",
    displayName: "GPT-image-2",
  },
];

function operation(
  overrides: Partial<SettingsOperation> = {},
): SettingsOperation {
  return {
    id: "operation-1",
    kind: "modelDownload",
    targetId: model.modelId,
    phase: "downloading",
    state: "running",
    completedUnits: 120_000_000,
    totalUnits: 485_000_000,
    unit: "bytes",
    cancellable: true,
    message: "Downloaded 4 of 18 files.",
    error: null,
    startedAt: "2026-07-16T10:00:00Z",
    updatedAt: "2026-07-16T10:00:01Z",
    ...overrides,
  };
}

function renderModels(
  overrides: Partial<React.ComponentProps<typeof ModelsSettings>> = {},
) {
  const handlers = {
    onDownload: vi.fn(),
    onCancelOperation: vi.fn(),
    onVerify: vi.fn(),
    onRemove: vi.fn(),
    onSetActive: vi.fn(),
    onImport: vi.fn(),
    onDownloadSpeechModels: vi.fn(),
    onVerifySpeechModels: vi.fn(),
    onRemoveSpeechModels: vi.fn(),
  };
  const result = render(
    <ModelsSettings
      models={[model]}
      runtimeSelection="native"
      operations={[]}
      operationsError={null}
      {...handlers}
      {...overrides}
    />,
  );
  return { ...handlers, ...result };
}

describe("ModelsSettings", () => {
  beforeEach(() => {
    openMock.mockReset();
  });

  it("names the on-device helper technology reported by the backend", () => {
    const { unmount } = renderModels();
    expect(screen.getByText("The on-device Core ML helper is ready.")).toBeInTheDocument();
    unmount();

    renderModels({
      models: [
        {
          ...model,
          artifactFormat: "sherpa_onnx_transducer",
          runtimeId: "sherpa_onnx",
          runtimeLabel: "ONNX",
        },
      ],
    });
    expect(screen.getByText("The on-device ONNX helper is ready.")).toBeInTheDocument();
  });

  it("offers one download action for a missing model", async () => {
    const handlers = renderModels();

    const row = screen.getByRole("article", {
      name: "Model Parakeet TDT 0.6B v3",
    });
    const download = within(row).getByRole("button", {
      name: "Download Parakeet TDT 0.6B v3",
    });
    expect(
      within(row).queryByRole("button", { name: /verify|remove|use|retry/i }),
    ).not.toBeInTheDocument();

    fireEvent.click(download);

    expect(handlers.onDownload).toHaveBeenCalledOnce();
    expect(handlers.onDownload).toHaveBeenCalledWith(model.modelId);
    await waitFor(() => expect(download).toBeEnabled());
    expect(row).not.toHaveTextContent(/copy the compiled core ml bundle/i);
  });

  it("keeps speech analysis operationally distinct with pinned diagnostics", () => {
    const handlers = renderModels({
      speechModels,
      operations: [
        operation({
          id: "speech-operation",
          kind: "speechModelsDownload",
          targetId: speechModels.modelSetId,
          completedUnits: 4_000_000,
          totalUnits: speechModels.totalBytes,
          cancellable: false,
          message: "Downloaded 4 of 26 files.",
        }),
      ],
    });

    const row = screen.getByRole("article", { name: "Speech analysis models" });
    expect(within(row).getByText("4 MB / 22.7 MB")).toBeInTheDocument();
    expect(
      within(row).getAllByText("fluid_audio_speech_analysis"),
    ).toHaveLength(2);
    expect(within(row).getByText(speechModels.vadRevision)).toBeInTheDocument();
    expect(within(row).getByText(speechModels.vadRepo)).toBeInTheDocument();
    expect(within(row).getByText(speechModels.diarizationRevision)).toBeInTheDocument();
    expect(
      within(row).getByText(speechModels.diarizationRepo),
    ).toBeInTheDocument();
    expect(within(row).getByText("MIT · CC-BY-4.0")).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: /cancel/i })).not.toBeInTheDocument();
    expect(handlers.onDownload).not.toHaveBeenCalled();
  });

  it("installs, retries, verifies, and removes only speech analysis models", async () => {
    const handlers = renderModels({ speechModels });
    const row = screen.getByRole("article", { name: "Speech analysis models" });
    fireEvent.click(within(row).getByRole("button", { name: "Install speech models" }));
    expect(handlers.onDownloadSpeechModels).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(
        within(row).getByRole("button", { name: "Install speech models" }),
      ).toBeEnabled(),
    );

    handlers.rerender(
      <ModelsSettings
        models={[model]}
        runtimeSelection="native"
        speechModels={{ ...speechModels, ready: true, installedFiles: 26, installedBytes: speechModels.totalBytes }}
        operations={[]}
        operationsError={null}
        onDownload={handlers.onDownload}
        onCancelOperation={handlers.onCancelOperation}
        onVerify={handlers.onVerify}
        onRemove={handlers.onRemove}
        onSetActive={handlers.onSetActive}
        onImport={handlers.onImport}
        onDownloadSpeechModels={handlers.onDownloadSpeechModels}
        onVerifySpeechModels={handlers.onVerifySpeechModels}
        onRemoveSpeechModels={handlers.onRemoveSpeechModels}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Verify speech models" }));
    await waitFor(() =>
      expect(handlers.onVerifySpeechModels).toHaveBeenCalledOnce(),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Verify speech models" })).toBeEnabled(),
    );
    const remove = screen.getByRole("button", { name: "Remove speech models" });
    fireEvent.click(remove);
    expect(handlers.onRemoveSpeechModels).not.toHaveBeenCalled();
    const dialog = screen.getByRole("alertdialog", { name: "Remove speech analysis models?" });
    expect(dialog).toHaveAccessibleDescription("Transcription models and projects are unaffected.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(handlers.onRemoveSpeechModels).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await waitFor(() => expect(remove).toBeEnabled());
  });

  it("uses refreshed durable speech failure as authoritative after terminal reconciliation", async () => {
    const handlers = renderModels({
      speechModels: {
        ...speechModels,
        lastErrorCode: "speechModels.verification.failed",
        lastErrorDetail: "Pinned model file hash mismatch.",
      },
      operations: [
        operation({
          id: "speech-failed",
          kind: "speechModelsDownload",
          targetId: speechModels.modelSetId,
          state: "failed",
          phase: "failed",
          cancellable: false,
          error: {
            code: "speechModels.download.failed",
            message: "Old operation failure.",
            recoveryAction: "Retry.",
            detail: "old detail",
          },
        }),
      ],
      reconciledOperationIds: new Set(["speech-failed"]),
    });

    const row = screen.getByRole("article", { name: "Speech analysis models" });
    expect(within(row).getByText("Failed")).toBeInTheDocument();
    expect(row).toHaveTextContent("speechModels.verification.failed");
    expect(row).toHaveTextContent("Pinned model file hash mismatch.");
    const retry = within(row).getByRole("button", { name: "Retry speech models" });
    fireEvent.click(retry);
    expect(handlers.onDownloadSpeechModels).toHaveBeenCalledOnce();
    await waitFor(() => expect(retry).toBeEnabled());
  });

  it("shows recovered byte and file progress with an ownership-scoped cancel action", async () => {
    const handlers = renderModels({
      operations: [operation()],
    });

    const row = screen.getByRole("article", {
      name: "Model Parakeet TDT 0.6B v3",
    });
    expect(within(row).getByText("120 MB / 485 MB")).toBeInTheDocument();
    expect(within(row).getByText("Downloaded 4 of 18 files.")).toBeInTheDocument();
    expect(
      within(row).getByRole("status", {
        name: "Parakeet TDT 0.6B v3 download status",
      }),
    ).toHaveAttribute("aria-live", "polite");
    expect(
      within(row).queryByRole("button", { name: /^(download|verify|remove|use)/i }),
    ).not.toBeInTheDocument();

    const cancel = within(row).getByRole("button", {
      name: "Cancel Parakeet TDT 0.6B v3 download",
    });
    fireEvent.click(cancel);
    expect(handlers.onCancelOperation).toHaveBeenCalledWith("operation-1");
    await waitFor(() => expect(cancel).toBeEnabled());
  });

  it("does not offer a duplicate download while catalog recovery is in progress", () => {
    renderModels({
      models: [
        {
          ...model,
          installStatus: "verifying",
          downloadedFiles: 18,
        },
      ],
    });

    const row = screen.getByRole("article", {
      name: "Model Parakeet TDT 0.6B v3",
    });
    expect(within(row).getByText("Recovering operation status…")).toHaveAttribute(
      "aria-live",
      "polite",
    );
    expect(
      within(row).queryByRole("button", {
        name: "Download Parakeet TDT 0.6B v3",
      }),
    ).not.toBeInTheDocument();
  });

  it("shows a stable failure and retries through download", async () => {
    const handlers = renderModels({
      operations: [
        operation({
          state: "failed",
          phase: "failed",
          cancellable: false,
          message: "Model download failed.",
          error: {
            code: "model.hash.mismatch",
            message: "Downloaded model verification failed.",
            recoveryAction: "Retry download.",
            detail: "weights.mlmodelc hash did not match",
          },
        }),
      ],
    });

    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Downloaded model verification failed.");
    expect(alert).toHaveTextContent("model.hash.mismatch");
    const retry = screen.getByRole("button", {
      name: "Retry Parakeet TDT 0.6B v3 download",
    });
    fireEvent.click(retry);
    expect(handlers.onDownload).toHaveBeenCalledWith(model.modelId);
    await waitFor(() => expect(retry).toBeEnabled());
  });

  it("shows ready actions and only offers Use for an inactive model", () => {
    const ready = {
      ...model,
      installStatus: "ready" as const,
      installedBytes: 485_000_000,
      downloadedFiles: 18,
      verifiedAt: "2026-07-16T10:00:00Z",
    };
    const handlers = renderModels({ models: [ready] });
    const row = screen.getByRole("article", {
      name: "Model Parakeet TDT 0.6B v3",
    });

    expect(
      within(row).getByRole("button", { name: "Verify Parakeet TDT 0.6B v3" }),
    ).toBeEnabled();
    expect(
      within(row).getByRole("button", { name: "Remove Parakeet TDT 0.6B v3" }),
    ).toBeEnabled();
    expect(
      within(row).queryByRole("button", { name: "Use Parakeet TDT 0.6B v3" }),
    ).not.toBeInTheDocument();

    handlers.rerender(
      <ModelsSettings
        models={[{ ...ready, isActive: false }]}
        runtimeSelection="native"
        operations={[]}
        operationsError={null}
        onDownload={handlers.onDownload}
        onCancelOperation={handlers.onCancelOperation}
        onVerify={handlers.onVerify}
        onRemove={handlers.onRemove}
        onSetActive={handlers.onSetActive}
        onImport={handlers.onImport}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Use Parakeet TDT 0.6B v3" }),
    ).toBeEnabled();
  });

  it("names the model and unaffected scope in an in-app dialog before removal", async () => {
    const handlers = renderModels({
      models: [{ ...model, installStatus: "ready", downloadedFiles: 18 }],
    });

    fireEvent.click(
      screen.getByRole("button", { name: "Remove Parakeet TDT 0.6B v3" }),
    );

    const dialog = screen.getByRole("alertdialog", { name: "Remove Parakeet TDT 0.6B v3?" });
    expect(dialog).toHaveAccessibleDescription("Other models and projects are unaffected.");
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    await waitFor(() => expect(cancel).toHaveFocus());
    fireEvent.click(cancel);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(handlers.onRemove).not.toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole("button", { name: "Remove Parakeet TDT 0.6B v3" }),
    );
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Remove" }),
    );
    expect(handlers.onRemove).toHaveBeenCalledWith(model.modelId);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Remove Parakeet TDT 0.6B v3" })).toBeEnabled(),
    );
  });

  it("shows provenance and error diagnostics with advanced manual recovery", async () => {
    openMock.mockResolvedValue("/Volumes/Models/parakeet");
    const handlers = renderModels({
      models: [
        {
          ...model,
          installStatus: "failed",
          lastErrorCode: "model.coreml.invalid",
          lastErrorDetail: "Compiled bundle metadata is invalid.",
        },
      ],
    });

    fireEvent.click(
      screen.getByText("Diagnostics for Parakeet TDT 0.6B v3"),
    );
    const diagnostics = screen.getByRole("group", {
      name: "Diagnostics for Parakeet TDT 0.6B v3",
    });
    expect(diagnostics).toHaveTextContent(model.sourceRepoId ?? "");
    expect(diagnostics).toHaveTextContent(model.sourceRevision ?? "");
    expect(diagnostics).toHaveTextContent(model.sourceLicense ?? "");
    expect(diagnostics).toHaveTextContent(model.runtimeId ?? "");
    expect(diagnostics).toHaveTextContent(model.localPath);
    expect(diagnostics).toHaveTextContent("model.coreml.invalid");

    fireEvent.click(screen.getByText("Advanced recovery"));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Import Parakeet TDT 0.6B v3 from folder",
      }),
    );
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
      }),
    );
    expect(handlers.onImport).toHaveBeenCalledWith(
      model.modelId,
      "/Volumes/Models/parakeet",
    );
  });

  it("shows a recoverable error when the native model folder picker fails", async () => {
    openMock
      .mockRejectedValueOnce(new Error("native dialog service unavailable"))
      .mockResolvedValueOnce(null);
    renderModels();

    fireEvent.click(screen.getByText("Advanced recovery"));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Import Parakeet TDT 0.6B v3 from folder",
      }),
    );

    const alert = await screen.findByRole("alert", {
      name: "Model folder picker failed",
    });
    expect(alert).toHaveTextContent("native dialog service unavailable");
    fireEvent.click(
      within(alert).getByRole("button", { name: "Try folder picker again" }),
    );
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(2));
  });

  it("distinguishes catalog loading from an unavailable catalog", () => {
    const loadingProps = { models: [], catalogState: "loading" as const };
    renderModels(loadingProps);

    expect(screen.getByRole("status", { name: "Loading model catalog" })).toBeInTheDocument();
    expect(screen.queryByText("Model catalog unavailable")).not.toBeInTheDocument();
  });

  it("uses catalog-unavailable copy when no models are exposed", () => {
    const failedProps = { models: [], catalogState: "failed" as const };
    renderModels(failedProps);

    expect(screen.getByText("Model catalog unavailable")).toBeInTheDocument();
    expect(
      screen.queryByText("No transcription models installed"),
    ).not.toBeInTheDocument();
  });

  it("disables model operation animation when reduced motion is requested", () => {
    const { container } = renderModels({ operations: [operation()] });

    const spinner = container.querySelector(".animate-spin");
    expect(spinner).toHaveClass("motion-reduce:animate-none");
  });

  it("renders operation sync errors as diagnostics", () => {
    renderModels({
      operationsError: {
        code: "settings.operations.pollFailed",
        message: "Settings operation recovery is temporarily unavailable.",
        detail: "journal unreadable",
      },
    });

    const warning = screen.getByRole("status", {
      name: "Model operation sync warning",
    });
    expect(warning).toHaveTextContent(
      "Settings operation recovery is temporarily unavailable.",
    );
    expect(warning).toHaveTextContent("settings.operations.pollFailed");
  });

  it("treats succeeded operations with persistence errors as warnings", () => {
    renderModels({
      operations: [
        operation({
          state: "succeeded",
          phase: "ready_with_persistence_warning",
          cancellable: false,
          completedUnits: 485_000_000,
          message: "Model is ready.",
          error: {
            code: "settings.operation.persistenceWarning",
            message: "Model is ready but progress history could not be saved.",
            recoveryAction: null,
            detail: "journal fsync failed",
          },
        }),
      ],
    });

    expect(
      screen.getByRole("status", { name: "Parakeet TDT 0.6B v3 warning" }),
    ).toHaveTextContent("Model is ready but progress history could not be saved.");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Retry Parakeet TDT 0.6B v3 download",
      }),
    ).not.toBeInTheDocument();
  });

  it("uses a reconciled missing catalog row instead of historical success", () => {
    renderModels({
      operations: [
        operation({
          state: "succeeded",
          phase: "ready",
          cancellable: false,
          completedUnits: 485_000_000,
          message: "Model is ready.",
        }),
      ],
      reconciledOperationIds: new Set(["operation-1"]),
    });

    expect(screen.getByText("Missing")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Download Parakeet TDT 0.6B v3",
      }),
    ).toBeEnabled();
    expect(
      screen.queryByRole("button", {
        name: "Verify Parakeet TDT 0.6B v3",
      }),
    ).not.toBeInTheDocument();
  });

  it("uses a reconciled ready catalog row instead of historical failure", () => {
    renderModels({
      models: [
        {
          ...model,
          installStatus: "ready",
          downloadedFiles: 18,
          installedBytes: 485_000_000,
        },
      ],
      operations: [
        operation({
          state: "failed",
          phase: "failed",
          cancellable: false,
          error: {
            code: "model.oldFailure",
            message: "Historical failure.",
            recoveryAction: "Retry.",
            detail: "Old failure detail.",
          },
        }),
      ],
      reconciledOperationIds: new Set(["operation-1"]),
    });

    const row = screen.getByRole("article", {
      name: "Model Parakeet TDT 0.6B v3",
    });
    expect(within(row).getByText("Ready")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Verify Parakeet TDT 0.6B v3",
      }),
    ).toBeEnabled();
  });

  it("renders a structured action error without hiding operation sync warnings", () => {
    renderModels({
      operationsError: {
        code: "settings.operations.pollFailed",
        message: "Operation recovery is unavailable.",
        detail: "journal unavailable",
      },
      actionError: {
        code: "model.download.startFailed",
        message: "The model download could not start.",
        detail: "repository refused the request",
        recoveryAction: "Check the network connection and retry.",
      },
    });

    const alert = screen.getByRole("alert", {
      name: "Model action failed",
    });
    expect(alert).toHaveTextContent("The model download could not start.");
    expect(alert).toHaveTextContent("model.download.startFailed");
    expect(alert).toHaveTextContent("repository refused the request");
    expect(alert).toHaveTextContent("Check the network connection and retry.");
    expect(
      screen.getByRole("status", { name: "Model operation sync warning" }),
    ).toBeInTheDocument();
  });

  it("avoids Core ML and Keychain copy on Linux", () => {
    installHostPlatform("linux");
    try {
      renderModels({
        generationModels,
        enabledGenerationModelIds: [],
        providerStatuses: [],
        onGenerationModelIdsChange: vi.fn(),
        onConfigureGenerationProvider: vi.fn(),
      });
      expect(screen.getByText("The on-device transcription helper is ready.")).toBeInTheDocument();
      expect(screen.getByRole("region", { name: "Generation models" })).toHaveTextContent(
        "Provider credentials stay in the system keyring.",
      );
      expect(document.body).not.toHaveTextContent(/Core ML|Keychain|macOS/);
    } finally {
      act(() => installHostPlatform(defaultHostPlatform));
    }
  });

  it("keeps generation model selection compact beside local model management", () => {
    renderModels({
      generationModels,
      enabledGenerationModelIds: [],
      providerStatuses: [
        {
          provider: "openai",
          displayName: "OpenAI",
          configured: true,
          source: "keychain",
        },
      ],
      onGenerationModelIdsChange: vi.fn(),
      onConfigureGenerationProvider: vi.fn(),
    });

    const generationSection = screen.getByRole("region", {
      name: "Generation models",
    });
    expect(
      within(generationSection).getByRole("combobox", {
        name: "Enabled generation models",
      }),
    ).toHaveTextContent("No remote models enabled");
    expect(
      within(generationSection).queryByRole("checkbox"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("article", { name: "Model Parakeet TDT 0.6B v3" }),
    ).toBeInTheDocument();
  });
});
