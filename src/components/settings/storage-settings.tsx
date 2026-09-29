import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  CheckCircle2,
  Database,
  FolderOpen,
  Loader2,
  RotateCw,
  Trash2,
  X,
} from "lucide-react";

import {
  normalizeAbsoluteProjectPathForComparison,
  type ProjectLocationPreference,
} from "@/lib/app-settings";
import { useHostPlatformCopy } from "@/lib/runtime/platform";
import type { AppSettingsTarget } from "@/lib/settings/target";
import type { SettingsCategoryHealth, SettingsComponentHealth } from "@/lib/settings/health";
import type { SettingsOperation } from "@/lib/settings/operations";
import {
  getStorageHealth,
  previewStorageCleanup,
  refreshStorageInventory,
  revealStorageInventoryItem,
  runStorageCleanup,
  type StorageCleanupPreview,
  type StorageCleanupTarget,
} from "@/lib/settings/storage";
import { Button } from "@/components/ui/button";
import { operationForKind, SettingsOperationStatus } from "./settings-operation-status";

export interface StorageSettingsProps {
  projectLocation: ProjectLocationPreference;
  onProjectLocationChange: (preference: ProjectLocationPreference) => void;
  activeProjectDir: string | null;
  operations?: SettingsOperation[];
  onOpenSettingsTarget?: (target: AppSettingsTarget) => void;
}

const activeStates = new Set(["queued", "running", "cancelling"]);
const terminalStates = new Set(["succeeded", "failed", "cancelled"]);

function errorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return error instanceof Error ? error.message : String(error);
}

function formatStorageBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${new Intl.NumberFormat("en", {
    maximumFractionDigits: value >= 10 ? 1 : 2,
  }).format(value)} ${units[unit]}`;
}

function numericProvenance(item: SettingsComponentHealth, key: string) {
  const value = Number(item.provenance[key]);
  return Number.isFinite(value) && value >= 0 ? value : null;
}

function storageStateLabel(item: SettingsComponentHealth) {
  switch (item.state) {
    case "ready":
      return "Ready";
    case "checking":
      return "Checking";
    case "actionRequired":
      return "Action required";
    case "failed":
      return "Failed";
    case "unavailable":
      return "Unavailable";
  }
}

function StorageRow({
  item,
  activeProjectDir,
  onReviewCleanup,
  onError,
  cleanupDisabled,
  onManageModels,
}: {
  item: SettingsComponentHealth;
  activeProjectDir: string | null;
  onReviewCleanup: (target: StorageCleanupTarget, trigger: HTMLButtonElement) => void;
  onError: (message: string) => void;
  cleanupDisabled: boolean;
  onManageModels: () => void;
}) {
  const platformCopy = useHostPlatformCopy();
  const bytes = numericProvenance(item, "bytes");
  const freeBytes = numericProvenance(item, "freeBytes");
  const path = item.provenance.path;
  const isCache = item.provenance.scope === "disposableAppCache";
  const isGlobalModels = item.provenance.scope === "globalModels";

  return (
    <div
      role="group"
      aria-label={`${item.label} storage`}
      className="grid min-w-0 gap-2 border-t py-3 text-xs md:grid-cols-[minmax(11rem,0.85fr)_minmax(15rem,1.5fr)_auto] md:items-center"
    >
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <Database className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span className="truncate font-medium text-foreground">{item.label}</span>
          <span
            className={`shrink-0 text-[10px] font-medium ${
              item.state === "ready"
                ? "text-emerald-700"
                : item.state === "failed"
                  ? "text-red-700"
                  : item.state === "checking"
                    ? "text-muted-foreground"
                    : "text-amber-700"
            }`}
          >
            {storageStateLabel(item)}
          </span>
        </div>
        {bytes !== null && item.state !== "unavailable" ? (
          <div className="mt-1 flex flex-wrap gap-x-3 text-[11px] tabular-nums text-muted-foreground">
            <span>{formatStorageBytes(bytes)} used</span>
            {freeBytes !== null ? <span>{formatStorageBytes(freeBytes)} free</span> : null}
          </div>
        ) : null}
      </div>

      <div className="min-w-0">
        {path ? (
          <code className="block truncate font-mono text-[11px] text-muted-foreground" title={path}>
            {path}
          </code>
        ) : (
          <span className="text-muted-foreground">{item.summary}</span>
        )}
        {item.diagnosticDetail ? (
          <span className="block text-[11px] text-red-700">{item.diagnosticDetail}</span>
        ) : null}
      </div>

      <div className="flex items-center gap-1.5 md:justify-end">
        {path ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            aria-label={platformCopy.revealLabel(item.label)}
            onClick={() => {
              onError("");
              void revealStorageInventoryItem(path, activeProjectDir).catch((error) =>
                onError(platformCopy.revealFailure(item.label, errorMessage(error))),
              );
            }}
          >
            <FolderOpen className="h-3.5 w-3.5" aria-hidden="true" />
            Reveal
          </Button>
        ) : null}
        {isCache ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2 text-red-700 hover:text-red-800"
            disabled={cleanupDisabled}
            onClick={(event) =>
              onReviewCleanup({ kind: "disposableAppCache" }, event.currentTarget)
            }
          >
            <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
            Review cleanup
          </Button>
        ) : null}
        {isGlobalModels ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            onClick={onManageModels}
            aria-label="Manage installed models"
          >
            Manage models
          </Button>
        ) : null}
      </div>
    </div>
  );
}

export function StorageSettings({
  projectLocation,
  onProjectLocationChange,
  activeProjectDir: _activeProjectDir,
  operations = [],
  onOpenSettingsTarget = () => undefined,
}: StorageSettingsProps) {
  const [healthResult, setHealthResult] = useState<{
    root: string | null;
    value: SettingsCategoryHealth;
  } | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [choosingFolder, setChoosingFolder] = useState(false);
  const [refreshStarting, setRefreshStarting] = useState(false);
  const [preview, setPreview] = useState<StorageCleanupPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [cleanupStarting, setCleanupStarting] = useState(false);
  const [cleanupError, setCleanupError] = useState<string | null>(null);
  const [localRefresh, setLocalRefresh] = useState<SettingsOperation | null>(null);
  const [localCleanup, setLocalCleanup] = useState<SettingsOperation | null>(null);
  const [pendingVersion, setPendingVersion] = useState(0);
  const rootRef = useRef<string | null>(null);
  const rootGenerationRef = useRef(0);
  const healthRequestRef = useRef(0);
  const observedOperationsRef = useRef(new Map<string, {
    root: string | null;
    generation: number;
    kind: "storageRefresh" | "storageCleanup";
  }>());
  const cleanupStartingRef = useRef(false);
  const refreshButtonRef = useRef<HTMLButtonElement | null>(null);
  const dialogRef = useRef<HTMLElement | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null);
  const restoreReviewFocusRef = useRef(false);
  const restoreFocusTargetRef = useRef<HTMLElement | null>(null);
  const health = healthResult?.root === null ? healthResult.value : null;
  function loadHealth(root: string | null, generation: number) {
    const requestId = ++healthRequestRef.current;
    setLoadError(null);
    return getStorageHealth(root)
      .then((value) => {
        if (
          rootRef.current === root &&
          rootGenerationRef.current === generation &&
          healthRequestRef.current === requestId
        ) {
          setHealthResult({ root, value });
        }
      })
      .catch((error) => {
        if (
          rootRef.current === root &&
          rootGenerationRef.current === generation &&
          healthRequestRef.current === requestId
        ) {
          setLoadError(errorMessage(error));
        }
      });
  }

  useEffect(() => {
    void loadHealth(null, rootGenerationRef.current);
  }, []);

  useEffect(() => {
    const generation = rootGenerationRef.current;
    for (const operation of operations) {
      if (
        (operation.kind === "storageRefresh" || operation.kind === "storageCleanup") &&
        activeStates.has(operation.state) &&
        !observedOperationsRef.current.has(operation.id)
      ) {
        observedOperationsRef.current.set(operation.id, {
          root: null,
          generation,
          kind: operation.kind,
        });
      }
    }

    for (const [operationId, request] of observedOperationsRef.current) {
      const terminal = operations.find(
        (operation) => operation.id === operationId && terminalStates.has(operation.state),
      );
      if (!terminal) continue;
      observedOperationsRef.current.delete(operationId);
      if (request.kind === "storageRefresh") {
        setLocalRefresh((current) => current?.id === operationId ? null : current);
      }
      if (request.kind === "storageCleanup") {
        setLocalCleanup((current) => current?.id === operationId ? null : current);
      }
      if (
        rootRef.current === request.root &&
        rootGenerationRef.current === request.generation
      ) {
        void loadHealth(request.root, request.generation);
      }
    }
  }, [operations, pendingVersion]);

  useEffect(() => {
    if (preview) {
      cancelButtonRef.current?.focus();
    } else if (restoreReviewFocusRef.current) {
      restoreReviewFocusRef.current = false;
      const target = restoreFocusTargetRef.current;
      const timeout = window.setTimeout(() => {
        if (
          target?.isConnected &&
          (!(target instanceof HTMLButtonElement) || !target.disabled)
        ) {
          target.focus();
        } else {
          refreshButtonRef.current?.focus();
        }
      }, 0);
      return () => window.clearTimeout(timeout);
    }
  }, [preview]);

  const lowSpaceItems = useMemo(
    () => health?.items.filter((item) =>
      item.diagnosticCode === "storage.lowSpace" &&
      (item.provenance.scope === "globalModels" ||
        item.provenance.scope === "disposableAppCache"),
    ) ?? [],
    [health],
  );
  const cacheItems = useMemo(
    () => health?.items.filter((item) => item.provenance.scope === "disposableAppCache") ?? [],
    [health],
  );
  const otherStorageItems = useMemo(
    () => health?.items.filter((item) => item.provenance.scope === "globalModels") ?? [],
    [health],
  );
  const refreshOperation = operationForKind(operations, "storageRefresh", localRefresh);
  const cleanupOperation = operationForKind(operations, "storageCleanup", localCleanup);
  const cleanupBusy = cleanupStarting || previewing || Boolean(
    cleanupOperation && activeStates.has(cleanupOperation.state),
  );

  function observeOperation(
    operation: SettingsOperation,
    root: string | null,
    generation: number,
  ) {
    if (operation.kind !== "storageRefresh" && operation.kind !== "storageCleanup") return;
    observedOperationsRef.current.set(operation.id, {
      root,
      generation,
      kind: operation.kind,
    });
    setPendingVersion((version) => version + 1);
  }

  async function chooseFolder() {
    setChoosingFolder(true);
    setActionError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose suggested project parent folder",
      });
      if (typeof selected !== "string") return;
      const parentPath = selected.trim();
      if (!normalizeAbsoluteProjectPathForComparison(parentPath)) {
        setActionError("The selected project parent must be an absolute folder path.");
        return;
      }
      onProjectLocationChange({ mode: "suggestedParent", parentPath });
    } catch (error) {
      setActionError(`The folder chooser could not open: ${errorMessage(error)}`);
    } finally {
      setChoosingFolder(false);
    }
  }

  async function refreshInventory() {
    const root = null;
    const generation = rootGenerationRef.current;
    setActionError(null);
    setRefreshStarting(true);
    try {
      const operation = await refreshStorageInventory(root);
      if (rootRef.current !== root || rootGenerationRef.current !== generation) return;
      setLocalRefresh(operation);
      if (terminalStates.has(operation.state)) {
        void loadHealth(root, generation);
      } else {
        observeOperation(operation, root, generation);
      }
    } catch (error) {
      setActionError(`Storage usage could not be refreshed: ${errorMessage(error)}`);
    } finally {
      setRefreshStarting(false);
    }
  }

  async function reviewCleanup(
    target: StorageCleanupTarget,
    trigger: HTMLButtonElement,
  ) {
    if (cleanupBusy) return;
    restoreFocusTargetRef.current = trigger;
    const root = null;
    const generation = rootGenerationRef.current;
    setActionError(null);
    setCleanupError(null);
    setPreviewing(true);
    try {
      const nextPreview = await previewStorageCleanup(target, root);
      if (rootRef.current === root && rootGenerationRef.current === generation) {
        setPreview(nextPreview);
      }
    } catch (error) {
      if (rootRef.current === root && rootGenerationRef.current === generation) {
        setActionError(`Cleanup could not be previewed: ${errorMessage(error)}`);
      }
    } finally {
      if (rootRef.current === root && rootGenerationRef.current === generation) {
        setPreviewing(false);
      }
    }
  }

  function closeCleanupPreview(force = false) {
    if (cleanupStartingRef.current && !force) return;
    restoreReviewFocusRef.current = true;
    setCleanupError(null);
    setPreview(null);
  }

  function handleCleanupDialogKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      if (!cleanupStartingRef.current) {
        event.preventDefault();
        closeCleanupPreview();
      }
      return;
    }
    if (event.key !== "Tab") return;
    event.preventDefault();
    const focusable = [
      closeButtonRef.current,
      cancelButtonRef.current,
      confirmButtonRef.current,
    ].filter((button): button is HTMLButtonElement => Boolean(button && !button.disabled));
    if (focusable.length === 0) {
      dialogRef.current?.focus();
      return;
    }
    const currentIndex = focusable.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.shiftKey
      ? (currentIndex <= 0 ? focusable.length : currentIndex) - 1
      : (currentIndex + 1) % focusable.length;
    focusable[nextIndex]?.focus();
  }

  async function confirmCleanup() {
    if (!preview || cleanupStartingRef.current) return;
    cleanupStartingRef.current = true;
    setCleanupStarting(true);
    const root = null;
    const generation = rootGenerationRef.current;
    const confirmedPreview = preview;
    setActionError(null);
    setCleanupError(null);
    try {
      const operation = await runStorageCleanup(
        confirmedPreview.target,
        confirmedPreview.confirmationToken,
        root,
      );
      if (rootRef.current !== root || rootGenerationRef.current !== generation) return;
      setLocalCleanup(operation);
      if (terminalStates.has(operation.state)) {
        void loadHealth(root, generation);
      } else {
        observeOperation(operation, root, generation);
      }
      closeCleanupPreview(true);
    } catch (error) {
      if (rootRef.current === root && rootGenerationRef.current === generation) {
        setCleanupError(`Cleanup could not start: ${errorMessage(error)}`);
      }
    } finally {
      cleanupStartingRef.current = false;
      setCleanupStarting(false);
    }
  }

  return (
    <section aria-label="Storage settings" className="grid gap-4 text-xs">
      <div
        data-settings-target="storage:projectLocation"
        tabIndex={-1}
        className="grid gap-2 border-t pt-3 outline-none focus-visible:ring-2 focus-visible:ring-ring md:grid-cols-[minmax(12rem,0.8fr)_minmax(18rem,1.5fr)_auto] md:items-center"
      >
        <div>
          <h2 className="text-sm font-semibold tracking-normal">New project location</h2>
          <p className="text-muted-foreground">Used only as the suggested parent for new projects.</p>
        </div>
        <div className="min-w-0">
          {projectLocation.mode === "suggestedParent" ? (
            <code
              className="block truncate rounded-sm bg-muted px-2 py-1.5 font-mono text-[11px] text-foreground"
              title={projectLocation.parentPath}
            >
              {projectLocation.parentPath}
            </code>
          ) : (
            <div className="flex items-center gap-2 font-medium text-foreground">
              <FolderOpen className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
              Ask for a folder
            </div>
          )}
        </div>
        <div className="flex items-center gap-1.5 md:justify-end">
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            disabled={choosingFolder}
            onClick={() => void chooseFolder()}
          >
            {choosingFolder ? <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /> : <FolderOpen className="h-3.5 w-3.5" aria-hidden="true" />}
            Choose folder
          </Button>
          {projectLocation.mode === "suggestedParent" ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2"
              onClick={() => onProjectLocationChange({ mode: "ask" })}
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
              Clear selection
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid gap-2">
        <div className="flex min-w-0 items-start justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold tracking-normal">Storage inventory</h2>
            <p className="text-muted-foreground">Global app-managed storage, measured on disk.</p>
          </div>
          <Button
            ref={refreshButtonRef}
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            aria-label="Refresh storage usage"
            disabled={refreshStarting || refreshOperation?.state === "queued" || refreshOperation?.state === "running"}
            onClick={() => void refreshInventory()}
          >
            <RotateCw className={`h-3.5 w-3.5 ${refreshOperation?.state === "running" ? "animate-spin motion-reduce:animate-none" : ""}`} aria-hidden="true" />
            Refresh
          </Button>
        </div>

        {lowSpaceItems.map((item) => {
          const volume = item.provenance.volume || item.provenance.path || "This volume";
          const blockedOperation = item.provenance.blockedOperation || "continue this operation";
          const freeBytes = numericProvenance(item, "freeBytes");
          return (
            <div key={item.id} role="alert" className="flex items-start gap-2 border-l-2 border-amber-500 py-1 pl-2 text-amber-800">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              <span>
                <strong>{volume}</strong> does not have enough free space to {blockedOperation}.
                {freeBytes !== null ? ` ${formatStorageBytes(freeBytes)} free.` : ""} Free storage, then retry.
              </span>
            </div>
          );
        })}

        {loadError ? (
          <div role="alert" className="flex items-center justify-between gap-3 border-l-2 border-red-500 pl-2 text-red-700">
            <span>Storage inventory could not be loaded: {loadError}</span>
            <Button type="button" variant="outline" size="sm" onClick={() => void loadHealth(null, rootGenerationRef.current)}>Retry</Button>
          </div>
        ) : null}
        {actionError ? <div role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">{actionError}</div> : null}
        {!health && !loadError ? <div role="status" className="flex items-center gap-2 text-muted-foreground"><Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />Inspecting storage</div> : null}

        <section
          data-settings-target="storage:cache"
          tabIndex={-1}
          aria-labelledby="application-cache-heading"
          className="grid gap-2 border-t pt-3 outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <div>
            <h3 id="application-cache-heading" className="font-semibold text-foreground">
              Application cache
            </h3>
            <p className="text-[11px] text-muted-foreground">
              Disposable app-owned files that can be reviewed before cleanup.
            </p>
          </div>
          {cacheItems.map((item) => (
            <StorageRow
              key={item.id}
              item={item}
              activeProjectDir={null}
              onReviewCleanup={(target, trigger) => void reviewCleanup(target, trigger)}
              onError={(message) => setActionError(message || null)}
              cleanupDisabled={cleanupBusy}
              onManageModels={() =>
                onOpenSettingsTarget({
                  category: "aiModels",
                  item: "transcription",
                })
              }
            />
          ))}
          {health && cacheItems.length === 0 ? (
            <p className="text-[11px] text-muted-foreground">
              No disposable application cache is reported.
            </p>
          ) : null}
        </section>

        <div className="grid" aria-busy={!health && !loadError}>
          {otherStorageItems.map((item) => (
            <StorageRow
              key={item.id}
              item={item}
              activeProjectDir={null}
              onReviewCleanup={(target, trigger) => void reviewCleanup(target, trigger)}
              onError={(message) => setActionError(message || null)}
              cleanupDisabled={cleanupBusy}
              onManageModels={() =>
                onOpenSettingsTarget({
                  category: "aiModels",
                  item: "transcription",
                })
              }
            />
          ))}
        </div>

        <SettingsOperationStatus operation={refreshOperation} ariaLabel="Storage refresh operation" />
        <SettingsOperationStatus operation={cleanupOperation} ariaLabel="Storage cleanup operation" />
      </div>

      {previewing ? <div role="status" className="flex items-center gap-2 text-muted-foreground"><Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />Building exact cleanup preview</div> : null}
      {preview ? (
        <div
          className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) closeCleanupPreview();
          }}
        >
        <section
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby="storage-cleanup-title"
          tabIndex={-1}
          className="grid w-full max-w-2xl gap-3 rounded-md border border-red-300 bg-background p-4 text-xs shadow-xl"
          onKeyDown={handleCleanupDialogKeyDown}
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 id="storage-cleanup-title" className="text-sm font-semibold">Confirm storage cleanup</h2>
              <p className="text-muted-foreground">Only the exact allowlisted items below will be removed.</p>
            </div>
            <Button ref={closeButtonRef} type="button" variant="ghost" size="icon" className="h-7 w-7" aria-label="Close cleanup preview" disabled={cleanupStarting} onClick={() => closeCleanupPreview()}><X className="h-3.5 w-3.5" aria-hidden="true" /></Button>
          </div>
          {preview.items.length > 0 ? (
            <ul className="grid max-h-48 gap-1 overflow-auto border-y py-2">
              {preview.items.map((item) => (
                <li key={item.path} className="grid min-w-0 grid-cols-[1fr_auto] gap-3">
                  <code className="truncate font-mono text-[11px]" title={item.path}>{item.path}</code>
                  <span className="tabular-nums text-muted-foreground">{formatStorageBytes(item.bytes)}</span>
                </li>
              ))}
            </ul>
          ) : (
            <div className="flex items-center gap-2 border-y py-3 text-muted-foreground"><CheckCircle2 className="h-3.5 w-3.5 text-emerald-700" aria-hidden="true" />No selected items are present.</div>
          )}
          {cleanupError ? (
            <div role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
              {cleanupError}
            </div>
          ) : null}
          <div className="flex items-center justify-between gap-3">
            <span className="font-medium tabular-nums">{formatStorageBytes(preview.totalBytes)} total</span>
            <div className="flex items-center gap-1.5">
              <Button ref={cancelButtonRef} type="button" variant="ghost" size="sm" className="h-7" disabled={cleanupStarting} onClick={() => closeCleanupPreview()}>Cancel</Button>
              <Button
                ref={confirmButtonRef}
                type="button"
                size="sm"
                className="h-7 bg-red-700 text-white hover:bg-red-800"
                disabled={cleanupStarting || preview.items.length === 0}
                onClick={() => void confirmCleanup()}
                aria-label={
                  cleanupStarting
                    ? `Starting ${preview.target.kind === "disposableAppCache" ? "disposable application cache" : "project render artifacts"} cleanup`
                    : `Remove ${preview.items.length} ${preview.target.kind === "disposableAppCache" ? "disposable application cache" : "project render artifact"} ${preview.items.length === 1 ? "item" : "items"}`
                }
              >
                <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
                {cleanupStarting ? "Starting cleanup" : `Remove ${preview.items.length} ${preview.items.length === 1 ? "item" : "items"}`}
              </Button>
            </div>
          </div>
        </section>
        </div>
      ) : null}
    </section>
  );
}
