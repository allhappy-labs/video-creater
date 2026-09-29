import { backendRequest } from "@/lib/runtime/backend-client";
import type { SettingsOperation } from "./settings/operations";

type ModelInstallStatus =
  | "missing"
  | "downloading"
  | "verifying"
  | "ready"
  | "failed";

export type RuntimeSelection = "native" | "unsupported_platform" | "unavailable";

type TranscriptionInstallStatus = ModelInstallStatus;
type TranscriptionRuntimeSelection = RuntimeSelection;

export interface TranscriptionModelStatus {
  modelId: string;
  displayName: string;
  isActive: boolean;
  installStatus: ModelInstallStatus;
  localPath: string;
  approximateSizeBytes: number;
  installedBytes?: number;
  downloadedFiles: number;
  totalFiles: number;
  sourceRepoId?: string | null;
  sourceRevision?: string | null;
  sourceLicense?: string | null;
  artifactFormat?: string;
  runtimeId?: string;
  /** Backend-provided name of the on-device helper technology, e.g. "Core ML" or "ONNX". */
  runtimeLabel?: string;
  lastErrorCode?: string | null;
  lastErrorDetail?: string | null;
  verifiedAt?: string | null;
}

export interface ProductionSpeechModelStatus {
  modelSetId: string;
  runtimeId: string;
  ready: boolean;
  installedFiles: number;
  totalFiles: number;
  installedBytes: number;
  totalBytes: number;
  rootPath: string;
  vadRepo: string;
  vadRevision: string;
  diarizationRepo: string;
  diarizationRevision: string;
  artifactFormat: string;
  licenses: string[];
  lastErrorCode?: string | null;
  lastErrorDetail?: string | null;
  lastErrorRecoveryAction?: string | null;
}

export function listTranscriptionModels(): Promise<TranscriptionModelStatus[]> {
  return backendRequest("list_transcription_models");
}

export function getProductionSpeechModelStatus(): Promise<ProductionSpeechModelStatus> {
  return backendRequest("get_production_speech_model_status");
}

export function downloadProductionSpeechModels(): Promise<SettingsOperation> {
  return backendRequest("download_production_speech_models");
}

export function verifyProductionSpeechModels(): Promise<ProductionSpeechModelStatus> {
  return backendRequest("verify_production_speech_models");
}

export function removeProductionSpeechModels(): Promise<ProductionSpeechModelStatus> {
  return backendRequest("remove_production_speech_models");
}

export function getActiveTranscriptionModel(): Promise<TranscriptionModelStatus> {
  return backendRequest("get_active_transcription_model");
}

export function setActiveTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return backendRequest("set_active_transcription_model", { modelId });
}

export function downloadTranscriptionModel(modelId: string): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("download_transcription_model", { modelId });
}

export function importTranscriptionModel(
  modelId: string,
  sourcePath: string,
): Promise<TranscriptionModelStatus> {
  return backendRequest("import_transcription_model", { modelId, sourcePath });
}

export function verifyTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return backendRequest("verify_transcription_model", { modelId });
}

export function removeTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return backendRequest("remove_transcription_model", { modelId });
}

export function getTranscriptionRuntimeStatus(modelId?: string): Promise<RuntimeSelection> {
  return backendRequest("get_transcription_runtime_status", { modelId: modelId ?? null });
}
