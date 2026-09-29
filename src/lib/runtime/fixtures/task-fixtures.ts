import type {
  JobProgressSnapshot,
  ProjectJobSummary,
  ProjectPreviewRenderComparisonRunResult,
  ProjectRenderReport,
  TemporalJobReconciliation,
  TemporalWorkerEnvironmentReport,
  VideoProject,
} from "../../project";
import { buildSampleRenderReport, type RenderPreviewComparisonRequest, type RenderReport } from "../../render";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import type { FixtureProjectStore } from "./fixture-project-store";

/**
 * DEV-only background task handlers (`tasksFixture: true` on the fixture marker). The first save of
 * the sample project seeds a running Temporal transcription, a completed MP4 export with a render
 * report, a failed draft render, and a DaVinci XML export queued ten minutes ago whose workflow never
 * started, so the Background tasks indicator, list and details appear. Temporal reconciliation fails
 * that stale export with "The workflow never started."; job progress reports no snapshots (the export
 * fixture reports its pending renders').
 * Reveal requests for recorded export paths land on `window.__EDITOR_FIXTURE_REVEALS__`, the one
 * reveal record every fixture uses. The export fixture (`export-fixtures.ts`) composes these handlers
 * with seeding off; both read the shared `FixtureProjectStore`.
 */

const sampleProjectId = "project-sample";
const fixtureJobIds = {
  transcription: "fixture-transcribe-media-1",
  export: "fixture-export-mp4",
  render: "fixture-render-draft",
  staleNleExport: "fixture-export-nle-stale",
} as const;

/** Mirrors the backend's grace before a queued job without a run id counts as never started. */
const temporalStartGraceMs = 2 * 60_000;
const workflowNeverStartedReason = "The workflow never started.";

const workerReport: TemporalWorkerEnvironmentReport = {
  ready: false,
  featureEnabled: true,
  taskQueue: "video-creater-workflows",
  localServiceTarget: "127.0.0.1:7233",
  localWebUiUrl: "http://127.0.0.1:8233",
  localDevCommand: "temporal server start-dev",
  workerRunCommand: "cargo run --bin video-creater-temporal-worker",
  featureName: "temporal-worker",
  tools: [
    { name: "temporal", available: true, path: "/usr/local/bin/temporal" },
    { name: "ffprobe", available: false, installHint: "Install FFmpeg so the worker can inspect media." },
  ],
};

function comparisonRequest(projectDir: string): RenderPreviewComparisonRequest {
  return {
    status: "pending",
    projectDir,
    projectReportId: fixtureJobIds.export,
    renderReportPath: `renders/${fixtureJobIds.export}/pipeline-report.json`,
    renderedVideo: `renders/${fixtureJobIds.export}/output.mp4`,
    durationSeconds: 6,
    frameTimeSeconds: 1.5,
    renderedFrames: [`renders/${fixtureJobIds.export}/frames/frame-0000.png`, `renders/${fixtureJobIds.export}/frames/frame-0036.png`],
  };
}

function exportReport(projectDir: string, createdAt: string): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id: fixtureJobIds.export,
    status: "completed",
    outputPath: `renders/${fixtureJobIds.export}/output.mp4`,
    durationSeconds: 6,
    streams: { video: true, audio: true },
    checks: { duration: "passed", streams: "passed", captionAlignment: "skipped" },
    artifacts: [`renders/${fixtureJobIds.export}/pipeline-report.json`],
    previewComparisonRequest: comparisonRequest(projectDir),
    previewComparison: null,
    logPath: `renders/${fixtureJobIds.export}/render.log`,
    createdAt,
  };
}

function seededJobs(now: number): ProjectJobSummary[] {
  const at = (minutesAgo: number) => new Date(now - minutesAgo * 60_000).toISOString();
  const workflowId = `video-creater/${sampleProjectId}/${fixtureJobIds.transcription}`;
  return [
    {
      id: fixtureJobIds.transcription,
      kind: "transcribe_media",
      status: "running",
      updatedAt: at(0.5),
      workflow: { workflowId, workflowType: "TranscribeMediaWorkflow", taskQueue: "video-creater-workflows", runId: "fixture-run-7f3c", activityTypes: ["transcribe_media"] },
      startRequest: {
        workflowId,
        workflowType: "TranscribeMediaWorkflow",
        taskQueue: "video-creater-workflows",
        input: { jobId: fixtureJobIds.transcription, mediaId: "media-1", languageMode: "auto" },
        searchAttributes: {},
        activityTypes: ["transcribe_media"],
        idReusePolicy: "rejectDuplicate",
      },
    },
    { id: fixtureJobIds.export, kind: "export_media", status: "completed", updatedAt: at(2) },
    { id: fixtureJobIds.render, kind: "render_draft", status: "failed", updatedAt: at(4) },
    staleNleExportJob(at(10)),
  ];
}

function staleNleExportJob(updatedAt: string): ProjectJobSummary {
  const workflowId = `video-creater/${sampleProjectId}/${fixtureJobIds.staleNleExport}`;
  const workflow = { workflowId, workflowType: "ExportNleXmlWorkflow", taskQueue: "video-creater-workflows", activityTypes: ["export_nle_xml"] };
  return {
    id: fixtureJobIds.staleNleExport,
    kind: "export_nle_xml",
    status: "queued",
    updatedAt,
    workflow: { ...workflow, runId: null },
    startRequest: {
      ...workflow,
      input: { jobId: fixtureJobIds.staleNleExport, format: "davinciFcpxml", outputPath: `exports/${sampleProjectId}-davinci.fcpxml` },
      searchAttributes: {},
      idReusePolicy: "rejectDuplicate",
    },
  };
}

/**
 * The fixture's workflow service knows no workflow, so like the backend reconciliation it fails a
 * queued or running job with a start request and no run id once the start grace has passed.
 */
function reconcileNeverStartedJobs(store: FixtureProjectStore, updatedAt: string): TemporalJobReconciliation {
  const committed = store.current;
  if (!committed) return { project: null, failedJobIds: [], serviceReachable: true, detail: null };
  const now = Date.parse(updatedAt);
  const stale = (job: ProjectJobSummary) =>
    (job.status === "queued" || job.status === "running") &&
    Boolean(job.startRequest) &&
    !job.workflow?.runId &&
    job.kind !== "generate_media" &&
    now - Date.parse(job.updatedAt) > temporalStartGraceMs;
  const failedJobIds = committed.jobs.filter(stale).map((job) => job.id);
  if (failedJobIds.length === 0) return { project: committed, failedJobIds, serviceReachable: true, detail: null };
  const project = store.record({
    ...committed,
    jobs: committed.jobs.map((job) => (failedJobIds.includes(job.id) ? { ...job, status: "failed", updatedAt, failureReason: workflowNeverStartedReason } : job)),
  });
  return { project, failedJobIds, serviceReachable: true, detail: null };
}

/**
 * Seeds the task fixtures into the sample project the first time it is saved into the fixture folder
 * (the store makes it schema v2, so the render review commands treat it as folder backed).
 */
function withSeededTasks(project: VideoProject, projectDir: string, now = Date.now()): VideoProject {
  if (project.id !== sampleProjectId || project.jobs.some((job) => job.id === fixtureJobIds.transcription)) return project;
  return {
    ...project,
    jobs: [...project.jobs, ...seededJobs(now)],
    renderReports: [...project.renderReports, exportReport(projectDir, new Date(now - 2 * 60_000).toISOString())],
  };
}

function failedRenderReport(): RenderReport {
  const report = buildSampleRenderReport("draftWebm");
  return {
    ...report,
    jobId: fixtureJobIds.render,
    summary: { ...report.summary, status: "failed", outputPath: null },
    errors: [
      {
        code: "font_missing",
        path: "timeline.tracks[0].items[0].style.fontFamily",
        message: "The title font “Canela” isn't installed.",
        fix: "Install the font, or pick another font for the title.",
      },
    ],
  };
}

/** Mirrors the desktop restriction: only export artifacts and render report outputs and logs. */
function recordedExportPaths(project: VideoProject | null): string[] {
  if (!project) return [];
  return [...(project.exportArtifacts ?? []).map((artifact) => artifact.path), ...project.renderReports.flatMap((report) => [report.outputPath, report.logPath])];
}

/** Like the desktop command, an absolute path under the project folder names the same recorded file. */
function projectRelativePath(projectDir: string, artifactPath: string): string {
  const root = `${projectDir.replace(/[\\/]+$/, "")}/`;
  return projectDir.trim() && artifactPath.startsWith(root) ? artifactPath.slice(root.length) : artifactPath;
}

interface TaskFixtureOptions {
  /** Seed the running, completed and failed tasks into the sample on its first save (default true). */
  readonly seed?: boolean;
}

/** Task handlers over the shared fixture project store; saves and reloads are the project fixture's. */
export function taskFixtureOperations(store: FixtureProjectStore, options: TaskFixtureOptions = {}): ReadonlyMap<string, FixtureOperationHandler> {
  if (options.seed ?? true) store.addSeed(withSeededTasks);
  return new Map<string, FixtureOperationHandler>([
    ["get_temporal_worker_environment_report", () => workerReport],
    ["load_job_progress_from_split_project_folder", (): JobProgressSnapshot[] => []],
    ["reconcile_temporal_jobs_in_split_project_folder", (input) => reconcileNeverStartedJobs(store, (input as { updatedAt: string }).updatedAt)],
    [
      "reveal_export_artifact_in_split_project_folder",
      (input) => {
        const { projectDir, artifactPath } = input as { projectDir: string; artifactPath: string };
        if (!recordedExportPaths(store.current).includes(projectRelativePath(projectDir, artifactPath))) throw "This file isn't a recorded export of this project.";
        (window.__EDITOR_FIXTURE_REVEALS__ ??= []).push({ projectDir, artifactPath });
        return undefined;
      },
    ],
    ["load_render_pipeline_report_from_split_project_folder", () => failedRenderReport()],
    [
      "run_preview_render_comparison_request_in_split_project_folder",
      (input) => {
        const { request, updatedAt } = input as { request: RenderPreviewComparisonRequest; updatedAt: string };
        const comparison = {
          status: "failed",
          comparedFrames: [
            { timelineSeconds: 0, previewFrame: "preview/frame-0000.png", renderedFrame: request.renderedFrames[0] ?? "", diffFrame: null, mismatchRatio: 0.0021, passed: true },
            { timelineSeconds: 1.5, previewFrame: "preview/frame-0036.png", renderedFrame: request.renderedFrames[1] ?? "", diffFrame: "diff/frame-0036.png", mismatchRatio: 0.0625, passed: false },
          ],
        };
        const committed = store.current;
        if (!committed) throw new Error("Open the sample project before comparing a render.");
        const projectReport: ProjectRenderReport = { ...exportReport(request.projectDir, updatedAt), previewComparisonRequest: { ...request, status: "completed" }, previewComparison: comparison };
        const project = store.record({ ...committed, renderReports: committed.renderReports.map((report) => (report.id === projectReport.id ? projectReport : report)) });
        const result: ProjectPreviewRenderComparisonRunResult = {
          project,
          renderReport: { ...buildSampleRenderReport("finalWebm"), jobId: request.projectReportId, previewComparison: comparison },
          projectRenderReport: projectReport,
          evidenceReport: `renders/${request.projectReportId}/preview-comparison.json`,
        };
        return result;
      },
    ],
  ]);
}
