import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSettingsHealthSnapshot } from "./health";
import {
  cancelSettingsOperation,
  listSettingsOperations,
  type SettingsOperation,
} from "./operations";
import { downloadTranscriptionModel } from "../transcription-models";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: invokeMock,
}));

const terminalWarning: SettingsOperation = {
  id: "operation-1",
  kind: "modelDownload",
  targetId: "nvidia/parakeet-tdt-0.6b-v3",
  phase: "ready_with_persistence_warning",
  state: "succeeded",
  completedUnits: 485_000_000,
  totalUnits: 485_000_000,
  unit: "bytes",
  cancellable: false,
  message: "Model is ready, but operation history could not be persisted.",
  error: {
    code: "settings.operation.terminalPersistenceFailed",
    message: "The model is ready, but operation history could not be saved.",
    recoveryAction: "Restart Video Creater to reconcile history.",
    detail: "journal write failed",
  },
  startedAt: "2026-07-16T10:00:00Z",
  updatedAt: "2026-07-16T10:01:00Z",
};

describe("settings Tauri adapters", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("requests the shared settings health snapshot by its exact command name", async () => {
    invokeMock.mockResolvedValue({
      generatedAt: "2026-07-16T10:00:00Z",
      overall: "ready",
      categories: {},
    });

    await getSettingsHealthSnapshot();

    expect(invokeMock).toHaveBeenCalledWith("get_settings_health_snapshot", {
      projectRoot: null,
    });
  });

  it("lists settings operations by its exact command name", async () => {
    invokeMock.mockResolvedValue([terminalWarning]);

    await expect(listSettingsOperations()).resolves.toEqual([terminalWarning]);
    expect(invokeMock).toHaveBeenCalledWith("list_settings_operations");
  });

  it("cancels an operation with the camel-case operationId payload", async () => {
    invokeMock.mockResolvedValue(terminalWarning);

    await cancelSettingsOperation("operation-1");

    expect(invokeMock).toHaveBeenCalledWith("cancel_settings_operation", {
      operationId: "operation-1",
    });
  });

  it("starts a transcription download with a camel-case modelId and returns its operation", async () => {
    invokeMock.mockResolvedValue(terminalWarning);

    await expect(
      downloadTranscriptionModel("nvidia/parakeet-tdt-0.6b-v3"),
    ).resolves.toEqual(terminalWarning);
    expect(invokeMock).toHaveBeenCalledWith("download_transcription_model", {
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
    });
  });
});
