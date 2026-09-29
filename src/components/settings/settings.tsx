import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  generationModelPreferenceId,
  loadAppSettingsPreferences,
  normalizeGenerationModelPreferenceIds,
  updateAppPreferencesFromIntent,
  type AppPreferenceIntent,
  type AppSettingsPreferences,
} from "@/lib/app-settings";
import { listGenerationModelCatalog } from "@/lib/project";
import {
  listProviderCredentialStatuses,
  type ProviderCredentialStatus,
} from "@/lib/provider-credentials";
import {
  cancelSettingsOperation,
  type SettingsOperation,
} from "@/lib/settings/operations";
import {
  type AppSettingsCategory,
  type AppSettingsTarget,
} from "@/lib/settings/target";
import {
  deriveSettingsReadiness,
  type SettingsReadiness,
} from "@/lib/settings/readiness";
import { useSettingsOperations } from "@/lib/settings/use-settings-operations";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import {
  downloadProductionSpeechModels,
  downloadTranscriptionModel,
  getActiveTranscriptionModel,
  getProductionSpeechModelStatus,
  getTranscriptionRuntimeStatus,
  importTranscriptionModel,
  listTranscriptionModels,
  removeProductionSpeechModels,
  removeTranscriptionModel,
  setActiveTranscriptionModel,
  verifyProductionSpeechModels,
  verifyTranscriptionModel,
  type ProductionSpeechModelStatus,
  type RuntimeSelection,
  type TranscriptionModelStatus,
} from "@/lib/transcription-models";
import {
  AdvancedSettings,
  type LocalModelImportResult,
} from "./advanced-settings";
import { GeneralSettings } from "./general-settings";
import type { GenerationSettingsModel } from "./generation-model-multiselect";
import {
  ModelsSettings,
  type ModelSettingsActionError,
} from "./models-settings";
import { ProvidersSettings } from "./providers-settings";
import { ProjectsSettings } from "./projects-settings";
import { RemoteAccessSettings } from "./remote-access-settings";
import {
  SettingsShell,
} from "./settings-shell";
import { StorageSettings } from "./storage-settings";

export type { SettingsCategory } from "./settings-shell";
export type { AppSettingsCategory, AppSettingsTarget } from "@/lib/settings/target";

export interface SettingsProps {
  initialCategory?: AppSettingsCategory;
  target?: AppSettingsTarget;
  navigationRequestId?: number;
  projectRoot: string | null;
  onBack: () => void | Promise<void>;
  preferences?: AppSettingsPreferences;
  onPreferencesAccepted?: (preferences: AppSettingsPreferences) => void;
  onReadinessChange?: (readiness: SettingsReadiness) => void;
  onOpenSystemHealth?: () => void;
}

type ModelOperationLane = "transcription" | "speech";

type ModelStatusRefreshResult = {
  transcriptionReconciled: boolean;
  speechReconciled: boolean;
  detail: string | null;
  speechDetail: string | null;
};

type PreferenceUpdateError = {
  intent: AppPreferenceIntent;
  requestId: number;
  detail: string;
};

const terminalStates = new Set(["succeeded", "failed", "cancelled"]);

function modelOperationLane(kind: string): ModelOperationLane | null {
  if (kind === "modelDownload") return "transcription";
  if (kind === "speechModelsDownload") return "speech";
  return null;
}

function unknownErrorDetail(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function structuredErrorField(
  error: unknown,
  field: "code" | "message" | "detail" | "recoveryAction",
): string | null {
  if (
    typeof error === "object" &&
    error !== null &&
    field in error &&
    typeof (error as Record<string, unknown>)[field] === "string"
  ) {
    const value = (error as Record<string, unknown>)[field];
    return typeof value === "string" ? value : null;
  }
  return null;
}

function normalizeModelActionError(
  error: unknown,
  fallback: ModelSettingsActionError,
): ModelSettingsActionError {
  return {
    code: structuredErrorField(error, "code") ?? fallback.code,
    message: structuredErrorField(error, "message") ?? fallback.message,
    detail:
      structuredErrorField(error, "detail") ??
      (error === null || error === undefined ? fallback.detail : unknownErrorDetail(error)),
    recoveryAction:
      structuredErrorField(error, "recoveryAction") ?? fallback.recoveryAction,
  };
}

function isGenerationSettingsKind(value: unknown): value is GenerationSettingsModel["kind"] {
  return value === "image" || value === "video" || value === "audio" || value === "upscale";
}

function generationSettingsModelLabel(model: {
  provider: string;
  id: string;
  displayName?: unknown;
}) {
  return typeof model.displayName === "string" && model.displayName.trim()
    ? model.displayName.trim()
    : `${model.provider}/${model.id}`;
}

function sameStringArray(left: readonly string[], right: readonly string[]) {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

type SettingsVisualQaModule = typeof import("@/lib/settings-visual-qa-fixtures");

export function Settings(props: SettingsProps) {
  const [visualQaModule, setVisualQaModule] = useState<SettingsVisualQaModule | null>(null);
  const fixtureRequested = import.meta.env.DEV && typeof window !== "undefined" && (
    window as Window & {
      __EDITOR_FIXTURE_RUNTIME__?: { enabled?: boolean; settingsFixtureId?: string };
    }
  ).__EDITOR_FIXTURE_RUNTIME__?.settingsFixtureId !== undefined;

  useEffect(() => {
    let cancelled = false;
    if (!fixtureRequested) return;
    void import("@/lib/settings-visual-qa-fixtures").then((module) => {
      if (!cancelled) setVisualQaModule(module);
    });
    return () => {
      cancelled = true;
    };
  }, [fixtureRequested]);

  if (fixtureRequested && !visualQaModule) {
    return (
      <div role="status" className="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
        Loading deterministic Settings state
      </div>
    );
  }
  const fixture = visualQaModule?.readSettingsVisualQaFixture() ?? null;
  return <SettingsContent {...props} />;
}

function SettingsContent({
  initialCategory = "aiModels",
  target,
  navigationRequestId = 0,
  projectRoot,
  onBack,
  preferences,
  onPreferencesAccepted,
  onReadinessChange,
  onOpenSystemHealth = () => {},
}: SettingsProps) {
  const [localAppSettings, setLocalAppSettings] =
    useState<AppSettingsPreferences>(loadAppSettingsPreferences);
  const appSettings = preferences ?? localAppSettings;
  const [desiredGenerationModelIds, setDesiredGenerationModelIds] = useState(
    () =>
      normalizeGenerationModelPreferenceIds(
        appSettings.enabledGenerationModelIds,
      ),
  );
  const [preferenceUpdateError, setPreferenceUpdateError] =
    useState<PreferenceUpdateError | null>(null);
  const [generationPreferenceUpdateError, setGenerationPreferenceUpdateError] =
    useState<PreferenceUpdateError | null>(null);
  const [internalNavigation, setInternalNavigation] = useState<{
    target: AppSettingsTarget;
    requestId: number;
    externalRequestId: number;
  } | null>(null);
  const [generationModels, setGenerationModels] = useState<GenerationSettingsModel[]>([]);
  const [providerCredentialStatuses, setProviderCredentialStatuses] = useState<
    ProviderCredentialStatus[]
  >([]);
  const [models, setModels] = useState<TranscriptionModelStatus[]>([]);
  const [modelCatalogState, setModelCatalogState] = useState<
    "loading" | "ready" | "failed"
  >("loading");
  const [activeModel, setActiveModel] = useState<TranscriptionModelStatus | null>(null);
  const [runtimeSelection, setRuntimeSelection] = useState<RuntimeSelection>("unavailable");
  const [speechModels, setSpeechModels] = useState<ProductionSpeechModelStatus | null>(null);
  const [transcriptionStatusHydrated, setTranscriptionStatusHydrated] = useState(false);
  const [speechStatusHydrated, setSpeechStatusHydrated] = useState(false);
  const [modelActionError, setModelActionError] = useState<ModelSettingsActionError | null>(null);
  const [reconciledTerminalOperationIds, setReconciledTerminalOperationIds] =
    useState<ReadonlySet<string>>(() => new Set());
  const modelActionRetryRef = useRef<(() => Promise<unknown>) | null>(null);
  const pendingTerminalOperations = useRef(new Map<string, ModelOperationLane>());
  const terminalRefreshInFlight = useRef(false);
  const modelStatusRequestRef = useRef(0);
  const latestModelStatusRefreshRef = useRef<Promise<ModelStatusRefreshResult> | null>(null);
  const preferenceRequestIdRef = useRef(0);
  const generationPreferenceRequestIdRef = useRef(0);
  const pendingGenerationPreferenceRequestIdsRef = useRef(new Set<number>());
  const acceptedGenerationModelIdsRef = useRef(
    normalizeGenerationModelPreferenceIds(
      appSettings.enabledGenerationModelIds,
    ),
  );
  const credentialStatusGenerationRef = useRef(0);
  const externalNavigationTarget: AppSettingsTarget = target ?? {
    category: initialCategory,
  };
  const activeInternalNavigation =
    internalNavigation?.externalRequestId === navigationRequestId ? internalNavigation : null;
  const navigationTarget = activeInternalNavigation?.target ?? externalNavigationTarget;
  const effectiveNavigationRequestId =
    activeInternalNavigation?.requestId ?? navigationRequestId;

  useEffect(() => {
    setInternalNavigation((current) =>
      current && current.externalRequestId !== navigationRequestId ? null : current,
    );
  }, [navigationRequestId]);
  const {
    operations,
    error: operationsError,
    hydrated: operationsHydrated,
    baselineTerminalOperationIds,
  } = useSettingsOperations();

  useEffect(() => {
    let cancelled = false;
    void listGenerationModelCatalog()
      .then((payload) => {
        if (cancelled) return;
        setGenerationModels(
          payload.loaded
            ? payload.generationModels.flatMap((model): GenerationSettingsModel[] => {
                if (
                  typeof model.provider !== "string" ||
                  typeof model.id !== "string" ||
                  !isGenerationSettingsKind(model.kind)
                ) {
                  return [];
                }
                return [{
                  provider: model.provider,
                  id: model.id,
                  kind: model.kind,
                  displayName: generationSettingsModelLabel(model),
                }];
              })
            : [],
        );
      })
      .catch(() => {
        if (!cancelled) setGenerationModels([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const requestGeneration = credentialStatusGenerationRef.current;
    void listProviderCredentialStatuses()
      .then((statuses) => {
        if (
          !cancelled &&
          credentialStatusGenerationRef.current === requestGeneration
        ) {
          setProviderCredentialStatuses(statuses);
        }
      })
      .catch(() => {
        if (
          !cancelled &&
          credentialStatusGenerationRef.current === requestGeneration
        ) {
          setProviderCredentialStatuses([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const acceptedIds = normalizeGenerationModelPreferenceIds(
      appSettings.enabledGenerationModelIds,
    );
    acceptedGenerationModelIdsRef.current = acceptedIds;
    if (pendingGenerationPreferenceRequestIdsRef.current.size === 0) {
      setDesiredGenerationModelIds((current) =>
        sameStringArray(current, acceptedIds) ? current : acceptedIds,
      );
    }
  }, [appSettings.enabledGenerationModelIds]);

  const refreshModelStatus = useCallback(() => {
    const requestId = ++modelStatusRequestRef.current;
    const request: Promise<ModelStatusRefreshResult> = (async () => {
      const [modelsResult, activeResult, runtimeResult, speechResult] = await Promise.allSettled([
        listTranscriptionModels(),
        getActiveTranscriptionModel(),
        getTranscriptionRuntimeStatus(),
        getProductionSpeechModelStatus(),
      ]);
      if (modelStatusRequestRef.current !== requestId) {
        const latestRequest = latestModelStatusRefreshRef.current;
        if (latestRequest) return latestRequest;
        return {
          transcriptionReconciled: false,
          speechReconciled: false,
          detail: "A newer model status request superseded this refresh.",
          speechDetail: "A newer speech-model status request superseded this refresh.",
        };
      }
      if (modelsResult.status === "fulfilled" && runtimeResult.status === "fulfilled") {
        setTranscriptionStatusHydrated(true);
      }
      if (speechResult.status === "fulfilled") {
        setSpeechStatusHydrated(true);
      }
      if (modelsResult.status === "fulfilled") {
        setModelCatalogState("ready");
        setModels(modelsResult.value);
      } else {
        setModelCatalogState("failed");
        setModels([]);
        if (!isBackendUnavailableError(modelsResult.reason)) {
          console.error("Failed to load transcription models", modelsResult.reason);
        }
      }
      if (activeResult.status === "fulfilled") setActiveModel(activeResult.value);
      else setActiveModel(null);
      if (runtimeResult.status === "fulfilled") setRuntimeSelection(runtimeResult.value);
      else setRuntimeSelection("unavailable");
      if (speechResult.status === "fulfilled") setSpeechModels(speechResult.value);
      else setSpeechModels(null);
      const failures = [
        modelsResult.status === "rejected" ? `Catalog: ${unknownErrorDetail(modelsResult.reason)}` : null,
        runtimeResult.status === "rejected" ? `Runtime: ${unknownErrorDetail(runtimeResult.reason)}` : null,
      ].filter((detail): detail is string => detail !== null);
      return {
        transcriptionReconciled: modelsResult.status === "fulfilled" && runtimeResult.status === "fulfilled",
        speechReconciled: speechResult.status === "fulfilled",
        detail: failures.length > 0 ? failures.join(" ") : null,
        speechDetail: speechResult.status === "rejected" ? `Speech models: ${unknownErrorDetail(speechResult.reason)}` : null,
      };
    })();
    latestModelStatusRefreshRef.current = request;
    return request;
  }, []);

  useEffect(() => {
    void refreshModelStatus().then(({ transcriptionReconciled, speechReconciled }) => {
      if (transcriptionReconciled) setTranscriptionStatusHydrated(true);
      if (speechReconciled) setSpeechStatusHydrated(true);
    });
  }, [refreshModelStatus]);

  const { transcriptionModelReady, speechModelsReady, runtimeReady } = deriveSettingsReadiness({
    models,
    activeModel,
    runtimeSelection,
    speechModels,
  });
  useEffect(() => {
    onReadinessChange?.({ transcriptionModelReady, speechModelsReady, runtimeReady });
  }, [onReadinessChange, runtimeReady, speechModelsReady, transcriptionModelReady]);

  const clearModelActionError = useCallback(() => {
    modelActionRetryRef.current = null;
    setModelActionError(null);
  }, []);

  const reconcilePendingTerminalOperations = useCallback(async () => {
    if (terminalRefreshInFlight.current || pendingTerminalOperations.current.size === 0) return;
    terminalRefreshInFlight.current = true;
    clearModelActionError();
    try {
      while (pendingTerminalOperations.current.size > 0) {
        const batch = [...pendingTerminalOperations.current.entries()];
        const result = await refreshModelStatus();
        if (result.transcriptionReconciled) setTranscriptionStatusHydrated(true);
        if (result.speechReconciled) setSpeechStatusHydrated(true);
        const reconciled: string[] = [];
        const failed = new Set<ModelOperationLane>();
        for (const [operationId, lane] of batch) {
          const laneReady = lane === "speech" ? result.speechReconciled : result.transcriptionReconciled;
          if (laneReady) {
            pendingTerminalOperations.current.delete(operationId);
            reconciled.push(operationId);
          } else failed.add(lane);
        }
        if (reconciled.length > 0) {
          setReconciledTerminalOperationIds((current) => new Set([...current, ...reconciled]));
        }
        if (failed.size > 0) {
          const details = [
            failed.has("transcription") ? result.detail : null,
            failed.has("speech") ? result.speechDetail : null,
          ].filter((detail): detail is string => detail !== null);
          modelActionRetryRef.current = reconcilePendingTerminalOperations;
          setModelActionError({
            code: "settings.models.refreshFailed",
            message: "The model operation finished, but Settings could not refresh its current status.",
            detail: details.length > 0 ? details.join(" ") : null,
            recoveryAction: "Retry the status refresh. The completed operation remains recoverable.",
          });
          return;
        }
        clearModelActionError();
      }
    } finally {
      terminalRefreshInFlight.current = false;
    }
  }, [clearModelActionError, refreshModelStatus]);

  useEffect(() => {
    if (!operationsHydrated) return;
    const retained: string[] = [];
    let pending = false;
    for (const operation of operations) {
      const lane = modelOperationLane(operation.kind);
      if (!lane || !terminalStates.has(operation.state)) continue;
      const laneHydrated = lane === "speech" ? speechStatusHydrated : transcriptionStatusHydrated;
      if (!laneHydrated) continue;
      if (baselineTerminalOperationIds.has(operation.id)) {
        if (!reconciledTerminalOperationIds.has(operation.id)) retained.push(operation.id);
        continue;
      }
      if (!reconciledTerminalOperationIds.has(operation.id) && !pendingTerminalOperations.current.has(operation.id)) {
        pendingTerminalOperations.current.set(operation.id, lane);
        pending = true;
      }
    }
    if (retained.length > 0) {
      setReconciledTerminalOperationIds((current) => new Set([...current, ...retained]));
    }
    if (pending) void reconcilePendingTerminalOperations();
  }, [
    baselineTerminalOperationIds,
    operations,
    operationsHydrated,
    reconciledTerminalOperationIds,
    reconcilePendingTerminalOperations,
    speechStatusHydrated,
    transcriptionStatusHydrated,
  ]);

  async function retryModelStatusRefresh() {
    clearModelActionError();
    const result = await refreshModelStatus();
    if (result.transcriptionReconciled) {
      setTranscriptionStatusHydrated(true);
      clearModelActionError();
      return true;
    }
    modelActionRetryRef.current = retryModelStatusRefresh;
    setModelActionError({
      code: "settings.models.refreshFailed",
      message: "Settings could not refresh the current model status.",
      detail: result.detail,
      recoveryAction: "Retry the status refresh.",
    });
    return false;
  }

  async function runModelAction(
    action: (modelId: string) => Promise<unknown>,
    modelId: string,
    fallback: ModelSettingsActionError,
  ) {
    clearModelActionError();
    let failed = false;
    try {
      await action(modelId);
    } catch (error) {
      failed = true;
      setModelActionError(normalizeModelActionError(error, fallback));
    }
    const result = await refreshModelStatus();
    if (!failed && result.transcriptionReconciled) {
      setTranscriptionStatusHydrated(true);
      clearModelActionError();
    } else if (!failed) {
      modelActionRetryRef.current = retryModelStatusRefresh;
      setModelActionError({
        code: "settings.models.refreshFailed",
        message: "The model action completed, but Settings could not refresh its current status.",
        detail: result.detail,
        recoveryAction: "Retry the status refresh.",
      });
    }
  }

  async function importLocalModelFromAdvanced(
    modelId: string,
    sourcePath: string,
  ): Promise<LocalModelImportResult> {
    clearModelActionError();
    try {
      await importTranscriptionModel(modelId, sourcePath);
    } catch (error) {
      setModelActionError(normalizeModelActionError(error, {
        code: "model.import.failed",
        message: "The model folder could not be imported.",
        detail: null,
        recoveryAction: "Check the selected folder and try the import again.",
      }));
      throw error;
    }
    const result = await refreshModelStatus();
    if (result.transcriptionReconciled) {
      setTranscriptionStatusHydrated(true);
      clearModelActionError();
      return { status: "imported" };
    }
    modelActionRetryRef.current = retryModelStatusRefresh;
    const detail =
      result.detail ?? "The current model inventory could not be refreshed.";
    setModelActionError({
      code: "settings.models.refreshFailed",
      message: "The model was imported, but Settings could not refresh its current status.",
      detail,
      recoveryAction: "Retry the status refresh.",
    });
    return { status: "importedRefreshFailed", detail };
  }

  async function runModelDownload(modelId: string) {
    clearModelActionError();
    try {
      await downloadTranscriptionModel(modelId);
    } catch (error) {
      setModelActionError(normalizeModelActionError(error, {
        code: "model.download.startFailed",
        message: "The model download could not start.",
        detail: null,
        recoveryAction: "Check the network connection and retry.",
      }));
    }
  }

  async function cancelOperation(operationId: string) {
    clearModelActionError();
    try {
      await cancelSettingsOperation(operationId);
    } catch (error) {
      setModelActionError(normalizeModelActionError(error, {
        code: "settings.operation.cancelFailed",
        message: "The model download could not be cancelled.",
        detail: null,
        recoveryAction: "Wait for the operation to settle, then retry or remove the model.",
      }));
    }
  }

  async function runSpeechAction(
    action: () => Promise<ProductionSpeechModelStatus>,
    fallback: ModelSettingsActionError,
  ) {
    clearModelActionError();
    try {
      setSpeechModels(await action());
    } catch (error) {
      const actionError = normalizeModelActionError(error, fallback);
      try {
        setSpeechModels(await getProductionSpeechModelStatus());
        setSpeechStatusHydrated(true);
        setModelActionError(actionError);
      } catch (statusError) {
        modelActionRetryRef.current = () => retrySpeechStatusRefresh(actionError);
        setModelActionError({
          ...actionError,
          detail: [actionError.detail, `Status refresh: ${unknownErrorDetail(statusError)}`]
            .filter(Boolean)
            .join(" "),
          recoveryAction: "Retry the status refresh. The original model action error remains available.",
        });
      }
    }
  }

  async function retrySpeechStatusRefresh(actionError: ModelSettingsActionError) {
    modelActionRetryRef.current = null;
    try {
      setSpeechModels(await getProductionSpeechModelStatus());
      setSpeechStatusHydrated(true);
      setModelActionError(actionError);
    } catch (statusError) {
      modelActionRetryRef.current = () => retrySpeechStatusRefresh(actionError);
      setModelActionError({
        ...actionError,
        detail: [actionError.detail, `Status refresh: ${unknownErrorDetail(statusError)}`]
          .filter(Boolean)
          .join(" "),
        recoveryAction: "Retry the status refresh. The original model action error remains available.",
      });
    }
  }

  function updateAppSettings(intent: AppPreferenceIntent) {
    const generationIds = intent.enabledGenerationModelIds
      ? normalizeGenerationModelPreferenceIds(
          intent.enabledGenerationModelIds,
        )
      : null;
    const generationRequestId = generationIds
      ? ++generationPreferenceRequestIdRef.current
      : null;
    const preferenceRequestId = generationIds
      ? null
      : ++preferenceRequestIdRef.current;
    const normalizedIntent = generationIds
      ? { ...intent, enabledGenerationModelIds: generationIds }
      : intent;
    if (generationIds !== null && generationRequestId !== null) {
      pendingGenerationPreferenceRequestIdsRef.current.add(
        generationRequestId,
      );
      setDesiredGenerationModelIds(generationIds);
      setGenerationPreferenceUpdateError(null);
    } else {
      setPreferenceUpdateError(null);
    }
    void updateAppPreferencesFromIntent(normalizedIntent)
      .then((accepted) => {
        if (generationRequestId !== null) {
          pendingGenerationPreferenceRequestIdsRef.current.delete(
            generationRequestId,
          );
          const acceptedIds = normalizeGenerationModelPreferenceIds(
            accepted.enabledGenerationModelIds,
          );
          acceptedGenerationModelIdsRef.current = acceptedIds;
          if (
            generationPreferenceRequestIdRef.current === generationRequestId
          ) {
            setDesiredGenerationModelIds(acceptedIds);
          }
          setGenerationPreferenceUpdateError((current) =>
            current && current.requestId <= generationRequestId
              ? null
              : current,
          );
        } else if (preferenceRequestId !== null) {
          setPreferenceUpdateError((current) =>
            current && current.requestId <= preferenceRequestId
              ? null
              : current,
          );
        }
        if (!preferences) setLocalAppSettings(accepted);
        onPreferencesAccepted?.(accepted);
      })
      .catch((error) => {
        if (generationRequestId !== null) {
          pendingGenerationPreferenceRequestIdsRef.current.delete(
            generationRequestId,
          );
          if (
            generationPreferenceRequestIdRef.current === generationRequestId
          ) {
            setDesiredGenerationModelIds(
              acceptedGenerationModelIdsRef.current,
            );
            setGenerationPreferenceUpdateError({
              intent: normalizedIntent,
              requestId: generationRequestId,
              detail: unknownErrorDetail(error),
            });
          }
        } else if (
          preferenceRequestId !== null &&
          preferenceRequestIdRef.current === preferenceRequestId
        ) {
          setPreferenceUpdateError({
            intent: normalizedIntent,
            requestId: preferenceRequestId,
            detail: unknownErrorDetail(error),
          });
        }
      });
  }

  function acceptProviderCredentialStatus(status: ProviderCredentialStatus) {
    credentialStatusGenerationRef.current += 1;
    setProviderCredentialStatuses((current) => {
      const provider = status.provider.trim().toLowerCase();
      const next = current.filter(
        (candidate) => candidate.provider.trim().toLowerCase() !== provider,
      );
      return [...next, status].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      );
    });
  }

  function openSettingsTarget(nextTarget: AppSettingsTarget) {
    setInternalNavigation((current) => ({
      target: nextTarget,
      requestId: (current?.requestId ?? navigationRequestId) + 1,
      externalRequestId: navigationRequestId,
    }));
  }

  const generationModelPreferenceIds = generationModels.flatMap((model) => {
    const preferenceId = generationModelPreferenceId(model);
    return preferenceId ? [preferenceId] : [];
  });
  const enabledGenerationModelIds = new Set(desiredGenerationModelIds);
  const disabledGenerationModelIds = generationModelPreferenceIds.filter(
    (modelId) => !enabledGenerationModelIds.has(modelId),
  );
  const visiblePreferenceUpdateError =
    generationPreferenceUpdateError ?? preferenceUpdateError;

  return (
    <SettingsShell
      initialCategory={navigationTarget.category}
      navigationRequestId={effectiveNavigationRequestId}
      navigationTarget={navigationTarget}
      focusTargetReady
      onBack={onBack}
      projectRoot={projectRoot}
      notice={
        visiblePreferenceUpdateError ? (
          <section
            role="alert"
            className="flex max-w-2xl flex-wrap items-center justify-between gap-2 border-l-2 border-red-500 pl-3 text-xs"
          >
            <div className="min-w-0">
              <p className="font-medium text-foreground">
                This change was not saved.
              </p>
              <p className="break-words text-muted-foreground">
                {visiblePreferenceUpdateError.detail}
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 shrink-0 px-2 text-xs"
              onClick={() =>
                updateAppSettings(visiblePreferenceUpdateError.intent)
              }
              aria-label="Retry saving settings change"
            >
              <RotateCcw className="h-3.5 w-3.5" aria-hidden="true" />
              Retry
            </Button>
          </section>
        ) : null
      }
    >
      {(activeCategory) => {
        if (activeCategory === "aiModels") {
          return (
            <>
              <ModelsSettings
              models={models}
              generationModels={generationModels}
              enabledGenerationModelIds={desiredGenerationModelIds}
              providerStatuses={providerCredentialStatuses}
              catalogState={modelCatalogState}
              runtimeSelection={runtimeSelection}
              operations={operations}
              operationsError={operationsError}
              reconciledOperationIds={reconciledTerminalOperationIds}
              actionError={modelActionError}
              onRetryAction={modelActionRetryRef.current ? () => modelActionRetryRef.current?.() : null}
              speechModels={speechModels}
              onDownload={runModelDownload}
              onCancelOperation={cancelOperation}
              onVerify={(modelId) => runModelAction(verifyTranscriptionModel, modelId, {
                code: "model.verify.failed",
                message: "The model could not be verified.",
                detail: null,
                recoveryAction: "Retry verification or download the model again.",
              })}
              onRemove={(modelId) => runModelAction(removeTranscriptionModel, modelId, {
                code: "model.remove.failed",
                message: "The model could not be removed.",
                detail: null,
                recoveryAction: "Close active transcription jobs and retry.",
              })}
              onSetActive={(modelId) => runModelAction(setActiveTranscriptionModel, modelId, {
                code: "model.activate.failed",
                message: "The active transcription model could not be changed.",
                detail: null,
                recoveryAction: "Verify the model, then try again.",
              })}
              onImport={(modelId, sourcePath) => runModelAction(
                (resolvedModelId) => importTranscriptionModel(resolvedModelId, sourcePath),
                modelId,
                {
                  code: "model.import.failed",
                  message: "The model folder could not be imported.",
                  detail: null,
                  recoveryAction: "Check the selected folder and try the import again.",
                },
              )}
              onDownloadSpeechModels={async () => {
                try {
                  await downloadProductionSpeechModels();
                } catch (error) {
                  setModelActionError(normalizeModelActionError(error, {
                    code: "speechModels.download.startFailed",
                    message: "Speech analysis model setup could not start.",
                    detail: null,
                    recoveryAction: "Check the network connection and retry.",
                  }));
                }
              }}
              onVerifySpeechModels={() => runSpeechAction(verifyProductionSpeechModels, {
                code: "speechModels.verification.failed",
                message: "Speech analysis model verification failed.",
                detail: null,
                recoveryAction: "Retry the verified download.",
              })}
              onRemoveSpeechModels={() => runSpeechAction(removeProductionSpeechModels, {
                code: "speechModels.remove.failed",
                message: "Speech analysis models could not be removed.",
                detail: null,
                recoveryAction: "Close active analysis jobs and retry.",
              })}
              onGenerationModelIdsChange={(enabledGenerationModelIds) =>
                updateAppSettings({ enabledGenerationModelIds })
              }
              onConfigureGenerationProvider={(provider) =>
                openSettingsTarget({ category: "integrations", provider })
              }
              />
            </>
          );
        }
        if (activeCategory === "projects") {
          return (
            <ProjectsSettings
              preferences={appSettings}
              onChange={updateAppSettings}
            />
          );
        }
        if (activeCategory === "general") {
          return (
            <GeneralSettings
              preferences={appSettings}
              onPreferencesChange={updateAppSettings}
            />
          );
        }
        if (activeCategory === "advanced") {
          return (
            <AdvancedSettings
              preferences={appSettings}
              projectRoot={projectRoot}
              models={models}
              onChange={updateAppSettings}
              onImportLocalModel={importLocalModelFromAdvanced}
              onRetryModelStatus={retryModelStatusRefresh}
              onOpenSystemHealth={onOpenSystemHealth}
            />
          );
        }
        if (activeCategory === "storage") {
          return (
            <StorageSettings
              projectLocation={appSettings.projectLocation}
              onProjectLocationChange={(projectLocation) => updateAppSettings({ projectLocation })}
              activeProjectDir={projectRoot}
              operations={operations}
              onOpenSettingsTarget={openSettingsTarget}
            />
          );
        }
        if (activeCategory === "remoteAccess") {
          return <RemoteAccessSettings />;
        }
        return (
          <ProvidersSettings
              disabledGenerationModelIds={disabledGenerationModelIds}
              generationModels={generationModels}
              onCredentialStatusChange={acceptProviderCredentialStatus}
              operations={operations}
          />
        );
      }}
    </SettingsShell>
  );
}
