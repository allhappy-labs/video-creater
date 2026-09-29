import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  ArrowLeft,
  BookOpen,
  CheckCircle2,
  Database,
  FolderOpen,
  Loader2,
  RefreshCw,
  RotateCw,
  Trash2,
  TriangleAlert,
  Wrench,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import type {
  UpdateProjectSettingsAction,
  VideoProject,
} from "@/lib/project";
import type {
  SettingsCategoryHealth,
  SettingsComponentHealth,
} from "@/lib/settings/health";
import { useHostPlatformCopy } from "@/lib/runtime/platform";
import type { SettingsOperation } from "@/lib/settings/operations";
import {
  getSkillsHealth,
  repairBundledSkills,
  type SkillRepairPreview,
} from "@/lib/settings/skills";
import {
  getStorageHealth,
  previewStorageCleanup,
  refreshStorageInventory,
  revealStorageInventoryItem,
  runStorageCleanup,
  type StorageCleanupPreview,
} from "@/lib/settings/storage";
import { useSettingsOperations } from "@/lib/settings/use-settings-operations";

export interface ProjectSettingsProps {
  project: VideoProject;
  projectDir: string;
  onApply: (
    action: UpdateProjectSettingsAction,
  ) => void | Promise<void>;
  onBack?: () => void;
}

type ProjectSettingsDraft = {
  name: string;
  width: string;
  height: string;
  fps: string;
  loudnessLufs: string;
  captions: string;
};

const supportedFps = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60] as const;
const captionModes = ["burn_in", "mux", "off"] as const;
const requiredSkillLabels: Record<string, string> = {
  "video-creater-video-pipeline": "Editing pipeline",
  "video-creater-graphics": "Video graphics",
  "video-creater-visuals": "Editor interface",
};
const controlClassName =
  "h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring";

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

function draftFromProject(project: VideoProject): ProjectSettingsDraft {
  return {
    name: project.name,
    width: String(project.renderSettings.width),
    height: String(project.renderSettings.height),
    fps: String(project.renderSettings.fps),
    loudnessLufs: String(project.renderSettings.loudnessLufs),
    captions: project.renderSettings.captions,
  };
}

function presetFor(width: string, height: string) {
  if (width === "1280" && height === "720") return "hd";
  if (width === "1920" && height === "1080") return "fhd";
  if (width === "3840" && height === "2160") return "uhd";
  return "custom";
}

function validDimension(value: number) {
  return (
    Number.isInteger(value) &&
    value >= 2 &&
    value <= 16_384 &&
    value % 2 === 0
  );
}

function formatStorageBytes(rawBytes: string | undefined) {
  const bytes = Number(rawBytes);
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
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

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-2 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center">
      <div>
        <h3 className="text-xs font-semibold text-foreground">{label}</h3>
        <p className="text-[11px] leading-5 text-muted-foreground">
          {description}
        </p>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-2 md:justify-end">
        {children}
      </div>
    </div>
  );
}

function ProjectStorage({
  project,
  projectDir,
}: {
  project: VideoProject;
  projectDir: string;
}) {
  const [health, setHealth] = useState<SettingsCategoryHealth | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const platformCopy = useHostPlatformCopy();
  const [refreshing, setRefreshing] = useState(false);
  const [cleanupPreview, setCleanupPreview] =
    useState<StorageCleanupPreview | null>(null);
  const [previewingCleanup, setPreviewingCleanup] = useState(false);
  const [runningCleanup, setRunningCleanup] = useState(false);
  const [pendingRefreshId, setPendingRefreshId] = useState<string | null>(null);
  const [pendingCleanupId, setPendingCleanupId] = useState<string | null>(null);
  const requestGenerationRef = useRef(0);
  const cleanupTriggerRef = useRef<HTMLButtonElement | null>(null);
  const cleanupConfirmRef = useRef<HTMLButtonElement | null>(null);
  const { operations } = useSettingsOperations();

  async function loadInventory(generation: number) {
    setLoadError(null);
    try {
      const nextHealth = await getStorageHealth(projectDir);
      if (requestGenerationRef.current === generation) {
        setHealth(nextHealth);
      }
    } catch (error) {
      if (requestGenerationRef.current === generation) {
        setLoadError(errorMessage(error));
      }
    }
  }

  useEffect(() => {
    const generation = ++requestGenerationRef.current;
    setHealth(null);
    setCleanupPreview(null);
    void loadInventory(generation);
    return () => {
      requestGenerationRef.current += 1;
    };
  }, [projectDir]);

  useEffect(() => {
    function terminalOperation(operationId: string | null) {
      if (!operationId) return null;
      const operation = operations.find(({ id }) => id === operationId);
      return operation && ["succeeded", "failed", "cancelled"].includes(operation.state)
        ? operation
        : null;
    }

    const refresh = terminalOperation(pendingRefreshId);
    if (refresh) {
      setPendingRefreshId(null);
      if (refresh.state === "succeeded") {
        void loadInventory(requestGenerationRef.current);
      } else {
        setActionError(
          refresh.error?.message ?? "Project storage refresh did not complete.",
        );
      }
    }

    const cleanup = terminalOperation(pendingCleanupId);
    if (cleanup) {
      setPendingCleanupId(null);
      if (cleanup.state === "succeeded") {
        void loadInventory(requestGenerationRef.current);
      } else {
        setActionError(
          cleanup.error?.message ?? "Project storage cleanup did not complete.",
        );
      }
    }
  }, [operations, pendingCleanupId, pendingRefreshId]);

  useEffect(() => {
    if (cleanupPreview) cleanupConfirmRef.current?.focus();
    else cleanupTriggerRef.current?.focus();
  }, [cleanupPreview]);

  const projectItems = useMemo(
    () =>
      health?.items.filter((item) =>
        item.provenance.scope?.startsWith("project"),
      ) ?? [],
    [health],
  );
  const refreshOperation = operations.find(({ id }) => id === pendingRefreshId);
  const cleanupOperation = operations.find(({ id }) => id === pendingCleanupId);

  function operationIsTerminal(operation: SettingsOperation) {
    return ["succeeded", "failed", "cancelled"].includes(operation.state);
  }

  async function refresh() {
    const generation = requestGenerationRef.current;
    setActionError(null);
    setRefreshing(true);
    try {
      const operation = await refreshStorageInventory(projectDir);
      if (requestGenerationRef.current !== generation) return;
      if (operationIsTerminal(operation)) {
        if (operation.state === "succeeded") await loadInventory(generation);
        else setActionError(operation.error?.message ?? "Project storage refresh did not complete.");
      } else {
        setPendingRefreshId(operation.id);
      }
    } catch (error) {
      if (requestGenerationRef.current === generation) {
        setActionError(`Storage could not be refreshed: ${errorMessage(error)}`);
      }
    } finally {
      if (requestGenerationRef.current === generation) setRefreshing(false);
    }
  }

  async function reveal(item: SettingsComponentHealth) {
    const path = item.provenance.path;
    if (!path) return;
    setActionError(null);
    try {
      await revealStorageInventoryItem(path, projectDir);
    } catch (error) {
      setActionError(platformCopy.revealFailure(item.label, errorMessage(error)));
    }
  }

  async function reviewRenderCleanup(trigger: HTMLButtonElement) {
    const artifactIds = project.renderReports.map((report) => report.id);
    if (artifactIds.length === 0) return;
    cleanupTriggerRef.current = trigger;
    setActionError(null);
    setPreviewingCleanup(true);
    try {
      setCleanupPreview(
        await previewStorageCleanup(
          { kind: "projectRenderArtifacts", artifactIds },
          projectDir,
        ),
      );
    } catch (error) {
      setActionError(`Cleanup could not be previewed: ${errorMessage(error)}`);
    } finally {
      setPreviewingCleanup(false);
    }
  }

  async function confirmRenderCleanup() {
    if (!cleanupPreview) return;
    setRunningCleanup(true);
    setActionError(null);
    try {
      const operation = await runStorageCleanup(
        cleanupPreview.target,
        cleanupPreview.confirmationToken,
        projectDir,
      );
      setCleanupPreview(null);
      if (operationIsTerminal(operation)) {
        if (operation.state === "succeeded") {
          await loadInventory(requestGenerationRef.current);
        } else {
          setActionError(operation.error?.message ?? "Project storage cleanup did not complete.");
        }
      } else {
        setPendingCleanupId(operation.id);
      }
    } catch (error) {
      setActionError(`Cleanup could not start: ${errorMessage(error)}`);
    } finally {
      setRunningCleanup(false);
    }
  }

  function handleCleanupKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && !runningCleanup) {
      event.preventDefault();
      setCleanupPreview(null);
    }
  }

  return (
    <section aria-labelledby="project-storage-heading" className="grid gap-3">
      <div className="flex items-start justify-between gap-3 border-t pt-4">
        <div>
          <h2 id="project-storage-heading" className="text-sm font-semibold">
            Storage
          </h2>
          <p className="mt-1 text-[11px] text-muted-foreground">
            Files contained by this project. Canonical manifests and accepted media are not cleanup targets.
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={refreshing || Boolean(pendingRefreshId)}
          onClick={() => void refresh()}
          aria-label="Refresh project storage"
        >
          <RotateCw
            className={`h-3.5 w-3.5 ${refreshing || pendingRefreshId ? "animate-spin motion-reduce:animate-none" : ""}`}
            aria-hidden="true"
          />
          Refresh
        </Button>
      </div>

      <div className="grid gap-1 rounded-sm border bg-muted/15 px-3 py-2 text-xs md:grid-cols-[minmax(10rem,0.8fr)_minmax(14rem,1.2fr)]">
        <span className="font-medium text-foreground">Project root</span>
        <code className="truncate font-mono text-[11px] text-muted-foreground" title={projectDir}>
          {projectDir}
        </code>
      </div>

      {loadError ? (
        <div role="alert" className="flex items-center justify-between gap-3 border-l-2 border-red-500 pl-2 text-xs text-red-700">
          <span>Project storage could not be loaded: {loadError}</span>
          <Button type="button" variant="outline" size="sm" onClick={() => void loadInventory(requestGenerationRef.current)}>
            Retry
          </Button>
        </div>
      ) : null}
      {actionError ? (
        <div role="alert" className="border-l-2 border-red-500 pl-2 text-xs text-red-700">
          {actionError}
        </div>
      ) : null}
      {refreshOperation ? (
        <div role="status" className="text-xs text-muted-foreground">
          {refreshOperation.message}
        </div>
      ) : null}
      {cleanupOperation ? (
        <div role="status" className="text-xs text-muted-foreground">
          {cleanupOperation.message}
        </div>
      ) : null}
      {!health && !loadError ? (
        <div role="status" className="flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
          Inspecting project storage
        </div>
      ) : null}

      <div className="grid gap-0">
        {projectItems.map((item) => {
          const path = item.provenance.path;
          const isRenders = item.provenance.scope === "projectRenderArtifacts";
          return (
            <div
              key={item.id}
              role="group"
              aria-label={`${item.label} storage`}
              className="grid min-w-0 gap-2 border-t py-3 text-xs md:grid-cols-[minmax(11rem,0.8fr)_minmax(14rem,1.3fr)_auto] md:items-center"
            >
              <div className="flex min-w-0 items-center gap-2">
                <Database className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                <div className="min-w-0">
                  <div className="truncate font-medium text-foreground">{item.label}</div>
                  <div className="text-[11px] tabular-nums text-muted-foreground">
                    {formatStorageBytes(item.provenance.bytes)} used
                  </div>
                </div>
              </div>
              <code className="truncate font-mono text-[11px] text-muted-foreground" title={path}>
                {path ?? item.summary}
              </code>
              <div className="flex items-center gap-1.5 md:justify-end">
                {path ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    aria-label={platformCopy.revealLabel(item.label)}
                    onClick={() => void reveal(item)}
                  >
                    <FolderOpen className="h-3.5 w-3.5" aria-hidden="true" />
                    Reveal
                  </Button>
                ) : null}
                {isRenders && project.renderReports.length > 0 ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="h-7 px-2 text-xs text-red-700 hover:text-red-800"
                    disabled={previewingCleanup || Boolean(pendingCleanupId)}
                    onClick={(event) => void reviewRenderCleanup(event.currentTarget)}
                  >
                    <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
                    Review cleanup
                  </Button>
                ) : null}
              </div>
            </div>
          );
        })}
      </div>

      {cleanupPreview ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" role="presentation">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="project-cleanup-title"
            className="grid w-full max-w-xl gap-3 rounded-md border bg-background p-4 text-xs shadow-xl"
            onKeyDown={handleCleanupKeyDown}
          >
            <div>
              <h2 id="project-cleanup-title" className="text-sm font-semibold">
                Confirm render artifact cleanup
              </h2>
              <p className="text-muted-foreground">
                Only the exact render files below are included. Canonical project files and media stay untouched.
              </p>
            </div>
            <ul className="grid max-h-52 gap-1 overflow-auto rounded-sm border bg-muted/25 p-2 font-mono text-[10px]">
              {cleanupPreview.items.map((item) => (
                <li key={item.path} className="flex justify-between gap-3">
                  <span className="break-all">{item.path}</span>
                  <span className="shrink-0 tabular-nums">{formatStorageBytes(String(item.bytes))}</span>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <Button type="button" variant="ghost" size="sm" disabled={runningCleanup} onClick={() => setCleanupPreview(null)}>
                Cancel
              </Button>
              <Button ref={cleanupConfirmRef} type="button" size="sm" disabled={runningCleanup} onClick={() => void confirmRenderCleanup()}>
                {runningCleanup ? "Starting cleanup" : "Delete reviewed render files"}
              </Button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}

function ProjectGuidance({ projectDir }: { projectDir: string }) {
  const [health, setHealth] = useState<SettingsCategoryHealth | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [repairError, setRepairError] = useState<string | null>(null);
  const [preview, setPreview] = useState<SkillRepairPreview | null>(null);
  const [repairing, setRepairing] = useState(false);
  const generationRef = useRef(0);
  const repairButtonRef = useRef<HTMLButtonElement | null>(null);
  const confirmButtonRef = useRef<HTMLButtonElement | null>(null);

  async function loadGuidance(generation: number) {
    setLoadError(null);
    try {
      const nextHealth = await getSkillsHealth(projectDir);
      if (generationRef.current === generation) setHealth(nextHealth);
    } catch (error) {
      if (generationRef.current === generation) setLoadError(errorMessage(error));
    }
  }

  useEffect(() => {
    const generation = ++generationRef.current;
    setHealth(null);
    setPreview(null);
    void loadGuidance(generation);
    return () => {
      generationRef.current += 1;
    };
  }, [projectDir]);

  useEffect(() => {
    if (preview) confirmButtonRef.current?.focus();
    else repairButtonRef.current?.focus();
  }, [preview]);

  const skillItems = useMemo(
    () => health?.items.filter((item) => Boolean(requiredSkillLabels[item.id])) ?? [],
    [health],
  );
  const damagedItems = skillItems.filter(
    (item) => item.state === "actionRequired" || item.state === "failed",
  );
  const rootIssue = health?.items.find(
    (item) => item.id === "skills.root" && item.state !== "ready",
  );
  const guidanceReady = skillItems.length > 0 && skillItems.every((item) => item.state === "ready");

  async function previewRepair() {
    const skillIds = damagedItems.map((item) => item.id);
    if (skillIds.length === 0) return;
    setRepairing(true);
    setRepairError(null);
    try {
      const result = await repairBundledSkills(projectDir, skillIds, null);
      setPreview(result.preview);
    } catch (error) {
      setRepairError(`Guidance repair could not be previewed: ${errorMessage(error)}`);
    } finally {
      setRepairing(false);
    }
  }

  async function confirmRepair() {
    if (!preview) return;
    const confirmedPreview = preview;
    setRepairing(true);
    setRepairError(null);
    try {
      await repairBundledSkills(
        projectDir,
        confirmedPreview.skillIds,
        confirmedPreview.affectedPaths,
      );
      setPreview(null);
      await loadGuidance(generationRef.current);
    } catch (error) {
      setRepairError(`Guidance repair failed: ${errorMessage(error)}`);
    } finally {
      setRepairing(false);
    }
  }

  function handleRepairKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && !repairing) {
      event.preventDefault();
      setPreview(null);
    }
  }

  return (
    <section aria-labelledby="project-guidance-heading" className="grid gap-3 border-t pt-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 id="project-guidance-heading" className="text-sm font-semibold">
            Agent guidance
          </h2>
          <p className="mt-1 text-[11px] text-muted-foreground">
            Bundled instructions used for editing, graphics, and interface work in this project.
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 px-2 text-xs"
          disabled={repairing}
          onClick={() => void loadGuidance(generationRef.current)}
          aria-label="Verify project guidance"
        >
          <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
          Verify
        </Button>
      </div>

      {!health && !loadError ? (
        <div role="status" className="flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
          Verifying bundled guidance
        </div>
      ) : null}
      {loadError ? (
        <div
          role="alert"
          className="flex items-center justify-between gap-3 border-l-2 border-red-500 pl-2 text-xs text-red-700"
        >
          <span>Project guidance could not be verified: {loadError}</span>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 shrink-0 px-2 text-xs"
            disabled={repairing}
            onClick={() => void loadGuidance(generationRef.current)}
            aria-label="Retry project guidance"
          >
            Retry
          </Button>
        </div>
      ) : null}
      {repairError ? (
        <div role="alert" className="border-l-2 border-red-500 pl-2 text-xs text-red-700">
          {repairError}
        </div>
      ) : null}

      {guidanceReady ? (
        <div className="rounded-sm border bg-muted/15 px-3 py-2 text-xs">
          <div className="flex items-center gap-2 font-medium text-foreground">
            <CheckCircle2 className="h-4 w-4 text-emerald-700" aria-hidden="true" />
            Bundled project guidance active
          </div>
          <details className="mt-2 text-[11px] text-muted-foreground">
            <summary className="cursor-pointer select-none font-medium text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring">
              View guidance details
            </summary>
            <div className="mt-2 grid gap-1 border-t pt-2">
              {skillItems.map((item) => (
                <div key={item.id} className="flex items-center justify-between gap-3">
                  <span>{requiredSkillLabels[item.id]}</span>
                  <span className="text-emerald-700">Verified</span>
                </div>
              ))}
            </div>
          </details>
        </div>
      ) : null}

      {rootIssue ? (
        <div className="grid gap-2 rounded-sm border border-amber-500/40 bg-amber-500/5 px-3 py-3 text-xs">
          <div className="flex items-start gap-2">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-700" aria-hidden="true" />
            <div>
              <div className="font-medium text-foreground">
                Project repository guidance is unavailable
              </div>
              <p className="text-[11px] text-muted-foreground">{rootIssue.summary}</p>
            </div>
          </div>
          {rootIssue.diagnosticDetail ? (
            <p className="border-t pt-2 text-[11px] text-muted-foreground">
              {rootIssue.diagnosticDetail}
            </p>
          ) : null}
          <p className="text-[11px] text-muted-foreground">
            Bundled editing guidance remains available to the app. Project-file verification and
            repair require a repository root containing AGENTS.md and .agents/skills.
          </p>
        </div>
      ) : null}

      {damagedItems.length > 0 ? (
        <div className="grid gap-3 rounded-sm border border-amber-500/40 bg-amber-500/5 px-3 py-3 text-xs">
          <div className="flex items-start gap-2">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-700" aria-hidden="true" />
            <div>
              <div className="font-medium text-foreground">Bundled project guidance needs repair</div>
              <p className="text-[11px] text-muted-foreground">
                Review the exact bundled files before replacing them. Custom skills and unrelated project instructions stay untouched.
              </p>
            </div>
          </div>
          <div className="grid gap-2">
            {damagedItems.map((item) => (
              <div key={item.id} className="border-t pt-2">
                <div className="font-medium text-foreground">{requiredSkillLabels[item.id]}</div>
                <div className="text-[11px] text-muted-foreground">{item.summary}</div>
              </div>
            ))}
          </div>
          <div>
            <Button
              ref={repairButtonRef}
              type="button"
              variant="outline"
              size="sm"
              className="h-7 px-2 text-xs"
              disabled={repairing}
              onClick={() => void previewRepair()}
              aria-label="Repair project guidance"
            >
              {repairing ? <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /> : <Wrench className="h-3.5 w-3.5" aria-hidden="true" />}
              Repair
            </Button>
          </div>
        </div>
      ) : null}

      {preview ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" role="presentation">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="guidance-repair-title"
            className="grid w-full max-w-xl gap-3 rounded-md border bg-background p-4 text-xs shadow-xl"
            onKeyDown={handleRepairKeyDown}
          >
            <div>
              <h2 id="guidance-repair-title" className="text-sm font-semibold">
                Confirm guidance repair
              </h2>
              <p className="text-muted-foreground">
                Only these exact bundled paths will be replaced. Differing content is backed up first.
              </p>
            </div>
            <ul className="grid max-h-52 gap-1 overflow-auto rounded-sm border bg-muted/25 p-2 font-mono text-[10px]">
              {preview.affectedPaths.map((path) => (
                <li key={path} className="break-all">{path}</li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <Button type="button" variant="ghost" size="sm" disabled={repairing} onClick={() => setPreview(null)}>
                Cancel
              </Button>
              <Button ref={confirmButtonRef} type="button" size="sm" disabled={repairing} onClick={() => void confirmRepair()}>
                {repairing ? "Repairing confirmed files" : "Repair confirmed files"}
              </Button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}

export function ProjectSettings({
  project,
  projectDir,
  onApply,
  onBack,
}: ProjectSettingsProps) {
  const [draft, setDraft] = useState(() => draftFromProject(project));
  const [validationError, setValidationError] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applyStatus, setApplyStatus] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const nameRef = useRef<HTMLInputElement | null>(null);
  const widthRef = useRef<HTMLInputElement | null>(null);
  const heightRef = useRef<HTMLInputElement | null>(null);
  const fpsRef = useRef<HTMLSelectElement | null>(null);
  const loudnessRef = useRef<HTMLInputElement | null>(null);
  const captionsRef = useRef<HTMLSelectElement | null>(null);

  useEffect(() => {
    setDraft(draftFromProject(project));
    setValidationError(null);
    setApplyError(null);
    setApplyStatus(null);
  }, [
    project.id,
    project.name,
    project.renderSettings.width,
    project.renderSettings.height,
    project.renderSettings.fps,
    project.renderSettings.loudnessLufs,
    project.renderSettings.captions,
  ]);

  function updateDraft(change: Partial<ProjectSettingsDraft>) {
    setDraft((current) => ({ ...current, ...change }));
    setValidationError(null);
    setApplyError(null);
    setApplyStatus(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = draft.name.trim();
    const width = Number(draft.width);
    const height = Number(draft.height);
    const fps = Number(draft.fps);
    const loudnessLufs = Number(draft.loudnessLufs);

    if (!name) {
      setValidationError("Project name is required.");
      nameRef.current?.focus();
      return;
    }
    if (!validDimension(width)) {
      setValidationError("Dimensions must be even values between 2 and 16384.");
      widthRef.current?.focus();
      return;
    }
    if (!validDimension(height)) {
      setValidationError("Dimensions must be even values between 2 and 16384.");
      heightRef.current?.focus();
      return;
    }
    if (!Number.isFinite(fps) || !supportedFps.includes(fps as (typeof supportedFps)[number])) {
      setValidationError("Choose a supported project frame rate.");
      fpsRef.current?.focus();
      return;
    }
    if (draft.loudnessLufs.trim() === "" || !Number.isFinite(loudnessLufs)) {
      setValidationError("Loudness must be a finite LUFS value.");
      loudnessRef.current?.focus();
      return;
    }
    if (!captionModes.includes(draft.captions as (typeof captionModes)[number])) {
      setValidationError("Choose a supported caption delivery mode.");
      captionsRef.current?.focus();
      return;
    }

    const action: UpdateProjectSettingsAction = {
      type: "updateProjectSettings",
      name,
      renderSettings: {
        width,
        height,
        fps,
        loudnessLufs,
        captions: draft.captions as VideoProject["renderSettings"]["captions"],
      },
    };
    setApplying(true);
    setApplyError(null);
    setApplyStatus(null);
    try {
      await onApply(action);
      setDraft((current) => ({ ...current, name }));
      setApplyStatus("Project settings applied.");
    } catch (error) {
      setApplyError(`Project settings could not be applied: ${errorMessage(error)}`);
    } finally {
      setApplying(false);
    }
  }

  return (
    <main aria-label="Project Settings" className="h-full min-h-0 overflow-auto bg-[#121314] text-foreground">
      <div className="mx-auto grid w-full max-w-5xl gap-5 px-6 py-5">
        <header className="flex min-w-0 items-start gap-3 border-b border-white/10 pb-4">
          {onBack ? (
            <Button type="button" variant="ghost" size="icon" className="h-8 w-8 shrink-0" onClick={onBack} aria-label="Back to editor">
              <ArrowLeft className="h-4 w-4" aria-hidden="true" />
            </Button>
          ) : null}
          <div className="min-w-0">
            <div className="text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
              Active project
            </div>
            <h1 className="truncate text-lg font-semibold">Project Settings · {project.name}</h1>
            <p className="mt-1 text-xs text-muted-foreground">
              Project-owned format, output, storage, and bundled agent guidance.
            </p>
          </div>
        </header>

        <form onSubmit={(event) => void submit(event)} className="grid gap-3" noValidate>
          <div>
            <h2 className="text-sm font-semibold">Format &amp; output</h2>
            <p className="mt-1 text-[11px] text-muted-foreground">
              Applied as one validated project action for future preview, render, and export work.
            </p>
          </div>

          <SettingRow label="Project name" description="Shown in the editor and project manifest.">
            <input
              ref={nameRef}
              aria-label="Project name"
              value={draft.name}
              className={`${controlClassName} w-full max-w-xs`}
              onChange={(event) => updateDraft({ name: event.currentTarget.value })}
            />
          </SettingRow>

          <SettingRow label="Frame size" description="Choose a standard frame or enter even custom dimensions.">
            <select
              aria-label="Project format preset"
              value={presetFor(draft.width, draft.height)}
              className={controlClassName}
              onChange={(event) => {
                if (event.currentTarget.value === "hd") updateDraft({ width: "1280", height: "720" });
                if (event.currentTarget.value === "fhd") updateDraft({ width: "1920", height: "1080" });
                if (event.currentTarget.value === "uhd") updateDraft({ width: "3840", height: "2160" });
              }}
            >
              <option value="hd">HD · 1280 × 720</option>
              <option value="fhd">Full HD · 1920 × 1080</option>
              <option value="uhd">UHD · 3840 × 2160</option>
              <option value="custom">Custom</option>
            </select>
            <input
              ref={widthRef}
              type="number"
              min={2}
              max={16384}
              step={2}
              aria-label="Project width"
              value={draft.width}
              className={`${controlClassName} w-24 tabular-nums`}
              onChange={(event) => updateDraft({ width: event.currentTarget.value })}
            />
            <span aria-hidden="true" className="text-muted-foreground">×</span>
            <input
              ref={heightRef}
              type="number"
              min={2}
              max={16384}
              step={2}
              aria-label="Project height"
              value={draft.height}
              className={`${controlClassName} w-24 tabular-nums`}
              onChange={(event) => updateDraft({ height: event.currentTarget.value })}
            />
          </SettingRow>

          <SettingRow label="Frame rate" description="Canonical frame rate for future project renders.">
            <select
              ref={fpsRef}
              aria-label="Project frame rate"
              value={draft.fps}
              className={controlClassName}
              onChange={(event) => updateDraft({ fps: event.currentTarget.value })}
            >
              {supportedFps.map((fps) => (
                <option key={fps} value={fps}>{fps} fps</option>
              ))}
            </select>
          </SettingRow>

          <SettingRow label="Audio loudness" description="Target loudness for future project exports.">
            <div className="flex items-center gap-2">
              <input
                ref={loudnessRef}
                type="number"
                step="0.1"
                aria-label="Project loudness target"
                value={draft.loudnessLufs}
                className={`${controlClassName} w-24 tabular-nums`}
                onChange={(event) => updateDraft({ loudnessLufs: event.currentTarget.value })}
              />
              <span className="text-[11px] text-muted-foreground">LUFS</span>
            </div>
          </SettingRow>

          <SettingRow label="Caption delivery" description="Burn captions in, embed a caption track, or leave them out.">
            <select
              ref={captionsRef}
              aria-label="Project caption mode"
              value={draft.captions}
              className={controlClassName}
              onChange={(event) => updateDraft({ captions: event.currentTarget.value })}
            >
              <option value="burn_in">Burn into video</option>
              <option value="mux">Embed as caption track</option>
              <option value="off">Off</option>
            </select>
          </SettingRow>

          {validationError ? (
            <p role="alert" className="border-l-2 border-red-500 pl-2 text-xs text-red-700">
              {validationError}
            </p>
          ) : null}
          {applyError ? (
            <p role="alert" className="border-l-2 border-red-500 pl-2 text-xs text-red-700">
              {applyError}
            </p>
          ) : null}
          {applyStatus ? (
            <p role="status" className="border-l-2 border-emerald-500 pl-2 text-xs text-emerald-700">
              {applyStatus}
            </p>
          ) : null}

          <div className="flex justify-end border-t pt-3">
            <Button type="submit" size="sm" disabled={applying}>
              {applying ? <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /> : null}
              {applying ? "Applying project settings" : "Apply project settings"}
            </Button>
          </div>
        </form>

        <ProjectStorage project={project} projectDir={projectDir} />
        <ProjectGuidance projectDir={projectDir} />

        <footer className="flex items-center gap-2 border-t pt-4 text-[11px] text-muted-foreground">
          <BookOpen className="h-3.5 w-3.5" aria-hidden="true" />
          Project settings persist with the canonical split project.
        </footer>
      </div>
    </main>
  );
}
