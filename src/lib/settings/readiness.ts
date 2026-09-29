import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import {
  getActiveTranscriptionModel,
  getProductionSpeechModelStatus,
  getTranscriptionRuntimeStatus,
  listTranscriptionModels,
  type ProductionSpeechModelStatus,
  type RuntimeSelection,
  type TranscriptionModelStatus,
} from "@/lib/transcription-models";

export interface SettingsReadiness {
  transcriptionModelReady: boolean;
  /** The on-device speech analysis models (VAD and speaker diarization) are installed. */
  speechModelsReady: boolean;
  runtimeReady: boolean;
}

export const defaultSettingsReadiness: Readonly<SettingsReadiness> = Object.freeze({
  transcriptionModelReady: false,
  speechModelsReady: false,
  runtimeReady: false,
});

interface SettingsReadinessSources {
  models: readonly TranscriptionModelStatus[];
  activeModel: TranscriptionModelStatus | null;
  runtimeSelection: RuntimeSelection;
  speechModels: ProductionSpeechModelStatus | null;
}

/** Derives editor readiness from the model statuses Settings shows. */
export function deriveSettingsReadiness({
  models,
  activeModel,
  runtimeSelection,
  speechModels,
}: SettingsReadinessSources): SettingsReadiness {
  const activeModelStatus = activeModel ?? models.find((model) => model.isActive) ?? null;
  return {
    transcriptionModelReady: activeModelStatus?.installStatus === "ready",
    speechModelsReady: speechModels?.ready === true,
    runtimeReady: runtimeSelection === "native",
  };
}

export function sameSettingsReadiness(left: SettingsReadiness, right: SettingsReadiness) {
  return (
    left.transcriptionModelReady === right.transcriptionModelReady &&
    left.speechModelsReady === right.speechModelsReady &&
    left.runtimeReady === right.runtimeReady
  );
}

function settledValue<T>(result: PromiseSettledResult<T>, fallback: T): T {
  if (result.status === "fulfilled") return result.value ?? fallback;
  if (!isBackendUnavailableError(result.reason)) {
    console.error("Failed to load model readiness", result.reason);
  }
  return fallback;
}

/**
 * Loads readiness from the same backend status sources Settings uses. A source that fails falls
 * back to its not-ready default, so an unavailable backend yields the default readiness.
 */
export async function loadSettingsReadiness(): Promise<SettingsReadiness> {
  const [modelsResult, activeResult, runtimeResult, speechResult] = await Promise.allSettled([
    listTranscriptionModels(),
    getActiveTranscriptionModel(),
    getTranscriptionRuntimeStatus(),
    getProductionSpeechModelStatus(),
  ]);
  const models = settledValue<TranscriptionModelStatus[]>(modelsResult, []);
  return deriveSettingsReadiness({
    models: Array.isArray(models) ? models : [],
    activeModel: settledValue<TranscriptionModelStatus | null>(activeResult, null),
    runtimeSelection: settledValue<RuntimeSelection>(runtimeResult, "unavailable"),
    speechModels: settledValue<ProductionSpeechModelStatus | null>(speechResult, null),
  });
}
