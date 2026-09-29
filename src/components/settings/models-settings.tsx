import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  Download,
  HardDrive,
  Loader2,
  RotateCw,
  Server,
  Trash2,
  XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ConfirmRemovalDialog, type RemovalConfirmation } from "./confirm-removal-dialog";
import type {
  SettingsOperation,
  SettingsOperationState,
} from "@/lib/settings/operations";
import type { SettingsOperationsSyncError } from "@/lib/settings/use-settings-operations";
import type {
  ProductionSpeechModelStatus,
  RuntimeSelection,
  TranscriptionModelStatus,
} from "@/lib/transcription-models";
import type { ProviderCredentialStatus } from "@/lib/provider-credentials";
import { useHostPlatformCopy, type HostPlatformCopy } from "@/lib/runtime/platform";
import {
  GenerationModelMultiselect,
  type GenerationSettingsModel,
} from "./generation-model-multiselect";
import { SettingsDiagnostics } from "./settings-diagnostics";
import { SettingsOperationProgress } from "./settings-operation-status";
import { SettingsStatus } from "./settings-status";

type ModelAction = (modelId: string) => void | Promise<unknown>;
type ModelImportAction = (
  modelId: string,
  sourcePath: string,
) => void | Promise<unknown>;
type OperationAction = (
  operationId: string,
) => void | Promise<unknown>;

export interface ModelSettingsActionError {
  code: string;
  message: string;
  detail: string | null;
  recoveryAction: string | null;
}

export interface ModelsSettingsProps {
  models: TranscriptionModelStatus[];
  generationModels?: GenerationSettingsModel[];
  enabledGenerationModelIds?: string[];
  providerStatuses?: ProviderCredentialStatus[];
  catalogState?: "loading" | "ready" | "failed";
  runtimeSelection: RuntimeSelection;
  operations: SettingsOperation[];
  operationsError: SettingsOperationsSyncError | null;
  reconciledOperationIds?: ReadonlySet<string>;
  actionError?: ModelSettingsActionError | null;
  onRetryAction?: (() => void | Promise<unknown>) | null;
  speechModels?: ProductionSpeechModelStatus | null;
  onDownload: ModelAction;
  onCancelOperation: OperationAction;
  onVerify: ModelAction;
  onRemove: ModelAction;
  onSetActive: ModelAction;
  onImport: ModelImportAction;
  onDownloadSpeechModels?: () => void | Promise<unknown>;
  onVerifySpeechModels?: () => void | Promise<unknown>;
  onRemoveSpeechModels?: () => void | Promise<unknown>;
  onGenerationModelIdsChange?: (modelIds: string[]) => void;
  onConfigureGenerationProvider?: (provider: string) => void;
}

const activeOperationStates: ReadonlySet<SettingsOperationState> = new Set([
  "queued",
  "running",
  "cancelling",
]);

function formatBytes(bytes: number) {
  if (bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1000 && unitIndex < units.length - 1) {
    value /= 1000;
    unitIndex += 1;
  }
  return `${new Intl.NumberFormat("en", {
    maximumFractionDigits: value >= 10 ? 1 : 2,
  }).format(value)} ${units[unitIndex]}`;
}

function runtimeCopy(
  runtimeSelection: RuntimeSelection,
  platformCopy: HostPlatformCopy,
  runtimeLabel?: string,
) {
  switch (runtimeSelection) {
    case "native":
      return {
        value: "Ready",
        detail: runtimeLabel
          ? `The on-device ${runtimeLabel} helper is ready.`
          : platformCopy.transcriptionHelperReady,
        tone: "ready" as const,
      };
    case "unsupported_platform":
      return {
        value: "Unsupported",
        detail: "This platform cannot run the native helper.",
        tone: "warning" as const,
      };
    case "unavailable":
      return {
        value: "Unavailable",
        detail: "The local helper did not start.",
        tone: "error" as const,
      };
  }
}

function latestModelOperation(
  operations: SettingsOperation[],
  modelId: string,
  reconciledOperationIds: ReadonlySet<string>,
) {
  return operations
    .filter(
      (operation) =>
        operation.kind === "modelDownload" &&
        operation.targetId === modelId &&
        (!reconciledOperationIds.has(operation.id) ||
          activeOperationStates.has(operation.state)),
    )
    .sort(
      (left, right) =>
        Date.parse(right.updatedAt) - Date.parse(left.updatedAt),
    )[0];
}

function latestSpeechModelOperation(
  operations: SettingsOperation[],
  modelSetId: string,
  reconciledOperationIds: ReadonlySet<string>,
) {
  return operations
    .filter(
      (operation) =>
        operation.kind === "speechModelsDownload" &&
        operation.targetId === modelSetId &&
        (!reconciledOperationIds.has(operation.id) ||
          activeOperationStates.has(operation.state)),
    )
    .sort(
      (left, right) =>
        Date.parse(right.updatedAt) - Date.parse(left.updatedAt),
    )[0];
}

function statusLabel(
  model: TranscriptionModelStatus,
  operation: SettingsOperation | undefined,
) {
  if (operation && activeOperationStates.has(operation.state)) {
    if (operation.state === "queued") {
      return "Queued";
    }
    if (operation.state === "cancelling") {
      return "Cancelling";
    }
    return "Downloading";
  }
  if (operation?.state === "succeeded") {
    return "Ready";
  }
  if (operation?.state === "failed" || model.installStatus === "failed") {
    return "Failed";
  }
  if (model.installStatus === "ready") {
    return "Ready";
  }
  if (model.installStatus === "verifying") {
    return "Verifying";
  }
  return "Missing";
}

function statusClasses(label: string) {
  switch (label) {
    case "Ready":
      return "border-emerald-200 bg-emerald-50 text-emerald-800";
    case "Failed":
      return "border-red-200 bg-red-50 text-red-800";
    case "Downloading":
    case "Queued":
    case "Cancelling":
    case "Verifying":
      return "border-blue-200 bg-blue-50 text-blue-800";
    default:
      return "border-amber-200 bg-amber-50 text-amber-800";
  }
}

function OperationStateIcon({ label }: { label: string }) {
  if (label === "Ready") {
    return <CheckCircle2 className="h-4 w-4" aria-hidden="true" />;
  }
  if (label === "Failed") {
    return <XCircle className="h-4 w-4" aria-hidden="true" />;
  }
  if (
    label === "Downloading" ||
    label === "Queued" ||
    label === "Cancelling" ||
    label === "Verifying"
  ) {
    return (
      <Loader2
        className="h-4 w-4 animate-spin motion-reduce:animate-none"
        aria-hidden="true"
      />
    );
  }
  return <HardDrive className="h-4 w-4" aria-hidden="true" />;
}

function DiagnosticValue({
  label,
  value,
}: {
  label: string;
  value: string | null | undefined;
}) {
  return (
    <div className="grid gap-0.5 md:grid-cols-[8rem_minmax(0,1fr)]">
      <span>{label}</span>
      <code className="break-all font-mono text-foreground">
        {value?.trim() || "Unavailable"}
      </code>
    </div>
  );
}

export function ModelsSettings({
  models,
  generationModels,
  enabledGenerationModelIds = [],
  providerStatuses = [],
  catalogState = "ready",
  runtimeSelection,
  operations,
  operationsError,
  reconciledOperationIds = new Set<string>(),
  actionError = null,
  onRetryAction = null,
  speechModels,
  onDownload,
  onCancelOperation,
  onVerify,
  onRemove,
  onSetActive,
  onImport,
  onDownloadSpeechModels,
  onVerifySpeechModels,
  onRemoveSpeechModels,
  onGenerationModelIdsChange,
  onConfigureGenerationProvider,
}: ModelsSettingsProps) {
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const [pickerError, setPickerError] = useState<{
    model: TranscriptionModelStatus;
    detail: string;
  } | null>(null);
  const [removal, setRemoval] = useState<RemovalConfirmation | null>(null);
  const platformCopy = useHostPlatformCopy();
  const runtime = runtimeCopy(
    runtimeSelection,
    platformCopy,
    models.find((model) => model.isActive)?.runtimeLabel,
  );

  async function runAction(key: string, action: () => void | Promise<unknown>) {
    setPendingAction(key);
    try {
      await action();
    } finally {
      setPendingAction((current) => (current === key ? null : current));
    }
  }

  async function importFromFolder(model: TranscriptionModelStatus) {
    setPickerError(null);
    await runAction(`import:${model.modelId}`, async () => {
      let selected: string | string[] | null;
      try {
        selected = await open({ directory: true, multiple: false });
      } catch (error) {
        setPickerError({
          model,
          detail:
            error instanceof Error ? error.message : String(error),
        });
        return;
      }
      const selectedPath = Array.isArray(selected) ? selected[0] : selected;
      if (!selectedPath) return;
      await onImport(model.modelId, selectedPath);
    });
  }

  return (
    <div className="grid gap-5">
      {generationModels &&
      onGenerationModelIdsChange &&
      onConfigureGenerationProvider ? (
        <section
          role="region"
          data-settings-target="aiModels:generationModels"
          tabIndex={-1}
          aria-labelledby="generation-models-settings-heading"
          className="grid gap-2 border-t pt-3 outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <div>
            <h2
              id="generation-models-settings-heading"
              className="text-sm font-semibold tracking-normal"
            >
              Generation models
            </h2>
            <p className="mt-1 text-xs text-muted-foreground">
              Choose the remote models shown in generation tools. Provider
              credentials stay in {platformCopy.credentialStore}.
            </p>
          </div>
          {generationModels.length > 0 ? (
            <GenerationModelMultiselect
              models={generationModels}
              enabledIds={enabledGenerationModelIds}
              providerStatuses={providerStatuses}
              onChange={onGenerationModelIdsChange}
              onConfigureProvider={onConfigureGenerationProvider}
            />
          ) : (
            <p className="text-xs text-muted-foreground">
              No generation models are available in the current catalog.
            </p>
          )}
        </section>
      ) : null}

      <section
        data-settings-target="aiModels:transcription"
        tabIndex={-1}
        aria-label="Transcription models"
        className="grid gap-3 outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
      <SettingsStatus
        ariaLabel="Transcription runtime"
        label="Transcription runtime"
        value={runtime.value}
        detail={runtime.detail}
        tone={runtime.tone}
        icon={<Server className="h-4 w-4" aria-hidden="true" />}
      />

      {speechModels !== undefined ? (
        <SpeechModelsRow
          status={speechModels}
          operation={
            speechModels
              ? latestSpeechModelOperation(
                  operations,
                  speechModels.modelSetId,
                  reconciledOperationIds,
                )
              : undefined
          }
          pendingAction={pendingAction}
          runAction={runAction}
          onRequestRemoval={setRemoval}
          {...(onDownloadSpeechModels ? { onDownload: onDownloadSpeechModels } : {})}
          {...(onVerifySpeechModels ? { onVerify: onVerifySpeechModels } : {})}
          {...(onRemoveSpeechModels ? { onRemove: onRemoveSpeechModels } : {})}
        />
      ) : null}

      {actionError ? (
        <div
          role="alert"
          aria-label="Model action failed"
          className="border-l-2 border-red-500 pl-3 text-xs text-red-800"
        >
          <div className="font-medium">{actionError.message}</div>
          <code className="font-mono text-[11px]">{actionError.code}</code>
          {actionError.detail ? (
            <div className="text-[11px]">{actionError.detail}</div>
          ) : null}
          {actionError.recoveryAction ? (
            <div className="text-[11px]">{actionError.recoveryAction}</div>
          ) : null}
          {onRetryAction ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="mt-2"
              onClick={() => void onRetryAction()}
            >
              Retry status refresh
            </Button>
          ) : null}
        </div>
      ) : null}

      {pickerError ? (
        <div
          role="alert"
          aria-label="Model folder picker failed"
          className="border-l-2 border-red-500 pl-3 text-xs text-red-800"
        >
          <div className="font-medium">The model folder chooser could not open.</div>
          <div className="text-[11px]">{pickerError.detail}</div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-2"
            onClick={() => void importFromFolder(pickerError.model)}
          >
            Try folder picker again
          </Button>
        </div>
      ) : null}

      {operationsError ? (
        <div
          role="status"
          aria-label="Model operation sync warning"
          className="border-l-2 border-amber-500 pl-3 text-xs text-amber-800"
        >
          <div className="font-medium">{operationsError.message}</div>
          <code className="font-mono text-[11px]">{operationsError.code}</code>
          <div className="text-[11px]">{operationsError.detail}</div>
        </div>
      ) : null}

      {models.length === 0 && catalogState === "loading" ? (
        <div
          role="status"
          aria-label="Loading model catalog"
          className="flex min-h-48 items-center justify-center gap-2 border-y border-dashed bg-muted/20 px-4 py-8 text-xs text-muted-foreground"
        >
          <Loader2
            className="h-4 w-4 animate-spin motion-reduce:animate-none"
            aria-hidden="true"
          />
          Loading model catalog
        </div>
      ) : null}

      {models.length === 0 && catalogState === "failed" ? (
        <div className="flex min-h-48 items-center justify-center border-y border-dashed bg-muted/20 px-4 py-8 text-center">
          <div className="max-w-md space-y-2">
            <HardDrive
              className="mx-auto h-5 w-5 text-muted-foreground"
              aria-hidden="true"
            />
            <h2 className="text-sm font-semibold tracking-normal">
              Model catalog unavailable
            </h2>
            <p className="text-xs text-muted-foreground">
              Video Creater could not load the transcription model catalog.
              Reopen Settings or inspect runtime diagnostics.
            </p>
          </div>
        </div>
      ) : null}

      {models.length === 0 && catalogState === "ready" ? (
        <div className="flex min-h-48 items-center justify-center border-y border-dashed bg-muted/20 px-4 py-8 text-center text-xs text-muted-foreground">
          No transcription models are available in this build.
        </div>
      ) : null}

      {models.map((model) => {
        const latestOperation = latestModelOperation(
          operations,
          model.modelId,
          reconciledOperationIds,
        );
        const activeOperation =
          latestOperation && activeOperationStates.has(latestOperation.state)
            ? latestOperation
            : undefined;
        const succeededOperation =
          latestOperation?.state === "succeeded" ? latestOperation : undefined;
        const failedOperation =
          model.installStatus !== "ready" &&
          latestOperation?.state === "failed"
            ? latestOperation
            : undefined;
        const effectiveReady =
          model.installStatus === "ready" || succeededOperation !== undefined;
        const effectiveFailed =
          !effectiveReady &&
          (model.installStatus === "failed" || failedOperation !== undefined);
        const catalogBusy =
          model.installStatus === "downloading" ||
          model.installStatus === "verifying";
        const label = statusLabel(model, latestOperation);
        const actionPending =
          pendingAction?.endsWith(`:${model.modelId}`) ?? false;
        const failureCode =
          failedOperation?.error?.code ?? model.lastErrorCode ?? null;
        const failureMessage =
          failedOperation?.error?.message ??
          model.lastErrorDetail ??
          "The model could not be installed or verified.";
        const cancelUnavailableReasonId = `model-${model.modelId}-cancel-unavailable`;

        return (
          <article
            key={model.modelId}
            aria-label={`Model ${model.displayName}`}
            className="border-t pt-3"
          >
            <div className="flex items-start justify-between gap-3 pb-3">
              <div className="min-w-0 space-y-1">
                <div className="flex min-w-0 flex-wrap items-center gap-2">
                  <h2 className="truncate text-sm font-semibold leading-none tracking-normal">
                    {model.displayName}
                  </h2>
                  {model.isActive ? (
                    <span className="inline-flex shrink-0 items-center rounded-md border border-primary/25 bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                      Active
                    </span>
                  ) : null}
                </div>
                <p className="text-xs text-muted-foreground">{model.modelId}</p>
              </div>
              <span
                className={`inline-flex shrink-0 items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${statusClasses(label)}`}
              >
                <OperationStateIcon label={label} />
                {label}
              </span>
            </div>

            <div className="grid gap-2 text-xs md:grid-cols-3">
              <div className="border-t pt-2">
                <div className="text-muted-foreground">Installed</div>
                <div className="font-medium">
                  {activeOperation
                    ? (
                        <SettingsOperationProgress
                          operation={activeOperation}
                          ariaLabel={`${model.displayName} download progress`}
                          {...(activeOperation.unit === "bytes"
                            ? { formatValue: formatBytes }
                            : {})}
                          unitLabel={
                            activeOperation.unit === "bytes"
                              ? ""
                              : activeOperation.unit ?? "units"
                          }
                        />
                      )
                    : `${formatBytes(model.installedBytes ?? 0)} / ${formatBytes(model.approximateSizeBytes)}`}
                </div>
              </div>
              <div className="border-t pt-2">
                <div className="text-muted-foreground">Files</div>
                <div className="font-medium">
                  {model.downloadedFiles} / {model.totalFiles} files
                </div>
              </div>
              <div className="border-t pt-2">
                <div className="text-muted-foreground">Runtime</div>
                <div className="truncate font-mono text-[11px] font-medium">
                  {model.runtimeId || "Unavailable"}
                </div>
              </div>
            </div>

            {activeOperation ? (
              <div className="mt-3 flex flex-col gap-3 border-t pt-3 sm:flex-row sm:items-center sm:justify-between">
                <div
                  role="status"
                  aria-label={`${model.displayName} download status`}
                  aria-live="polite"
                  aria-atomic="true"
                  className="min-w-0 text-xs text-muted-foreground"
                >
                  <div className="font-medium text-foreground">
                    {activeOperation.phase}
                  </div>
                  <div>{activeOperation.message}</div>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={!activeOperation.cancellable || actionPending}
                  aria-describedby={
                    !activeOperation.cancellable
                      ? cancelUnavailableReasonId
                      : undefined
                  }
                  onClick={() =>
                    void runAction(`cancel:${model.modelId}`, () =>
                      onCancelOperation(activeOperation.id),
                    )
                  }
                  aria-label={`Cancel ${model.displayName} download`}
                >
                  <XCircle className="h-4 w-4" aria-hidden="true" />
                  Cancel
                </Button>
                {!activeOperation.cancellable ? (
                  <p
                    id={cancelUnavailableReasonId}
                    className="text-xs text-muted-foreground sm:max-w-52"
                  >
                    This download is in a phase that cannot be safely cancelled.
                  </p>
                ) : null}
              </div>
            ) : null}

            {!activeOperation && catalogBusy ? (
              <div className="mt-3 border-t pt-3 text-xs text-muted-foreground">
                <div aria-live="polite" className="font-medium text-foreground">
                  Recovering operation status…
                </div>
                <div>
                  Video Creater is reconnecting this model to its operation
                  journal.
                </div>
              </div>
            ) : null}

            {effectiveFailed ? (
              <div
                role="alert"
                className="mt-3 border-l-2 border-red-500 pl-3 text-xs text-red-800"
              >
                <div className="font-medium">{failureMessage}</div>
                {failureCode ? (
                  <code className="font-mono text-[11px]">{failureCode}</code>
                ) : null}
                {failedOperation?.error?.detail ? (
                  <div className="text-[11px]">
                    {failedOperation.error.detail}
                  </div>
                ) : null}
              </div>
            ) : null}

            {succeededOperation?.error ? (
              <div
                role="status"
                aria-label={`${model.displayName} warning`}
                className="mt-3 border-l-2 border-amber-500 pl-3 text-xs text-amber-800"
              >
                <div className="font-medium">
                  {succeededOperation.error.message}
                </div>
                <code className="font-mono text-[11px]">
                  {succeededOperation.error.code}
                </code>
                {succeededOperation.error.detail ? (
                  <div className="text-[11px]">
                    {succeededOperation.error.detail}
                  </div>
                ) : null}
              </div>
            ) : null}

            {!activeOperation && !catalogBusy ? (
              <div className="mt-3 flex flex-col gap-3 border-t pt-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-muted-foreground">
                  {effectiveReady
                    ? model.verifiedAt
                      ? `Model verified ${new Date(model.verifiedAt).toLocaleString()}`
                      : "Model verified for local transcription."
                    : effectiveFailed
                      ? "Retry the verified open-source download."
                      : latestOperation?.state === "cancelled"
                        ? "Download cancelled. No staged model was published."
                        : "Download and verify the open-source model automatically."}
                </p>
                <div className="flex flex-wrap gap-2">
                  {effectiveReady ? (
                    <>
                      {!model.isActive ? (
                        <Button
                          type="button"
                          size="sm"
                          disabled={actionPending}
                          onClick={() =>
                            void runAction(`use:${model.modelId}`, () =>
                              onSetActive(model.modelId),
                            )
                          }
                          aria-label={`Use ${model.displayName}`}
                        >
                          <CheckCircle2
                            className="h-4 w-4"
                            aria-hidden="true"
                          />
                          Use
                        </Button>
                      ) : null}
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={actionPending}
                        onClick={() =>
                          void runAction(`verify:${model.modelId}`, () =>
                            onVerify(model.modelId),
                          )
                        }
                        aria-label={`Verify ${model.displayName}`}
                      >
                        <RotateCw className="h-4 w-4" aria-hidden="true" />
                        Verify
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="text-red-700 hover:text-red-800"
                        disabled={actionPending}
                        onClick={() =>
                          setRemoval({
                            title: `Remove ${model.displayName}?`,
                            description: "Other models and projects are unaffected.",
                            onConfirm: () =>
                              void runAction(`remove:${model.modelId}`, () =>
                                onRemove(model.modelId),
                              ),
                          })
                        }
                        aria-label={`Remove ${model.displayName}`}
                      >
                        <Trash2 className="h-4 w-4" aria-hidden="true" />
                        Remove
                      </Button>
                    </>
                  ) : (
                    <Button
                      type="button"
                      size="sm"
                      disabled={actionPending}
                      onClick={() =>
                        void runAction(`download:${model.modelId}`, () =>
                          onDownload(model.modelId),
                        )
                      }
                      aria-label={`${
                        effectiveFailed ? "Retry" : "Download"
                      } ${model.displayName}${
                        effectiveFailed ? " download" : ""
                      }`}
                    >
                      <Download className="h-4 w-4" aria-hidden="true" />
                      {effectiveFailed ? "Retry download" : "Download model"}
                    </Button>
                  )}
                </div>
              </div>
            ) : null}

            <SettingsDiagnostics
              label={`Diagnostics for ${model.displayName}`}
            >
              <DiagnosticValue label="Repository" value={model.sourceRepoId} />
              <DiagnosticValue label="Revision" value={model.sourceRevision} />
              <DiagnosticValue label="License" value={model.sourceLicense} />
              <DiagnosticValue label="Artifact" value={model.artifactFormat} />
              <DiagnosticValue label="Runtime" value={model.runtimeId} />
              <DiagnosticValue label="Storage path" value={model.localPath} />
              <DiagnosticValue label="Error code" value={failureCode} />
              <DiagnosticValue
                label="Error detail"
                value={model.lastErrorDetail}
              />
            </SettingsDiagnostics>

            <SettingsDiagnostics label="Advanced recovery">
              <p>
                Import an already downloaded model folder only when automatic
                setup cannot reach the source repository.
              </p>
              <div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={Boolean(activeOperation) || actionPending}
                  onClick={() => void importFromFolder(model)}
                  aria-label={`Import ${model.displayName} from folder`}
                >
                  <HardDrive className="h-4 w-4" aria-hidden="true" />
                  Import folder
                </Button>
              </div>
            </SettingsDiagnostics>
          </article>
        );
      })}
      </section>
      <ConfirmRemovalDialog
        confirmation={removal}
        onClose={() => setRemoval(null)}
      />
    </div>
  );
}

function SpeechModelsRow({
  status,
  operation,
  pendingAction,
  runAction,
  onRequestRemoval,
  onDownload,
  onVerify,
  onRemove,
}: {
  status: ProductionSpeechModelStatus | null | undefined;
  operation: SettingsOperation | undefined;
  pendingAction: string | null;
  runAction: (
    key: string,
    action: () => void | Promise<unknown>,
  ) => Promise<void>;
  onRequestRemoval: (confirmation: RemovalConfirmation) => void;
  onDownload?: () => void | Promise<unknown>;
  onVerify?: () => void | Promise<unknown>;
  onRemove?: () => void | Promise<unknown>;
}) {
  const active =
    operation && activeOperationStates.has(operation.state)
      ? operation
      : undefined;
  const failed = operation?.state === "failed" ? operation : undefined;
  const durableFailure = status?.lastErrorCode
    ? {
        code: status.lastErrorCode,
        message:
          status.lastErrorDetail ?? "Speech analysis model setup failed.",
        detail: status.lastErrorDetail,
        recoveryAction: status.lastErrorRecoveryAction,
      }
    : undefined;
  const ready =
    !durableFailure &&
    !failed &&
    (status?.ready === true || operation?.state === "succeeded");
  const label = active
    ? active.state === "queued"
      ? "Queued"
      : "Downloading"
    : failed || durableFailure
      ? "Failed"
      : ready
        ? "Ready"
        : "Missing";
  const actionPending = pendingAction?.startsWith("speech:") ?? false;

  return (
    <article aria-label="Speech analysis models" className="border-t pt-3">
      <div className="flex items-start justify-between gap-3 pb-3">
        <div className="min-w-0 space-y-1">
          <h2 className="text-sm font-semibold leading-none tracking-normal">
            Speech analysis
          </h2>
          <p className="text-xs text-muted-foreground">
            Separate on-device VAD and speaker diarization models for dead-air
            detection and speaker masks.
          </p>
        </div>
        <span
          className={`inline-flex shrink-0 items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${statusClasses(label)}`}
        >
          <OperationStateIcon label={label} />
          {label}
        </span>
      </div>

      {status ? (
        <>
          <div className="grid gap-2 text-xs md:grid-cols-3">
            <div className="border-t pt-2">
              <div className="text-muted-foreground">Installed</div>
              <div className="font-medium">
                {active
                  ? (
                      <SettingsOperationProgress
                        operation={active}
                        ariaLabel="Speech analysis download progress"
                        {...(active.unit === "bytes"
                          ? { formatValue: formatBytes }
                          : {})}
                        unitLabel={active.unit === "bytes" ? "" : active.unit ?? "units"}
                      />
                    )
                  : `${formatBytes(status.installedBytes)} / ${formatBytes(status.totalBytes)}`}
              </div>
            </div>
            <div className="border-t pt-2">
              <div className="text-muted-foreground">Files</div>
              <div className="font-medium">
                {status.installedFiles} / {status.totalFiles} files
              </div>
            </div>
            <div className="border-t pt-2">
              <div className="text-muted-foreground">Runtime</div>
              <div className="truncate font-mono text-[11px] font-medium">
                {status.runtimeId}
              </div>
            </div>
          </div>

          <SettingsDiagnostics label="Speech analysis diagnostics">
            <DiagnosticValue label="Model set" value={status.modelSetId} />
            <DiagnosticValue label="Runtime" value={status.runtimeId} />
            <DiagnosticValue label="VAD repository" value={status.vadRepo} />
            <DiagnosticValue label="VAD revision" value={status.vadRevision} />
            <DiagnosticValue
              label="Diarization repository"
              value={status.diarizationRepo}
            />
            <DiagnosticValue
              label="Diarization revision"
              value={status.diarizationRevision}
            />
            <DiagnosticValue
              label="Artifact"
              value={status.artifactFormat}
            />
            <DiagnosticValue
              label="Licenses"
              value={status.licenses.join(" · ")}
            />
            <DiagnosticValue label="Storage path" value={status.rootPath} />
            <DiagnosticValue label="Error code" value={status.lastErrorCode} />
            <DiagnosticValue
              label="Error detail"
              value={status.lastErrorDetail}
            />
          </SettingsDiagnostics>
        </>
      ) : (
        <p className="border-t pt-3 text-xs text-muted-foreground">
          Speech analysis model status is unavailable.
        </p>
      )}

      {active ? (
        <div
          role="status"
          aria-label="Speech analysis download status"
          aria-live="polite"
          aria-atomic="true"
          className="mt-3 border-t pt-3 text-xs text-muted-foreground"
        >
          <div className="font-medium text-foreground">
            {active.phase}
          </div>
          <div>{active.message}</div>
          {!active.cancellable ? (
            <div className="mt-1">
              This verified multi-file install cannot be safely cancelled.
            </div>
          ) : null}
        </div>
      ) : null}

      {failed?.error || durableFailure ? (
        <div
          role="alert"
          className="mt-3 border-l-2 border-red-500 pl-3 text-xs text-red-800"
        >
          <div className="font-medium">
            {failed?.error?.message ?? durableFailure?.message}
          </div>
          <code className="font-mono text-[11px]">
            {failed?.error?.code ?? durableFailure?.code}
          </code>
          {failed?.error?.detail ?? durableFailure?.detail ? (
            <div className="text-[11px]">
              {failed?.error?.detail ?? durableFailure?.detail}
            </div>
          ) : null}
        </div>
      ) : null}

      {!active && status ? (
        <div className="mt-3 flex justify-end gap-2 border-t pt-3">
          {ready ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={actionPending || !onVerify}
                onClick={() =>
                  void runAction("speech:verify", () => onVerify?.())
                }
                aria-label="Verify speech models"
              >
                <RotateCw className="h-4 w-4" aria-hidden="true" />
                Verify
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="text-red-700 hover:text-red-800"
                disabled={actionPending || !onRemove}
                onClick={() =>
                  onRequestRemoval({
                    title: "Remove speech analysis models?",
                    description: "Transcription models and projects are unaffected.",
                    onConfirm: () =>
                      void runAction("speech:remove", () => onRemove?.()),
                  })
                }
                aria-label="Remove speech models"
              >
                <Trash2 className="h-4 w-4" aria-hidden="true" />
                Remove
              </Button>
            </>
          ) : (
            <Button
              type="button"
              size="sm"
              disabled={actionPending || !onDownload}
              onClick={() =>
                void runAction("speech:download", () => onDownload?.())
              }
              aria-label={
                failed || durableFailure
                  ? "Retry speech models"
                  : "Install speech models"
              }
            >
              <Download className="h-4 w-4" aria-hidden="true" />
              {failed || durableFailure ? "Retry" : "Install"}
            </Button>
          )}
        </div>
      ) : null}
    </article>
  );
}
