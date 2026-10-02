import { loadAppSettingsPreferences } from "@/lib/app-settings";
import { generationJobAssetId, jobExportSettings, startRequestStringInput } from "@/lib/jobs/activity-records";
import { videoExportJobIdProfile } from "@/lib/jobs/ids";
import { mergeJobState } from "@/lib/jobs/merge-job-state";
import { taskRecords, workflowServiceUnreachableDetail, type TaskKind, type TaskRecord, type TaskRuntime } from "@/lib/jobs/task-records";
import {
  cancelRenderJobInSplitProjectFolder,
  loadJobProgressFromSplitProjectFolder,
  readProjectSnapshotFromSplitProjectFolder,
  reconcileTemporalJobsInSplitProjectFolder,
  type GeneratedAsset,
  type JobProgressSnapshot,
  type VideoProject,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { createGenerationService } from "../services/generation-service";
import { createSpeechService } from "../services/speech-service";
import type { EditorSliceCreator, EditorState, EditorStore } from "./editor-store";
import { createProgressMonitor, createTemporalReconciler } from "./job-monitors";
import type { ExportPreset } from "./ui-slice";

type ActiveRender = NonNullable<TaskRuntime["activeRender"]>;
type AgentTurn = NonNullable<TaskRuntime["agentTurn"]>;

/** Cancels the running agent turn; plan 06 registers it. Resolves false when nothing was cancelled. */
type AgentCancel = (turnId: string) => Promise<boolean> | boolean;

/** Re-runs a stored export plan for the failed job; the export service (plan 07 Task 5) registers it. */
type ExportRunner = (plan: unknown, jobId: string) => Promise<boolean>;

export interface JobsSlice {
  /** Background task records derived by `taskRecords` from the project and the runtime state below. */
  readonly tasks: readonly TaskRecord[];
  /** The in-process render started in this session; its cancel needs the attempt id. */
  readonly activeRender: ActiveRender | null;
  /** The agent turn in flight, owned by the agent slice (plan 06). */
  readonly agentTurn: AgentTurn | null;
  /** Export plans kept in memory by job id, so Retry can re-run them. */
  readonly exportPlans: ReadonlyMap<string, unknown>;
  /** The latest progress snapshot (0–1) per running job; bookkeeping outside the project and undo. */
  readonly jobProgress: ReadonlyMap<string, number>;
  /** Why the last Temporal reconciliation couldn't use the workflow service (its detail); null when it could. */
  readonly workflowServiceIssue: string | null;
  /**
   * Enables polling for this editor session (`EditorRoot` mount) and resets the backoff. The folder
   * is reloaded while any task is not terminal: every second, then every five after a minute.
   * Progress snapshots are polled every 500 ms while a task runs, and Temporal jobs are reconciled
   * with their workflows on start and every 30 s while a Temporal-backed task is active.
   */
  startPolling(): void;
  /** Stops polling (`EditorRoot` unmount) until the next `startPolling`. */
  stopPolling(): void;
  /** Re-derives `tasks` and reconciles the poll timer; the store calls it on every change. */
  syncJobs(): void;
  /** Merges a backend project's job state (`mergeJobState`) against the latest project. */
  mergeLoadedProject(loaded: VideoProject): Promise<boolean>;
  setActiveRender(render: ActiveRender | null): void;
  setAgentTurn(turn: AgentTurn | null): void;
  rememberExportPlan(jobId: string, plan: unknown): void;
  /** Registers the agent turn cancel; returns the unregister function. */
  registerAgentCancel(cancel: AgentCancel): () => void;
  /** Registers the export plan runner; returns the unregister function. */
  registerExportRunner(runner: ExportRunner): () => void;
  /**
   * Starts the queued generations an applied agent bundle recorded (`GenerationService.startRecorded`).
   * A start that fails leaves its reason in `lastError`; the applied edit stays.
   */
  startRecordedGenerations(jobIds: readonly string[]): void;
  /** Stops following the generations an agent Undo removed (`GenerationService.forgetRemoved`). */
  forgetRemovedGenerations(assetIds: readonly string[]): void;
  /** Cancels a task whose record offers cancel. Resolves false when unavailable or failed. */
  cancelTask(id: string): Promise<boolean>;
  /** Retries a failed, retryable task by kind. Resolves false when not retryable or failed. */
  retryTask(id: string): Promise<boolean>;
  openTaskDetails(id: string): void;
}

const pollIntervalMs = 1_000;
const backoffIntervalMs = 5_000;
const backoffAfterMs = 60_000;
/** A merge re-computed this many times because writes kept landing waits for the next poll. */
const maxMergeAttempts = 3;

const terminalStatuses: ReadonlySet<TaskRecord["status"]> = new Set(["completed", "failed", "cancelled"]);

/** Fields `mergeJobState` may change; everything else always comes from the current project. */
const workerOwnedKeys = [
  "contentRevision",
  "updatedAt",
  "codexThreadId",
  "jobs",
  "generatedAssets",
  "renderReports",
  "exportArtifacts",
  "media",
  "transcripts",
  "mediaAnalysis",
  "mediaSilenceRanges",
] as const satisfies readonly (keyof VideoProject)[];

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isProject(value: unknown): value is VideoProject {
  return typeof value === "object" && value !== null && "timeline" in value && "jobs" in value;
}

/**
 * Only split project folders can be reloaded; the browser sample has a `browser://` placeholder dir.
 * The agent slice uses the same rule for chat sessions and history.
 */
export function pollable(project: VideoProject, projectDir: string): boolean {
  return project.schemaVersion >= 2 && projectDir.trim().length > 0 && !projectDir.startsWith("browser://");
}

function sameWorkerState(left: VideoProject, right: VideoProject): boolean {
  return workerOwnedKeys.every((key) => left[key] === right[key] || JSON.stringify(left[key]) === JSON.stringify(right[key]));
}

type TaskInputs = Pick<EditorState, "project" | "projectDir" | "activeRender" | "agentTurn" | "exportPlans" | "jobProgress" | "workflowServiceIssue">;

function deriveTasks(state: TaskInputs): TaskRecord[] {
  return taskRecords(state.project, {
    projectDir: state.projectDir,
    executionBackend: loadAppSettingsPreferences().generationExecutionBackend,
    activeRender: state.activeRender,
    agentTurn: state.agentTurn,
    exportPlanJobIds: new Set(state.exportPlans.keys()),
    progressByJobId: state.jobProgress,
    workflowServiceIssue: state.workflowServiceIssue,
  });
}

/** Mirrors the task record rule: a completed generation whose missing output has a provider URL. */
function progressMap(snapshots: readonly JobProgressSnapshot[]): ReadonlyMap<string, number> {
  return new Map(snapshots.map((snapshot) => [snapshot.jobId, snapshot.progress]));
}

function sameProgress(left: ReadonlyMap<string, number>, right: ReadonlyMap<string, number>): boolean {
  return left.size === right.size && [...left].every(([jobId, progress]) => right.get(jobId) === progress);
}

function downloadRetryable(asset: GeneratedAsset, project: VideoProject): boolean {
  return (
    asset.status === "completed" &&
    asset.outputs.some((output) => !project.media.some((media) => media.id === output.mediaId) && Boolean(output.sourceUrl?.trim()))
  );
}

/**
 * The job's export choices: its recorded export settings (or its Temporal start request's) when it has
 * them; otherwise an in-process export without its stored plan keeps the profile its job id names, and
 * a plain render retries as the legacy default, a draft WebM.
 */
function exportPresetFromJob(project: VideoProject, jobId: string, kind: TaskKind): ExportPreset {
  const job = project.jobs.find((candidate) => candidate.id === jobId);
  const read = (key: string) => (job ? startRequestStringInput(job, key) : null);
  const render = kind === "render";
  const settings = job ? jobExportSettings(job) : null;
  return {
    jobId,
    profile: settings?.profile ?? read("profile") ?? videoExportJobIdProfile(jobId) ?? (render ? "webm" : null),
    quality: settings?.quality ?? read("quality") ?? (render ? "draft" : null),
    nleFormat: read("format"),
    settings,
  };
}

export function createJobsSlice(init: { readonly projectDir: string; readonly project: VideoProject }, store: () => EditorStore): EditorSliceCreator<JobsSlice> {
  return (set, get) => {
    let enabled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let timerDelay = 0;
    let inFlight = false;
    let pollingSince = 0;
    let activeIds: ReadonlySet<string> = new Set();
    let derivedFrom: readonly unknown[] = [];
    let agentCancel: AgentCancel | null = null;
    let exportRunner: ExportRunner | null = null;

    function clearTimer(): void {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    }

    /** Runs or stops the timer: polling needs the session enabled, a pollable project and an active task. */
    function reconcile(resetDelay: boolean): void {
      const { project, projectDir } = get();
      if (!enabled || activeIds.size === 0 || !pollable(project, projectDir)) {
        clearTimer();
        return;
      }
      if (inFlight || (timer !== null && !(resetDelay && timerDelay > pollIntervalMs))) return;
      clearTimer();
      timerDelay = Date.now() - pollingSince >= backoffAfterMs ? backoffIntervalMs : pollIntervalMs;
      timer = setTimeout(() => void poll(), timerDelay);
    }

    async function poll(): Promise<void> {
      timer = null;
      inFlight = true;
      const { projectDir } = get();
      try {
        const loaded: unknown = await readProjectSnapshotFromSplitProjectFolder({ projectDir });
        if (enabled && isProject(loaded) && get().projectDir === projectDir) await get().mergeLoadedProject(loaded);
      } catch (error) {
        // Without a backend nothing can be reloaded; the next `startPolling` tries again.
        if (isBackendUnavailableError(error)) enabled = false;
      } finally {
        inFlight = false;
        reconcile(false);
      }
    }

    /** Re-derives tasks when their inputs change; a task that newly became active resets the backoff. */
    function sync(force: boolean): void {
      const state = get();
      const inputs = [state.project, state.projectDir, state.activeRender, state.agentTurn, state.exportPlans, state.jobProgress, state.workflowServiceIssue];
      const changed = force || inputs.some((input, index) => input !== derivedFrom[index]);
      if (!changed) return;
      derivedFrom = inputs;
      const tasks = deriveTasks(state);
      const active = new Set(tasks.filter((task) => !terminalStatuses.has(task.status)).map((task) => task.id));
      const started = [...active].some((id) => !activeIds.has(id));
      activeIds = active;
      if (started) pollingSince = Date.now();
      // Unchanged records keep their identity, so task selectors don't re-render on unrelated edits.
      if (JSON.stringify(tasks) !== JSON.stringify(state.tasks)) set({ tasks });
      reconcile(started);
      progressMonitor.sync();
      temporalReconciler.sync();
    }

    const monitoredProjectDir = () => (enabled && pollable(get().project, get().projectDir) ? get().projectDir : null);

    const progressMonitor = createProgressMonitor({
      load: () => loadJobProgressFromSplitProjectFolder({ projectDir: get().projectDir }),
      hasRunningTask: () => monitoredProjectDir() !== null && get().tasks.some((task) => task.status === "running"),
      onSnapshots: (snapshots) => {
        const jobProgress = progressMap(snapshots);
        if (!sameProgress(jobProgress, get().jobProgress)) set({ jobProgress });
      },
    });

    const temporalReconciler = createTemporalReconciler({
      reconcile: () => reconcileTemporalJobsInSplitProjectFolder({ projectDir: get().projectDir, updatedAt: new Date().toISOString() }),
      hasActiveTemporalTask: () => monitoredProjectDir() !== null && get().tasks.some((task) => !terminalStatuses.has(task.status) && task.workflow?.backend === "temporal"),
      onResult: async (result) => {
        const issue = result.serviceReachable ? null : result.detail?.trim() || workflowServiceUnreachableDetail;
        if (get().workflowServiceIssue !== issue) set({ workflowServiceIssue: issue });
        if (result.project && enabled) await get().mergeLoadedProject(result.project);
      },
    });

    const generation = () => createGenerationService(store());

    return {
      tasks: deriveTasks({ ...init, activeRender: null, agentTurn: null, exportPlans: new Map(), jobProgress: new Map(), workflowServiceIssue: null }),
      activeRender: null,
      agentTurn: null,
      exportPlans: new Map(),
      jobProgress: new Map(),
      workflowServiceIssue: null,

      startPolling() {
        enabled = true;
        pollingSince = Date.now();
        sync(true);
        reconcile(true);
        progressMonitor.start();
        if (pollable(get().project, get().projectDir)) temporalReconciler.start();
      },

      stopPolling() {
        enabled = false;
        clearTimer();
        progressMonitor.stop();
        temporalReconciler.stop();
      },

      syncJobs: () => sync(false),

      async mergeLoadedProject(loaded) {
        for (let attempt = 0; attempt < maxMergeAttempts; attempt += 1) {
          const base = get().project;
          const { project, externalChange } = mergeJobState(base, loaded);
          if (!externalChange && sameWorkerState(base, project)) return true;
          if (await get().mergeExternalState(project, { externalChange, base })) return true;
        }
        return false;
      },

      setActiveRender: (render) => set({ activeRender: render }),
      setAgentTurn: (turn) => set({ agentTurn: turn }),
      rememberExportPlan: (jobId, plan) => set((state) => ({ exportPlans: new Map(state.exportPlans).set(jobId, plan) })),

      registerAgentCancel(cancel) {
        agentCancel = cancel;
        return () => {
          if (agentCancel === cancel) agentCancel = null;
        };
      },

      registerExportRunner(runner) {
        exportRunner = runner;
        return () => {
          if (exportRunner === runner) exportRunner = null;
        };
      },

      startRecordedGenerations(jobIds) {
        if (jobIds.length > 0) generation().startRecorded(jobIds);
      },

      forgetRemovedGenerations(assetIds) {
        if (assetIds.length > 0) generation().forgetRemoved(assetIds);
      },

      async cancelTask(id) {
        const record = get().tasks.find((task) => task.id === id);
        if (!record?.cancel.available) return false;
        const { activeRender, projectDir } = get();
        if (activeRender?.jobId === id) {
          try {
            const result = await cancelRenderJobInSplitProjectFolder({ projectDir, jobId: id, attemptId: activeRender.attemptId, updatedAt: new Date().toISOString() });
            await get().mergeLoadedProject(result.project);
            return true;
          } catch (error) {
            get().setLastError(errorMessage(error));
            return false;
          }
        }
        if (record.kind === "generation") return generation().cancelGeneration(id);
        if (record.kind === "agent" && agentCancel) return (await agentCancel(id)) !== false;
        return false;
      },

      async retryTask(id) {
        const record = get().tasks.find((task) => task.id === id);
        if (!record?.retry) return false;
        const { project } = get();
        switch (record.kind) {
          case "generation": {
            // Agent bundles record `job-<assetId>` jobs, so the task id may name the job.
            const job = project.jobs.find((candidate) => candidate.id === id);
            const assetId = job ? generationJobAssetId(job) : id;
            const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
            if (asset && downloadRetryable(asset, project)) return generation().retryDownload(assetId);
            return (await generation().rerun(assetId)) !== null;
          }
          case "render":
          case "export": {
            const plan = get().exportPlans.get(id);
            if (plan !== undefined && exportRunner) return exportRunner(plan, id);
            get().openExportPopover(exportPresetFromJob(project, id, record.kind));
            return true;
          }
          case "transcription": {
            const job = project.jobs.find((candidate) => candidate.id === id);
            const mediaId = job ? startRequestStringInput(job, "mediaId") : null;
            if (!job || !mediaId) return false;
            return createSpeechService(store()).transcribe(mediaId, startRequestStringInput(job, "languageMode") ?? undefined);
          }
          case "analysis":
          case "agent":
            return false;
        }
      },

      openTaskDetails: (id) => set({ taskDetailsId: id }),
    };
  };
}
