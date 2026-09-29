import { useMemo } from "react";
import { loadAppSettingsPreferences, type AppSettingsPreferences } from "@/lib/app-settings";
import { artifactFileName, saveRangeMediaName } from "@/lib/export/export-naming";
import { isExportStartPlan, replanExport, type ExportStartPlan } from "@/lib/export/export-plan";
import { draftExportDimensions, fallbackExportProfileAvailability, mediaExportOutputPath } from "@/lib/export/profiles";
import { generatedMediaExportJobId, generatedNleExportJobId, generatedRenderAttemptId, generatedSaveRangeJobId } from "@/lib/jobs/ids";
import { workflowStartFailureActions } from "@/lib/jobs/start-failure";
import { buildTemporalJobSummary, exportJobWithStartRequest } from "@/lib/jobs/temporal-fallback";
import { mediaDisplayName } from "@/lib/media/names";
import { safeProjectMediaPath } from "@/lib/media/preview-source";
import { getRuntimeMode } from "@/lib/runtime/runtime-mode";
import { showBrowserCompletionNotification } from "@/lib/settings/render-system";
import {
  buildTemporalExportMediaStartRequest,
  buildTemporalExportProjectBundleStartRequest,
  buildTemporalStartResultAction,
  exportNleXmlToSplitProjectFolder,
  exportPalmierProjectPackageToSplitProjectFolder,
  getExportProfileAvailabilityReport,
  importMediaToProject,
  loadRenderPipelineReportFromSplitProjectFolder,
  loadSplitProjectFromFolder,
  renderMediaToSplitProjectFolder,
  startTemporalWorkflow,
  type ExportProfileAvailability,
  type NleXmlExportFormat,
  type ProjectJobSummary,
  type ProjectMediaRenderResult,
  type TemporalWorkflowStartResult,
  type VideoProject,
} from "@/lib/project";
import type { RenderReport } from "@/lib/render";
import type { TimelineRangeSelection } from "@/lib/timeline-ops/navigation";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";
import { createRevealService } from "./reveal-service";

/**
 * Store-bound exports, following the pre-cut `exportInDesktopProcess`, `exportMediaProfile`,
 * `exportNleXml` and `exportPalmierProjectPackage` flows. Every export shows up as a background task;
 * a finished one shows an "Exported <name>" toast with "Show in folder". Methods resolve false when
 * blocked or failed, with the reason in `lastError`.
 */
export interface ExportService {
  /**
   * Starts a video export under a fresh job id: rendered in the desktop process, or as a Temporal
   * workflow when the `generationExecutionBackend` preference is "temporal". In process it resolves
   * when the render settles; with Temporal, once the workflow has started.
   */
  exportVideo(plan: ExportStartPlan): Promise<boolean>;
  exportNleXml(format: NleXmlExportFormat): Promise<boolean>;
  /** The Palmier project package, in process or as a Temporal workflow by preference. */
  exportProjectPackage(): Promise<boolean>;
  /**
   * Pre-cut `saveTimelineRangeAsMedia`: renders the active timeline's range as a draft WebM in the
   * desktop process, imports the output into Media, reveals it there and toasts. A failure leaves
   * the task failed, with Retry saving the same range again.
   */
  saveRangeAsMedia(range: TimelineRangeSelection): Promise<boolean>;
}

interface ExportServiceOptions {
  /** Defaults to the accepted app preferences, as the pre-cut editor read them. */
  readonly preferences?: () => AppSettingsPreferences;
}

/**
 * A finished export's toast name and the project-recorded path "Show in folder" reveals. A Temporal
 * export's saved file is known only once its workflow records the export artifact, so the artifact
 * recorded under the job id, when there is one, replaces both at completion.
 */
interface ExportOutcome {
  readonly name: string;
  readonly artifactPath: string;
  /**
   * The workflow saves a named export file and records it under the job id. Its job can read as
   * completed before that file exists (an older workflow completed it after encoding), so the toast
   * waits for the recorded file.
   */
  readonly awaitsArtifact?: boolean;
}

/** Temporal exports started in this session, by job id, awaiting completion for their toast. */
interface ExportRuntime {
  readonly awaiting: Map<string, ExportOutcome>;
  unsubscribe: (() => void) | null;
}

const runtimes = new WeakMap<EditorStore, ExportRuntime>();

type RenderInput = Omit<Parameters<typeof renderMediaToSplitProjectFolder>[0], "attemptId" | "updatedAt">;

/** Kept by job id like an export plan, so Retry on a failed saved range renders the same range again. */
interface SaveRangePlan {
  readonly kind: "saveRange";
  readonly range: TimelineRangeSelection;
}

function isSaveRangePlan(value: unknown): value is SaveRangePlan {
  return typeof value === "object" && value !== null && (value as { kind?: unknown }).kind === "saveRange" && "range" in value;
}

const nleLabels: Record<NleXmlExportFormat, string> = { premiereXmeml: "Premiere XML", davinciFcpxml: "DaVinci XML" };

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function now(): string {
  return new Date().toISOString();
}

function isProject(value: unknown): value is VideoProject {
  return typeof value === "object" && value !== null && "timeline" in value && "jobs" in value;
}

function runtimeFor(store: EditorStore): ExportRuntime {
  let runtime = runtimes.get(store);
  if (!runtime) {
    runtime = { awaiting: new Map(), unsubscribe: null };
    runtimes.set(store, runtime);
  }
  return runtime;
}

/** The persisted render report's first error says what failed in plain words; else the command error. */
function renderFailureMessage(report: RenderReport | null, error: unknown): string {
  const reported = report?.errors[0]?.message.trim();
  return reported || errorMessage(error);
}

export function createExportService(store: EditorStore, options: ExportServiceOptions = {}): ExportService {
  const state = () => store.getState();
  const preferences = options.preferences ?? loadAppSettingsPreferences;
  const runtime = runtimeFor(store);
  const reveal = createRevealService(store);

  function block(reason: string): false {
    state().setLastError(reason);
    return false;
  }

  function splitProjectDir(): string | null {
    const { project, projectDir } = state();
    return project.schemaVersion >= 2 && projectDir.trim().length > 0 ? projectDir : null;
  }

  function toastExported({ name, artifactPath }: ExportOutcome): void {
    if (preferences().renderCompletionNotifications) {
      showBrowserCompletionNotification(`Exported ${name}`);
    }
    state().pushToast({
      title: `Exported ${name}`,
      action: { label: getRuntimeMode() === "browser" ? "Download" : "Show in folder", onSelect: () => void reveal.revealExportArtifact(artifactPath) },
    });
  }

  /**
   * The saved export file an in-process export records; an older backend records none, so the render
   * report's output path is the recorded file.
   */
  function inProcessOutcome(plan: ExportStartPlan, result: ProjectMediaRenderResult): ExportOutcome {
    const saved = result.exportArtifact?.path.trim();
    if (saved) return { name: artifactFileName(saved), artifactPath: saved };
    const reported = state().project.renderReports.find((report) => report.id === plan.jobId)?.outputPath.trim();
    return { name: plan.name, artifactPath: reported || result.outputPath };
  }

  /** Toasts tracked Temporal exports once polling merges their completion; failures stay in the task row. */
  function settleAwaited(project: VideoProject): void {
    for (const [jobId, outcome] of [...runtime.awaiting]) {
      const job = project.jobs.find((candidate) => candidate.id === jobId);
      if (!job || (job.status !== "completed" && job.status !== "failed" && job.status !== "cancelled")) continue;
      const saved = project.exportArtifacts?.find((artifact) => artifact.jobId === jobId)?.path.trim();
      if (job.status === "completed" && !saved && outcome.awaitsArtifact) continue;
      runtime.awaiting.delete(jobId);
      if (job.status !== "completed") continue;
      toastExported(saved ? { name: artifactFileName(saved), artifactPath: saved } : outcome);
    }
  }

  function track(jobId: string, outcome: ExportOutcome): void {
    runtime.awaiting.set(jobId, outcome);
    runtime.unsubscribe ??= store.subscribe((next, previous) => {
      if (next.project !== previous.project) settleAwaited(next.project);
    });
    state().startPolling();
  }

  /** The workflow never started: fail the job so the task doesn't stay queued, then say why. */
  async function failStart(job: ProjectJobSummary, reason: string): Promise<false> {
    await state().applyActions(workflowStartFailureActions(job, now()));
    return block(reason);
  }

  /** Pre-cut `recordQueuedWorkflowJob` then `startQueuedTemporalWorkflow`; a job that fails to start is failed with its reason. */
  async function startTemporal(job: ProjectJobSummary, outcome: ExportOutcome): Promise<boolean> {
    if ((await state().applyActions([{ type: "recordJob", job }])) === null) return false;
    let result: TemporalWorkflowStartResult;
    try {
      result = await startTemporalWorkflow({ job });
    } catch (error) {
      return failStart(job, errorMessage(error));
    }
    if (result.status !== "started" || !result.runId) return failStart(job, result.message);
    track(job.id, outcome);
    const action = await buildTemporalStartResultAction({ job, runId: result.runId, updatedAt: now() });
    return (await state().applyActions([action])) !== null;
  }

  /**
   * Pre-cut `exportInDesktopProcess`: the render records its own job; a failure reloads the folder and
   * report. Resolves null when the render failed (the reason in `lastError`) or was cancelled.
   */
  async function renderInProcess(input: RenderInput): Promise<ProjectMediaRenderResult | null> {
    const { jobId, projectDir } = input;
    const attemptId = generatedRenderAttemptId();
    const startedAt = now();
    state().setActiveRender({ jobId, attemptId, startedAt });
    const finish = () => {
      if (state().activeRender?.jobId === jobId) state().setActiveRender(null);
    };
    try {
      const result = await renderMediaToSplitProjectFolder({ ...input, attemptId, updatedAt: startedAt });
      await state().mergeLoadedProject(result.project);
      finish();
      return result;
    } catch (error) {
      const [loaded, report] = await Promise.all([
        loadSplitProjectFromFolder({ projectDir }).catch(() => null),
        loadRenderPipelineReportFromSplitProjectFolder({ projectDir, jobId }).catch(() => null),
      ]);
      if (isProject(loaded)) await state().mergeLoadedProject(loaded);
      finish();
      // A render cancelled from the task row rejects too; the cancelled row says enough.
      if (!state().project.jobs.some((job) => job.id === jobId && job.status === "cancelled")) block(renderFailureMessage(report, error));
      return null;
    }
  }

  /**
   * Pre-cut import of the rendered range: the backend copies it into media, named after the timeline
   * and range, and saves the folder.
   */
  async function importRenderedRange(projectDir: string, jobId: string, outputPath: string, name: string): Promise<boolean> {
    try {
      const sourcePath = safeProjectMediaPath(projectDir, outputPath);
      if (!sourcePath) throw new Error("The rendered file isn't inside the project folder.");
      const result = await importMediaToProject({ projectDir, project: state().project, sourcePaths: [sourcePath], names: { [sourcePath]: name } });
      const [media] = result.imported;
      if (!media) throw new Error("The rendered file couldn't be imported.");
      await state().mergeLoadedProject(result.project);
      state().setActiveTab("media");
      if (state().openSheetId !== null) state().openSheet("media");
      state().setRevealMediaId(media.id);
      state().pushToast({ title: `Saved ${mediaDisplayName(media)} to Media` });
      return true;
    } catch (error) {
      // The render job completed; failing it keeps the task row honest and offers Retry.
      await state().applyActions([{ type: "updateJobStatus", jobId, status: "failed", updatedAt: now() }]);
      return block(`The range rendered but couldn't be added to Media: ${errorMessage(error)}`);
    }
  }

  return {
    async exportVideo(stored) {
      const plan = replanExport(stored, generatedMediaExportJobId(stored.profile));
      if (!plan) return block("This export can't be started again. Open Export to choose its settings.");
      if (plan.blockedReason) return block(plan.blockedReason);
      state().setLastError(null);
      state().rememberExportPlan(plan.jobId, plan);
      if (preferences().generationExecutionBackend !== "temporal") {
        const result = await renderInProcess({ ...plan.render, exportSettings: plan.settings });
        if (!result) return false;
        toastExported(inProcessOutcome(plan, result));
        return true;
      }
      try {
        const job = await buildTemporalJobSummary("export_media", plan.temporal.projectId, plan.jobId, now());
        const startRequest = await buildTemporalExportMediaStartRequest(plan.temporal);
        const recorded = { ...exportJobWithStartRequest(job, startRequest), exportSettings: plan.settings };
        return await startTemporal(recorded, { name: plan.name, artifactPath: plan.outputPath, awaitsArtifact: plan.render.output !== undefined });
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async exportNleXml(format) {
      const label = nleLabels[format];
      const projectDir = splitProjectDir();
      if (!projectDir) return block(`${label} export requires a saved split project folder.`);
      state().setLastError(null);
      try {
        const result = await exportNleXmlToSplitProjectFolder({ projectDir, format, jobId: generatedNleExportJobId(format), updatedAt: now() });
        await state().mergeLoadedProject(result.project);
        toastExported({ name: label, artifactPath: result.exportPath });
        return true;
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async exportProjectPackage() {
      const projectDir = splitProjectDir();
      if (!projectDir) return block("Project package export requires a saved split project folder.");
      const { project } = state();
      const jobId = generatedMediaExportJobId("palmierProject");
      const outputPath = mediaExportOutputPath(project.id, "palmierProject", "palmier", jobId);
      const name = "project package";
      state().setLastError(null);
      try {
        if (preferences().generationExecutionBackend !== "temporal") {
          const result = await exportPalmierProjectPackageToSplitProjectFolder({ projectDir, jobId, outputPath, updatedAt: now() });
          await state().mergeLoadedProject(result.project);
          toastExported({ name, artifactPath: result.exportPath });
          return true;
        }
        const job = await buildTemporalJobSummary("export_media", project.id, jobId, now());
        const startRequest = await buildTemporalExportProjectBundleStartRequest({ projectId: project.id, projectDir, jobId, outputPath });
        return await startTemporal(exportJobWithStartRequest(job, startRequest), { name, artifactPath: outputPath });
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async saveRangeAsMedia(range) {
      const projectDir = splitProjectDir();
      if (!projectDir) return block("Save this project to a folder before saving a range as media.");
      const { project } = state();
      const jobId = generatedSaveRangeJobId();
      const plan: SaveRangePlan = { kind: "saveRange", range: { startSeconds: range.startSeconds, endSeconds: range.endSeconds } };
      state().setLastError(null);
      state().rememberExportPlan(jobId, plan);
      // Always in process: the pre-cut editor had no Temporal workflow for saving a range, so the
      // generationExecutionBackend preference does not apply here.
      const result = await renderInProcess({
        projectDir,
        projectId: project.id,
        profile: "webm",
        quality: "draft",
        ...draftExportDimensions(project.renderSettings.width, project.renderSettings.height),
        jobId,
        rangeStartSeconds: range.startSeconds,
        rangeEndSeconds: range.endSeconds,
      });
      return result ? importRenderedRange(projectDir, jobId, result.outputPath, saveRangeMediaName(project, range)) : false;
    },
  };
}

/**
 * Registers the export runner the jobs slice's Retry uses (re-running a stored plan under a new job
 * id). `EditorRoot` calls it on mount; the returned cleanup unregisters it and stops awaiting exports.
 */
export function startExportRuntime(store: EditorStore): () => void {
  const unregister = store.getState().registerExportRunner((plan) => {
    if (isExportStartPlan(plan)) return createExportService(store).exportVideo(plan);
    if (isSaveRangePlan(plan)) return createExportService(store).saveRangeAsMedia(plan.range);
    return Promise.resolve(false);
  });
  return () => {
    unregister();
    const runtime = runtimes.get(store);
    if (!runtime) return;
    runtime.unsubscribe?.();
    runtime.unsubscribe = null;
    runtime.awaiting.clear();
  };
}

const noExporterReason = "Video export needs the desktop app's native exporter.";

/** The export capability report; without one (browser and fixture runtimes) every profile is unavailable. */
export async function loadExportProfiles(): Promise<readonly ExportProfileAvailability[]> {
  try {
    const report = await getExportProfileAvailabilityReport();
    if (Array.isArray(report) && report.length > 0) return report;
  } catch {
    // Falls through to the unavailable profiles below.
  }
  return fallbackExportProfileAvailability.map((profile) => ({
    ...profile,
    unavailableReason: noExporterReason,
    qualityUnavailableReasons: { draft: noExporterReason, final: noExporterReason },
  }));
}

/** The export service bound to the editor store; stable for the lifetime of the store. */
export function useExportService(): ExportService {
  const store = useEditorStoreApi();
  return useMemo(() => createExportService(store), [store]);
}
