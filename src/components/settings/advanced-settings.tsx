import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Activity, HardDrive, Loader2, RotateCcw, ServerCog } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  defaultAppPreferences,
  type AppPreferenceIntent,
  type AppSettingsPreferences,
} from "@/lib/app-settings";
import type { TranscriptionModelStatus } from "@/lib/transcription-models";
import { AgentBackendSettings } from "./agent-backend-settings";
import { AgentMcpSettings } from "./agent-mcp-settings";

export interface AdvancedSettingsProps {
  preferences: AppSettingsPreferences;
  projectRoot: string | null;
  models: TranscriptionModelStatus[];
  onChange: (patch: AppPreferenceIntent) => void;
  onImportLocalModel: (modelId: string, sourcePath: string) => Promise<LocalModelImportResult> | LocalModelImportResult | void;
  onRetryModelStatus: () => Promise<boolean>;
  onOpenSystemHealth: () => void;
}

export type LocalModelImportResult =
  | { status: "imported" }
  | { status: "importedRefreshFailed"; detail: string };

const controlClassName =
  "h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring";

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : String(error);
}

export function AdvancedSettings({
  preferences,
  projectRoot,
  models,
  onChange,
  onImportLocalModel,
  onRetryModelStatus,
  onOpenSystemHealth,
}: AdvancedSettingsProps) {
  const [resetOpen, setResetOpen] = useState(false);
  const [selectedModelId, setSelectedModelId] = useState(models[0]?.modelId ?? "");
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importRefreshError, setImportRefreshError] = useState<string | null>(null);
  const [retryingImportStatus, setRetryingImportStatus] = useState(false);
  const resetTriggerRef = useRef<HTMLButtonElement | null>(null);
  const resetDialogRef = useRef<HTMLElement | null>(null);
  const resetCancelRef = useRef<HTMLButtonElement | null>(null);
  const resetConfirmRef = useRef<HTMLButtonElement | null>(null);
  const restoreResetFocusRef = useRef(false);
  const selectedModel = models.find((model) => model.modelId === selectedModelId) ?? models[0];

  useEffect(() => {
    if (resetOpen) {
      resetCancelRef.current?.focus();
    } else if (restoreResetFocusRef.current) {
      restoreResetFocusRef.current = false;
      resetTriggerRef.current?.focus();
    }
  }, [resetOpen]);

  async function importModelFolder() {
    if (!selectedModel || importing) return;
    setImporting(true);
    setImportError(null);
    setImportRefreshError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Import ${selectedModel.displayName} model folder`,
      });
      const sourcePath = Array.isArray(selected) ? selected[0] : selected;
      if (!sourcePath) return;
      const result = await onImportLocalModel(selectedModel.modelId, sourcePath);
      if (result?.status === "importedRefreshFailed") {
        setImportRefreshError(result.detail);
      }
    } catch (error) {
      setImportError(errorMessage(error));
    } finally {
      setImporting(false);
    }
  }

  async function retryImportedModelStatus() {
    if (retryingImportStatus) return;
    setRetryingImportStatus(true);
    try {
      if (await onRetryModelStatus()) setImportRefreshError(null);
    } catch (error) {
      setImportRefreshError(errorMessage(error));
    } finally {
      setRetryingImportStatus(false);
    }
  }

  function closeResetDialog() {
    restoreResetFocusRef.current = true;
    setResetOpen(false);
  }

  function handleResetDialogKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeResetDialog();
      return;
    }
    if (event.key !== "Tab") return;
    event.preventDefault();
    const focusable = [resetCancelRef.current, resetConfirmRef.current].filter(
      (button): button is HTMLButtonElement => Boolean(button && !button.disabled),
    );
    if (focusable.length === 0) {
      resetDialogRef.current?.focus();
      return;
    }
    const currentIndex = focusable.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.shiftKey
      ? (currentIndex <= 0 ? focusable.length : currentIndex) - 1
      : (currentIndex + 1) % focusable.length;
    focusable[nextIndex]?.focus();
  }

  function resetPreferences() {
    const {
      schemaVersion: _schemaVersion,
      ...patch
    } = defaultAppPreferences;
    onChange(patch);
    closeResetDialog();
  }

  return (
    <section aria-label="Advanced settings" className="grid max-w-3xl text-xs">
      <section
        data-settings-target="advanced:execution"
        tabIndex={-1}
        aria-labelledby="advanced-execution-heading"
        className="grid gap-3 border-t py-3 outline-none focus-visible:ring-2 focus-visible:ring-ring md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center"
      >
        <div className="flex items-start gap-2">
          <ServerCog className="mt-0.5 h-4 w-4 text-muted-foreground" aria-hidden="true" />
          <div>
            <h2 id="advanced-execution-heading" className="text-xs font-semibold">Execution</h2>
            <p className="text-[11px] leading-5 text-muted-foreground">
              Choose where generation and media export work is coordinated.
            </p>
          </div>
        </div>
        <div className="grid gap-1 md:justify-items-end">
          <select
            aria-label="Generation execution backend"
            value={preferences.generationExecutionBackend}
            className={controlClassName}
            onChange={(event) =>
              onChange({
                generationExecutionBackend: event.currentTarget.value as "inProcess" | "temporal",
              })
            }
          >
            <option value="inProcess">Desktop execution</option>
            <option value="temporal">Temporal execution</option>
          </select>
          <span className="text-[11px] text-muted-foreground">
            {preferences.generationExecutionBackend === "inProcess"
              ? "Desktop execution"
              : "Temporal execution"}
          </span>
        </div>
      </section>

      <AgentBackendSettings preferences={preferences} onChange={onChange} />

      <AgentMcpSettings projectRoot={projectRoot} />

      <section
        aria-labelledby="advanced-system-health-heading"
        className="grid gap-3 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center"
      >
        <div className="flex items-start gap-2">
          <Activity className="mt-0.5 h-4 w-4 text-muted-foreground" aria-hidden="true" />
          <div>
            <h2 id="advanced-system-health-heading" className="text-xs font-semibold">System Health</h2>
            <p className="text-[11px] leading-5 text-muted-foreground">
              Inspect rendering, local AI, agent, project, and environment readiness.
            </p>
          </div>
        </div>
        <div className="md:justify-self-end">
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            onClick={onOpenSystemHealth}
          >
            <Activity className="h-3.5 w-3.5" aria-hidden="true" />
            Open System Health
          </Button>
        </div>
      </section>

      <section
        data-settings-target="advanced:recovery"
        tabIndex={-1}
        aria-labelledby="advanced-recovery-heading"
        className="grid gap-3 border-t py-3 outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <div>
          <h2 id="advanced-recovery-heading" className="text-xs font-semibold">Recovery</h2>
          <p className="text-[11px] leading-5 text-muted-foreground">
            Import an existing local model folder or restore app-owned preferences.
          </p>
        </div>

        <div className="grid gap-3 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center">
          <div className="flex items-start gap-2">
            <HardDrive className="mt-0.5 h-4 w-4 text-muted-foreground" aria-hidden="true" />
            <div>
              <h3 className="font-semibold text-foreground">Local model folder</h3>
              <p className="text-[11px] leading-5 text-muted-foreground">
                Use a folder that already contains the selected model's required files.
              </p>
            </div>
          </div>
          <div className="flex min-w-0 flex-wrap items-center gap-2 md:justify-end">
            {selectedModel ? (
              <>
                <select
                  aria-label="Local model to import"
                  value={selectedModel.modelId}
                  className={controlClassName}
                  onChange={(event) => setSelectedModelId(event.currentTarget.value)}
                >
                  {models.map((model) => (
                    <option key={model.modelId} value={model.modelId}>{model.displayName}</option>
                  ))}
                </select>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 px-2"
                  disabled={importing}
                  onClick={() => void importModelFolder()}
                  aria-label={`Import ${selectedModel.displayName} from folder`}
                >
                  {importing ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
                  ) : (
                    <HardDrive className="h-3.5 w-3.5" aria-hidden="true" />
                  )}
                  Import folder
                </Button>
              </>
            ) : (
              <span className="text-[11px] text-muted-foreground">No local model definitions are available.</span>
            )}
          </div>
          {importError ? (
            <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700 md:col-span-2">
              Model folder import failed: {importError}
            </p>
          ) : null}
          {importRefreshError ? (
            <div role="alert" className="flex flex-wrap items-center justify-between gap-2 border-l-2 border-amber-500 pl-2 text-amber-800 md:col-span-2">
              <span>Imported, status refresh failed: {importRefreshError}</span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 px-2"
                disabled={retryingImportStatus}
                onClick={() => void retryImportedModelStatus()}
                aria-label="Retry imported model status refresh"
              >
                {retryingImportStatus ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
                ) : null}
                Retry status refresh
              </Button>
            </div>
          ) : null}
        </div>

        <div className="grid gap-3 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center">
          <div className="flex items-start gap-2">
            <RotateCcw className="mt-0.5 h-4 w-4 text-muted-foreground" aria-hidden="true" />
            <div>
              <h3 className="font-semibold text-foreground">Reset app preferences</h3>
              <p className="text-[11px] leading-5 text-muted-foreground">
                Restore app defaults. Projects, credentials, models, and media are not removed.
              </p>
            </div>
          </div>
          <div className="md:justify-self-end">
            <Button
              ref={resetTriggerRef}
              type="button"
              variant="outline"
              size="sm"
              className="h-7 px-2 text-red-700 hover:text-red-800"
              onClick={() => setResetOpen(true)}
              aria-label="Reset app preferences"
            >
              Reset preferences
            </Button>
          </div>
        </div>
      </section>

      {resetOpen ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" role="presentation">
          <section
            ref={resetDialogRef}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="reset-preferences-title"
            aria-describedby="reset-preferences-description"
            tabIndex={-1}
            className="grid w-full max-w-md gap-3 rounded-md border bg-background p-4 shadow-xl"
            onKeyDown={handleResetDialogKeyDown}
          >
            <div>
              <h2 id="reset-preferences-title" className="text-sm font-semibold">Reset app preferences?</h2>
              <p id="reset-preferences-description" className="mt-1 text-xs text-muted-foreground">
                This restores General, Projects, Storage, model selection, and execution defaults.
              </p>
            </div>
            <div className="flex justify-end gap-2">
              <Button ref={resetCancelRef} type="button" variant="ghost" size="sm" onClick={closeResetDialog}>Cancel</Button>
              <Button
                ref={resetConfirmRef}
                type="button"
                size="sm"
                className="bg-red-700 text-white hover:bg-red-800"
                onClick={resetPreferences}
                aria-label="Confirm reset app preferences"
              >
                Reset preferences
              </Button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}
