import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import type {
  ProductionSpeechModelStatus,
  TranscriptionModelStatus,
} from "@/lib/transcription-models";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  defaultSettingsReadiness,
  deriveSettingsReadiness,
  loadSettingsReadiness,
} from "./readiness";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn() }));

const backendMock = vi.mocked(backendRequest);

function transcriptionModel(
  overrides: Partial<TranscriptionModelStatus> = {},
): TranscriptionModelStatus {
  return {
    modelId: "nvidia/parakeet-tdt-0.6b-v3",
    displayName: "Parakeet TDT 0.6B v3",
    isActive: true,
    installStatus: "ready",
    localPath: "/models/parakeet",
    approximateSizeBytes: 485_000_000,
    downloadedFiles: 18,
    totalFiles: 18,
    ...overrides,
  };
}

function speechModels(ready: boolean): ProductionSpeechModelStatus {
  return {
    modelSetId: "production-speech-analysis-v1",
    runtimeId: "onnx",
    ready,
    installedFiles: ready ? 7 : 0,
    totalFiles: 7,
    installedBytes: ready ? 344_000_000 : 0,
    totalBytes: 344_000_000,
    rootPath: "/models/speech-analysis",
    vadRepo: "silero-vad",
    vadRevision: "main",
    diarizationRepo: "speaker-diarization",
    diarizationRevision: "main",
    artifactFormat: "ONNX",
    licenses: ["MIT"],
  };
}

function mockBackend(responses: Record<string, unknown>) {
  backendMock.mockImplementation(async (command: string) => {
    if (!(command in responses)) throw new Error(`unexpected command ${command}`);
    const response = responses[command];
    if (response instanceof Error) throw response;
    return response;
  });
}

describe("settings readiness", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    backendMock.mockReset();
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("reports ready when the active model, speech models and native runtime are installed", async () => {
    mockBackend({
      list_transcription_models: [transcriptionModel()],
      get_active_transcription_model: transcriptionModel(),
      get_transcription_runtime_status: "native",
      get_production_speech_model_status: speechModels(true),
    });

    await expect(loadSettingsReadiness()).resolves.toEqual({
      transcriptionModelReady: true,
      speechModelsReady: true,
      runtimeReady: true,
    });
    expect(backendMock).toHaveBeenCalledWith("get_transcription_runtime_status", {
      modelId: null,
    });
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it("falls back to the active entry in the model list when no active model is reported", async () => {
    mockBackend({
      list_transcription_models: [
        transcriptionModel({ modelId: "other", isActive: false, installStatus: "missing" }),
        transcriptionModel(),
      ],
      get_active_transcription_model: null,
      get_transcription_runtime_status: "native",
      get_production_speech_model_status: speechModels(true),
    });

    await expect(loadSettingsReadiness()).resolves.toMatchObject({
      transcriptionModelReady: true,
    });
  });

  it("reports not ready when models are missing and the runtime is unsupported", async () => {
    mockBackend({
      list_transcription_models: [transcriptionModel({ installStatus: "missing" })],
      get_active_transcription_model: transcriptionModel({ installStatus: "downloading" }),
      get_transcription_runtime_status: "unsupported_platform",
      get_production_speech_model_status: speechModels(false),
    });

    await expect(loadSettingsReadiness()).resolves.toEqual(defaultSettingsReadiness);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it("returns the defaults quietly when the backend is unavailable", async () => {
    backendMock.mockRejectedValue(new BackendUnavailableError());

    await expect(loadSettingsReadiness()).resolves.toEqual(defaultSettingsReadiness);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it("keeps the sources that loaded and logs a failing one", async () => {
    const failure = new Error("speech model store is unreadable");
    mockBackend({
      list_transcription_models: [transcriptionModel()],
      get_active_transcription_model: transcriptionModel(),
      get_transcription_runtime_status: "native",
      get_production_speech_model_status: failure,
    });

    await expect(loadSettingsReadiness()).resolves.toEqual({
      transcriptionModelReady: true,
      speechModelsReady: false,
      runtimeReady: true,
    });
    expect(consoleErrorSpy).toHaveBeenCalledWith("Failed to load model readiness", failure);
  });

  it("derives readiness the same way from Settings state", () => {
    expect(
      deriveSettingsReadiness({
        models: [transcriptionModel({ isActive: true, installStatus: "failed" })],
        activeModel: null,
        runtimeSelection: "native",
        speechModels: null,
      }),
    ).toEqual({ transcriptionModelReady: false, speechModelsReady: false, runtimeReady: true });
  });
});
